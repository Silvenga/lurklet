use crate::RedditBackend;

/// Backend for Reddit's native mobile API.
pub struct MobileBackend {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Mobile API operations are not implemented yet.")
    )]
    pub(crate) http: wreq::Client,
}

impl RedditBackend for MobileBackend {}

#[cfg(test)]
mod tests {
    use crate::RedditClient;
    use serde::Deserialize;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ResponseFixture {
        status: u16,
        headers: BTreeMap<String, String>,
        body: Value,
    }

    #[tokio::test]
    async fn when_a_request_matches_a_fixture_then_the_response_should_match_the_snapshot() {
        let server = MockServer::start().await;
        let fixture: ResponseFixture = serde_json::from_str(include_str!(
            "../../tests/fixtures/harness/synthetic-response.json"
        ))
        .expect("valid synthetic response fixture");
        let mut response = ResponseTemplate::new(fixture.status).set_body_json(fixture.body);
        for (name, value) in fixture.headers {
            response = response.insert_header(name, value);
        }
        Mock::given(method("POST"))
            .and(path("/contract-probe"))
            .and(query_param("mode", "offline"))
            .and(header("content-type", "application/json"))
            .and(body_json(
                json!({"operationName": "ContractProbe", "variables": {"id": "fixture"}}),
            ))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_mock_server("https://reddit.invalid", &server)
            .build()
            .expect("client should build");

        let response = client
            .backend()
            .http
            .post("https://reddit.invalid/contract-probe?mode=offline")
            .json(&json!({"variables": {"id": "fixture"}, "operationName": "ContractProbe"}))
            .send()
            .await
            .expect("mock request should succeed");
        let status = response.status().as_u16();
        let content_type = response.headers()["content-type"]
            .to_str()
            .unwrap()
            .to_owned();
        let body: Value = response.json().await.expect("response should be JSON");

        insta::assert_json_snapshot!(json!({"status": status, "content_type": content_type, "body": body}), @r#"
        {
          "body": {
            "data": {
              "contractProbe": {
                "id": "fixture",
                "message": "Synthetic offline harness response"
              }
            }
          },
          "content_type": "application/json",
          "status": 200
        }
        "#);
    }

    #[tokio::test]
    async fn when_a_request_does_not_match_then_the_mock_should_return_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/contract-probe"))
            .and(body_json(json!({"operationName": "ContractProbe"})))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_mock_server("https://reddit.invalid", &server)
            .build()
            .expect("client should build");

        let response = client
            .backend()
            .http
            .post("https://reddit.invalid/contract-probe")
            .json(&json!({"operationName": "UnexpectedOperation"}))
            .send()
            .await
            .expect("mock server should respond");

        assert_eq!(response.status().as_u16(), 404);
        assert!(response.error_for_status().is_err());
    }

    #[tokio::test]
    async fn when_an_origin_is_unregistered_then_the_request_should_be_blocked() {
        let server = MockServer::start().await;
        let client = RedditClient::builder()
            .with_mock_server("https://reddit.invalid", &server)
            .build()
            .expect("client should build");

        let result = client.backend().http.get(server.uri()).send().await;

        let error = result.expect_err("unregistered origin must be blocked");
        assert!(format!("{error:?}").contains("unregistered upstream origin in offline test"));
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_no_mock_is_configured_then_the_request_should_be_blocked() {
        let server = MockServer::start().await;
        let client = RedditClient::builder()
            .build()
            .expect("client should build");

        let result = client.backend().http.get(server.uri()).send().await;

        let error = result.expect_err("requests without a mock must be blocked");
        assert!(format!("{error:?}").contains("unregistered upstream origin in offline test"));
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_a_response_redirects_then_the_client_should_not_follow_it() {
        let server = MockServer::start().await;
        let destination = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/redirect"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", destination.uri()))
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_mock_server("https://reddit.invalid", &server)
            .build()
            .expect("client should build");

        let response = client
            .backend()
            .http
            .get("https://reddit.invalid/redirect")
            .send()
            .await
            .expect("mock server should respond");

        assert_eq!(response.status().as_u16(), 302);
        assert!(destination.received_requests().await.unwrap().is_empty());
    }
}
