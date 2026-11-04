use crate::error::ApiError;
use crate::routes::{Markdown, PostFlair};
use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
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
    permalink: Option<String>,
    #[schema(required)]
    flair: Option<PostFlair>,
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
    markdown: Option<Markdown>,
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
            permalink,
            flair,
            author,
            community,
            created_at,
            score,
            comment_count,
            is_nsfw,
            is_spoiler,
            markdown,
            attachments,
            crosspost_id,
        } = post;
        Self {
            id: id.as_str().to_owned(),
            title,
            permalink,
            flair: flair.map(Into::into),
            author: author.map(Into::into),
            community: community.map(Into::into),
            created_at,
            score,
            comment_count,
            is_nsfw,
            is_spoiler,
            markdown: markdown.map(Into::into),
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
    Ok(Json(Post::from(post)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiCacheOptions;
    use crate::routes::testing::{Call, FakeBackend};
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use lurklet_reddit::Error;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    fn fixture() -> RedditPost {
        RedditPost {
            id: "18t5968".into(),
            title: "Example post".into(),
            permalink: Some("/r/rust/comments/18t5968/example_post/".into()),
            flair: Some(lurklet_reddit::PostFlair {
                text: Some(":example: News".into()),
                background_color: Some("#228822".into()),
                text_color: Some(lurklet_reddit::FlairTextColor::Light),
                parts: vec![
                    lurklet_reddit::FlairPart::Emoji {
                        url: "https://emoji.redditmedia.com/example/news".into(),
                        alt: ":example:".into(),
                    },
                    lurklet_reddit::FlairPart::Text {
                        text: " News".into(),
                    },
                ],
            }),
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
            markdown: Some("**Untrusted** Markdown".into()),
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

    fn app(result: Result<RedditPost, Error>, calls: Arc<Mutex<Vec<Call>>>) -> Router {
        let client = RedditClient::from_backend(FakeBackend {
            post: Some(result),
            calls,
            ..FakeBackend::default()
        });
        let (router, _) = router().split_for_parts();
        ApiCacheOptions::default().wrap(router.with_state(client))
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
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());

        let response = app.oneshot(request()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();

        assert_eq!(*calls.lock().unwrap(), [Call::Post("18t5968".into())]);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=3600");
        assert_eq!(
            body,
            json!({
                "id": "18t5968",
                "title": "Example post",
                "permalink": "/r/rust/comments/18t5968/example_post/",
                "flair": {"text": ":example: News", "backgroundColor": "#228822", "textColor": "light", "parts": [
                    {"kind": "emoji", "url": "https://emoji.redditmedia.com/example/news", "alt": ":example:"},
                    {"kind": "text", "text": " News"}
                ]},
                "author": {"id": "author", "name": "example-user"},
                "community": {"id": null, "name": "rust"},
                "createdAt": "1970-01-01T00:00:00Z",
                "score": -1,
                "commentCount": 42,
                "isNsfw": false,
                "isSpoiler": true,
                "markdown": {"content": "**Untrusted** Markdown", "media": []},
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
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());
        let request = Request::builder()
            .uri("/posts/t3_18t5968")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(*calls.lock().unwrap(), [Call::Post("18t5968".into())]);
    }

    #[tokio::test]
    async fn when_the_client_fails_then_the_endpoint_should_propagate_problem_details() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Err(Error::Unavailable), calls.clone());

        let response = app.oneshot(request()).await.unwrap();

        assert_eq!(*calls.lock().unwrap(), [Call::Post("18t5968".into())]);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
    }

    #[tokio::test]
    async fn when_the_path_cannot_be_decoded_then_the_endpoint_should_not_call_the_client() {
        let calls = Arc::new(Mutex::new(Vec::new()));
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
        assert!(calls.lock().unwrap().is_empty());
    }
}
