use lurklet_reddit::{Markdown as RedditMarkdown, MarkdownMedia as RedditMarkdownMedia};
use serde::Serialize;
use utoipa::ToSchema;

/// Untrusted Markdown with resolved, unproxied media references.
#[derive(Serialize, ToSchema)]
pub struct Markdown {
    content: String,
    media: Vec<MarkdownMedia>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct MarkdownMedia {
    id: String,
    #[schema(required)]
    mime_type: Option<String>,
    url: String,
    #[schema(required)]
    width: Option<u32>,
    #[schema(required)]
    height: Option<u32>,
}

impl From<RedditMarkdown> for Markdown {
    fn from(markdown: RedditMarkdown) -> Self {
        Self {
            content: markdown.content,
            media: markdown.media.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RedditMarkdownMedia> for MarkdownMedia {
    fn from(media: RedditMarkdownMedia) -> Self {
        Self {
            id: media.id,
            mime_type: media.mime_type,
            url: media.url,
            width: media.width,
            height: media.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn when_markdown_is_converted_then_the_api_should_preserve_media_metadata_and_optional_values()
    {
        for (mime_type, width, height) in [
            (Some("image/gif"), Some(300), Some(200)),
            (None, None, None),
        ] {
            let markdown = RedditMarkdown {
                content: "![gif](provider|id)".into(),
                media: vec![RedditMarkdownMedia {
                    id: "provider|id".into(),
                    mime_type: mime_type.map(String::from),
                    url: "https://external-preview.redd.it/asset.gif?auto=webp&s=signature".into(),
                    width,
                    height,
                }],
            };

            let actual = serde_json::to_value(Markdown::from(markdown)).unwrap();

            assert_eq!(
                actual,
                json!({
                    "content": "![gif](provider|id)",
                    "media": [{"id": "provider|id", "mimeType": mime_type, "url": "https://external-preview.redd.it/asset.gif?auto=webp&s=signature", "width": width, "height": height}]
                })
            );
        }
    }
}
