use lurklet_reddit::{
    AnonymousSession, CommentForest, CommentSort, CommentThread, Error, MediaFile, Post,
    RedditBackend, RedditId, Subreddit, SubredditPage, SubredditSort,
};
use std::sync::{Arc, Mutex};

#[derive(Debug, PartialEq)]
pub enum Call {
    Post(RedditId),
    Subreddit(String),
    Listing {
        subreddit: String,
        sort: SubredditSort,
        after: Option<String>,
    },
    Comments {
        id: RedditId,
        sort: CommentSort,
        cursor: Option<String>,
    },
    Thread {
        id: RedditId,
        sort: CommentSort,
    },
}

/// Records route inputs and rejects operations that the test did not configure.
#[derive(Default)]
pub struct FakeBackend {
    pub post: Option<Result<Post, Error>>,
    pub subreddit: Option<Result<Subreddit, Error>>,
    pub listing: Option<Result<SubredditPage, Error>>,
    pub comments: Option<Result<CommentForest, Error>>,
    pub thread: Option<Result<CommentThread, Error>>,
    pub calls: Arc<Mutex<Vec<Call>>>,
}

impl RedditBackend for FakeBackend {
    async fn get_post(&self, id: &RedditId) -> Result<Post, Error> {
        self.calls.lock().unwrap().push(Call::Post(id.clone()));
        self.post.clone().expect("unexpected post request")
    }

    async fn get_subreddit(&self, name: &str) -> Result<Subreddit, Error> {
        self.calls
            .lock()
            .unwrap()
            .push(Call::Subreddit(name.into()));
        self.subreddit
            .clone()
            .expect("unexpected subreddit request")
    }

    async fn list_subreddit_posts(
        &self,
        subreddit: &str,
        sort: SubredditSort,
        after: Option<&str>,
    ) -> Result<SubredditPage, Error> {
        self.calls.lock().unwrap().push(Call::Listing {
            subreddit: subreddit.into(),
            sort,
            after: after.map(Into::into),
        });
        self.listing.clone().expect("unexpected listing request")
    }

    async fn get_post_comments(
        &self,
        id: &RedditId,
        sort: CommentSort,
        cursor: Option<&str>,
    ) -> Result<CommentForest, Error> {
        self.calls.lock().unwrap().push(Call::Comments {
            id: id.clone(),
            sort,
            cursor: cursor.map(Into::into),
        });
        self.comments.clone().expect("unexpected comments request")
    }

    async fn get_comment_thread(
        &self,
        id: &RedditId,
        sort: CommentSort,
    ) -> Result<CommentThread, Error> {
        self.calls.lock().unwrap().push(Call::Thread {
            id: id.clone(),
            sort,
        });
        self.thread.clone().expect("unexpected thread request")
    }

    async fn get_media(
        &self,
        _: &str,
        _: lurklet_reddit::MediaOptions<'_>,
    ) -> Result<MediaFile, Error> {
        panic!("data routes must not fetch media")
    }

    async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
        panic!("routes must leave bootstrap to the backend")
    }
}
