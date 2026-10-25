use crate::error::ApiError;
use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use lurklet_reddit::{
    NamedReference as RedditNamedReference, Post as RedditPost,
    PostAttachment as RedditPostAttachment, RedditBackend, RedditClient, RedditId,
    VideoFormat as RedditVideoFormat, VideoSource as RedditVideoSource,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

const CACHE_POLICY: &str = "public, max-age=300";

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Path)]
struct GetPostInput {
    /// Post ID, as returned by the API.
    #[param(value_type = String)]
    id: RedditId,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct Post {
    id: String,
    title: String,
    #[schema(required)]
    author: Option<NamedReference>,
    #[schema(required)]
    community: Option<NamedReference>,
    created_at: DateTime<Utc>,
    #[schema(required)]
    score: Option<i64>,
    #[schema(required)]
    comment_count: Option<u64>,
    is_nsfw: bool,
    is_spoiler: bool,
    /// Untrusted Markdown; embedded media URLs are not proxied.
    #[schema(required)]
    body_markdown: Option<String>,
    attachments: Vec<PostAttachment>,
    #[schema(required)]
    crosspost_id: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct NamedReference {
    #[schema(required)]
    id: Option<String>,
    #[schema(required)]
    name: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum PostAttachment {
    Link {
        url: String,
    },
    #[serde(rename_all = "camelCase")]
    Image {
        url: String,
        #[schema(required)]
        alt_text: Option<String>,
        #[schema(required)]
        caption: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Video {
        sources: Vec<VideoSource>,
        #[schema(required)]
        poster_url: Option<String>,
    },
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct VideoSource {
    url: String,
    format: VideoFormat,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
enum VideoFormat {
    Mp4,
    Hls,
    Dash,
}

impl From<RedditPost> for Post {
    fn from(post: RedditPost) -> Self {
        let RedditPost {
            id,
            title,
            author,
            community,
            created_at,
            score,
            comment_count,
            is_nsfw,
            is_spoiler,
            body_markdown,
            attachments,
            crosspost_id,
        } = post;
        Self {
            id: id.as_str().to_owned(),
            title,
            author: author.map(Into::into),
            community: community.map(Into::into),
            created_at,
            score,
            comment_count,
            is_nsfw,
            is_spoiler,
            body_markdown,
            attachments: attachments.into_iter().map(Into::into).collect(),
            crosspost_id: crosspost_id.map(|id| id.as_str().to_owned()),
        }
    }
}

impl From<RedditNamedReference> for NamedReference {
    fn from(reference: RedditNamedReference) -> Self {
        let RedditNamedReference { id, name } = reference;
        Self {
            id: id.map(|id| id.as_str().to_owned()),
            name,
        }
    }
}

impl From<RedditPostAttachment> for PostAttachment {
    fn from(attachment: RedditPostAttachment) -> Self {
        match attachment {
            RedditPostAttachment::Link { url } => Self::Link { url },
            RedditPostAttachment::Image {
                url,
                alt_text,
                caption,
            } => Self::Image {
                url,
                alt_text,
                caption,
            },
            RedditPostAttachment::Video {
                sources,
                poster_url,
            } => Self::Video {
                sources: sources.into_iter().map(Into::into).collect(),
                poster_url,
            },
        }
    }
}

impl From<RedditVideoSource> for VideoSource {
    fn from(source: RedditVideoSource) -> Self {
        let RedditVideoSource { url, format } = source;
        Self {
            url,
            format: match format {
                RedditVideoFormat::Mp4 => VideoFormat::Mp4,
                RedditVideoFormat::Hls => VideoFormat::Hls,
                RedditVideoFormat::Dash => VideoFormat::Dash,
            },
        }
    }
}

/// Builds the post endpoint and its API-owned contract.
pub fn router<B: RedditBackend + 'static>() -> OpenApiRouter<RedditClient<B>> {
    OpenApiRouter::new().routes(routes!(get_post))
}

#[utoipa::path(
    get,
    path = "/posts/{id}",
    operation_id = "get_post",
    summary = "Retrieve one post without comments or related posts",
    params(GetPostInput),
    responses(
        ApiError,
        (status = 200, description = "A readable post", body = Post)
    )
)]
async fn get_post<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    input: Result<Path<GetPostInput>, PathRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Path(input) = input.map_err(|_| ApiError::BadRequest)?;
    let post = reddit.get_post(&input.id).await?;
    Ok((
        [(header::CACHE_CONTROL, CACHE_POLICY)],
        Json(Post::from(post)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResponseCache;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode};
    use axum::routing::get;
    use lurklet_reddit::{
        AnonymousSession, ClockError, CommentForest, CommentSort, CommentThread, Error,
        ResponseError, RetryAfter, StorageError, Subreddit, SubredditPage, SubredditSort,
    };
    use serde_json::{Value, json};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, UNIX_EPOCH};
    use tower::ServiceExt;

    struct FakeBackend {
        result: Result<RedditPost, Error>,
        calls: Arc<AtomicUsize>,
    }

    impl RedditBackend for FakeBackend {
        async fn get_subreddit(&self, _: &str) -> Result<Subreddit, Error> {
            panic!("the post endpoint must not fetch subreddit metadata")
        }

        async fn get_post_comments(
            &self,
            _: &RedditId,
            _: CommentSort,
            _: Option<&str>,
        ) -> Result<CommentForest, Error> {
            panic!("the post endpoint must not fetch comments")
        }

        async fn get_comment_thread(
            &self,
            _: &RedditId,
            _: CommentSort,
        ) -> Result<CommentThread, Error> {
            panic!("the post endpoint must not fetch comment threads")
        }

        async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
            panic!("the endpoint must leave bootstrap to the client backend")
        }

        async fn get_post(&self, id: &RedditId) -> Result<RedditPost, Error> {
            assert_eq!(id, &"18t5968".into());
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.result.clone()
        }

        async fn list_subreddit_posts(
            &self,
            _: &str,
            _: SubredditSort,
            _: Option<&str>,
        ) -> Result<SubredditPage, Error> {
            panic!("the post endpoint must not list subreddit posts")
        }
    }

    fn fixture() -> RedditPost {
        RedditPost {
            id: "18t5968".into(),
            title: "Example post".into(),
            author: Some(RedditNamedReference {
                id: Some("author".into()),
                name: Some("example-user".into()),
            }),
            community: Some(RedditNamedReference {
                id: None,
                name: Some("rust".into()),
            }),
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            score: Some(-1),
            comment_count: Some(42),
            is_nsfw: false,
            is_spoiler: true,
            body_markdown: Some("**Untrusted** Markdown".into()),
            attachments: vec![
                RedditPostAttachment::Link {
                    url: "https://example.test/link".into(),
                },
                RedditPostAttachment::Image {
                    url: "https://example.test/image.png".into(),
                    alt_text: Some("Image description".into()),
                    caption: None,
                },
                RedditPostAttachment::Video {
                    sources: vec![
                        RedditVideoSource {
                            url: "https://example.test/video.mp4".into(),
                            format: RedditVideoFormat::Mp4,
                        },
                        RedditVideoSource {
                            url: "https://example.test/video.m3u8".into(),
                            format: RedditVideoFormat::Hls,
                        },
                        RedditVideoSource {
                            url: "https://example.test/video.mpd".into(),
                            format: RedditVideoFormat::Dash,
                        },
                    ],
                    poster_url: Some("https://example.test/poster.png".into()),
                },
            ],
            crosspost_id: Some("original".into()),
        }
    }

    fn app(result: Result<RedditPost, Error>, calls: Arc<AtomicUsize>) -> Router {
        let client = RedditClient::from_backend(FakeBackend { result, calls });
        let (router, _) = router().split_for_parts();
        router.with_state(client)
    }

    fn request() -> Request<Body> {
        Request::builder()
            .uri("/posts/18t5968")
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn when_a_post_is_requested_then_the_endpoint_should_return_its_api_model_and_cache_policy()
     {
        let calls = Arc::new(AtomicUsize::new(0));
        let app = app(Ok(fixture()), calls.clone());

        let response = app.oneshot(request()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=300");
        assert_eq!(
            body,
            json!({
                "id": "18t5968",
                "title": "Example post",
                "author": {"id": "author", "name": "example-user"},
                "community": {"id": null, "name": "rust"},
                "createdAt": "1970-01-01T00:00:00Z",
                "score": -1,
                "commentCount": 42,
                "isNsfw": false,
                "isSpoiler": true,
                "bodyMarkdown": "**Untrusted** Markdown",
                "attachments": [
                    {"kind": "link", "url": "https://example.test/link"},
                    {"kind": "image", "url": "https://example.test/image.png", "altText": "Image description", "caption": null},
                    {"kind": "video", "sources": [
                        {"url": "https://example.test/video.mp4", "format": "mp4"},
                        {"url": "https://example.test/video.m3u8", "format": "hls"},
                        {"url": "https://example.test/video.mpd", "format": "dash"}
                    ], "posterUrl": "https://example.test/poster.png"}
                ],
                "crosspostId": "original"
            })
        );
    }

    #[tokio::test]
    async fn when_a_post_fullname_is_supplied_then_the_client_should_receive_the_bare_id() {
        let calls = Arc::new(AtomicUsize::new(0));
        let app = app(Ok(fixture()), calls.clone());
        let request = Request::builder()
            .uri("/posts/t3_18t5968")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn when_the_client_fails_then_problem_details_should_be_cacheable_for_ten_seconds() {
        for (error, status, retry_after) in [
            (Error::Unavailable, StatusCode::NOT_FOUND, None),
            (Error::NotFound, StatusCode::NOT_FOUND, None),
            (
                Error::UnexpectedStatus { status: 404 },
                StatusCode::BAD_GATEWAY,
                None,
            ),
            (
                Error::Authentication { status: 401 },
                StatusCode::BAD_GATEWAY,
                None,
            ),
            (
                Error::AccessDenied { status: 403 },
                StatusCode::BAD_GATEWAY,
                None,
            ),
            (
                Error::InvalidResponse(ResponseError::InvalidPostResponse),
                StatusCode::BAD_GATEWAY,
                None,
            ),
            (Error::OperationFailed, StatusCode::BAD_GATEWAY, None),
            (
                Error::RateLimited { retry_after: None },
                StatusCode::SERVICE_UNAVAILABLE,
                None,
            ),
            (
                Error::RateLimited {
                    retry_after: Some(RetryAfter::Delay(Duration::from_secs(120))),
                },
                StatusCode::SERVICE_UNAVAILABLE,
                Some("120"),
            ),
            (
                Error::Upstream {
                    status: 503,
                    retry_after: Some(RetryAfter::At(UNIX_EPOCH + Duration::from_secs(1445412480))),
                },
                StatusCode::SERVICE_UNAVAILABLE,
                Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            ),
            (
                Error::Storage(StorageError::Unavailable),
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            ),
            (
                Error::Clock(ClockError::BeforeEpoch),
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            ),
            (
                Error::InvalidState {
                    field: "private-state",
                },
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            ),
            (
                Error::InvalidRequest {
                    field: "private-field",
                },
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            ),
            (
                Error::SessionChanged,
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            ),
        ] {
            let calls = Arc::new(AtomicUsize::new(0));
            let app = app(Err(error), calls.clone());

            let response = app.oneshot(request()).await.unwrap();
            let actual_status = response.status();
            let headers = response.headers().clone();
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap())
                    .unwrap();

            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(actual_status, status);
            assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
            assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=10");
            assert_eq!(
                headers
                    .get(header::RETRY_AFTER)
                    .map(|value| value.to_str().unwrap()),
                retry_after
            );
            assert_eq!(
                body,
                json!({"type": "about:blank", "title": status.canonical_reason().unwrap(), "status": status.as_u16()})
            );
        }
    }

    #[tokio::test]
    async fn when_the_path_cannot_be_decoded_then_the_endpoint_should_not_call_the_client() {
        let calls = Arc::new(AtomicUsize::new(0));
        let app = app(Ok(fixture()), calls.clone());
        let request = Request::builder()
            .uri("/posts/%FF")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=10"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn when_a_post_is_cached_then_get_and_head_should_reuse_it_without_calling_the_client() {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let app = cache.wrap(app(Ok(fixture()), calls.clone()));

        for (method, outcome) in [
            (Method::GET, "miss"),
            (Method::GET, "hit"),
            (Method::HEAD, "hit"),
        ] {
            let mut request = request();
            *request.method_mut() = method.clone();

            let response = app.clone().oneshot(request).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 16384).await.unwrap();

            assert_eq!(status, StatusCode::OK);
            assert_eq!(headers["x-lurklet-cache"], outcome);
            assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=300");
            assert!(headers.contains_key(header::AGE));
            assert_eq!(body.is_empty(), method == Method::HEAD);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn when_an_expired_post_becomes_unavailable_then_the_endpoint_should_serve_stale_content()
    {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let seed = cache.wrap(Router::new().route(
            "/posts/{id}",
            get(|| async {
                (
                    [(header::CACHE_CONTROL, "public, max-age=0")],
                    Json(Post::from(fixture())),
                )
            }),
        ));
        let response = seed.oneshot(request()).await.unwrap();
        let previous = to_bytes(response.into_body(), 16384).await.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let app = cache.wrap(app(Err(Error::Unavailable), calls.clone()));

        let response = app.oneshot(request()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 16384).await.unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["x-lurklet-cache"], "stale");
        assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=0");
        assert!(headers.contains_key(header::AGE));
        assert_eq!(body, previous);
    }
}
