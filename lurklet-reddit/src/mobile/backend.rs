use super::Config;
use super::token::validate_subject;
use crate::{
    AnonymousSession, Clock, Error, InMemoryStore, Installation, RedditBackend, ResponseObserver,
    StateSnapshot, StateStore, Storage,
};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use uuid::Uuid;

/// Backend for Reddit's native mobile API.
pub struct MobileBackend<S: StateStore = InMemoryStore> {
    pub(crate) http: wreq::Client,
    pub(crate) config: Config,
    pub(crate) storage: Storage<S>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) observer: Option<Arc<dyn ResponseObserver>>,
}

impl<S: StateStore> MobileBackend<S> {
    fn cached_session(
        &self,
        snapshot: &StateSnapshot,
        now: u64,
    ) -> Result<Option<AnonymousSession>, Error> {
        match &snapshot.state.installation {
            Some(installation) if installation.id.get_version_num() != 4 => {
                return Err(Error::InvalidState {
                    field: "installation_id",
                });
            }
            None if snapshot.state.authentication.session.is_some()
                || snapshot.state.authentication.loid.is_some() =>
            {
                return Err(Error::InvalidState {
                    field: "installation",
                });
            }
            Some(_) | None => {}
        }
        if let Some(session) = &snapshot.state.authentication.session
            && session.expires_at_ms > now
        {
            validate_subject(session.access_token.expose_secret()).map_err(|_| {
                tracing::debug!("Stored token failed anonymous-subject validation");
                Error::InvalidState {
                    field: "access_token",
                }
            })?;
            return Ok(Some(session.clone()));
        }
        if snapshot.state.authentication.session.is_some() {
            tracing::trace!("Stored anonymous session has expired");
        }
        Ok(None)
    }
}

impl<S: StateStore> RedditBackend for MobileBackend<S> {
    async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
        let observed = self.storage.completed.load(Ordering::Acquire);
        let snapshot = self.storage.store().load().await?;
        if let Some(session) = self.cached_session(&snapshot, self.clock.now_ms()?)? {
            tracing::trace!("Reusing cached anonymous session");
            return Ok(session);
        }

        tracing::trace!("No valid cached session; entering acquisition coordination");
        let mut acquisition = self.storage.acquisition.lock().await;
        let mut snapshot = self.storage.store().load().await?;
        if let Some(session) = self.cached_session(&snapshot, self.clock.now_ms()?)? {
            tracing::trace!("Reusing session published by another caller");
            return Ok(session);
        }
        if self.storage.completed.load(Ordering::Acquire) != observed
            && let Some((revision, error)) = acquisition.as_ref()
            && *revision == snapshot.revision
        {
            tracing::trace!("Sharing the failed acquisition result with a waiting caller");
            return Err(error.clone());
        }

        if snapshot.state.installation.is_none() {
            tracing::debug!("Creating installation identity in caller-provided storage");
            let mut state = snapshot.state.clone();
            state.installation = Some(Installation {
                id: Uuid::new_v4(),
                created_at_ms: self.clock.now_ms()?,
            });
            snapshot = self
                .storage
                .store()
                .compare_exchange(snapshot.revision, state)
                .await?
                .ok_or(Error::SessionChanged)?;
        }

        let revision = snapshot.revision;
        let result = self.acquire(snapshot).await;
        if let Err(error) = &result {
            tracing::debug!(error = %error, "Anonymous session acquisition failed; authentication state was not replaced");
        }
        *acquisition = result
            .as_ref()
            .err()
            .cloned()
            .map(|error| (revision, error));
        self.storage.completed.fetch_add(1, Ordering::Release);
        result
    }
}

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
