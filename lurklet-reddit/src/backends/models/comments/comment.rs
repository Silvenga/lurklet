use crate::{NamedReference, RedditId};
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Comment content without upstream tree or rendering formats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comment {
    /// Deleted comments may have no remotely addressable ID.
    pub id: Option<RedditId>,
    pub state: CommentState,
    pub author: Option<NamedReference>,
    pub created_at: Option<DateTime<Utc>>,
    /// Unrendered, untrusted Markdown; consumers must restrict HTML and URL schemes.
    pub body_markdown: Option<String>,
    pub score: Option<i64>,
    pub is_op: Option<bool>,
    pub is_pinned: Option<bool>,
    pub initially_collapsed: Option<bool>,
}

/// The reported comment state, not an inference from a missing body or author.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentState {
    Present,
    Deleted,
    Removed,
}
