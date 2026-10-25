use crate::error::ApiError;
use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use lurklet_reddit::{
    Error as RedditError, ListedPost as RedditListedPost, RedditBackend, RedditClient,
    SubredditPage as RedditSubredditPage, SubredditSort as RedditSubredditSort,
    Thumbnail as RedditThumbnail, TimeRange as RedditTimeRange,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

const CACHE_POLICY: &str = "public, max-age=300";

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Path)]
struct ListSubredditPostsInput {
    /// Community name, without the r/ prefix.
    subreddit: String,
}

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
struct ListSubredditPostsQuery {
    #[serde(default)]
    #[param(inline, required = false)]
    sort: Sort,
    /// Only valid with top sorting; defaults to all.
    #[param(inline)]
    time_range: Option<TimeRange>,
    /// Pass nextCursor unchanged with the same subreddit, sort, and time range.
    after: Option<String>,
}

#[derive(Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(default = "hot")]
enum Sort {
    #[default]
    Hot,
    New,
    Top,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
enum TimeRange {
    Hour,
    Day,
    Week,
    Month,
    Year,
    All,
}

impl From<TimeRange> for RedditTimeRange {
    fn from(range: TimeRange) -> Self {
        match range {
            TimeRange::Hour => Self::Hour,
            TimeRange::Day => Self::Day,
            TimeRange::Week => Self::Week,
            TimeRange::Month => Self::Month,
            TimeRange::Year => Self::Year,
            TimeRange::All => Self::All,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct SubredditPage {
    posts: Vec<ListedPost>,
    #[schema(required)]
    next_cursor: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListedPost {
    id: String,
    #[schema(required)]
    title: Option<String>,
    #[schema(required)]
    thumbnail: Option<Thumbnail>,
    #[schema(required)]
    score: Option<i64>,
    #[schema(required)]
    comment_count: Option<u64>,
    #[schema(required)]
    author_name: Option<String>,
    #[schema(required)]
    created_at: Option<DateTime<Utc>>,
    is_nsfw: bool,
    is_spoiler: bool,
    is_pinned: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct Thumbnail {
    url: String,
    #[schema(required)]
    obfuscated_url: Option<String>,
    #[schema(required)]
    is_obfuscated: Option<bool>,
    #[schema(required)]
    width: Option<u32>,
    #[schema(required)]
    height: Option<u32>,
}

impl From<RedditSubredditPage> for SubredditPage {
    fn from(page: RedditSubredditPage) -> Self {
        let RedditSubredditPage { posts, next_cursor } = page;
        Self {
            posts: posts.into_iter().map(Into::into).collect(),
            next_cursor,
        }
    }
}

impl From<RedditListedPost> for ListedPost {
    fn from(post: RedditListedPost) -> Self {
        let RedditListedPost {
            id,
            title,
            thumbnail,
            score,
            comment_count,
            author_name,
            created_at,
            is_nsfw,
            is_spoiler,
            is_pinned,
        } = post;
        Self {
            id: id.as_str().to_owned(),
            title,
            thumbnail: thumbnail.map(Into::into),
            score,
            comment_count,
            author_name,
            created_at,
            is_nsfw,
            is_spoiler,
            is_pinned,
        }
    }
}

impl From<RedditThumbnail> for Thumbnail {
    fn from(thumbnail: RedditThumbnail) -> Self {
        let RedditThumbnail {
            url,
            obfuscated_url,
            is_obfuscated,
            width,
            height,
        } = thumbnail;
        Self {
            url,
            obfuscated_url,
            is_obfuscated,
            width,
            height,
        }
    }
}

/// Builds the subreddit listing endpoint and its API-owned contract.
pub fn router<B: RedditBackend + 'static>() -> OpenApiRouter<RedditClient<B>> {
    OpenApiRouter::new().routes(routes!(list_subreddit_posts))
}

#[utoipa::path(
    get,
    path = "/subreddits/{subreddit}/posts",
    operation_id = "list_subreddit_posts",
    summary = "List one page of subreddit posts without fetching post details",
    params(ListSubredditPostsInput, ListSubredditPostsQuery),
    responses(
        ApiError,
        (status = 200, description = "A page of posts with an optional continuation token", body = SubredditPage)
    )
)]
async fn list_subreddit_posts<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    input: Result<Path<ListSubredditPostsInput>, PathRejection>,
    query: Result<Query<ListSubredditPostsQuery>, QueryRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Path(input) = input.map_err(|_| ApiError::BadRequest)?;
    let Query(query) = query.map_err(|_| ApiError::BadRequest)?;
    let sort = match (query.sort, query.time_range) {
        (Sort::Hot, None) => RedditSubredditSort::Hot,
        (Sort::New, None) => RedditSubredditSort::New,
        (Sort::Top, range) => {
            RedditSubredditSort::Top(range.map(Into::into).unwrap_or(RedditTimeRange::All))
        }
        (Sort::Hot | Sort::New, Some(_)) => return Err(ApiError::BadRequest),
    };
    let page = reddit
        .list_subreddit_posts(&input.subreddit, sort, query.after.as_deref())
        .await
        .map_err(|error| match error {
            RedditError::InvalidRequest { field: "after" } => ApiError::BadRequest,
            error => error.into(),
        })?;
    Ok((
        [(header::CACHE_CONTROL, CACHE_POLICY)],
        Json(SubredditPage::from(page)),
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
        AnonymousSession, CommentForest, CommentSort, CommentThread, Post as RedditPost, RedditId,
    };
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    #[derive(Debug, PartialEq)]
    struct ListingCall {
        subreddit: String,
        sort: RedditSubredditSort,
        after: Option<String>,
    }

    struct FakeBackend {
        result: Result<RedditSubredditPage, RedditError>,
        calls: Arc<Mutex<Vec<ListingCall>>>,
    }

    impl RedditBackend for FakeBackend {
        async fn get_post_comments(
            &self,
            _: &RedditId,
            _: CommentSort,
            _: Option<&str>,
        ) -> Result<CommentForest, RedditError> {
            panic!("the listing endpoint must not fetch comments")
        }

        async fn get_comment_thread(
            &self,
            _: &RedditId,
            _: CommentSort,
        ) -> Result<CommentThread, RedditError> {
            panic!("the listing endpoint must not fetch comment threads")
        }

        async fn bootstrap(&self) -> Result<AnonymousSession, RedditError> {
            panic!("the endpoint must leave bootstrap to the client backend")
        }

        async fn get_post(&self, _: &RedditId) -> Result<RedditPost, RedditError> {
            panic!("the listing endpoint must not fetch post details")
        }

        async fn list_subreddit_posts(
            &self,
            subreddit: &str,
            sort: RedditSubredditSort,
            after: Option<&str>,
        ) -> Result<RedditSubredditPage, RedditError> {
            self.calls.lock().unwrap().push(ListingCall {
                subreddit: subreddit.into(),
                sort,
                after: after.map(Into::into),
            });
            self.result.clone()
        }
    }

    fn fixture() -> RedditSubredditPage {
        RedditSubredditPage {
            posts: vec![
                RedditListedPost {
                    id: "first".into(),
                    title: Some("First post".into()),
                    thumbnail: Some(RedditThumbnail {
                        url: "https://example.test/image.png".into(),
                        obfuscated_url: Some("https://example.test/blurred.png".into()),
                        is_obfuscated: Some(true),
                        width: Some(100),
                        height: None,
                    }),
                    score: Some(-1),
                    comment_count: Some(42),
                    author_name: Some("example-user".into()),
                    created_at: Some(DateTime::from_timestamp(0, 0).unwrap()),
                    is_nsfw: true,
                    is_spoiler: false,
                    is_pinned: true,
                },
                RedditListedPost {
                    id: "second".into(),
                    title: None,
                    thumbnail: None,
                    score: None,
                    comment_count: None,
                    author_name: None,
                    created_at: None,
                    is_nsfw: false,
                    is_spoiler: false,
                    is_pinned: false,
                },
            ],
            next_cursor: Some("next+/=% token".into()),
        }
    }

    fn app(
        result: Result<RedditSubredditPage, RedditError>,
        calls: Arc<Mutex<Vec<ListingCall>>>,
    ) -> Router {
        let reddit = RedditClient::from_backend(FakeBackend { result, calls });
        router()
            .split_for_parts()
            .0
            .method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed })
            .with_state(reddit)
    }

    fn request(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    #[test]
    fn when_the_listing_contract_is_generated_then_indicator_flags_should_be_required_booleans() {
        let specification = router::<FakeBackend>().into_openapi();

        let document = serde_json::to_value(specification).unwrap();
        let schema = &document["components"]["schemas"]["ListedPost"];

        for field in ["isNsfw", "isSpoiler", "isPinned"] {
            assert_eq!(schema["properties"][field]["type"], "boolean");
            assert!(
                schema["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(field))
            );
        }
    }

    #[test]
    fn when_the_contract_is_generated_then_query_choices_should_be_inline_and_camel_case() {
        let specification = router::<FakeBackend>().into_openapi();

        let document = serde_json::to_value(specification).unwrap();
        let parameters = &document["paths"]["/subreddits/{subreddit}/posts"]["get"]["parameters"];

        assert_eq!(
            parameters[1],
            json!({
                "name": "sort", "in": "query", "required": false,
                "schema": {"type": "string", "default": "hot", "enum": ["hot", "new", "top"]}
            })
        );
        assert_eq!(
            parameters[2],
            json!({
                "name": "timeRange", "in": "query", "required": false,
                "description": "Only valid with top sorting; defaults to all.",
                "schema": {"type": "string", "enum": ["hour", "day", "week", "month", "year", "all"]}
            })
        );
    }

    #[tokio::test]
    async fn when_a_listing_is_requested_then_inputs_should_be_forwarded_with_the_documented_defaults()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());

        for (query, sort, after) in [
            ("", RedditSubredditSort::Hot, None),
            ("?sort=hot", RedditSubredditSort::Hot, None),
            ("?sort=new", RedditSubredditSort::New, None),
            (
                "?sort=top",
                RedditSubredditSort::Top(RedditTimeRange::All),
                None,
            ),
            (
                "?sort=top&timeRange=hour",
                RedditSubredditSort::Top(RedditTimeRange::Hour),
                None,
            ),
            (
                "?sort=top&timeRange=day",
                RedditSubredditSort::Top(RedditTimeRange::Day),
                None,
            ),
            (
                "?sort=top&timeRange=week",
                RedditSubredditSort::Top(RedditTimeRange::Week),
                None,
            ),
            (
                "?sort=top&timeRange=month",
                RedditSubredditSort::Top(RedditTimeRange::Month),
                None,
            ),
            (
                "?sort=top&timeRange=year",
                RedditSubredditSort::Top(RedditTimeRange::Year),
                None,
            ),
            (
                "?sort=top&timeRange=all",
                RedditSubredditSort::Top(RedditTimeRange::All),
                None,
            ),
            (
                "?sort=new&after=opaque%2B%2F%3D%25%20token",
                RedditSubredditSort::New,
                Some("opaque+/=% token"),
            ),
        ] {
            let request = request(&format!("/subreddits/Rust/posts{query}"));

            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK, "{query}");
            assert_eq!(
                *calls.lock().unwrap(),
                [ListingCall {
                    subreddit: "Rust".into(),
                    sort,
                    after: after.map(Into::into),
                }],
                "{query}"
            );
            calls.lock().unwrap().clear();
        }
    }

    #[tokio::test]
    async fn when_inputs_are_invalid_then_the_endpoint_should_reject_them_before_calling_the_client()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());

        for uri in [
            "/subreddits/%FF/posts",
            "/subreddits/rust/posts?sort=best",
            "/subreddits/rust/posts?sort=HOT",
            "/subreddits/rust/posts?sort=hot&sort=new",
            "/subreddits/rust/posts?timeRange=week",
            "/subreddits/rust/posts?sort=hot&timeRange=day",
            "/subreddits/rust/posts?sort=new&timeRange=all",
            "/subreddits/rust/posts?sort=top&timeRange=invalid",
            "/subreddits/rust/posts?sort=top&timeRange=DAY",
        ] {
            let response = app.clone().oneshot(request(uri)).await.unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=10"
            );
            assert_eq!(
                serde_json::from_slice::<Value>(
                    &to_bytes(response.into_body(), 4096).await.unwrap()
                )
                .unwrap(),
                json!({"type": "about:blank", "title": "Bad Request", "status": 400})
            );
        }
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_a_page_is_returned_then_the_api_model_should_preserve_order_nulls_and_camel_case_fields()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());

        let response = app
            .oneshot(request("/subreddits/rust/posts"))
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 16_384).await.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=300");
        assert_eq!(calls.lock().unwrap().len(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({
                "posts": [
                    {
                        "id": "first", "title": "First post",
                        "thumbnail": {
                            "url": "https://example.test/image.png",
                            "obfuscatedUrl": "https://example.test/blurred.png",
                            "isObfuscated": true, "width": 100, "height": null
                        },
                        "score": -1, "commentCount": 42, "authorName": "example-user",
                        "createdAt": "1970-01-01T00:00:00Z",
                        "isNsfw": true, "isSpoiler": false, "isPinned": true
                    },
                    {
                        "id": "second", "title": null, "thumbnail": null,
                        "score": null, "commentCount": null, "authorName": null,
                        "createdAt": null, "isNsfw": false, "isSpoiler": false, "isPinned": false
                    }
                ],
                "nextCursor": "next+/=% token"
            })
        );
    }

    #[tokio::test]
    async fn when_a_page_is_empty_then_the_endpoint_should_preserve_its_continuation_without_fetching_more()
     {
        for next_cursor in [None, Some("next-token".to_owned())] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let app = app(
                Ok(RedditSubredditPage {
                    posts: Vec::new(),
                    next_cursor: next_cursor.clone(),
                }),
                calls.clone(),
            );

            let response = app
                .oneshot(request("/subreddits/rust/posts"))
                .await
                .unwrap();
            let status = response.status();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                json!({"posts": [], "nextCursor": next_cursor})
            );
            assert_eq!(calls.lock().unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_the_client_rejects_a_listing_then_the_endpoint_should_distinguish_missing_content_and_invalid_cursors()
     {
        for (error, status) in [
            (RedditError::NotFound, StatusCode::NOT_FOUND),
            (RedditError::Unavailable, StatusCode::NOT_FOUND),
            (
                RedditError::InvalidRequest { field: "after" },
                StatusCode::BAD_REQUEST,
            ),
            (
                RedditError::InvalidRequest { field: "variables" },
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let app = app(Err(error), calls.clone());

            let response = app
                .oneshot(request("/subreddits/rust/posts?after=token"))
                .await
                .unwrap();

            assert_eq!(response.status(), status);
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/problem+json"
            );
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=10"
            );
            assert_eq!(
                serde_json::from_slice::<Value>(
                    &to_bytes(response.into_body(), 4096).await.unwrap()
                )
                .unwrap(),
                json!({"type": "about:blank", "title": status.canonical_reason().unwrap(), "status": status.as_u16()})
            );
            assert_eq!(calls.lock().unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_listing_queries_differ_then_the_cache_should_keep_pages_separate_and_reuse_equivalent_requests()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = cache.wrap(app(Ok(fixture()), calls.clone()));

        for (uri, outcome) in [
            ("/subreddits/rust/posts?sort=new&after=one", "miss"),
            ("/subreddits/rust/posts?after=one&sort=new", "hit"),
            ("/subreddits/rust/posts?sort=new&after=two", "miss"),
            ("/subreddits/rust/posts?sort=hot&after=one", "miss"),
            ("/subreddits/rust/posts?sort=top&timeRange=day", "miss"),
            ("/subreddits/rust/posts?sort=top&timeRange=week", "miss"),
            ("/subreddits/other/posts?sort=new&after=one", "miss"),
        ] {
            let response = app.clone().oneshot(request(uri)).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            assert_eq!(response.headers()["x-lurklet-cache"], outcome, "{uri}");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=300"
            );
        }
        let mut head = request("/subreddits/rust/posts?sort=new&after=one");
        *head.method_mut() = Method::HEAD;

        let response = app.oneshot(head).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-lurklet-cache"], "hit");
        assert!(
            to_bytes(response.into_body(), 16_384)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(calls.lock().unwrap().len(), 6);
    }

    #[tokio::test]
    async fn when_an_expired_listing_cannot_refresh_then_the_endpoint_should_serve_the_stored_page()
    {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let seed = cache.wrap(Router::new().route(
            "/subreddits/rust/posts",
            get(|| async {
                (
                    [(header::CACHE_CONTROL, "public, max-age=0")],
                    Json(SubredditPage::from(fixture())),
                )
            }),
        ));
        let response = seed
            .oneshot(request("/subreddits/rust/posts"))
            .await
            .unwrap();
        let expected = to_bytes(response.into_body(), 16_384).await.unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = cache.wrap(app(Err(RedditError::NotFound), calls.clone()));

        let response = app
            .oneshot(request("/subreddits/rust/posts"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-lurklet-cache"], "stale");
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=0"
        );
        assert!(response.headers().contains_key(header::AGE));
        assert_eq!(
            to_bytes(response.into_body(), 16_384).await.unwrap(),
            expected
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
    }
}
