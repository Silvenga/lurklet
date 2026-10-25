use crate::backends::mobile::MobileBackend;
use crate::backends::mobile::graphql::{PersistedQuery, decode_response};
use crate::backends::mobile::posts::{PostData, Variables};
use crate::{Error, Post, StateStore};

impl<S: StateStore> MobileBackend<S> {
    pub(in crate::backends::mobile) async fn get_post(&self, id: &str) -> Result<Post, Error> {
        let query = PersistedQuery {
            name: "PostsByIds".into(),
            sha256_hash: "29a8d4c6e135fffb95fc4cd303c57ad926c6c5ff582f4c4577c8a69b7f322d91".into(),
        };
        let response = self
            .execute_graphql_post(&query, &Variables::new(id))
            .await?;
        let response = decode_response(response, PostData::into_post);
        tracing::trace!(available = response.data.is_some(), "Completed post lookup");
        response.into_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::mobile::testing::{
        self, GRAPHQL_ORIGIN, PATH, TOKEN_FAKE, TestClock, cached_storage, graphql_client,
    };
    use crate::{RedditClient, ResponseCapture, ResponseError, ResponseObserver, RetryAfter};
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-post.json"
        ))
        .unwrap()
    }

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<ResponseCapture>>>);

    impl ResponseObserver for Recorder {
        fn on_response(&self, capture: ResponseCapture) {
            self.0.lock().unwrap().push(capture);
        }
    }

    #[tokio::test]
    async fn when_a_captured_post_is_replayed_then_the_public_model_should_match_the_sanitized_result()
     {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/captured-post.json"
        ))
        .unwrap();
        let server = MockServer::start().await;
        let reply = ResponseTemplate::new(fixture["status"].as_u64().unwrap() as u16)
            .insert_header(
                "content-type",
                fixture["headers"]["content-type"].as_str().unwrap(),
            )
            .set_body_json(fixture["body"].clone());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(reply)
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let post = client.get_post("fixturepost").await.unwrap();

        assert_eq!(
            serde_json::to_value(post).unwrap(),
            fixture["expected_post"]
        );
    }

    #[tokio::test]
    async fn when_a_post_is_requested_then_the_backend_should_bootstrap_and_send_one_minimal_persisted_request()
     {
        let server = MockServer::start().await;
        let request: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-post-request.json"
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
            .and(header("x-apollo-operation-name", "PostsByIds"))
            .and(header(
                "x-apollo-operation-id",
                "29a8d4c6e135fffb95fc4cd303c57ad926c6c5ff582f4c4577c8a69b7f322d91",
            ))
            .and(body_json(request))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture()))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, testing::storage());

        let post = client.get_post("18t5968").await.unwrap();

        assert_eq!(post.id.as_str(), "18t5968");
        assert_eq!(post.body_markdown.as_deref(), Some("Hello **world**."));
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn when_optional_data_has_errors_then_the_backend_should_keep_the_usable_post_and_raw_capture()
     {
        let server = MockServer::start().await;
        let mut body = fixture();
        body["data"]["postsInfoByIds"][0]["score"] = Value::Null;
        body["errors"] =
            json!([{"message": "private-fixture-error", "path": ["postsInfoByIds", 0, "score"]}]);
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body.clone()))
            .expect(1)
            .mount(&server)
            .await;
        let storage = cached_storage();
        let before = storage.store().load().await.unwrap();
        let recorder = Recorder::default();
        let client = RedditClient::builder()
            .with_storage(storage.clone())
            .with_clock(TestClock::default())
            .with_response_observer(recorder.clone())
            .with_mock_server(GRAPHQL_ORIGIN, &server)
            .build()
            .unwrap();

        let post = client.get_post("18t5968").await.unwrap();

        assert_eq!(post.score, None);
        assert_eq!(post.comment_count, Some(3));
        assert_eq!(storage.store().load().await.unwrap(), before);
        {
            let captures = recorder.0.lock().unwrap();
            assert_eq!(captures.len(), 1);
            assert_eq!(captures[0].operation, "PostsByIds");
            assert_eq!(
                serde_json::from_slice::<Value>(&captures[0].body).unwrap(),
                body
            );
        }
        assert!(
            !serde_json::to_string(&post)
                .unwrap()
                .contains("private-fixture")
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_a_post_is_unavailable_then_the_backend_should_return_an_error_without_recovery_requests()
     {
        let server = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"data": {"postsInfoByIds": [null]}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let result = client.get_post("missing").await;

        assert!(matches!(result, Err(Error::Unavailable)));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_the_persisted_query_is_not_found_then_the_backend_should_report_an_operation_failure()
     {
        let server = MockServer::start().await;
        Mock::given(path("/")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "errors": [{"message": "PersistedQueryNotFound", "extensions": {"code": "NOT_FOUND"}}]
        }))).expect(1).mount(&server).await;
        let client = graphql_client(&server, cached_storage());

        let result = client.get_post("18t5968").await;

        assert!(matches!(result, Err(Error::OperationFailed)));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_a_crosspost_is_returned_then_the_backend_should_not_fetch_its_original() {
        let server = MockServer::start().await;
        let mut body = fixture();
        body["data"]["postsInfoByIds"][0]["crosspostRoot"] =
            json!({"post": {"id": "t3_original", "postTitle": "Original"}});
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let post = client.get_post("18t5968").await.unwrap();

        assert_eq!(
            post.crosspost_id.as_ref().map(|id| id.as_str()),
            Some("original")
        );
        assert_eq!(post.title, "Example title");
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_graphql_returns_no_usable_post_and_errors_then_the_backend_should_not_report_missing_content()
     {
        for data in [
            Value::Null,
            json!({"postsInfoByIds": null}),
            json!({"postsInfoByIds": [null]}),
            json!({"postsInfoByIds": [{"id": "t3_18t5968", "postTitle": null}]}),
        ] {
            let server = MockServer::start().await;
            let body = json!({"data": data, "errors": [{"message": "private-fixture-error", "extensions": {"code": "INVALID_INPUT"}}]});
            Mock::given(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let result = client.get_post("18t5968").await;

            assert!(matches!(result, Err(Error::OperationFailed)));
            assert!(!format!("{result:?}").contains("private-fixture-error"));
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_http_denies_access_then_the_backend_should_preserve_http_failure_without_retrying()
     {
        let server = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(403).set_body_string("<html>blocked</html>"))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let result = client.get_post("18t5968").await;

        assert!(matches!(result, Err(Error::AccessDenied { status: 403 })));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_reddit_rate_limits_the_lookup_then_the_backend_should_preserve_retry_after() {
        let server = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("retry-after", "60")
                    .set_body_json(json!({"errors": [{"message": "rate limited"}]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let result = client.get_post("18t5968").await;

        assert!(
            matches!(result, Err(Error::RateLimited { retry_after: Some(RetryAfter::Delay(delay)) }) if delay == Duration::from_secs(60))
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_the_response_is_malformed_then_the_backend_should_return_a_common_response_error()
    {
        for (body, content_type, expected) in [
            ("not json", "application/json", ResponseError::InvalidJson),
            (
                r#"{"data":{}}"#,
                "application/json",
                ResponseError::InvalidOperationResponse,
            ),
            (
                r#"{"data":{}}"#,
                "multipart/mixed; boundary=fixture",
                ResponseError::InvalidOperationResponse,
            ),
        ] {
            let server = MockServer::start().await;
            Mock::given(path("/"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_string(body)
                        .insert_header("content-type", content_type),
                )
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let result = client.get_post("18t5968").await;

            assert!(matches!(result, Err(Error::InvalidResponse(reason)) if reason == expected));
        }
    }

    #[tokio::test]
    async fn when_the_post_exceeds_the_response_limit_then_the_backend_should_not_expose_an_incomplete_post()
     {
        let server = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture()))
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_storage(cached_storage())
            .with_clock(TestClock::default())
            .with_max_response_bytes(16)
            .with_mock_server(GRAPHQL_ORIGIN, &server)
            .build()
            .unwrap();

        let result = client.get_post("18t5968").await;

        assert!(matches!(
            result,
            Err(Error::InvalidResponse(ResponseError::BodyTooLarge))
        ));
    }
}
