use lurklet_reddit::{AnonymousSession, Error, Post, RedditBackend, SubredditPage, SubredditSort};

#[derive(Default)]
pub struct FakeBackend {
    pub session: Option<AnonymousSession>,
    pub post: Option<(String, Result<Post, Error>)>,
    pub listing: Option<ListingCall>,
}

pub struct ListingCall {
    pub subreddit: String,
    pub sort: SubredditSort,
    pub after: Option<String>,
    pub result: Result<SubredditPage, Error>,
}

impl RedditBackend for FakeBackend {
    async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
        Ok(self
            .session
            .as_ref()
            .expect("unexpected bootstrap call")
            .clone())
    }

    async fn get_post(&self, id: &str) -> Result<Post, Error> {
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
