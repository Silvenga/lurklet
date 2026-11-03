use crate::{
    AnonymousSession, CommentForest, CommentSort, CommentThread, Error, GraphQlResponse, MediaFile,
    MobileBackend, Post, RedditBackend, RedditClientBuilder, RedditId, RequestAllowlist,
    StateStore, Subreddit, SubredditPage, SubredditSort,
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
    /// Opens one media URL unchanged, without authentication, redirects, or related requests.
    /// The body streams independently of the client; consume it to EOF to establish completion.
    pub async fn get_media(&self, url: &str) -> Result<MediaFile, Error> {
        self.backend.get_media(url).await
    }

    /// Reads a bare name's community metadata without fetching its feed or assets.
    /// Unavailable-community explanations are results; a null resource is Error::Unavailable.
    pub async fn get_subreddit(&self, name: &str) -> Result<Subreddit, Error> {
        self.backend.get_subreddit(name).await
    }

    /// Reads comments for a post ID. A cursor replaces its selected placeholder, not the whole forest.
    pub async fn get_post_comments(
        &self,
        post_id: &RedditId,
        sort: CommentSort,
        cursor: Option<&str>,
    ) -> Result<CommentForest, Error> {
        self.backend.get_post_comments(post_id, sort, cursor).await
    }

    /// Opens a comment ID with ancestor context. Further focused-thread expansion is not enabled.
    pub async fn get_comment_thread(
        &self,
        comment_id: &RedditId,
        sort: CommentSort,
    ) -> Result<CommentThread, Error> {
        self.backend.get_comment_thread(comment_id, sort).await
    }

    /// Lists one page without fetching post details or advancing shared navigation state.
    /// The cursor is opaque and must be reused with the same bare subreddit name and sort.
    pub async fn list_subreddit_posts(
        &self,
        subreddit: &str,
        sort: SubredditSort,
        after: Option<&str>,
    ) -> Result<SubredditPage, Error> {
        self.backend
            .list_subreddit_posts(subreddit, sort, after)
            .await
    }

    /// Retrieves one post ID without fetching related posts or comments.
    /// Unavailable means no readable post was returned, not proof that the ID does not exist.
    pub async fn get_post(&self, id: &RedditId) -> Result<Post, Error> {
        self.backend.get_post(id).await
    }

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
    #[test]
    fn when_a_client_is_cloned_then_clones_should_share_the_backend() {
        let client = RedditClient::builder().build().unwrap();

        let cloned = client.clone();

        assert!(Arc::ptr_eq(&client.backend, &cloned.backend));
    }
}
