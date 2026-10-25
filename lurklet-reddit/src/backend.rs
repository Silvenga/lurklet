use crate::{AnonymousSession, Error, Post};
use std::future::Future;

/// Backend contract for Reddit operations, independent of upstream endpoints.
pub trait RedditBackend: Send + Sync {
    /// Restores or acquires a valid anonymous session.
    fn bootstrap(&self) -> impl Future<Output = Result<AnonymousSession, Error>> + Send;

    /// Retrieves one bare post ID, reporting unavailable content without inferring nonexistence.
    fn get_post(&self, id: &str) -> impl Future<Output = Result<Post, Error>> + Send;
}
