use crate::mobile::backend::MobileBackend;
use crate::mobile::graphql::{PersistedQuery, encode_post};
use crate::mobile::http_response::HttpResponse;
use crate::{Error, RedditBackend, StateStore, TransportError};
use serde::Serialize;
use std::time::Instant;
use wreq::header::{HeaderMap, HeaderName, HeaderValue};

impl<S: StateStore> MobileBackend<S> {
    /// Sends one persisted POST and returns the unclassified response, with no replay or fallback.
    pub(super) async fn execute_graphql_post<V: Serialize + Sync + ?Sized>(
        &self,
        query: &PersistedQuery,
        variables: &V,
    ) -> Result<HttpResponse, Error> {
        let body = encode_post(query, variables)
            .map_err(|_| Error::InvalidRequest { field: "variables" })?;
        let mut operation_headers = HeaderMap::new();
        for (name, value) in [
            ("content-type", "application/json; charset=utf-8"),
            ("x-apollo-operation-name", query.name.as_str()),
            ("x-apollo-operation-id", query.sha256_hash.as_str()),
            ("accept", self.config.graphql_accept.as_str()),
            ("x-reddit-retry", self.config.graphql_retry_header.as_str()),
        ] {
            let value =
                HeaderValue::from_str(value).map_err(|_| Error::InvalidRequest { field: name })?;
            operation_headers.insert(HeaderName::from_static(name), value);
        }

        self.bootstrap().await?;
        let state = self.storage.store().load().await?.state;
        let mut headers = self.api_headers(&state)?;
        headers.extend(operation_headers);
        tracing::trace!(operation = query.name, "Sending persisted GraphQL POST");
        let started = Instant::now();
        let response = self
            .http
            .post(&self.config.graphql_endpoint)
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(TransportError::from)?;
        Ok(self.read_response(&query.name, response, started).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile::backend::testing::{
        self, PATH, TOKEN_FAKE, TestClock, cached_storage, graphql_client, query,
    };
    use crate::{RedditClient, ResponseCapture, ResponseObserver, Storage};
    use serde::Serializer;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{body_json, header, headers, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<ResponseCapture>>>);

    impl ResponseObserver for Recorder {
        fn on_response(&self, response: ResponseCapture) {
            self.0.lock().unwrap().push(response);
        }
    }

    #[tokio::test]
    async fn when_authentication_is_needed_then_post_should_use_a_separate_bearer_request() {
        let server = MockServer::start().await;
        let query = query();
        let variables = json!({"ids": ["t3_one", "t3_one"], "extra": null});
        Mock::given(method("POST"))
            .and(path(PATH))
            .and(header("authorization", "Basic b2hYcG9xclpZdWIxa2c6"))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST")).and(path("/"))
            .and(header("authorization", format!("Bearer {TOKEN_FAKE}")))
            .and(header("content-type", "application/json; charset=utf-8"))
            .and(header("x-apollo-operation-name", "ReadFixture"))
            .and(header("x-apollo-operation-id", query.sha256_hash.as_str()))
            .and(headers("x-reddit-retry", vec!["attempt=0", "max=3"]))
            .and(headers("accept", vec!["multipart/mixed;deferSpec=20220824", "application/graphql-response+json", "application/json"]))
            .and(header("x-reddit-device-id", "12345678-1234-4234-8234-123456789abc"))
            .and(header("x-reddit-loid", "opaque-fixture-loid"))
            .and(body_json(json!({"operationName": query.name, "variables": variables, "extensions": {"persistedQuery": {"version": 1, "sha256Hash": query.sha256_hash}}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {"fixture": true}})))
            .expect(1).mount(&server).await;
        let client = graphql_client(&server, testing::storage());

        let response = client
            .backend()
            .execute_graphql_post(&query, &variables)
            .await
            .unwrap();
        let requests = server.received_requests().await.unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].headers["x-reddit-retry"], "attempt=0, max=3");
        assert!(!requests[0].headers.contains_key("x-apollo-operation-id"));
        assert!(!requests[0].headers.contains_key("x-reddit-device-id"));
        assert!(!requests[0].headers.contains_key("x-reddit-retry"));
        assert!(!requests[1].headers.contains_key("x-reddit-direction"));
        assert!(
            !requests[1]
                .headers
                .keys()
                .any(|name| name.as_str().starts_with("__request_tag_"))
        );
    }

    #[tokio::test]
    async fn when_post_is_rejected_then_transport_should_preserve_the_response_without_retries_or_renewal()
     {
        for status in [400, 401, 403, 429, 503] {
            let server = MockServer::start().await;
            let body = if status == 403 {
                "<html>blocked</html>"
            } else {
                r#"{"errors":[{"message":"fixture"}]}"#
            };
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(
                    ResponseTemplate::new(status)
                        .set_body_string(body)
                        .insert_header("retry-after", "60"),
                )
                .expect(1)
                .mount(&server)
                .await;
            let storage = cached_storage();
            let before = storage.store().load().await.unwrap();
            let client = graphql_client(&server, storage.clone());

            let response = client
                .backend()
                .execute_graphql_post(&query(), &json!({}))
                .await
                .unwrap();

            assert_eq!(response.status, status);
            assert_eq!(response.headers["retry-after"], "60");
            assert_eq!(response.body, body.as_bytes());
            assert_eq!(storage.store().load().await.unwrap(), before);
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_a_hash_is_unknown_then_transport_should_not_upload_query_text_or_hide_errors() {
        let server = MockServer::start().await;
        let query = PersistedQuery {
            name: "UnknownFixture".into(),
            sha256_hash: "unregistered".into(),
        };
        let body = r#"{"data":null,"errors":[{"message":"PersistedQueryNotFound","extensions":{"code":"NOT_FOUND"}}]}"#;
        Mock::given(path("/")).and(body_json(json!({"operationName": "UnknownFixture", "variables": {}, "extensions": {"persistedQuery": {"version": 1, "sha256Hash": "unregistered"}}})))
            .respond_with(ResponseTemplate::new(200).set_body_string(body)).expect(1).mount(&server).await;
        let client = graphql_client(&server, cached_storage());

        let response = client
            .backend()
            .execute_graphql_post(&query, &json!({}))
            .await
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, body.as_bytes());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_post_redirects_then_credentials_should_not_be_forwarded() {
        let server = MockServer::start().await;
        let destination = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", destination.uri()))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let response = client
            .backend()
            .execute_graphql_post(&query(), &json!({}))
            .await
            .unwrap();

        assert_eq!(response.status, 302);
        assert_eq!(response.headers["location"], destination.uri());
        assert!(destination.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_transport_settings_are_overridden_then_post_and_capture_should_use_them() {
        let server = MockServer::start().await;
        let query = PersistedQuery {
            name: format!("{}Fixture", "Configured"),
            sha256_hash: "b".repeat(64),
        };
        Mock::given(path("/configured"))
            .and(header("accept", "application/json"))
            .and(headers("x-reddit-retry", vec!["attempt=0", "max=0"]))
            .and(header("x-apollo-operation-name", query.name.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&server)
            .await;
        let recorder = Recorder::default();
        let client = RedditClient::builder()
            .with_storage(cached_storage())
            .with_clock(TestClock::default())
            .with_graphql_endpoint("https://configured.invalid/configured")
            .with_graphql_accept("application/json")
            .with_graphql_retry_header("attempt=0, max=0")
            .with_response_observer(recorder.clone())
            .with_mock_server("https://configured.invalid", &server)
            .build()
            .unwrap();

        let response = client
            .backend()
            .execute_graphql_post(&query, &json!({}))
            .await
            .unwrap();

        assert_eq!(response.status, 200);
        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].operation, query.name);
    }

    struct Unserializable;

    #[tokio::test]
    async fn when_a_graphql_body_exceeds_the_limit_then_post_should_preserve_the_truncation_result()
    {
        let server = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("too long"))
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_storage(cached_storage())
            .with_clock(TestClock::default())
            .with_max_response_bytes(4)
            .with_mock_server("https://gql-fed.reddit.com", &server)
            .build()
            .unwrap();

        let response = client
            .backend()
            .execute_graphql_post(&query(), &json!({}))
            .await
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"too ");
        assert!(response.body_truncated);
    }

    impl Serialize for Unserializable {
        fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            use serde::ser::Error;
            Err(S::Error::custom("private variable value"))
        }
    }

    #[tokio::test]
    async fn when_request_construction_fails_then_bootstrap_should_not_be_attempted() {
        let server = MockServer::start().await;
        let client = graphql_client(&server, Storage::default());
        let invalid_header = PersistedQuery {
            name: "bad\nheader".into(),
            sha256_hash: "a".repeat(64),
        };

        let encoding = client
            .backend()
            .execute_graphql_post(&query(), &Unserializable)
            .await;
        let header = client
            .backend()
            .execute_graphql_post(&invalid_header, &json!({}))
            .await;

        assert!(matches!(
            encoding,
            Err(Error::InvalidRequest { field: "variables" })
        ));
        assert!(matches!(
            header,
            Err(Error::InvalidRequest {
                field: "x-apollo-operation-name"
            })
        ));
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
