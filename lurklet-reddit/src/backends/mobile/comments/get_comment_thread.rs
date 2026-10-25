use crate::backends::mobile::MobileBackend;
use crate::backends::mobile::comments::{Forest, ThreadVariables, View};
use crate::backends::mobile::graphql::{PersistedQuery, decode_response};
use crate::{CommentSort, CommentThread, Error, RedditId, StateStore};
use serde::Deserialize;

#[derive(Deserialize)]
struct ThreadData {
    #[serde(rename = "commentById", deserialize_with = "Option::deserialize")]
    comment: Option<Thread>,
}

#[derive(Deserialize)]
struct Thread {
    #[serde(rename = "postInfo")]
    post: Option<PostReference>,
    #[serde(deserialize_with = "Option::deserialize")]
    children: Option<Forest>,
}

#[derive(Deserialize)]
struct PostReference {
    id: Option<RedditId>,
}

impl<S: StateStore> MobileBackend<S> {
    pub(in crate::backends::mobile) async fn get_comment_thread(
        &self,
        comment_id: &RedditId,
        sort: CommentSort,
    ) -> Result<CommentThread, Error> {
        let query = PersistedQuery {
            name: "GetCommentByIdWithChildren".into(),
            sha256_hash: "c40b533f29205854251e1b705b34f1eb71f93d9f65ace97861be4c74f91e5be7".into(),
        };
        let response = self
            .execute_graphql_post(&query, &ThreadVariables::new(comment_id, sort))
            .await?;
        decode_response(response, |data: ThreadData| {
            data.comment
                .and_then(|thread| thread.children.map(|forest| (thread.post, forest)))
                .map(|(post, forest)| {
                    forest
                        .into_forest(View::Thread)
                        .map(|forest| CommentThread {
                            post_id: post.and_then(|post| post.id),
                            forest,
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
    use crate::CommentItem;
    use crate::backends::mobile::testing::{cached_storage, graphql_client};
    use serde_json::{Value, json};
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn when_a_thread_is_opened_then_the_backend_should_use_the_comment_operation_and_keep_ancestor_context()
     {
        let server = MockServer::start().await;
        let request: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-comment-thread-request.json"
        ))
        .unwrap();
        let body: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-comment-thread.json"
        ))
        .unwrap();
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header(
                "x-apollo-operation-name",
                "GetCommentByIdWithChildren",
            ))
            .and(header(
                "x-apollo-operation-id",
                "c40b533f29205854251e1b705b34f1eb71f93d9f65ace97861be4c74f91e5be7",
            ))
            .and(body_json(request))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let thread = client
            .get_comment_thread(&"anchor".into(), CommentSort::Top)
            .await
            .unwrap();

        assert_eq!(thread.post_id, Some("example".into()));
        let CommentItem::Comment { comment, .. } = &thread.forest.items[0] else {
            panic!("expected ancestor")
        };
        assert_eq!(comment.id, None);
        assert!(
            !serde_json::to_string(&thread)
                .unwrap()
                .contains("future-focused-cursor")
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_a_thread_or_its_forest_is_null_then_the_backend_should_report_unavailable_content()
     {
        for data in [
            json!({"commentById": null}),
            json!({"commentById": {"postInfo": null, "children": null}}),
        ] {
            let server = MockServer::start().await;
            Mock::given(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": data})))
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let result = client
                .get_comment_thread(&"anchor".into(), CommentSort::Best)
                .await;

            assert!(matches!(result, Err(Error::Unavailable)));
        }
    }
}
