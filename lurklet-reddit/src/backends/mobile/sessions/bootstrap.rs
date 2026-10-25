use crate::backends::mobile::MobileBackend;
use crate::{AnonymousSession, Error, Installation, StateSnapshot, StateStore};
use std::sync::atomic::Ordering;
use uuid::Uuid;
use wreq::header::HeaderValue;

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
            let token = session.access_token.expose_secret();
            if token.trim().is_empty() || HeaderValue::from_str(token).is_err() {
                return Err(Error::InvalidState {
                    field: "access_token",
                });
            }
            return Ok(Some(session.clone()));
        }
        if snapshot.state.authentication.session.is_some() {
            tracing::trace!("Stored anonymous session has expired");
        }
        Ok(None)
    }
}

impl<S: StateStore> MobileBackend<S> {
    pub(in crate::backends::mobile) async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
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
        let result = self.acquire_token(snapshot).await;
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
    use super::*;
    use crate::backends::mobile::testing::{self, NOW, PATH, TOKEN_FAKE, TestClock};
    use crate::{
        AccessToken, ClientState, InMemoryStore, RedditClient, RetryAfter, Storage, StorageError,
    };
    use futures_util::future::join_all;
    use std::time::Duration;
    use tokio::time::timeout;
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
            access_token: AccessToken::new(TOKEN_FAKE),
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
    async fn when_a_cached_session_is_restored_then_bootstrap_should_not_make_a_request() {
        let mut state = testing::state();
        let expected = AnonymousSession {
            access_token: AccessToken::new("opaque-cached-token"),
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
    async fn when_a_restored_token_is_empty_then_bootstrap_should_reject_it() {
        let mut state = testing::state();
        state.authentication.session = Some(AnonymousSession {
            access_token: AccessToken::new(""),
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

    struct UnavailableStore;

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
