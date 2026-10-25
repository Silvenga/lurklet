use crate::backends::mobile::MobileBackend;
use crate::backends::mobile::graphql::{PersistedQuery, decode_response};
use crate::backends::mobile::subreddits::{SubredditData, Variables};
use crate::{Error, StateStore, Subreddit};

impl<S: StateStore> MobileBackend<S> {
    pub(in crate::backends::mobile) async fn get_subreddit(
        &self,
        name: &str,
    ) -> Result<Subreddit, Error> {
        let query = PersistedQuery {
            name: "SubredditInfoByName".into(),
            sha256_hash: "8c1815b07b762aee3c086cc84306f14b3907018e9ee152046c8e4d08ad4c7b88".into(),
        };
        let response = self
            .execute_graphql_post(&query, &Variables::new(name))
            .await?;
        let decoded = decode_response(response, |data: SubredditData| Ok(data.into_subreddit()));
        tracing::debug!(outcome = ?decoded.outcome(), "Subreddit metadata lookup completed");
        decoded.into_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::mobile::testing::{self, cached_storage, graphql_client};
    use crate::{ResponseCapture, ResponseError, ResponseObserver, RetryAfter};
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn response() -> Value {
        serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-info.json"
        ))
        .unwrap()
    }

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<ResponseCapture>>>);

    impl ResponseObserver for Recorder {
        fn on_response(&self, response: ResponseCapture) {
            self.0.lock().unwrap().push(response);
        }
    }

    #[tokio::test]
    async fn when_captured_communities_are_replayed_then_lookup_should_preserve_counts_styles_and_nullable_banners()
     {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/captured-subreddit-info.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(case["body"].clone()))
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let result = client
                .get_subreddit(case["name"].as_str().unwrap())
                .await
                .unwrap();

            assert_eq!(serde_json::to_value(result).unwrap(), case["expected"]);
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_a_session_is_valid_then_metadata_should_use_one_persisted_post_without_follow_up_requests()
     {
        let server = MockServer::start().await;
        let request: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-info-request.json"
        ))
        .unwrap();
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header(
                "authorization",
                format!("Bearer {}", testing::TOKEN_FAKE),
            ))
            .and(header("content-type", "application/json; charset=utf-8"))
            .and(header("x-apollo-operation-name", "SubredditInfoByName"))
            .and(header(
                "x-apollo-operation-id",
                "8c1815b07b762aee3c086cc84306f14b3907018e9ee152046c8e4d08ad4c7b88",
            ))
            .and(body_json(request))
            .respond_with(ResponseTemplate::new(200).set_body_json(response()))
            .expect(1)
            .mount(&server)
            .await;
        let storage = cached_storage();
        let before = storage.store().load().await.unwrap();
        let client = graphql_client(&server, storage.clone());

        let actual = client.get_subreddit("rust").await.unwrap();
        let after = storage.store().load().await.unwrap();
        let requests = server.received_requests().await.unwrap();

        let expected = serde_json::from_value::<SubredditData>(response()["data"].clone())
            .unwrap()
            .into_subreddit()
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(requests.len(), 1);
        assert_eq!(after, before);
    }

    #[tokio::test]
    async fn when_no_session_exists_then_metadata_should_bootstrap_once_before_reading_the_community()
     {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(testing::PATH))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response()))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, testing::storage());

        let actual = client.get_subreddit("rust").await.unwrap();
        let requests = server.received_requests().await.unwrap();

        assert!(matches!(actual, Subreddit::Available(_)));
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].url.path(), testing::PATH);
        assert_eq!(requests[1].url.path(), "/");
    }

    #[tokio::test]
    async fn when_an_unavailable_object_is_returned_then_lookup_should_succeed_without_attempting_opt_in()
     {
        let server = MockServer::start().await;
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-unavailable.json"
        ))
        .unwrap();
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let actual = client.get_subreddit("example").await.unwrap();

        assert!(
            matches!(actual, Subreddit::Unavailable(fields) if fields.forbidden_reason.as_deref() == Some("BANNED"))
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_partial_metadata_is_usable_then_lookup_should_return_it_and_capture_the_original_errors()
     {
        let server = MockServer::start().await;
        let mut fixture = response();
        fixture["errors"] = json!([{
            "message": "Optional wiki metadata could not be resolved",
            "path": ["subredditInfoByName", "wiki"]
        }]);
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture.clone()))
            .expect(1)
            .mount(&server)
            .await;
        let recorder = Recorder::default();
        let client = crate::RedditClient::builder()
            .with_storage(cached_storage())
            .with_clock(testing::TestClock::default())
            .with_mock_server(testing::GRAPHQL_ORIGIN, &server)
            .with_response_observer(recorder.clone())
            .build()
            .unwrap();

        let actual = client.get_subreddit("rust").await.unwrap();

        assert!(matches!(actual, Subreddit::Available(_)));
        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].operation, "SubredditInfoByName");
        assert_eq!(
            serde_json::from_slice::<Value>(&captures[0].body).unwrap(),
            fixture
        );
        assert!(!captures[0].body_truncated);
    }

    #[tokio::test]
    async fn when_the_resource_is_null_then_lookup_should_report_unavailable_not_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"data": {"subredditInfoByName": null}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let actual = client.get_subreddit("missing").await;

        assert!(matches!(actual, Err(Error::Unavailable)));
    }

    #[tokio::test]
    async fn when_metadata_is_absent_and_graphql_reports_errors_then_lookup_should_preserve_operation_failure()
     {
        for fixture in [
            json!({"data": {"subredditInfoByName": null}, "errors": [{"extensions": {"code": "NOT_FOUND"}}]}),
            json!({"errors": [{"message": "PersistedQueryNotFound", "extensions": {"code": "PERSISTED_QUERY_NOT_FOUND"}}]}),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(fixture))
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let actual = client.get_subreddit("rust").await;

            assert!(matches!(actual, Err(Error::OperationFailed)));
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_metadata_is_malformed_or_has_an_unknown_runtime_type_then_lookup_should_report_a_decode_failure()
     {
        for fixture in [
            json!({"data": {}}),
            json!({"data": {"subredditInfoByName": {"__typename": "FutureSubreddit"}}}),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(fixture))
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let actual = client.get_subreddit("rust").await;

            assert!(matches!(
                actual,
                Err(Error::InvalidResponse(
                    ResponseError::InvalidOperationResponse
                ))
            ));
        }
    }

    #[tokio::test]
    async fn when_http_fails_then_lookup_should_preserve_transport_classification_without_replay() {
        for status in [401, 403, 429, 503] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(
                    ResponseTemplate::new(status)
                        .insert_header("retry-after", "7")
                        .set_body_json(response()),
                )
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let actual = client.get_subreddit("rust").await;

            match status {
                401 => assert!(matches!(actual, Err(Error::Authentication { status: 401 }))),
                403 => assert!(matches!(actual, Err(Error::AccessDenied { status: 403 }))),
                429 => assert!(
                    matches!(actual, Err(Error::RateLimited { retry_after: Some(RetryAfter::Delay(delay)) }) if delay == Duration::from_secs(7))
                ),
                503 => assert!(
                    matches!(actual, Err(Error::Upstream { status: 503, retry_after: Some(RetryAfter::Delay(delay)) }) if delay == Duration::from_secs(7))
                ),
                _ => unreachable!(),
            }
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }
}
