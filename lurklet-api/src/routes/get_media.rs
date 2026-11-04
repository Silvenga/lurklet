use crate::error::ApiError;
use axum::body::Body;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use headers::{ContentLength, ContentRange, HeaderMapExt};
use lurklet_reddit::{Error as RedditError, MediaOptions, RedditBackend, RedditClient};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
struct GetMediaInput {
    /// Complete media URL, encoded as a query parameter; its path and query are forwarded unchanged.
    url: String,
}

/// Builds the streaming media endpoint with single byte-range support.
pub fn router<B: RedditBackend + 'static>() -> OpenApiRouter<RedditClient<B>> {
    OpenApiRouter::new().routes(routes!(get_media))
}

#[utoipa::path(
    get,
    path = "/media",
    operation_id = "get_media",
    summary = "Stream an image or MP4 file, optionally selecting one byte range",
    description = "Single byte ranges are supported. Invalid, unsupported, and multipart Range headers are ignored. HEAD ignores Range. MP4s and ranged requests bypass persistent caching, while successful responses remain cacheable by clients and CDNs.",
    params(GetMediaInput,
        ("Range" = Option<String>, Header, description = "One byte range, such as bytes=0-1023, bytes=1024-, or bytes=-1024"),
        ("If-Range" = Option<String>, Header, description = "Upstream entity tag or HTTP date. Sent only with a supported Range")
    ),
    responses(
        ApiError,
        (status = 200, description = "Complete binary file content", content(("image/*"), ("video/mp4")), headers(
            ("Accept-Ranges" = String, description = "Range support advertised by the upstream"),
            ("ETag" = String, description = "Upstream representation validator, when supplied"),
            ("Last-Modified" = String, description = "Upstream modification date, when supplied")
        )),
        (status = 206, description = "Single byte range", content(("image/*"), ("video/mp4")), headers(
            ("Content-Range" = String, description = "Inclusive byte offsets and complete representation length"),
            ("Content-Length" = u64, description = "Length of this partial response, when supplied"),
            ("Accept-Ranges" = String, description = "Range support advertised by the upstream"),
            ("ETag" = String, description = "Upstream representation validator, when supplied"),
            ("Last-Modified" = String, description = "Upstream modification date, when supplied")
        )),
        (status = 416, description = "Range not satisfiable", headers(
            ("Content-Range" = String, description = "bytes */length, when the upstream supplies a complete length")
        ))
    )
)]
async fn get_media<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    method: Method,
    headers: HeaderMap,
    input: Result<Query<GetMediaInput>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(input) = input.map_err(|_| ApiError::BadRequest)?;
    let range = (method == Method::GET && headers.get_all(header::RANGE).iter().count() == 1)
        .then(|| headers.get(header::RANGE)?.to_str().ok()?.parse().ok())
        .flatten();
    let options = MediaOptions {
        if_range: range
            .as_ref()
            .and_then(|_| headers.get(header::IF_RANGE))
            .and_then(|value| value.to_str().ok()),
        range,
    };
    let media = match reddit.get_media_with_options(&input.url, options).await {
        Err(RedditError::MediaRangeNotSatisfiable { content_length }) => {
            let mut response = (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CACHE_CONTROL, "no-store")],
            )
                .into_response();
            if let Some(length) = content_length {
                response
                    .headers_mut()
                    .typed_insert(ContentRange::unsatisfied_bytes(length));
            }
            return Ok(response);
        }
        result => result.map_err(|error| match error {
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
        })?,
    };
    let content_type =
        HeaderValue::from_str(&media.content_type).map_err(|_| ApiError::BadGateway)?;
    let mut response = Response::new(Body::from_stream(media.body));
    if media.content_range.is_some() {
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
    }
    for (name, value) in [
        (header::CONTENT_RANGE, media.content_range),
        (header::ACCEPT_RANGES, media.accept_ranges),
        (header::ETAG, media.etag),
        (header::LAST_MODIFIED, media.last_modified),
    ] {
        if let Some(value) = value {
            response.headers_mut().insert(
                name,
                HeaderValue::from_str(&value).map_err(|_| ApiError::BadGateway)?,
            );
        }
    }
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    if let Some(length) = media.content_length {
        response.headers_mut().typed_insert(ContentLength(length));
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApiCacheOptions, RateLimits, ResponseCache, application};
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
    use std::fs;
    use std::future::IntoFuture;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    type Calls = Arc<Mutex<Vec<(String, Option<String>, Option<String>)>>>;

    struct FakeBackend {
        error: Option<RedditError>,
        content_type: &'static str,
        kind: MediaKind,
        body: [&'static [u8]; 2],
        length: Option<u64>,
        content_range: Option<&'static str>,
        calls: Calls,
        chunks: Arc<AtomicUsize>,
    }

    impl RedditBackend for FakeBackend {
        async fn get_media(
            &self,
            url: &str,
            options: MediaOptions<'_>,
        ) -> Result<MediaFile, RedditError> {
            self.calls.lock().unwrap().push((
                url.into(),
                options.range.map(|range| range.to_string()),
                options.if_range.map(str::to_owned),
            ));
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            let chunks = self.chunks.clone();
            Ok(MediaFile {
                content_type: self.content_type.into(),
                kind: self.kind,
                content_length: self.length,
                content_range: self.content_range.map(str::to_owned),
                accept_ranges: self.content_range.map(|_| "bytes".into()),
                etag: self.content_range.map(|_| "\"fixture\"".into()),
                last_modified: self
                    .content_range
                    .map(|_| "Wed, 21 Oct 2015 07:28:00 GMT".into()),
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
        calls: Calls,
        chunks: Arc<AtomicUsize>,
    ) -> Router {
        let routes = router()
            .split_for_parts()
            .0
            .with_state(RedditClient::from_backend(FakeBackend {
                error,
                content_type: "video/mp4",
                kind: MediaKind::Mp4,
                body: [b"hello", b" world"],
                length,
                content_range: None,
                calls,
                chunks,
            }));
        ApiCacheOptions::default().wrap(routes)
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
                [(
                    "https://preview.redd.it/file.png?x=2&x=1&s=a%2Bb".into(),
                    None,
                    None
                )]
            );
            assert_eq!(response.headers()[header::CONTENT_TYPE], "video/mp4");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=3600"
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
                content_range: None,
                calls: calls.clone(),
                chunks: chunks.clone(),
            });
            let app = application(
                reddit,
                &RateLimits::default(),
                caching.then_some(&cache),
                &ApiCacheOptions {
                    api_cache_max_age_seconds: 42,
                },
            );

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
                assert_eq!(
                    response.headers()[header::CACHE_CONTROL],
                    "public, max-age=42"
                );
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
    async fn when_range_headers_are_supported_then_the_api_should_forward_selection_and_relay_partial_or_unsatisfiable_results()
     {
        for caching in [false, true] {
            for (range, error, status) in [
                ("bytes=0-3", None, StatusCode::PARTIAL_CONTENT),
                ("bytes=7-", None, StatusCode::PARTIAL_CONTENT),
                ("bytes=-4", None, StatusCode::PARTIAL_CONTENT),
                (
                    "bytes=11-",
                    Some(RedditError::MediaRangeNotSatisfiable {
                        content_length: Some(11),
                    }),
                    StatusCode::RANGE_NOT_SATISFIABLE,
                ),
            ] {
                let directory = assert_fs::TempDir::new().unwrap();
                let cache = ResponseCache::new(directory.path()).await.unwrap();
                let calls = Arc::new(Mutex::new(Vec::new()));
                let reddit = RedditClient::from_backend(FakeBackend {
                    error,
                    content_type: "video/mp4",
                    kind: MediaKind::Mp4,
                    body: [b"fi", b"le"],
                    length: Some(4),
                    content_range: Some(if range == "bytes=0-3" {
                        "bytes 0-3/11"
                    } else {
                        "bytes 7-10/11"
                    }),
                    calls: calls.clone(),
                    chunks: Arc::new(AtomicUsize::new(0)),
                });
                let app = application(
                    reddit,
                    &RateLimits::default(),
                    caching.then_some(&cache),
                    &ApiCacheOptions {
                        api_cache_max_age_seconds: 42,
                    },
                );
                let request = Request::builder()
                    .uri("/api/media?url=https%3A%2F%2Fv.redd.it%2Ffile")
                    .header(header::RANGE, range)
                    .header(header::IF_RANGE, "\"fixture\"")
                    .header(header::ACCEPT_ENCODING, "gzip, br, deflate, zstd")
                    .extension(ConnectInfo("192.0.2.1:1234".parse::<SocketAddr>().unwrap()))
                    .body(Body::empty())
                    .unwrap();

                let response = app.oneshot(request).await.unwrap();

                assert_eq!(response.status(), status);
                assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
                assert!(!response.headers().contains_key("x-lurklet-cache"));
                assert_eq!(
                    *calls.lock().unwrap(),
                    [(
                        "https://v.redd.it/file".into(),
                        Some(range.into()),
                        Some("\"fixture\"".into())
                    )]
                );
                if status == StatusCode::PARTIAL_CONTENT {
                    assert_eq!(
                        response.headers()[header::CACHE_CONTROL],
                        "public, max-age=42"
                    );
                    assert_eq!(response.headers()[header::CONTENT_LENGTH], "4");
                    assert_eq!(response.headers()[header::CONTENT_TYPE], "video/mp4");
                    assert_eq!(
                        response.headers()[header::CONTENT_RANGE],
                        if range == "bytes=0-3" {
                            "bytes 0-3/11"
                        } else {
                            "bytes 7-10/11"
                        }
                    );
                    assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
                    assert_eq!(response.headers()[header::ETAG], "\"fixture\"");
                    assert_eq!(
                        response.headers()[header::LAST_MODIFIED],
                        "Wed, 21 Oct 2015 07:28:00 GMT"
                    );
                    assert_eq!(
                        to_bytes(response.into_body(), 4096).await.unwrap(),
                        b"file".as_slice()
                    );
                } else {
                    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */11");
                    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                    assert!(
                        to_bytes(response.into_body(), 4096)
                            .await
                            .unwrap()
                            .is_empty()
                    );
                }
                assert_eq!(
                    fs::read_dir(directory.path().join("v1")).unwrap().count(),
                    0
                );
                assert_eq!(
                    fs::read_dir(directory.path().join(".tmp")).unwrap().count(),
                    0
                );
            }
        }
    }

    #[tokio::test]
    async fn when_range_headers_are_unsupported_or_the_method_is_head_then_the_api_should_request_a_complete_file()
     {
        for (method, ranges) in [
            (Method::HEAD, vec!["bytes=0-3"]),
            (Method::GET, vec!["bytes=0-3,7-10"]),
            (Method::GET, vec!["items=0-3"]),
            (Method::GET, vec!["bytes=3-0"]),
            (Method::GET, vec!["bytes=0-3", "bytes=7-10"]),
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let app = app(None, Some(11), calls.clone(), Arc::new(AtomicUsize::new(0)));
            let mut request = request();
            *request.method_mut() = method.clone();
            for range in ranges {
                request
                    .headers_mut()
                    .append(header::RANGE, range.parse().unwrap());
            }
            request
                .headers_mut()
                .insert(header::IF_RANGE, "\"fixture\"".parse().unwrap());

            let response = app.oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_LENGTH], "11");
            assert_eq!(
                *calls.lock().unwrap(),
                [(
                    "https://preview.redd.it/file.png?x=2&x=1&s=a%2Bb".into(),
                    None,
                    None
                )]
            );
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            assert_eq!(
                body.as_ref(),
                if method == Method::HEAD {
                    b"".as_slice()
                } else {
                    b"hello world".as_slice()
                }
            );
        }
    }

    #[tokio::test]
    async fn when_mp4_downloads_complete_then_full_and_range_requests_should_bypass_persistent_caching()
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
        assert!(!head.headers().contains_key("x-lurklet-cache"));
        assert_eq!(head.headers()["content-length"], "11");
        assert!(head.bytes().await.unwrap().is_empty());
        assert_eq!(chunks.load(Ordering::SeqCst), 0);
        for range in [None, Some("bytes=0-3"), None, Some("bytes=-3")] {
            let mut request = client.get(&url);
            if let Some(range) = range {
                request = request.header("range", range);
            }
            let response = request.send().await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["content-length"], "11");
            assert!(!response.headers().contains_key("x-lurklet-cache"));
            assert_eq!(response.headers()["cache-control"], "public, max-age=3600");
            assert!(!response.headers().contains_key("accept-ranges"));
            assert!(!response.headers().contains_key("content-range"));
            assert_eq!(response.bytes().await.unwrap(), b"hello world".as_slice());
        }
        let head = client.head(&url).send().await.unwrap();
        assert!(!head.headers().contains_key("x-lurklet-cache"));
        assert!(head.bytes().await.unwrap().is_empty());
        server.abort();

        assert_eq!(calls.lock().unwrap().len(), 6);
        assert_eq!(chunks.load(Ordering::SeqCst), 8);
        assert_eq!(
            fs::read_dir(directory.path().join("v1")).unwrap().count(),
            0
        );
        assert_eq!(
            fs::read_dir(directory.path().join(".tmp")).unwrap().count(),
            0
        );
    }
}
