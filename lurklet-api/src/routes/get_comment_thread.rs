use crate::error::ApiError;
use crate::routes::{CommentForest, CommentSort};
use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use lurklet_reddit::{CommentThread as RedditCommentThread, RedditBackend, RedditClient, RedditId};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

const CACHE_POLICY: &str = "public, max-age=300";

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Path)]
struct GetCommentThreadInput {
    /// Comment ID, including an anchor returned by a continueThread placeholder.
    #[param(value_type = String)]
    id: RedditId,
}

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
struct GetCommentThreadQuery {
    #[serde(default)]
    #[param(inline, required = false)]
    sort: CommentSort,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CommentThread {
    #[schema(required)]
    post_id: Option<String>,
    forest: CommentForest,
}

impl From<RedditCommentThread> for CommentThread {
    fn from(thread: RedditCommentThread) -> Self {
        let RedditCommentThread { post_id, forest } = thread;
        Self {
            post_id: post_id.map(|id| id.as_str().to_owned()),
            forest: forest.into(),
        }
    }
}

/// Builds the focused comment-thread endpoint and its API-owned contract.
pub fn router<B: RedditBackend + 'static>() -> OpenApiRouter<RedditClient<B>> {
    OpenApiRouter::new().routes(routes!(get_comment_thread))
}

#[utoipa::path(
    get,
    path = "/comments/{id}/thread",
    operation_id = "get_comment_thread",
    summary = "Open a comment thread with ancestor context",
    description = "The forest may include ancestors of the requested comment. It is not an inline replacement fragment, and further expansion in this view is not supported.",
    params(GetCommentThreadInput, GetCommentThreadQuery),
    responses(
        ApiError,
        (status = 200, description = "Thread context and its post reference", body = CommentThread)
    )
)]
async fn get_comment_thread<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    input: Result<Path<GetCommentThreadInput>, PathRejection>,
    query: Result<Query<GetCommentThreadQuery>, QueryRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Path(input) = input.map_err(|_| ApiError::BadRequest)?;
    let Query(query) = query.map_err(|_| ApiError::BadRequest)?;
    let thread = reddit
        .get_comment_thread(&input.id, query.sort.into())
        .await?;
    Ok((
        [(header::CACHE_CONTROL, CACHE_POLICY)],
        Json(CommentThread::from(thread)),
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
        AnonymousSession, Comment, CommentForest as RedditCommentForest, CommentItem,
        CommentSort as RedditCommentSort, CommentState, Error as RedditError, Post, SubredditPage,
        SubredditSort,
    };
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    #[derive(Debug, PartialEq)]
    struct ThreadCall {
        id: RedditId,
        sort: RedditCommentSort,
    }

    struct FakeBackend {
        result: Result<RedditCommentThread, RedditError>,
        calls: Arc<Mutex<Vec<ThreadCall>>>,
    }

    impl RedditBackend for FakeBackend {
        async fn get_comment_thread(
            &self,
            id: &RedditId,
            sort: RedditCommentSort,
        ) -> Result<RedditCommentThread, RedditError> {
            self.calls.lock().unwrap().push(ThreadCall {
                id: id.clone(),
                sort,
            });
            self.result.clone()
        }

        async fn get_post_comments(
            &self,
            _: &RedditId,
            _: RedditCommentSort,
            _: Option<&str>,
        ) -> Result<RedditCommentForest, RedditError> {
            panic!("the thread endpoint must not fetch or expand the post discussion")
        }

        async fn get_post(&self, _: &RedditId) -> Result<Post, RedditError> {
            panic!("the thread endpoint must not fetch post details")
        }

        async fn list_subreddit_posts(
            &self,
            _: &str,
            _: SubredditSort,
            _: Option<&str>,
        ) -> Result<SubredditPage, RedditError> {
            panic!("the thread endpoint must not list subreddit posts")
        }

        async fn bootstrap(&self) -> Result<AnonymousSession, RedditError> {
            panic!("the endpoint must leave bootstrap to the client backend")
        }
    }

    fn fixture() -> RedditCommentThread {
        let ancestor = Comment {
            id: None,
            state: CommentState::Deleted,
            author: None,
            created_at: None,
            body_markdown: None,
            score: None,
            is_op: None,
            is_pinned: None,
            initially_collapsed: None,
        };
        let child = Comment {
            id: Some("anchor".into()),
            state: CommentState::Present,
            body_markdown: Some("A reply".into()),
            ..ancestor.clone()
        };
        RedditCommentThread {
            post_id: Some("post".into()),
            forest: RedditCommentForest {
                applied_sort: Some(RedditCommentSort::New),
                items: vec![CommentItem::Comment {
                    comment: ancestor,
                    replies: vec![CommentItem::Comment {
                        comment: child,
                        replies: vec![
                            CommentItem::More {
                                cursor: None,
                                count_hint: Some(2),
                            },
                            CommentItem::ContinueThread { comment_id: None },
                        ],
                    }],
                }],
            },
        }
    }

    fn app(
        result: Result<RedditCommentThread, RedditError>,
        calls: Arc<Mutex<Vec<ThreadCall>>>,
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
    async fn when_a_thread_is_requested_then_ids_and_sorts_should_be_forwarded_without_extra_reads()
    {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());

        for (uri, sort) in [
            ("/comments/anchor/thread", RedditCommentSort::Best),
            (
                "/comments/t1_anchor/thread?sort=best",
                RedditCommentSort::Best,
            ),
            ("/comments/anchor/thread?sort=top", RedditCommentSort::Top),
            ("/comments/anchor/thread?sort=new", RedditCommentSort::New),
        ] {
            let response = app.clone().oneshot(request(uri)).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=300"
            );
            assert_eq!(
                *calls.lock().unwrap(),
                [ThreadCall {
                    id: "anchor".into(),
                    sort
                }]
            );
            calls.lock().unwrap().clear();
        }
    }

    #[tokio::test]
    async fn when_a_thread_contains_ancestors_then_the_response_should_preserve_context_and_nonactionable_placeholders()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());

        let response = app
            .oneshot(request("/comments/anchor/thread"))
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), 16_384).await.unwrap();

        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({
                "postId": "post",
                "forest": {
                    "appliedSort": "new",
                    "items": [{
                        "kind": "comment",
                        "comment": {
                            "id": null, "state": "deleted", "author": null, "createdAt": null,
                            "bodyMarkdown": null, "score": null, "isOp": null,
                            "isPinned": null, "initiallyCollapsed": null
                        },
                        "replies": [{
                            "kind": "comment",
                            "comment": {
                                "id": "anchor", "state": "present", "author": null, "createdAt": null,
                                "bodyMarkdown": "A reply", "score": null, "isOp": null,
                                "isPinned": null, "initiallyCollapsed": null
                            },
                            "replies": [
                                {"kind": "more", "cursor": null, "countHint": 2},
                                {"kind": "continueThread", "commentId": null}
                            ]
                        }]
                    }]
                }
            })
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_thread_inputs_cannot_be_decoded_then_the_client_should_not_be_called() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(fixture()), calls.clone());

        for uri in [
            "/comments/%FF/thread",
            "/comments/anchor/thread?sort=hot",
            "/comments/anchor/thread?sort=BEST",
            "/comments/anchor/thread?sort=best&sort=new",
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
    async fn when_a_thread_is_unavailable_then_the_endpoint_should_use_shared_problem_details() {
        for error in [RedditError::Unavailable, RedditError::NotFound] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let app = app(Err(error), calls.clone());

            let response = app
                .oneshot(request("/comments/anchor/thread"))
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=10"
            );
            assert_eq!(
                serde_json::from_slice::<Value>(
                    &to_bytes(response.into_body(), 4096).await.unwrap()
                )
                .unwrap(),
                json!({"type": "about:blank", "title": "Not Found", "status": 404})
            );
            assert_eq!(calls.lock().unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_a_thread_has_no_post_reference_and_an_empty_forest_then_the_endpoint_should_preserve_both()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(
            Ok(RedditCommentThread {
                post_id: None,
                forest: RedditCommentForest {
                    applied_sort: None,
                    items: Vec::new(),
                },
            }),
            calls.clone(),
        );

        let response = app
            .oneshot(request("/comments/anchor/thread"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            serde_json::from_slice::<Value>(&to_bytes(response.into_body(), 4096).await.unwrap())
                .unwrap(),
            json!({"postId": null, "forest": {"appliedSort": null, "items": []}})
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_thread_requests_differ_then_caching_should_separate_ids_and_sorts() {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = cache.wrap(app(Ok(fixture()), calls.clone()));

        for (uri, outcome) in [
            ("/comments/anchor/thread?sort=best", "miss"),
            ("/comments/anchor/thread?sort=best", "hit"),
            ("/comments/anchor/thread?sort=top", "miss"),
            ("/comments/other/thread?sort=best", "miss"),
        ] {
            let response = app.clone().oneshot(request(uri)).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            assert_eq!(response.headers()["x-lurklet-cache"], outcome, "{uri}");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=300"
            );
            assert!(response.headers().contains_key(header::AGE));
        }
        let mut head = request("/comments/anchor/thread?sort=best");
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
        assert_eq!(calls.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn when_a_thread_refresh_fails_then_the_stored_context_and_policy_should_be_preserved() {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let seed = cache.wrap(Router::new().route(
            "/comments/anchor/thread",
            get(|| async {
                (
                    [(header::CACHE_CONTROL, "public, max-age=0")],
                    Json(CommentThread::from(fixture())),
                )
            }),
        ));
        let response = seed
            .oneshot(request("/comments/anchor/thread"))
            .await
            .unwrap();
        let expected = to_bytes(response.into_body(), 16_384).await.unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = cache.wrap(app(Err(RedditError::Unavailable), calls.clone()));

        let response = app
            .oneshot(request("/comments/anchor/thread"))
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
