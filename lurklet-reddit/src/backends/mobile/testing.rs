use crate::backends::mobile::graphql::PersistedQuery;
use crate::{
    AccessToken, AnonymousSession, ClientState, Clock, ClockError, InMemoryStore, Installation,
    MobileBackend, RedditClient, Storage,
};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::yield_now;
use tokio::time::timeout;
use uuid::Uuid;
use wiremock::{MockServer, ResponseTemplate};

pub const NOW: u64 = 1_700_000_000_000;
pub const ORIGIN: &str = "https://www.reddit.com";
pub const GRAPHQL_ORIGIN: &str = "https://gql-fed.reddit.com";
pub const PATH: &str = "/auth/v2/oauth/access-token/loid";
pub const TOKEN_FAKE: &str =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJsb2lkIn0.c3ludGhldGlj";

#[derive(Clone)]
pub struct TestClock(pub Arc<AtomicU64>);

impl Default for TestClock {
    fn default() -> Self {
        Self(Arc::new(AtomicU64::new(NOW)))
    }
}

impl Clock for TestClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}

pub fn state() -> ClientState {
    ClientState {
        installation: Some(Installation {
            id: Uuid::parse_str("12345678-1234-4234-8234-123456789abc").unwrap(),
            created_at_ms: NOW,
        }),
        ..ClientState::default()
    }
}

pub fn storage() -> Storage {
    Storage::new(InMemoryStore::with_state(state()))
}

pub fn response() -> ResponseTemplate {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/mobile/synthetic-token.json"
    ))
    .unwrap();
    ResponseTemplate::new(200)
        .set_body_json(fixture)
        .insert_header("x-reddit-loid", "opaque-fixture-loid")
        .insert_header("x-reddit-session", "must-not-be-replayed")
}

pub fn client(
    server: &MockServer,
    storage: Storage,
    clock: TestClock,
) -> RedditClient<MobileBackend> {
    RedditClient::builder()
        .with_storage(storage)
        .with_clock(clock)
        .with_mock_server(ORIGIN, server)
        .build()
        .unwrap()
}

pub async fn wait_for_request(server: &MockServer) {
    timeout(Duration::from_secs(5), async {
        loop {
            if !server.received_requests().await.unwrap().is_empty() {
                break;
            }
            yield_now().await;
        }
    })
    .await
    .expect("request did not reach mock server");
}

pub fn query() -> PersistedQuery {
    PersistedQuery {
        name: "ReadFixture".into(),
        sha256_hash: "a".repeat(64),
    }
}

pub fn cached_storage() -> Storage {
    let mut state = state();
    state.authentication.session = Some(AnonymousSession {
        access_token: AccessToken::new(TOKEN_FAKE),
        expires_at_ms: NOW + 60_000,
    });
    Storage::new(InMemoryStore::with_state(state))
}

pub fn graphql_client(server: &MockServer, storage: Storage) -> RedditClient {
    RedditClient::builder()
        .with_storage(storage)
        .with_clock(TestClock::default())
        .with_mock_server(ORIGIN, server)
        .with_mock_server(GRAPHQL_ORIGIN, server)
        .build()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::Mock;
    use wiremock::matchers::{method, path};

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
