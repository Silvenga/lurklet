use crate::ListedPost;
use serde::Serialize;

/// One page of organic posts with an opaque, caller-carried continuation token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubredditPage {
    pub posts: Vec<ListedPost>,
    /// Pass unchanged with the same subreddit and sort; None means no usable continuation.
    pub next_cursor: Option<String>,
}
