use crate::backends::mobile::Config;
use crate::{
    AnonymousSession, Clock, CommentForest, CommentSort, CommentThread, Error, InMemoryStore, Post,
    RedditBackend, RedditId, ResponseObserver, StateStore, Storage, Subreddit, SubredditPage,
    SubredditSort,
};
use std::sync::Arc;

/// Backend for Reddit's native mobile API.
pub struct MobileBackend<S: StateStore = InMemoryStore> {
    pub(crate) http: wreq::Client,
    pub(crate) config: Config,
    pub(crate) storage: Storage<S>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) observer: Option<Arc<dyn ResponseObserver>>,
}

impl<S: StateStore> RedditBackend for MobileBackend<S> {
    async fn get_subreddit(&self, name: &str) -> Result<Subreddit, Error> {
        MobileBackend::get_subreddit(self, name).await
    }

    async fn get_post_comments(
        &self,
        post_id: &RedditId,
        sort: CommentSort,
        cursor: Option<&str>,
    ) -> Result<CommentForest, Error> {
        MobileBackend::get_post_comments(self, post_id, sort, cursor).await
    }

    async fn get_comment_thread(
        &self,
        comment_id: &RedditId,
        sort: CommentSort,
    ) -> Result<CommentThread, Error> {
        MobileBackend::get_comment_thread(self, comment_id, sort).await
    }

    async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
        MobileBackend::bootstrap(self).await
    }

    async fn get_post(&self, id: &RedditId) -> Result<Post, Error> {
        MobileBackend::get_post(self, id).await
    }

    async fn list_subreddit_posts(
        &self,
        subreddit: &str,
        sort: SubredditSort,
        after: Option<&str>,
    ) -> Result<SubredditPage, Error> {
        MobileBackend::list_subreddit_posts(self, subreddit, sort, after).await
    }
}
