use crate::error::ApiError;
use crate::routes::{CommentForest, CommentSort};
use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use lurklet_reddit::{Error as RedditError, RedditBackend, RedditClient, RedditId};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

const CACHE_POLICY: &str = "public, max-age=300";

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Path)]
struct GetPostCommentsInput {
    /// Post ID, as returned by the API.
    #[param(value_type = String)]
    id: RedditId,
}

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
struct GetPostCommentsQuery {
    #[serde(default)]
    #[param(inline, required = false)]
    sort: CommentSort,
    /// Pass a selected placeholder's cursor unchanged with the same post and requested sort.
    cursor: Option<String>,
}

/// Builds the post-comments endpoint and its API-owned contract.
pub fn router<B: RedditBackend + 'static>() -> OpenApiRouter<RedditClient<B>> {
    OpenApiRouter::new().routes(routes!(get_post_comments))
}

#[utoipa::path(
    get,
    path = "/posts/{id}/comments",
    operation_id = "get_post_comments",
    summary = "Read a post's comments or expand one selected placeholder",
    description = "Without a cursor, returns the initial forest. With a cursor, the returned items replace only the selected placeholder, not the whole discussion.",
    params(GetPostCommentsInput, GetPostCommentsQuery),
    responses(
        ApiError,
        (status = 200, description = "Ordered comments and expansion placeholders", body = CommentForest)
    )
)]
async fn get_post_comments<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    input: Result<Path<GetPostCommentsInput>, PathRejection>,
    query: Result<Query<GetPostCommentsQuery>, QueryRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Path(input) = input.map_err(|_| ApiError::BadRequest)?;
    let Query(query) = query.map_err(|_| ApiError::BadRequest)?;
    let forest = reddit
        .get_post_comments(&input.id, query.sort.into(), query.cursor.as_deref())
        .await
        .map_err(|error| match error {
            RedditError::InvalidRequest { field: "cursor" } => ApiError::BadRequest,
            error => error.into(),
        })?;
    Ok((
        [(header::CACHE_CONTROL, CACHE_POLICY)],
        Json(CommentForest::from(forest)),
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
        AnonymousSession, CommentForest as RedditCommentForest, CommentItem,
        CommentSort as RedditCommentSort, CommentThread, MediaFile, Post, Subreddit, SubredditPage,
        SubredditSort,
    };
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    #[derive(Debug, PartialEq)]
    struct CommentCall {
        id: RedditId,
        sort: RedditCommentSort,
        cursor: Option<String>,
    }

    struct FakeBackend {
        result: Result<RedditCommentForest, RedditError>,
        calls: Arc<Mutex<Vec<CommentCall>>>,
    }

    impl RedditBackend for FakeBackend {
        async fn get_media(&self, _: &str) -> Result<MediaFile, RedditError> {
            panic!("the comments endpoint must not fetch media")
        }

        async fn get_subreddit(&self, _: &str) -> Result<Subreddit, RedditError> {
            panic!("the comments endpoint must not fetch subreddit metadata")
        }

        async fn get_post_comments(
            &self,
            id: &RedditId,
            sort: RedditCommentSort,
            cursor: Option<&str>,
        ) -> Result<RedditCommentForest, RedditError> {
            self.calls.lock().unwrap().push(CommentCall {
                id: id.clone(),
                sort,
                cursor: cursor.map(Into::into),
            });
            self.result.clone()
        }

        async fn get_comment_thread(
            &self,
            _: &RedditId,
            _: RedditCommentSort,
        ) -> Result<CommentThread, RedditError> {
            panic!("the post-comments endpoint must not open a focused thread")
        }

        async fn get_post(&self, _: &RedditId) -> Result<Post, RedditError> {
            panic!("the comments endpoint must not fetch post details")
        }

        async fn list_subreddit_posts(
            &self,
            _: &str,
            _: SubredditSort,
            _: Option<&str>,
        ) -> Result<SubredditPage, RedditError> {
            panic!("the comments endpoint must not list subreddit posts")
        }

        async fn bootstrap(&self) -> Result<AnonymousSession, RedditError> {
            panic!("the endpoint must leave bootstrap to the client backend")
        }
    }

    fn fixture() -> RedditCommentForest {
        RedditCommentForest {
            applied_sort: Some(RedditCommentSort::Top),
            items: vec![CommentItem::More {
                cursor: Some("next-token".into()),
                count_hint: Some(3),
            }],
        }
    }

    fn app(
        result: Result<RedditCommentForest, RedditError>,
        calls: Arc<Mutex<Vec<CommentCall>>>,
    ) -> Router {
        router()
            .split_for_parts()
            .0
            .with_state(RedditClient::from_backend(FakeBackend { result, calls }))
    }

    fn request(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn when_comments_are_requested_then_ids_sorts_and_cursors_should_be_forwarded_without_extra_reads()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());

        for (uri, sort, cursor) in [
            ("/posts/post/comments", RedditCommentSort::Best, None),
            (
                "/posts/t3_post/comments?sort=best",
                RedditCommentSort::Best,
                None,
            ),
            (
                "/posts/post/comments?sort=top",
                RedditCommentSort::Top,
                None,
            ),
            (
                "/posts/post/comments?sort=new",
                RedditCommentSort::New,
                None,
            ),
            (
                "/posts/post/comments?sort=new&cursor=opaque%2B%2F%3D%25%20token",
                RedditCommentSort::New,
                Some("opaque+/=% token"),
            ),
        ] {
            let response = app.clone().oneshot(request(uri)).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::OK, "{uri}");
            assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=300");
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                json!({
                    "appliedSort": "top", "items": [{"kind": "more", "cursor": "next-token", "countHint": 3}]
                })
            );
            assert_eq!(
                *calls.lock().unwrap(),
                [CommentCall {
                    id: "post".into(),
                    sort,
                    cursor: cursor.map(Into::into)
                }]
            );
            calls.lock().unwrap().clear();
        }
    }

    #[tokio::test]
    async fn when_inputs_cannot_be_decoded_then_the_client_should_not_be_called() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());

        for uri in [
            "/posts/%FF/comments",
            "/posts/post/comments?sort=hot",
            "/posts/post/comments?sort=BEST",
            "/posts/post/comments?sort=best&sort=new",
        ] {
            let response = app.clone().oneshot(request(uri)).await.unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=10"
            );
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/problem+json"
            );
        }
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_the_client_rejects_comments_then_invalid_cursors_and_missing_content_should_have_distinct_statuses()
     {
        for (error, status) in [
            (
                RedditError::InvalidRequest { field: "cursor" },
                StatusCode::BAD_REQUEST,
            ),
            (
                RedditError::InvalidRequest { field: "variables" },
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (RedditError::Unavailable, StatusCode::NOT_FOUND),
            (RedditError::NotFound, StatusCode::NOT_FOUND),
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let app = app(Err(error), calls.clone());

            let response = app
                .oneshot(request("/posts/post/comments?cursor=token"))
                .await
                .unwrap();

            assert_eq!(response.status(), status);
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
    async fn when_the_forest_is_empty_then_the_endpoint_should_return_success_without_fabricating_comments()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(
            Ok(RedditCommentForest {
                applied_sort: None,
                items: Vec::new(),
            }),
            calls.clone(),
        );

        let response = app.oneshot(request("/posts/post/comments")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            serde_json::from_slice::<Value>(&to_bytes(response.into_body(), 4096).await.unwrap())
                .unwrap(),
            json!({"appliedSort": null, "items": []})
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_comment_requests_differ_then_caching_should_separate_ids_sorts_and_placeholders()
    {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = cache.wrap(app(Ok(fixture()), calls.clone()));

        for (uri, outcome) in [
            ("/posts/post/comments?sort=best", "miss"),
            ("/posts/post/comments?sort=best", "hit"),
            ("/posts/post/comments?sort=top", "miss"),
            ("/posts/other/comments?sort=best", "miss"),
            ("/posts/post/comments?sort=best&cursor=a", "miss"),
            ("/posts/post/comments?cursor=a&sort=best", "hit"),
            ("/posts/post/comments?sort=best&cursor=b", "miss"),
        ] {
            let response = app.clone().oneshot(request(uri)).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            assert_eq!(response.headers()["x-lurklet-cache"], outcome, "{uri}");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=300"
            );
            assert!(response.headers().contains_key(header::AGE));
            to_bytes(response.into_body(), 16_384).await.unwrap();
        }
        let mut head = request("/posts/post/comments?sort=best");
        *head.method_mut() = Method::HEAD;

        let response = app.oneshot(head).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-lurklet-cache"], "hit");
        assert!(
            to_bytes(response.into_body(), 4096)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(calls.lock().unwrap().len(), 5);
    }

    #[tokio::test]
    async fn when_a_comment_refresh_fails_then_the_stored_forest_and_policy_should_be_preserved() {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let seed = cache.wrap(Router::new().route(
            "/posts/post/comments",
            get(|| async {
                (
                    [(header::CACHE_CONTROL, "public, max-age=0")],
                    Json(CommentForest::from(fixture())),
                )
            }),
        ));
        let response = seed.oneshot(request("/posts/post/comments")).await.unwrap();
        let expected = to_bytes(response.into_body(), 4096).await.unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = cache.wrap(app(Err(RedditError::Unavailable), calls.clone()));

        let response = app.oneshot(request("/posts/post/comments")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-lurklet-cache"], "stale");
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=0"
        );
        assert!(response.headers().contains_key(header::AGE));
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            expected
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
    }
}
