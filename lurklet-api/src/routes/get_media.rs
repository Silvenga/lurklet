use crate::error::ApiError;
use axum::body::Body;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, header};
use axum::response::Response;
use headers::{ContentLength, HeaderMapExt};
use lurklet_reddit::{Error as RedditError, RedditBackend, RedditClient};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

const CACHE_POLICY: &str = "public, max-age=300";

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
struct GetMediaInput {
    /// Complete media URL, encoded as a query parameter; its path and query are forwarded unchanged.
    url: String,
}

/// Builds the full-file streaming media endpoint.
pub fn router<B: RedditBackend + 'static>() -> OpenApiRouter<RedditClient<B>> {
    OpenApiRouter::new().routes(routes!(get_media))
}

#[utoipa::path(
    get,
    path = "/media",
    operation_id = "get_media",
    summary = "Stream a complete image or MP4 file without range support",
    params(GetMediaInput),
    responses(
        ApiError,
        (status = 200, description = "Binary file content", content(("image/*"), ("video/mp4")))
    )
)]
async fn get_media<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    input: Result<Query<GetMediaInput>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(input) = input.map_err(|_| ApiError::BadRequest)?;
    let media = reddit
        .get_media(&input.url)
        .await
        .map_err(|error| match error {
            RedditError::InvalidRequest { field: "media_url" } => ApiError::BadRequest,
            RedditError::MediaStatus { status: 404, .. } => ApiError::NotFound,
            RedditError::MediaStatus {
                status: status @ (429 | 500..=599),
                retry_after,
            } => RedditError::Upstream {
                status,
                retry_after,
            }
            .into(),
            error => error.into(),
        })?;
    let content_type =
        HeaderValue::from_str(&media.content_type).map_err(|_| ApiError::BadGateway)?;
    let mut response = Response::new(Body::from_stream(media.body));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CACHE_POLICY),
    );
    if let Some(length) = media.content_length {
        response.headers_mut().typed_insert(ContentLength(length));
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RateLimit, ResponseCache, application};
    use axum::Router;
    use axum::body::to_bytes;
    use axum::extract::ConnectInfo;
    use axum::http::{Method, Request, StatusCode};
    use bytes::Bytes;
    use futures_util::{StreamExt, stream};
    use lurklet_reddit::{
        AnonymousSession, CommentForest, CommentSort, CommentThread, MediaFile, MediaKind, Post,
        RedditId, ResponseError, RetryAfter, Subreddit, SubredditPage, SubredditSort,
    };
    use std::future::IntoFuture;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    struct FakeBackend {
        error: Option<RedditError>,
        content_type: &'static str,
        kind: MediaKind,
        body: [&'static [u8]; 2],
        length: Option<u64>,
        calls: Arc<Mutex<Vec<String>>>,
        chunks: Arc<AtomicUsize>,
    }

    impl RedditBackend for FakeBackend {
        async fn get_media(&self, url: &str) -> Result<MediaFile, RedditError> {
            self.calls.lock().unwrap().push(url.into());
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            let chunks = self.chunks.clone();
            Ok(MediaFile {
                content_type: self.content_type.into(),
                kind: self.kind,
                content_length: self.length,
                body: Box::pin(
                    stream::iter([
                        Ok(Bytes::from_static(self.body[0])),
                        Ok(Bytes::from_static(self.body[1])),
                    ])
                    .inspect(move |_| {
                        chunks.fetch_add(1, Ordering::SeqCst);
                    }),
                ),
            })
        }
        async fn get_subreddit(&self, _: &str) -> Result<Subreddit, RedditError> {
            panic!("media must not fetch metadata")
        }
        async fn get_post(&self, _: &RedditId) -> Result<Post, RedditError> {
            panic!("media must not fetch posts")
        }
        async fn get_post_comments(
            &self,
            _: &RedditId,
            _: CommentSort,
            _: Option<&str>,
        ) -> Result<CommentForest, RedditError> {
            panic!("media must not fetch comments")
        }
        async fn get_comment_thread(
            &self,
            _: &RedditId,
            _: CommentSort,
        ) -> Result<CommentThread, RedditError> {
            panic!("media must not fetch threads")
        }
        async fn list_subreddit_posts(
            &self,
            _: &str,
            _: SubredditSort,
            _: Option<&str>,
        ) -> Result<SubredditPage, RedditError> {
            panic!("media must not fetch feeds")
        }
        async fn bootstrap(&self) -> Result<AnonymousSession, RedditError> {
            panic!("media must not bootstrap")
        }
    }

    fn app(
        error: Option<RedditError>,
        length: Option<u64>,
        calls: Arc<Mutex<Vec<String>>>,
        chunks: Arc<AtomicUsize>,
    ) -> Router {
        router()
            .split_for_parts()
            .0
            .with_state(RedditClient::from_backend(FakeBackend {
                error,
                content_type: "video/mp4",
                kind: MediaKind::Mp4,
                body: [b"hello", b" world"],
                length,
                calls,
                chunks,
            }))
    }

    fn request() -> Request<Body> {
        let query = form_urlencoded::Serializer::new(String::new())
            .append_pair("url", "https://preview.redd.it/file.png?x=2&x=1&s=a%2Bb")
            .finish();
        Request::builder()
            .uri(format!("/media?{query}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn when_media_is_requested_then_the_url_should_be_forwarded_unchanged_and_the_body_should_remain_lazy()
     {
        for length in [None, Some(11)] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let chunks = Arc::new(AtomicUsize::new(0));
            let app = app(None, length, calls.clone(), chunks.clone());

            let response = app.oneshot(request()).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(chunks.load(Ordering::SeqCst), 0);
            assert_eq!(
                *calls.lock().unwrap(),
                ["https://preview.redd.it/file.png?x=2&x=1&s=a%2Bb"]
            );
            assert_eq!(response.headers()[header::CONTENT_TYPE], "video/mp4");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=300"
            );
            assert_eq!(
                response
                    .headers()
                    .get(header::CONTENT_LENGTH)
                    .map(|value| value.to_str().unwrap()),
                length.map(|_| "11")
            );
            assert!(!response.headers().contains_key(header::ACCEPT_RANGES));
            let mut body = response.into_body().into_data_stream();
            assert_eq!(body.next().await.unwrap().unwrap(), b"hello".as_slice());
            assert_eq!(chunks.load(Ordering::SeqCst), 1);
            drop(body);
            assert_eq!(chunks.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn when_svg_media_is_served_through_the_api_then_browser_policies_should_preserve_streaming_and_caching()
     {
        const SVG: [&[u8]; 2] = [
            br#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            b"<script>alert(1)</script></svg>",
        ];
        let expected = SVG.concat();

        for caching in [false, true] {
            let directory = assert_fs::TempDir::new().unwrap();
            let cache = ResponseCache::new(directory.path()).await.unwrap();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let chunks = Arc::new(AtomicUsize::new(0));
            let reddit = RedditClient::from_backend(FakeBackend {
                error: None,
                content_type: "image/svg+xml",
                kind: MediaKind::Image,
                body: SVG,
                length: Some(expected.len() as u64),
                calls: calls.clone(),
                chunks: chunks.clone(),
            });
            let app = application(reddit, &RateLimit::default(), caching.then_some(&cache));

            for (method, outcome) in [
                (Method::HEAD, "miss"),
                (Method::GET, "miss"),
                (Method::GET, "hit"),
                (Method::HEAD, "hit"),
            ] {
                let request = Request::builder()
                    .method(method.clone())
                    .uri("/api/media?url=https%3A%2F%2Fi.redd.it%2Ffixture.svg")
                    .extension(ConnectInfo("192.0.2.1:1234".parse::<SocketAddr>().unwrap()))
                    .body(Body::empty())
                    .unwrap();
                let previous_chunks = chunks.load(Ordering::SeqCst);

                let response = app.clone().oneshot(request).await.unwrap();

                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(chunks.load(Ordering::SeqCst), previous_chunks);
                assert_eq!(response.headers()[header::CONTENT_TYPE], "image/svg+xml");
                assert_eq!(
                    response.headers()[header::CONTENT_LENGTH],
                    expected.len().to_string()
                );
                assert_eq!(response.headers()[header::CACHE_CONTROL], CACHE_POLICY);
                assert_eq!(
                    response.headers()[header::CONTENT_SECURITY_POLICY],
                    "default-src 'none'; sandbox; style-src 'unsafe-inline'; media-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
                );
                assert_eq!(
                    response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                    "nosniff"
                );
                assert_eq!(
                    response.headers().get("x-lurklet-cache"),
                    caching.then(|| HeaderValue::from_static(outcome)).as_ref()
                );
                assert!(!response.headers().contains_key(header::ACCEPT_RANGES));
                let body = to_bytes(response.into_body(), 4096).await.unwrap();
                assert_eq!(
                    body.as_ref(),
                    if method == Method::HEAD {
                        b""
                    } else {
                        expected.as_slice()
                    }
                );
            }
            assert_eq!(calls.lock().unwrap().len(), if caching { 2 } else { 4 });
            assert_eq!(chunks.load(Ordering::SeqCst), if caching { 2 } else { 4 });
        }
    }

    #[tokio::test]
    async fn when_media_input_is_missing_then_the_endpoint_should_not_call_the_client() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(None, None, calls.clone(), Arc::new(AtomicUsize::new(0)));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/media")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=10"
        );
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_opening_media_fails_then_errors_should_be_mapped_without_forwarding_upstream_redirects_or_partial_statuses()
     {
        for (error, expected, retry) in [
            (
                RedditError::InvalidRequest { field: "media_url" },
                StatusCode::BAD_REQUEST,
                None,
            ),
            (
                RedditError::MediaStatus {
                    status: 404,
                    retry_after: None,
                },
                StatusCode::NOT_FOUND,
                None,
            ),
            (
                RedditError::MediaStatus {
                    status: 206,
                    retry_after: None,
                },
                StatusCode::BAD_GATEWAY,
                None,
            ),
            (
                RedditError::MediaStatus {
                    status: 302,
                    retry_after: None,
                },
                StatusCode::BAD_GATEWAY,
                None,
            ),
            (
                RedditError::MediaStatus {
                    status: 403,
                    retry_after: None,
                },
                StatusCode::BAD_GATEWAY,
                None,
            ),
            (
                RedditError::MediaStatus {
                    status: 429,
                    retry_after: Some(RetryAfter::Delay(Duration::from_secs(3))),
                },
                StatusCode::SERVICE_UNAVAILABLE,
                Some("3"),
            ),
            (
                RedditError::MediaStatus {
                    status: 503,
                    retry_after: Some(RetryAfter::Delay(Duration::from_secs(5))),
                },
                StatusCode::SERVICE_UNAVAILABLE,
                Some("5"),
            ),
            (
                RedditError::InvalidResponse(ResponseError::UnsupportedMediaType),
                StatusCode::BAD_GATEWAY,
                None,
            ),
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let app = app(
                Some(error),
                None,
                calls.clone(),
                Arc::new(AtomicUsize::new(0)),
            );

            let response = app.oneshot(request()).await.unwrap();

            assert_eq!(response.status(), expected);
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/problem+json"
            );
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=10"
            );
            assert_eq!(
                response
                    .headers()
                    .get(header::RETRY_AFTER)
                    .map(|value| value.to_str().unwrap()),
                retry
            );
            assert_eq!(calls.lock().unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_http_downloads_complete_then_known_length_files_should_be_cached_without_range_support()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let chunks = Arc::new(AtomicUsize::new(0));
        let app = cache.wrap(app(None, Some(11), calls.clone(), chunks.clone()));
        let uri = request().uri().to_string();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}{uri}", listener.local_addr().unwrap());
        let server = tokio::spawn(axum::serve(listener, app).into_future());
        let client = wreq::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();

        let head = client.head(&url).send().await.unwrap();
        assert_eq!(head.headers()["x-lurklet-cache"], "miss");
        assert_eq!(head.headers()["content-length"], "11");
        assert!(head.bytes().await.unwrap().is_empty());
        assert_eq!(chunks.load(Ordering::SeqCst), 0);
        for outcome in ["miss", "hit"] {
            let response = client
                .get(&url)
                .header("range", "bytes=0-3")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["content-length"], "11");
            assert_eq!(response.headers()["x-lurklet-cache"], outcome);
            assert!(!response.headers().contains_key("accept-ranges"));
            assert!(!response.headers().contains_key("content-range"));
            assert_eq!(response.bytes().await.unwrap(), b"hello world".as_slice());
        }
        let head = client.head(&url).send().await.unwrap();
        assert_eq!(head.headers()["x-lurklet-cache"], "hit");
        assert!(head.bytes().await.unwrap().is_empty());
        server.abort();

        assert_eq!(calls.lock().unwrap().len(), 2);
        assert_eq!(chunks.load(Ordering::SeqCst), 2);
    }
}
