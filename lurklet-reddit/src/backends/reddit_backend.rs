use crate::{AnonymousSession, Error, Post, SubredditPage, SubredditSort};
use std::future::Future;

/// Backend contract for Reddit operations, independent of upstream endpoints.
pub trait RedditBackend: Send + Sync {
    /// Restores or acquires a valid anonymous session.
    fn bootstrap(&self) -> impl Future<Output = Result<AnonymousSession, Error>> + Send;

    /// Retrieves one bare post ID, reporting unavailable content without inferring nonexistence.
    fn get_post(&self, id: &str) -> impl Future<Output = Result<Post, Error>> + Send;

    /// Lists one page from a bare subreddit name, reusing an opaque cursor only with the same sort.
    fn list_subreddit_posts(
        &self,
        subreddit: &str,
        sort: SubredditSort,
        after: Option<&str>,
    ) -> impl Future<Output = Result<SubredditPage, Error>> + Send;
}
