use crate::{NamedReference, RedditId};
use chrono::{DateTime, Utc};
use serde::Serialize;

/// A readable post, independent of the upstream API used to retrieve it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Post {
    pub id: RedditId,
    pub title: String,
    pub author: Option<NamedReference>,
    pub community: Option<NamedReference>,
    pub created_at: DateTime<Utc>,
    pub score: Option<i64>,
    pub comment_count: Option<u64>,
    pub is_nsfw: bool,
    pub is_spoiler: bool,
    /// Unrendered, untrusted Markdown. Renderers must restrict embedded HTML and URL schemes.
    pub body_markdown: Option<String>,
    pub attachments: Vec<PostAttachment>,
    pub crosspost_id: Option<RedditId>,
}

/// Displayable content belonging to a post; galleries retain their item order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PostAttachment {
    Link {
        url: String,
    },
    Image {
        url: String,
        alt_text: Option<String>,
        caption: Option<String>,
    },
    Video {
        sources: Vec<VideoSource>,
        poster_url: Option<String>,
    },
}

/// One returned playback source; choosing a source belongs to the consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VideoSource {
    pub url: String,
    pub format: VideoFormat,
}

/// The format of a video source, not a promise of native browser playback support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoFormat {
    Mp4,
    Hls,
    Dash,
}
