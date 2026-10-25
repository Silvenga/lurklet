use crate::backends::mobile::Config;
use crate::{
    AnonymousSession, Clock, Error, InMemoryStore, Post, RedditBackend, ResponseObserver,
    StateStore, Storage, SubredditPage, SubredditSort,
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
    async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
        MobileBackend::bootstrap(self).await
    }

    async fn get_post(&self, id: &str) -> Result<Post, Error> {
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
