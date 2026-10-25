use crate::{
    AnonymousSession, Error, GraphQlResponse, MobileBackend, RedditBackend, RedditClientBuilder,
    RequestAllowlist, StateStore,
};
use std::sync::Arc;

/// A Reddit client backed by a replaceable operation implementation.
pub struct RedditClient<B: RedditBackend = MobileBackend> {
    backend: Arc<B>,
}

impl RedditClient {
    /// Creates a builder for the mobile API client.
    pub fn builder() -> RedditClientBuilder {
        RedditClientBuilder::default()
    }
}

impl<S: StateStore> RedditClient<MobileBackend<S>> {
    /// Retrieves the mobile client's advertised GET allowlist without enabling GET or caching it.
    /// Ok means a response was received; inspect its outcome for HTTP, GraphQL, or decode failures.
    pub async fn get_request_allowlist(&self) -> Result<GraphQlResponse<RequestAllowlist>, Error> {
        self.backend.get_request_allowlist().await
    }
}

impl<B: RedditBackend> RedditClient<B> {
    /// Ensures an anonymous session is available without logging in.
    pub async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
        self.backend.bootstrap().await
    }

    /// Creates a client from a custom backend without initializing HTTP transport.
    pub fn from_backend(backend: B) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    #[cfg(test)]
    pub(crate) fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B: RedditBackend> Clone for RedditClient<B> {
    fn clone(&self) -> Self {
        Self {
            backend: Arc::clone(&self.backend),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeBackend;

    impl RedditBackend for FakeBackend {
        async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
            Err(Error::SessionChanged)
        }
    }

    #[test]
    fn when_a_custom_backend_is_provided_then_construction_should_use_it() {
        let backend = FakeBackend;

        let client = RedditClient::from_backend(backend);

        let _: &FakeBackend = client.backend.as_ref();
    }

    #[test]
    fn when_a_client_is_cloned_then_clones_should_share_the_backend() {
        let client = RedditClient::from_backend(FakeBackend);

        let cloned = client.clone();

        assert!(Arc::ptr_eq(&client.backend, &cloned.backend));
    }
}
