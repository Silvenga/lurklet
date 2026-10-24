use crate::{
    ClientState, Clock, ClockError, InMemoryStore, Installation, MobileBackend, RedditClient,
    Storage,
};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::yield_now;
use tokio::time::timeout;
use uuid::Uuid;
use wiremock::{MockServer, ResponseTemplate};

pub(super) const NOW: u64 = 1_700_000_000_000;
pub(super) const ORIGIN: &str = "https://www.reddit.com";
pub(super) const PATH: &str = "/auth/v2/oauth/access-token/loid";
pub(super) const TOKEN: &str =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJsb2lkIn0.c3ludGhldGlj";

#[derive(Clone)]
pub(super) struct TestClock(pub Arc<AtomicU64>);

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

pub(super) fn state() -> ClientState {
    ClientState {
        installation: Some(Installation {
            id: Uuid::parse_str("12345678-1234-4234-8234-123456789abc").unwrap(),
            created_at_ms: NOW,
        }),
        ..ClientState::default()
    }
}

pub(super) fn storage() -> Storage {
    Storage::new(InMemoryStore::with_state(state()))
}

pub(super) fn response() -> ResponseTemplate {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/mobile/synthetic-token.json"
    ))
    .unwrap();
    ResponseTemplate::new(200)
        .set_body_json(fixture)
        .insert_header("x-reddit-loid", "opaque-fixture-loid")
        .insert_header("x-reddit-session", "must-not-be-replayed")
}

pub(super) fn client(
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

pub(super) async fn wait_for_request(server: &MockServer) {
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
