use crate::{
    AnonymousSession, CommentForest, CommentSort, CommentThread, Error, GraphQlResponse,
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

    struct FakeBackend;

    impl RedditBackend for FakeBackend {
        async fn get_subreddit(&self, name: &str) -> Result<Subreddit, Error> {
            assert_eq!(name, "Rust");
            Err(Error::Unavailable)
        }

        async fn get_post_comments(
            &self,
            post_id: &RedditId,
            sort: CommentSort,
            cursor: Option<&str>,
        ) -> Result<CommentForest, Error> {
            assert_eq!(
                (post_id, sort, cursor),
                (&"18t5968".into(), CommentSort::Top, Some("opaque"))
            );
            Ok(CommentForest {
                applied_sort: Some(sort),
                items: Vec::new(),
            })
        }

        async fn get_comment_thread(
            &self,
            comment_id: &RedditId,
            sort: CommentSort,
        ) -> Result<CommentThread, Error> {
            assert_eq!((comment_id, sort), (&"child".into(), CommentSort::New));
            Ok(CommentThread {
                post_id: Some("18t5968".into()),
                forest: CommentForest {
                    applied_sort: Some(sort),
                    items: Vec::new(),
                },
            })
        }

        async fn list_subreddit_posts(
            &self,
            subreddit: &str,
            sort: SubredditSort,
            after: Option<&str>,
        ) -> Result<SubredditPage, Error> {
            assert_eq!(
                (subreddit, sort, after),
                ("rust", SubredditSort::New, Some("opaque"))
            );
            Ok(SubredditPage {
                posts: Vec::new(),
                next_cursor: None,
            })
        }

        async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
            Err(Error::SessionChanged)
        }

        async fn get_post(&self, id: &RedditId) -> Result<Post, Error> {
            assert_eq!(id, &"18t5968".into());
            Err(Error::Unavailable)
        }
    }

    #[tokio::test]
    async fn when_subreddit_metadata_is_requested_then_the_client_should_forward_the_name_unchanged()
     {
        let client = RedditClient::from_backend(FakeBackend);

        let result = client.get_subreddit("Rust").await;

        assert!(matches!(result, Err(Error::Unavailable)));
    }

    #[tokio::test]
    async fn when_comments_are_requested_then_the_client_should_forward_post_sort_and_cursor() {
        let client = RedditClient::from_backend(FakeBackend);

        let result = client
            .get_post_comments(&"18t5968".into(), CommentSort::Top, Some("opaque"))
            .await
            .unwrap();

        assert_eq!(
            result,
            CommentForest {
                applied_sort: Some(CommentSort::Top),
                items: Vec::new()
            }
        );
    }

    #[tokio::test]
    async fn when_a_thread_is_requested_then_the_client_should_forward_the_comment_identity() {
        let client = RedditClient::from_backend(FakeBackend);

        let result = client
            .get_comment_thread(&"child".into(), CommentSort::New)
            .await
            .unwrap();

        assert_eq!(result.post_id, Some("18t5968".into()));
        assert_eq!(result.forest.applied_sort, Some(CommentSort::New));
    }

    #[tokio::test]
    async fn when_a_listing_is_requested_then_the_client_should_forward_its_inputs_to_the_selected_backend()
     {
        let client = RedditClient::from_backend(FakeBackend);

        let page = client
            .list_subreddit_posts("rust", SubredditSort::New, Some("opaque"))
            .await
            .unwrap();

        assert_eq!(
            page,
            SubredditPage {
                posts: Vec::new(),
                next_cursor: None
            }
        );
    }

    #[tokio::test]
    async fn when_a_post_is_requested_then_the_client_should_forward_to_the_selected_backend() {
        let client = RedditClient::from_backend(FakeBackend);

        let post = client.get_post(&"18t5968".into()).await;

        assert!(matches!(post, Err(Error::Unavailable)));
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
