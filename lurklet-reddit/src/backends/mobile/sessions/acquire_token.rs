use crate::backends::mobile::MobileBackend;
use crate::backends::mobile::sessions::TokenResponse;
use crate::{
    AccessToken, AnonymousSession, Error, ResponseError, StateSnapshot, StateStore, TransportError,
};
use serde_json::{Value, json};
use std::time::Instant;
use wreq::header::HeaderValue;

impl<S: StateStore> MobileBackend<S> {
    pub(super) async fn acquire_token(
        &self,
        snapshot: StateSnapshot,
    ) -> Result<AnonymousSession, Error> {
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
        let (headers, body) = self.read_bootstrap_response(response, started).await?;
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
        if token.access_token.trim().is_empty()
            || HeaderValue::from_str(&token.access_token).is_err()
        {
            return Err(ResponseError::InvalidAccessToken.into());
        }
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
    use crate::backends::mobile::testing::{self, NOW, ORIGIN, PATH, TOKEN_FAKE, TestClock};
    use crate::{InMemoryStore, RedditClient, RetryAfter, Storage};
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    #[tokio::test]
    async fn when_replaying_a_sanitized_capture_then_bootstrap_should_match_the_session_snapshot() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/captured-anonymous-token.json"
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
                        "access_token": TOKEN_FAKE, "expires_in": 3600, "expiry_ts": expiry_ts
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
            access_token: AccessToken::new(TOKEN_FAKE),
            expires_at_ms: NOW,
        });
        state.authentication.loid = Some("opaque%2Fvalue.unchanged".into());
        let installation = state.installation.clone();
        let storage = Storage::new(InMemoryStore::with_state(state));
        Mock::given(path(PATH))
            .and(header("x-reddit-loid", "opaque%2Fvalue.unchanged"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"access_token": TOKEN_FAKE, "expires_in": 42})),
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
    async fn when_reddit_returns_an_opaque_token_then_bootstrap_should_store_and_reuse_it() {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"access_token": "opaque-token", "expires_in": 60})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = testing::client(&server, testing::storage(), TestClock::default());

        let first = client.bootstrap().await.unwrap();
        let cached = client.bootstrap().await.unwrap();

        assert_eq!(first.access_token.expose_secret(), "opaque-token");
        assert_eq!(first, cached);
    }

    #[tokio::test]
    async fn when_a_response_is_invalid_then_authentication_state_should_not_change() {
        let cases = [
            ("not-json".into(), ResponseError::InvalidJson),
            ("{}".into(), ResponseError::InvalidTokenResponse),
            (
                json!({"access_token": "", "expires_in": 60}).to_string(),
                ResponseError::InvalidAccessToken,
            ),
            (
                json!({"access_token": "   ", "expires_in": 60}).to_string(),
                ResponseError::InvalidAccessToken,
            ),
            (
                json!({"access_token": "bad\nheader", "expires_in": 60}).to_string(),
                ResponseError::InvalidAccessToken,
            ),
            (
                json!({"access_token": TOKEN_FAKE, "expires_in": 0}).to_string(),
                ResponseError::InvalidExpiry,
            ),
            (
                json!({"access_token": TOKEN_FAKE, "expires_in": u64::MAX}).to_string(),
                ResponseError::InvalidExpiry,
            ),
            (
                json!({"access_token": TOKEN_FAKE, "expires_in": -1}).to_string(),
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
}
