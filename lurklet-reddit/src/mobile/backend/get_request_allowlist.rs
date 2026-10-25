use crate::mobile::backend::MobileBackend;
use crate::mobile::graphql::decode_response;
use crate::{Error, GraphQlDecodeError, GraphQlResponse, RequestAllowlist, StateStore};
use serde_json::{Value, json};

impl<S: StateStore> MobileBackend<S> {
    pub(crate) async fn get_request_allowlist(
        &self,
    ) -> Result<GraphQlResponse<RequestAllowlist>, Error> {
        let response = self
            .execute_graphql_post(&self.config.request_allowlist_query, &json!({}))
            .await?;
        Ok(decode_response(response, |data| {
            let value = data
                .get("zeroRttClientAllowedQueries")
                .ok_or(GraphQlDecodeError::InvalidData)?;
            if value.is_null() {
                return Ok(None);
            }
            let client_name = value
                .get("clientName")
                .and_then(Value::as_str)
                .ok_or(GraphQlDecodeError::InvalidData)?;
            let queries = value
                .get("queries")
                .and_then(Value::as_array)
                .ok_or(GraphQlDecodeError::InvalidData)?;
            let queries = queries
                .iter()
                .map(|query| {
                    query
                        .get("name")
                        .and_then(Value::as_str)
                        .map(String::from)
                        .ok_or(GraphQlDecodeError::InvalidData)
                })
                .collect::<Result<_, _>>()?;
            Ok(Some(RequestAllowlist {
                client_name: client_name.into(),
                queries,
            }))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile::backend::testing::{
        self, GRAPHQL_ORIGIN, PATH, TestClock, cached_storage, graphql_client,
    };
    use crate::{GraphQlErrorKind, GraphQlOutcome, RedditClient, StateStore};
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn when_a_captured_allowlist_is_replayed_then_projection_and_http_metadata_should_be_preserved()
     {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/mobile/captured-request-allowlist.json"
        ))
        .unwrap();
        let server = MockServer::start().await;
        let mut reply = ResponseTemplate::new(fixture["status"].as_u64().unwrap() as u16)
            .set_body_json(fixture["body"].clone());
        for (name, value) in fixture["headers"].as_object().unwrap() {
            reply = reply.insert_header(name.as_str(), value.as_str().unwrap());
        }
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(reply)
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let response = client.get_request_allowlist().await.unwrap();

        assert_eq!(response.outcome(), GraphQlOutcome::Complete);
        assert_eq!(response.envelope.as_ref(), fixture["body"].as_object());
        assert_eq!(response.data.as_ref().unwrap().client_name, "android-app");
        assert!(
            response
                .response_headers
                .contains(&("x-ratelimit-remaining".into(), b"599.0".to_vec()))
        );
        let expected: Vec<_> = fixture["body"]["data"]["zeroRttClientAllowedQueries"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|query| query["name"].as_str().unwrap())
            .collect();
        assert_eq!(response.data.unwrap().queries, expected);
    }

    #[tokio::test]
    async fn when_allowlist_fields_cannot_be_projected_then_raw_data_should_survive() {
        let server = MockServer::start().await;
        let body = json!({"data": {"zeroRttClientAllowedQueries": {"clientName": "android-app", "queries": [{"name": "Known"}, {"futureShape": true}]}}});
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body.clone()))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let response = client.get_request_allowlist().await.unwrap();

        assert_eq!(response.outcome(), GraphQlOutcome::Failed);
        assert_eq!(response.decode_error, Some(GraphQlDecodeError::InvalidData));
        assert_eq!(response.envelope.as_ref(), body.as_object());
        assert!(response.data.is_none());
    }

    #[tokio::test]
    async fn when_allowlist_is_requested_then_the_public_client_should_bootstrap_and_post_the_pinned_operation()
     {
        let server = MockServer::start().await;
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/mobile/synthetic-request-allowlist.json"
        ))
        .unwrap();
        Mock::given(path(PATH))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST")).and(path("/"))
            .and(header("x-apollo-operation-name", "GetRequestAllowlist"))
            .and(body_json(json!({"operationName": "GetRequestAllowlist", "variables": {}, "extensions": {"persistedQuery": {"version": 1, "sha256Hash": "421cc25fa66813463ea7169d6e1b93d6a3cd46d929a9e4eda7403de4c491e487"}}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture.clone()))
            .expect(1).mount(&server).await;
        let client = graphql_client(&server, testing::storage());

        let response = client.get_request_allowlist().await.unwrap();

        assert_eq!(response.outcome(), GraphQlOutcome::Complete);
        assert_eq!(response.envelope.as_ref(), fixture.as_object());
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
        insta::assert_json_snapshot!(response.data, @r#"
        {
          "client_name": "android-app",
          "queries": [
            "PostsByIds",
            "FutureFixture",
            "PostsByIds"
          ]
        }
        "#);
    }

    #[tokio::test]
    async fn when_allowlist_data_is_partial_then_the_client_should_preserve_data_errors_and_state()
    {
        let server = MockServer::start().await;
        let body = json!({"data": {"zeroRttClientAllowedQueries": {"clientName": "android-app", "queries": []}}, "errors": [{"message": "fixture", "extensions": {"code": "NOT_AUTHENTICATED"}}]});
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body.clone()))
            .expect(1)
            .mount(&server)
            .await;
        let storage = cached_storage();
        let before = storage.store().load().await.unwrap();
        let client = graphql_client(&server, storage.clone());

        let response = client.get_request_allowlist().await.unwrap();

        assert_eq!(response.outcome(), GraphQlOutcome::Partial);
        assert_eq!(response.error_kinds(), [GraphQlErrorKind::NotAuthenticated]);
        assert!(response.data.unwrap().queries.is_empty());
        assert_eq!(response.envelope.as_ref(), body.as_object());
        assert_eq!(storage.store().load().await.unwrap(), before);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn when_the_allowlist_field_is_null_then_it_should_not_be_replaced_with_an_empty_list() {
        let server = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"data": {"zeroRttClientAllowedQueries": null}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let response = client.get_request_allowlist().await.unwrap();

        assert!(response.data.is_none());
        assert_eq!(
            response.raw_data(),
            Some(&json!({"zeroRttClientAllowedQueries": null}))
        );
    }

    #[tokio::test]
    async fn when_allowlist_identity_is_overridden_then_headers_and_body_should_use_the_new_pair() {
        let server = MockServer::start().await;
        Mock::given(path("/"))
            .and(header("x-apollo-operation-name", "CustomAllowlist"))
            .and(header("x-apollo-operation-id", "custom-hash"))
            .and(body_json(json!({"operationName": "CustomAllowlist", "variables": {}, "extensions": {"persistedQuery": {"version": 1, "sha256Hash": "custom-hash"}}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": null})))
            .expect(1).mount(&server).await;
        let client = RedditClient::builder()
            .with_storage(cached_storage())
            .with_clock(TestClock::default())
            .with_request_allowlist_query("CustomAllowlist", "custom-hash")
            .with_mock_server(GRAPHQL_ORIGIN, &server)
            .build()
            .unwrap();

        let response = client.get_request_allowlist().await.unwrap();

        assert_eq!(response.outcome(), GraphQlOutcome::Complete);
    }
}
