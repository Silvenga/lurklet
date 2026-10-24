use super::MobileBackend;
use crate::{
    AccessToken, AnonymousSession, Error, ResponseError, StateSnapshot, StateStore, TransportError,
};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Instant;
use wreq::header::HeaderValue;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    expiry_ts: Option<u64>,
}

pub(super) fn validate_subject(token: &str) -> Result<(), ResponseError> {
    let mut segments = token.split('.');
    let header = segments.next().ok_or(ResponseError::InvalidJwt)?;
    let payload = segments.next().ok_or(ResponseError::InvalidJwt)?;
    let signature = segments.next().ok_or(ResponseError::InvalidJwt)?;
    if header.is_empty()
        || payload.is_empty()
        || signature.is_empty()
        || segments.next().is_some()
        || HeaderValue::from_str(token).is_err()
    {
        return Err(ResponseError::InvalidJwt);
    }
    let header = URL_SAFE_NO_PAD
        .decode(header)
        .map_err(|_| ResponseError::InvalidJwt)?;
    let header: Value = serde_json::from_slice(&header).map_err(|_| ResponseError::InvalidJwt)?;
    if !header.is_object() || URL_SAFE_NO_PAD.decode(signature).is_err() {
        return Err(ResponseError::InvalidJwt);
    }
    let payload = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| ResponseError::InvalidJwt)?;
    let claims: Value = serde_json::from_slice(&payload).map_err(|_| ResponseError::InvalidJwt)?;
    if claims.get("sub").and_then(Value::as_str) != Some("loid") {
        return Err(ResponseError::UnexpectedSubject);
    }
    // Claims are decoded for protocol validation, not cryptographically verified.
    Ok(())
}

impl<S: StateStore> MobileBackend<S> {
    pub(super) async fn acquire(&self, snapshot: StateSnapshot) -> Result<AnonymousSession, Error> {
        let headers = self.auth_headers(&snapshot.state)?;
        let started_at_ms = self.clock.now_ms()?;
        let started = Instant::now();
        tracing::debug!("Acquiring anonymous Reddit session");
        let response = self
            .http
            .post(&self.config.auth_endpoint)
            .headers(headers)
            .json(&json!({"scopes": self.config.scopes}))
            .send()
            .await
            .map_err(TransportError::from)?;
        let (headers, body) = self.read_response("bootstrap", response, started).await?;
        let loid = headers
            .get("x-reddit-loid")
            .map(|value| {
                value
                    .to_str()
                    .map(String::from)
                    .map_err(|_| ResponseError::InvalidLoid)
            })
            .transpose()?;
        let body: Value = serde_json::from_slice(&body).map_err(|_| ResponseError::InvalidJson)?;
        let token: TokenResponse =
            serde_json::from_value(body).map_err(|_| ResponseError::InvalidTokenResponse)?;
        validate_subject(&token.access_token)?;
        let expires_at_ms = token
            .expires_in
            .checked_mul(1000)
            .filter(|duration| *duration > 0)
            .and_then(|duration| started_at_ms.checked_add(duration))
            .ok_or(ResponseError::InvalidExpiry)?;
        if expires_at_ms <= self.clock.now_ms()? {
            return Err(ResponseError::ExpiredOnArrival.into());
        }
        if token
            .expiry_ts
            .is_some_and(|expiry| expiry.checked_mul(1000) != Some(expires_at_ms))
        {
            tracing::trace!("Token expiry fields differ; session expiration uses expires_in");
        }
        let session = AnonymousSession {
            access_token: AccessToken::new(token.access_token),
            expires_at_ms,
        };
        let mut state = snapshot.state;
        state.authentication.session = Some(session.clone());
        if let Some(loid) = loid {
            state.authentication.loid = Some(loid);
        }
        self.storage
            .store()
            .compare_exchange(snapshot.revision, state)
            .await?
            .ok_or_else(|| {
                tracing::debug!("Discarding token response because stored session state changed");
                Error::SessionChanged
            })?;
        tracing::debug!("Anonymous Reddit session acquired");
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile::testing::{self, NOW, ORIGIN, PATH, TOKEN, TestClock};
    use crate::{ClientState, InMemoryStore, RedditClient, RetryAfter, Storage, StorageError};
    use futures_util::future::join_all;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use tokio::time::timeout;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    #[tokio::test]
    async fn when_replaying_a_sanitized_capture_then_bootstrap_should_match_the_session_snapshot() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/mobile/captured-anonymous-token.json"
        ))
        .unwrap();
        let server = MockServer::start().await;
        let mut response = ResponseTemplate::new(fixture["status"].as_u64().unwrap() as u16)
            .set_body_json(fixture["body"].clone());
        for (name, value) in fixture["headers"].as_object().unwrap() {
            response = response.insert_header(name.as_str(), value.as_str().unwrap());
        }
        Mock::given(method("POST"))
            .and(path(PATH))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let storage = testing::storage();
        let client = testing::client(&server, storage.clone(), TestClock::default());

        let session = client.bootstrap().await.unwrap();

        assert_eq!(
            storage.store().load().await.unwrap().state.analytics,
            Default::default()
        );
        insta::assert_json_snapshot!(session, @r#"
        {
          "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJsb2lkIn0.c3ludGhldGlj",
          "expires_at_ms": 1700086399000
        }
        "#);
    }

    #[tokio::test]
    async fn when_bootstrapping_then_the_contract_and_session_should_match_the_fixture() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(PATH))
            .and(header("authorization", "Basic b2hYcG9xclpZdWIxa2c6"))
            .and(header("content-type", "application/json; charset=utf-8"))
            .and(header(
                "user-agent",
                "Reddit/Version 2026.42.0/Build 2642021/Android 14",
            ))
            .and(header(
                "client-vendor-id",
                "12345678-1234-4234-8234-123456789abc",
            ))
            .and(header("x-reddit-compression", "1"))
            .and(body_json(
                json!({"scopes": ["*", "email", "pii", "adsread", "adsedit"]}),
            ))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        let storage = testing::storage();
        let client = testing::client(&server, storage.clone(), TestClock::default());

        let session = client.bootstrap().await.unwrap();
        let cached = client.clone().bootstrap().await.unwrap();
        let saved = storage.store().load().await.unwrap();

        assert_eq!(session, cached);
        let mut expected = testing::state();
        expected.authentication.session = Some(session.clone());
        expected.authentication.loid = Some("opaque-fixture-loid".into());
        assert_eq!(
            saved,
            StateSnapshot {
                revision: 1,
                state: expected
            }
        );
        let requests = server.received_requests().await.unwrap();
        for absent in [
            "x-reddit-device-id",
            "device-name",
            "x-reddit-width",
            "x-reddit-height",
            "x-reddit-session",
            "x-reddit-p-device-id",
            "x-dev-ad-id",
            "x-reddit-loid",
            "x-reddit-qos",
        ] {
            assert!(
                !requests[0].headers.contains_key(absent),
                "unexpected header {absent}"
            );
        }
        insta::assert_json_snapshot!(session, @r#"
        {
          "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJsb2lkIn0.c3ludGhldGlj",
          "expires_at_ms": 1700003600000
        }
        "#);
    }

    #[tokio::test]
    async fn when_expiry_fields_disagree_then_expires_in_should_remain_authoritative() {
        for expiry_ts in [1, u64::MAX] {
            let server = MockServer::start().await;
            let clock = TestClock::default();
            let response_clock = clock.clone();
            Mock::given(path(PATH))
                .respond_with(move |_: &Request| {
                    response_clock.0.store(NOW + 500, Ordering::SeqCst);
                    ResponseTemplate::new(200).set_body_json(json!({
                        "access_token": TOKEN, "expires_in": 3600, "expiry_ts": expiry_ts
                    }))
                })
                .expect(1)
                .mount(&server)
                .await;
            let client = testing::client(&server, testing::storage(), clock);

            let session = client.bootstrap().await.unwrap();

            assert_eq!(session.expires_at_ms, NOW + 3_600_000);
        }
    }

    #[tokio::test]
    async fn when_a_token_expires_then_renewal_should_preserve_installation_and_loid() {
        let server = MockServer::start().await;
        let mut state = testing::state();
        state.authentication.session = Some(AnonymousSession {
            access_token: AccessToken::new(TOKEN),
            expires_at_ms: NOW,
        });
        state.authentication.loid = Some("opaque%2Fvalue.unchanged".into());
        let installation = state.installation.clone();
        let storage = Storage::new(InMemoryStore::with_state(state));
        Mock::given(path(PATH))
            .and(header("x-reddit-loid", "opaque%2Fvalue.unchanged"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"access_token": TOKEN, "expires_in": 42})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = testing::client(&server, storage.clone(), TestClock::default());

        let session = client.bootstrap().await.unwrap();
        let saved = storage.store().load().await.unwrap();

        assert_eq!(session.expires_at_ms, NOW + 42_000);
        assert_eq!(saved.state.installation, installation);
        assert_eq!(
            saved.state.authentication.loid.as_deref(),
            Some("opaque%2Fvalue.unchanged")
        );
    }

    #[tokio::test]
    async fn when_clients_share_storage_then_concurrent_bootstraps_should_share_one_request() {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(testing::response().set_delay(Duration::from_millis(50)))
            .expect(1)
            .mount(&server)
            .await;
        let storage = Storage::default();
        let clients: Vec<_> = (0..12)
            .map(|_| testing::client(&server, storage.clone(), TestClock::default()))
            .collect();

        let results = join_all(clients.iter().map(RedditClient::bootstrap)).await;
        let saved = storage.store().load().await.unwrap();

        let expected = AnonymousSession {
            access_token: AccessToken::new(TOKEN),
            expires_at_ms: NOW + 3_600_000,
        };
        for result in results {
            assert_eq!(result.unwrap(), expected);
        }
        assert_eq!(saved.state.installation.unwrap().id.get_version_num(), 4);
        assert_eq!(saved.revision, 2);
    }

    #[tokio::test]
    async fn when_concurrent_acquisition_fails_then_waiters_should_share_the_failure() {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_delay(Duration::from_millis(50))
                    .insert_header("retry-after", "120"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = testing::client(&server, testing::storage(), TestClock::default());

        let results = join_all((0..12).map(|_| client.bootstrap())).await;

        for result in results {
            assert!(
                matches!(result, Err(Error::RateLimited { retry_after: Some(RetryAfter::Delay(delay)) }) if delay == Duration::from_secs(120))
            );
        }
    }

    #[tokio::test]
    async fn when_acquisition_is_cancelled_then_a_later_attempt_should_not_deadlock() {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(testing::response().set_delay(Duration::from_secs(30)))
            .expect(1)
            .mount(&server)
            .await;
        let storage = testing::storage();
        let client = testing::client(&server, storage.clone(), TestClock::default());
        let pending = tokio::spawn({
            let client = client.clone();
            async move { client.bootstrap().await }
        });
        testing::wait_for_request(&server).await;

        pending.abort();
        assert!(pending.await.unwrap_err().is_cancelled());
        server.reset().await;
        Mock::given(path(PATH))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        let result = timeout(Duration::from_secs(5), client.bootstrap())
            .await
            .unwrap();

        assert!(result.is_ok());
        assert_eq!(storage.store().load().await.unwrap().revision, 1);
    }

    #[tokio::test]
    async fn when_state_changes_during_acquisition_then_the_old_result_should_not_overwrite_it() {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(testing::response().set_delay(Duration::from_millis(100)))
            .expect(1)
            .mount(&server)
            .await;
        let storage = testing::storage();
        let client = testing::client(&server, storage.clone(), TestClock::default());
        let pending = tokio::spawn(async move { client.bootstrap().await });
        testing::wait_for_request(&server).await;
        let mut replacement = testing::state();
        replacement.authentication.loid = Some("newer-session".into());
        let saved = storage
            .store()
            .compare_exchange(0, replacement)
            .await
            .unwrap()
            .unwrap();

        let result = pending.await.unwrap();

        assert!(matches!(result, Err(Error::SessionChanged)));
        assert_eq!(storage.store().load().await.unwrap(), saved);
    }

    #[tokio::test]
    async fn when_http_status_is_unsuccessful_then_errors_should_remain_distinguishable() {
        for status in [400, 401, 403, 429, 503, 302] {
            let server = MockServer::start().await;
            Mock::given(path(PATH))
                .respond_with(
                    ResponseTemplate::new(status)
                        .insert_header("retry-after", "Wed, 21 Oct 2037 07:28:00 GMT"),
                )
                .expect(1)
                .mount(&server)
                .await;
            let storage = testing::storage();
            let before = storage.store().load().await.unwrap();
            let client = testing::client(&server, storage.clone(), TestClock::default());

            let error = client.bootstrap().await.unwrap_err();

            match status {
                401 => assert!(matches!(error, Error::Authentication { status: 401 })),
                403 => assert!(matches!(error, Error::AccessDenied { status: 403 })),
                429 => assert!(matches!(
                    error,
                    Error::RateLimited {
                        retry_after: Some(RetryAfter::At(_))
                    }
                )),
                503 => assert!(matches!(error, Error::Upstream { status: 503, .. })),
                _ => assert!(
                    matches!(error, Error::UnexpectedStatus { status: actual } if actual == status)
                ),
            }
            assert_eq!(storage.store().load().await.unwrap(), before);
        }
    }

    #[tokio::test]
    async fn when_a_response_is_invalid_then_authentication_state_should_not_change() {
        let wrong_subject = format!(
            "eyJhbGciOiJIUzI1NiJ9.{}.c3ludGhldGlj",
            URL_SAFE_NO_PAD.encode(r#"{"sub":"user"}"#)
        );
        let cases = [
            ("not-json".into(), ResponseError::InvalidJson),
            ("{}".into(), ResponseError::InvalidTokenResponse),
            (
                json!({"access_token": "not-jwt", "expires_in": 60}).to_string(),
                ResponseError::InvalidJwt,
            ),
            (
                json!({"access_token": wrong_subject, "expires_in": 60}).to_string(),
                ResponseError::UnexpectedSubject,
            ),
            (
                json!({"access_token": TOKEN, "expires_in": 0}).to_string(),
                ResponseError::InvalidExpiry,
            ),
            (
                json!({"access_token": TOKEN, "expires_in": u64::MAX}).to_string(),
                ResponseError::InvalidExpiry,
            ),
            (
                json!({"access_token": TOKEN, "expires_in": -1}).to_string(),
                ResponseError::InvalidTokenResponse,
            ),
        ];
        for (body, expected) in cases {
            let server = MockServer::start().await;
            Mock::given(path(PATH))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_string(body)
                        .insert_header("x-reddit-loid", "must-not-publish"),
                )
                .expect(1)
                .mount(&server)
                .await;
            let storage = testing::storage();
            let before = storage.store().load().await.unwrap();
            let client = testing::client(&server, storage.clone(), TestClock::default());

            let result = client.bootstrap().await;

            assert!(matches!(result, Err(Error::InvalidResponse(reason)) if reason == expected));
            assert_eq!(storage.store().load().await.unwrap(), before);
        }
    }

    #[tokio::test]
    async fn when_a_response_is_oversized_then_the_client_should_reject_it() {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_clock(TestClock::default())
            .with_storage(testing::storage())
            .with_max_response_bytes(8)
            .with_mock_server(ORIGIN, &server)
            .build()
            .unwrap();

        let result = client.bootstrap().await;

        assert!(matches!(
            result,
            Err(Error::InvalidResponse(ResponseError::BodyTooLarge))
        ));
    }

    #[tokio::test]
    async fn when_the_token_expires_in_transit_then_the_client_should_not_publish_it() {
        let server = MockServer::start().await;
        let clock = TestClock::default();
        let response_clock = clock.clone();
        Mock::given(path(PATH))
            .respond_with(move |_: &Request| {
                response_clock.0.store(NOW + 3_600_000, Ordering::SeqCst);
                testing::response()
            })
            .expect(1)
            .mount(&server)
            .await;
        let client = testing::client(&server, testing::storage(), clock);

        let result = client.bootstrap().await;

        assert!(matches!(
            result,
            Err(Error::InvalidResponse(ResponseError::ExpiredOnArrival))
        ));
    }

    #[tokio::test]
    async fn when_transport_times_out_then_consumers_should_receive_a_timeout_error() {
        use crate::TransportErrorKind;

        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(testing::response().set_delay(Duration::from_secs(5)))
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_storage(testing::storage())
            .with_clock(TestClock::default())
            .with_request_timeout(Duration::from_millis(100))
            .with_mock_server(ORIGIN, &server)
            .build()
            .unwrap();

        let result = client.bootstrap().await;

        assert!(
            matches!(result, Err(Error::Transport(error)) if error.kind == TransportErrorKind::Timeout)
        );
    }

    struct UnavailableStore;

    #[tokio::test]
    async fn when_request_defaults_are_overridden_then_bootstrap_should_use_the_overrides() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/custom-token"))
            .and(header("authorization", "Basic Y3VzdG9tLWNsaWVudDo="))
            .and(header("user-agent", "debug-client"))
            .and(header("x-reddit-compression", "0"))
            .and(body_json(json!({"scopes": ["custom-scope"]})))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_storage(testing::storage())
            .with_clock(TestClock::default())
            .with_auth_endpoint("https://auth.invalid/custom-token")
            .with_graphql_endpoint("https://content.invalid/graphql")
            .with_client_id("custom-client")
            .with_user_agent("debug-client")
            .with_scopes(vec!["custom-scope".into()])
            .with_reddit_compression("0")
            .with_mock_server("https://auth.invalid", &server)
            .build()
            .unwrap();

        let result = client.bootstrap().await;

        assert!(result.is_ok());
        assert!(
            server.received_requests().await.unwrap()[0].headers["authorization"]
                .to_str()
                .unwrap()
                .starts_with("Basic ")
        );
    }

    #[tokio::test]
    async fn when_a_cached_session_is_restored_then_bootstrap_should_not_make_a_request() {
        let mut state = testing::state();
        let expected = AnonymousSession {
            access_token: AccessToken::new(TOKEN),
            expires_at_ms: NOW + 1000,
        };
        state.authentication.session = Some(expected.clone());
        let storage = Storage::new(InMemoryStore::with_state(state));
        let client = RedditClient::builder()
            .with_storage(storage.clone())
            .with_clock(TestClock::default())
            .build()
            .unwrap();

        let session = client.bootstrap().await.unwrap();

        assert_eq!(session, expected);
        assert_eq!(storage.store().load().await.unwrap().revision, 0);
    }

    #[tokio::test]
    async fn when_an_attempt_fails_then_a_later_explicit_attempt_should_be_able_to_succeed() {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&server)
            .await;
        let client = testing::client(&server, testing::storage(), TestClock::default());
        let first = client.bootstrap().await;
        server.verify().await;
        server.reset().await;
        Mock::given(path(PATH))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;

        let second = client.bootstrap().await;

        assert!(matches!(first, Err(Error::Upstream { status: 503, .. })));
        assert!(second.is_ok());
    }

    #[tokio::test]
    async fn when_restored_state_has_invalid_identity_then_bootstrap_should_reject_it() {
        let mut state = testing::state();
        state.authentication.session = Some(AnonymousSession {
            access_token: AccessToken::new("not-a-token"),
            expires_at_ms: NOW + 1000,
        });
        let client = RedditClient::builder()
            .with_storage(Storage::new(InMemoryStore::with_state(state)))
            .with_clock(TestClock::default())
            .build()
            .unwrap();

        let result = client.bootstrap().await;

        assert!(matches!(
            result,
            Err(Error::InvalidState {
                field: "access_token"
            })
        ));
    }

    impl StateStore for UnavailableStore {
        async fn load(&self) -> Result<StateSnapshot, StorageError> {
            Err(StorageError::Unavailable)
        }

        async fn compare_exchange(
            &self,
            _: u64,
            _: ClientState,
        ) -> Result<Option<StateSnapshot>, StorageError> {
            Err(StorageError::Unavailable)
        }
    }

    #[tokio::test]
    async fn when_storage_fails_then_consumers_should_receive_a_storage_error() {
        let client = RedditClient::builder()
            .with_storage(Storage::new(UnavailableStore))
            .build()
            .unwrap();

        let result = client.bootstrap().await;

        assert!(matches!(
            result,
            Err(Error::Storage(StorageError::Unavailable))
        ));
    }
}
