use crate::RedditId;
use chrono::{DateTime, Utc};
use serde::Serialize;

/// A compact listing row; its ID is the navigation target for the full post.
/// Missing NSFW, spoiler, and pinned indicators default to false.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListedPost {
    pub id: RedditId,
    pub title: Option<String>,
    pub thumbnail: Option<Thumbnail>,
    pub score: Option<i64>,
    pub comment_count: Option<u64>,
    pub author_name: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub is_nsfw: bool,
    pub is_spoiler: bool,
    pub is_pinned: bool,
}

/// A feed preview, including the upstream information needed to honor obfuscation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Thumbnail {
    pub url: String,
    pub obfuscated_url: Option<String>,
    pub is_obfuscated: Option<bool>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}
