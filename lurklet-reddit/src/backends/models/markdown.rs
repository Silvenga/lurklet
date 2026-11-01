use serde::Serialize;

/// Untrusted Markdown and the media referenced by its image destinations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Markdown {
    pub content: String,
    pub media: Vec<MarkdownMedia>,
}

/// A resolved media reference with dimensions for the selected rendition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MarkdownMedia {
    pub id: String,
    pub mime_type: Option<String>,
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl From<&str> for Markdown {
    fn from(content: &str) -> Self {
        Self {
            content: content.into(),
            media: Vec::new(),
        }
    }
}
