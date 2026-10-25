use crate::{Comment, CommentSort, RedditId};
use serde::Serialize;

/// Ordered discussion roots, or replacement items for one selected continuation location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentForest {
    pub applied_sort: Option<CommentSort>,
    pub items: Vec<CommentItem>,
}

/// A comment or an independent expansion location; there is no discussion-wide cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommentItem {
    Comment {
        comment: Comment,
        replies: Vec<CommentItem>,
    },
    More {
        /// Opaque token for get_post_comments; None preserves a non-actionable placeholder.
        cursor: Option<String>,
        /// Upstream hint, not an exact child count or proof of completion.
        count_hint: Option<u64>,
    },
    ContinueThread {
        /// Real anchor for get_comment_thread; None means no supported action is available.
        comment_id: Option<RedditId>,
    },
}

/// A thread-context view that may include ancestors; it is not an inline expansion fragment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentThread {
    pub post_id: Option<RedditId>,
    pub forest: CommentForest,
}
