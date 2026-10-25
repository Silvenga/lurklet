use lurklet_reddit::{
    AnonymousSession, CommentForest, CommentSort, CommentThread, Error, Post, RedditBackend,
    RedditId, Subreddit, SubredditPage, SubredditSort,
};

#[derive(Default)]
pub struct FakeBackend {
    pub session: Option<AnonymousSession>,
    pub post: Option<(RedditId, Result<Post, Error>)>,
    pub subreddit: Option<(String, Result<Subreddit, Error>)>,
    pub listing: Option<ListingCall>,
    pub comments: Option<CommentsCall>,
    pub thread: Option<ThreadCall>,
}

pub struct CommentsCall {
    pub post_id: RedditId,
    pub sort: CommentSort,
    pub cursor: Option<String>,
    pub result: Result<CommentForest, Error>,
}

pub struct ThreadCall {
    pub comment_id: RedditId,
    pub sort: CommentSort,
    pub result: Result<CommentThread, Error>,
}

pub struct ListingCall {
    pub subreddit: String,
    pub sort: SubredditSort,
    pub after: Option<String>,
    pub result: Result<SubredditPage, Error>,
}

impl RedditBackend for FakeBackend {
    async fn get_subreddit(&self, name: &str) -> Result<Subreddit, Error> {
        let (expected, result) = self
            .subreddit
            .as_ref()
            .expect("unexpected subreddit lookup");
        assert_eq!(name, expected);
        result.clone()
    }

    async fn get_post_comments(
        &self,
        post_id: &RedditId,
        sort: CommentSort,
        cursor: Option<&str>,
    ) -> Result<CommentForest, Error> {
        let expected = self.comments.as_ref().expect("unexpected comment request");
        assert_eq!(
            (post_id, sort, cursor),
            (&expected.post_id, expected.sort, expected.cursor.as_deref())
        );
        expected.result.clone()
    }

    async fn get_comment_thread(
        &self,
        comment_id: &RedditId,
        sort: CommentSort,
    ) -> Result<CommentThread, Error> {
        let expected = self.thread.as_ref().expect("unexpected thread request");
        assert_eq!((comment_id, sort), (&expected.comment_id, expected.sort));
        expected.result.clone()
    }

    async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
        Ok(self
            .session
            .as_ref()
            .expect("unexpected bootstrap call")
            .clone())
    }

    async fn get_post(&self, id: &RedditId) -> Result<Post, Error> {
        let (expected, result) = self.post.as_ref().expect("unexpected post lookup");
        assert_eq!(id, expected);
        result.clone()
    }

    async fn list_subreddit_posts(
        &self,
        subreddit: &str,
        sort: SubredditSort,
        after: Option<&str>,
    ) -> Result<SubredditPage, Error> {
        let expected = self.listing.as_ref().expect("unexpected subreddit listing");
        assert_eq!(
            (subreddit, sort, after),
            (
                expected.subreddit.as_str(),
                expected.sort,
                expected.after.as_deref()
            )
        );
        expected.result.clone()
    }
}
