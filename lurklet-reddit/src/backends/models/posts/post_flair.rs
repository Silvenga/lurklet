use serde::Serialize;

/// Subreddit-defined post flair, independent of the upstream presentation format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PostFlair {
    pub text: Option<String>,
    pub background_color: Option<String>,
    pub text_color: Option<FlairTextColor>,
    pub parts: Vec<FlairPart>,
}

/// The foreground preference supplied by the subreddit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlairTextColor {
    Light,
    Dark,
}

/// An ordered flair segment; emoji URLs must be treated as untrusted media.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlairPart {
    Text { text: String },
    Emoji { url: String, alt: String },
}
