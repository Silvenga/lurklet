use crate::{
    AnonymousSession, CommentForest, CommentSort, CommentThread, Error, Post, RedditId, Subreddit,
    SubredditPage, SubredditSort,
};
use std::future::Future;

/// Backend contract for Reddit operations, independent of upstream endpoints.
pub trait RedditBackend: Send + Sync {
    /// Reads metadata or an unavailable-community explanation for a bare subreddit name.
    fn get_subreddit(&self, name: &str) -> impl Future<Output = Result<Subreddit, Error>> + Send;

    /// Reads a discussion or one ordinary continuation fragment for the same post and sort.
    fn get_post_comments(
        &self,
        post_id: &RedditId,
        sort: CommentSort,
        cursor: Option<&str>,
    ) -> impl Future<Output = Result<CommentForest, Error>> + Send;

    /// Opens a comment ID with its returned ancestor context, without further expansion.
    fn get_comment_thread(
        &self,
        comment_id: &RedditId,
        sort: CommentSort,
    ) -> impl Future<Output = Result<CommentThread, Error>> + Send;

    /// Restores or acquires a valid anonymous session.
    fn bootstrap(&self) -> impl Future<Output = Result<AnonymousSession, Error>> + Send;

    /// Retrieves one post ID, reporting unavailable content without inferring nonexistence.
    fn get_post(&self, id: &RedditId) -> impl Future<Output = Result<Post, Error>> + Send;

    /// Lists one page from a bare subreddit name, reusing an opaque cursor only with the same sort.
    fn list_subreddit_posts(
        &self,
        subreddit: &str,
        sort: SubredditSort,
        after: Option<&str>,
    ) -> impl Future<Output = Result<SubredditPage, Error>> + Send;
}
