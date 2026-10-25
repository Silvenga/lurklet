use crate::backends::mobile::MobileBackend;
use crate::backends::mobile::comments::{Continuation, Forest, PostVariables, View};
use crate::backends::mobile::graphql::{PersistedQuery, decode_response};
use crate::{CommentForest, CommentSort, Error, StateStore};
use serde::Deserialize;

#[derive(Deserialize)]
struct PostCommentsData {
    #[serde(rename = "postInfoById", deserialize_with = "Option::deserialize")]
    post: Option<PostComments>,
}

#[derive(Deserialize)]
struct PostComments {
    #[serde(rename = "commentForest", deserialize_with = "Option::deserialize")]
    forest: Option<Forest>,
}

impl<S: StateStore> MobileBackend<S> {
    pub(in crate::backends::mobile) async fn get_post_comments(
        &self,
        post_id: &str,
        sort: CommentSort,
        cursor: Option<&str>,
    ) -> Result<CommentForest, Error> {
        let continuation = cursor
            .map(|cursor| Continuation::decode(cursor, post_id, sort))
            .transpose()?;
        let query = PersistedQuery {
            name: "PostComments".into(),
            sha256_hash: "314f29c2494fd78c7844ef2c6b09acb4bef5b37ab50b4ddc01691d6d5c8433f8".into(),
        };
        let variables = PostVariables::new(
            post_id,
            sort,
            continuation.as_ref().map(|cursor| cursor.after.as_str()),
        );
        let response = self.execute_graphql_post(&query, &variables).await?;
        decode_response(response, |data: PostCommentsData| {
            data.post
                .and_then(|post| post.forest)
                .map(|forest| {
                    forest.into_forest(View::Post {
                        post_id,
                        sort,
                        continuation: continuation.as_ref(),
                    })
                })
                .transpose()
        })
        .into_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::mobile::testing::{
        self, PATH, TOKEN_FAKE, cached_storage, graphql_client,
    };
    use crate::{CommentItem, ResponseError};
    use serde_json::{Value, json};
    use wiremock::matchers::{body_json, body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-post-comments.json"
        ))
        .unwrap()
    }

    #[tokio::test]
    async fn when_initial_comments_need_authentication_then_the_backend_should_bootstrap_and_send_one_persisted_read()
     {
        let server = MockServer::start().await;
        let request: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-post-comments-request.json"
        ))
        .unwrap();
        Mock::given(path(PATH))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", format!("Bearer {TOKEN_FAKE}")))
            .and(header("x-apollo-operation-name", "PostComments"))
            .and(header(
                "x-apollo-operation-id",
                "314f29c2494fd78c7844ef2c6b09acb4bef5b37ab50b4ddc01691d6d5c8433f8",
            ))
            .and(body_json(request))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture()))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, testing::storage());

        let forest = client
            .get_post_comments("example", CommentSort::Best, None)
            .await
            .unwrap();

        assert_eq!(forest.items.len(), 3);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn when_a_nested_placeholder_is_expanded_then_the_backend_should_use_the_post_operation_and_preserve_branch_depth()
     {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .and(|request: &Request| {
                serde_json::from_slice::<Value>(&request.body).unwrap()["variables"]
                    .get("after")
                    .is_none()
            })
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture()))
            .expect(1)
            .mount(&server)
            .await;
        let fragment = json!({"data": {"postInfoById": {"commentForest": {"appliedSort": "CONFIDENCE", "trees": [
            {"depth": 2, "parentId": "t1_child", "node": {"__typename": "Comment", "id": "t1_expanded", "content": {"markdown": "More reply"}}}
        ], "pageInfo": {"hasNextPage": false}}}}});
        Mock::given(method("POST")).and(path("/"))
            .and(header("x-apollo-operation-name", "PostComments"))
            .and(body_partial_json(json!({"variables": {"id": "t3_example", "after": "nested-more+/=", "count": 200, "maxDepth": 10, "truncate": 0}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(fragment)).expect(1).mount(&server).await;
        let storage = cached_storage();
        let before = storage.store().load().await.unwrap();
        let client = graphql_client(&server, storage.clone());

        let initial = client
            .get_post_comments("example", CommentSort::Best, None)
            .await
            .unwrap();
        let CommentItem::Comment { replies, .. } = &initial.items[0] else {
            panic!("expected parent")
        };
        let CommentItem::Comment { replies, .. } = &replies[0] else {
            panic!("expected child")
        };
        let CommentItem::More {
            cursor: Some(cursor),
            ..
        } = &replies[0]
        else {
            panic!("expected continuation")
        };
        let expanded = client
            .get_post_comments("example", CommentSort::Best, Some(cursor))
            .await
            .unwrap();

        assert_eq!(expanded.items.len(), 1);
        let CommentItem::Comment { comment, replies } = &expanded.items[0] else {
            panic!("expected replacement root")
        };
        assert_eq!(comment.id.as_ref().unwrap().as_str(), "expanded");
        assert!(replies.is_empty());
        assert_eq!(storage.store().load().await.unwrap(), before);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn when_an_ordinary_root_cursor_is_expanded_then_the_backend_should_not_change_to_thread_routing()
     {
        let server = MockServer::start().await;
        Mock::given(path("/"))
            .and(header("x-apollo-operation-name", "PostComments"))
            .and(body_partial_json(
                json!({"variables": {"id": "t3_example", "after": "root-more", "count": 200}}),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({"data": {"postInfoById": {"commentForest": {"trees": []}}}}),
                ),
            )
            .expect(1)
            .mount(&server)
            .await;
        let cursor = Continuation {
            post_id: "example".into(),
            sort: CommentSort::Top,
            after: "root-more".into(),
            depth: 0,
            parent_id: None,
        }
        .encode()
        .unwrap();
        let client = graphql_client(&server, cached_storage());

        let forest = client
            .get_post_comments("example", CommentSort::Top, Some(&cursor))
            .await
            .unwrap();

        assert!(forest.items.is_empty());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_a_comment_cursor_has_invalid_context_then_the_backend_should_fail_before_authentication()
     {
        let server = MockServer::start().await;
        let client = graphql_client(&server, testing::storage());
        let wrong = Continuation {
            post_id: "other".into(),
            sort: CommentSort::Best,
            after: "opaque".into(),
            depth: 0,
            parent_id: None,
        }
        .encode()
        .unwrap();

        for token in ["invalid!", wrong.as_str()] {
            let result = client
                .get_post_comments("example", CommentSort::Best, Some(token))
                .await;

            assert!(matches!(
                result,
                Err(Error::InvalidRequest { field: "cursor" })
            ));
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_comment_resources_are_null_then_the_backend_should_not_report_empty_success_or_not_found()
     {
        for data in [
            Value::Null,
            json!({"postInfoById": null}),
            json!({"postInfoById": {"commentForest": null}}),
        ] {
            let server = MockServer::start().await;
            Mock::given(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": data})))
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let result = client
                .get_post_comments("example", CommentSort::Best, None)
                .await;

            assert!(matches!(result, Err(Error::Unavailable)));
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_errors_affect_only_unused_comment_fields_then_the_backend_should_keep_the_usable_forest()
     {
        let server = MockServer::start().await;
        let mut body = fixture();
        body["errors"] = json!([{"message": "private-fixture-error", "path": ["postInfoById", "commentForest", "trees", 0, "node", "moderationInfo"]}]);
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let forest = client
            .get_post_comments("example", CommentSort::Best, None)
            .await
            .unwrap();

        assert_eq!(forest.items.len(), 3);
        assert!(
            !serde_json::to_string(&forest)
                .unwrap()
                .contains("private-fixture-error")
        );
    }

    #[tokio::test]
    async fn when_comment_requests_fail_then_the_backend_should_preserve_common_failures_without_recovery_requests()
     {
        for (status, body, kind) in [
            (403, "<html>blocked</html>", "access"),
            (
                200,
                r#"{"errors":[{"message":"PersistedQueryNotFound","extensions":{"code":"NOT_FOUND"}}]}"#,
                "operation",
            ),
            (200, r#"{"data":{"postInfoById":{}}}"#, "data"),
        ] {
            let server = MockServer::start().await;
            Mock::given(path("/"))
                .respond_with(ResponseTemplate::new(status).set_body_string(body))
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let result = client
                .get_post_comments("example", CommentSort::Best, None)
                .await;

            match kind {
                "access" => assert!(matches!(result, Err(Error::AccessDenied { status: 403 }))),
                "operation" => assert!(matches!(result, Err(Error::OperationFailed))),
                "data" => assert!(matches!(
                    result,
                    Err(Error::InvalidResponse(
                        ResponseError::InvalidOperationResponse
                    ))
                )),
                _ => unreachable!(),
            }
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }
}
