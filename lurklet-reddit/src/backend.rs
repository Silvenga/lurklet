use crate::{AnonymousSession, Error};
use std::future::Future;

/// Backend contract for Reddit operations, independent of upstream endpoints.
pub trait RedditBackend: Send + Sync {
    /// Restores or acquires a valid anonymous session.
    fn bootstrap(&self) -> impl Future<Output = Result<AnonymousSession, Error>> + Send;
}
