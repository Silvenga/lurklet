use crate::{Markdown, MarkdownMedia};
use serde::Deserialize;
use std::iter;
use url::Url;

/// Mobile content normalized without exposing Reddit's rich-text representation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    markdown: Option<String>,
    richtext_media: Option<Vec<Asset>>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum Asset {
    #[serde(rename = "ImageAsset", alias = "AnimatedImageAsset")]
    Image(Box<ImageAsset>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct ImageAsset {
    id: Option<String>,
    status: Option<String>,
    mimetype: Option<String>,
    url: Option<String>,
    #[serde(flatten)]
    dimensions: Dimensions,
    small: Option<MediaSource>,
    medium: Option<MediaSource>,
    large: Option<MediaSource>,
    xlarge: Option<MediaSource>,
    xxlarge: Option<MediaSource>,
    xxxlarge: Option<MediaSource>,
}

#[derive(Clone, Default, Deserialize)]
struct Dimensions {
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Clone, Deserialize)]
struct MediaSource {
    url: Option<String>,
    dimensions: Option<Dimensions>,
}

impl Content {
    /// Preserves Markdown while resolving valid images to their largest signed preview.
    pub fn into_markdown(self) -> Option<Markdown> {
        Some(Markdown {
            content: self.markdown?,
            media: self
                .richtext_media
                .into_iter()
                .flatten()
                .filter_map(|asset| {
                    let Asset::Image(image) = asset else {
                        return None;
                    };
                    if image.status.as_deref() != Some("VALID") {
                        return None;
                    }
                    let original = MediaSource {
                        url: image.url,
                        dimensions: Some(image.dimensions),
                    };
                    let selected = [
                        image.small,
                        image.medium,
                        image.large,
                        image.xlarge,
                        image.xxlarge,
                        image.xxxlarge,
                    ]
                    .into_iter()
                    .flatten()
                    .chain(iter::once(original.clone()))
                    .filter(|source| {
                        source
                            .url
                            .as_deref()
                            .and_then(|url| Url::parse(url).ok())
                            .is_some_and(|url| {
                                url.scheme() == "https"
                                    && matches!(
                                        url.host_str(),
                                        Some("preview.redd.it" | "external-preview.redd.it")
                                    )
                                    && url
                                        .query_pairs()
                                        .any(|(key, value)| key == "s" && !value.is_empty())
                            })
                    })
                    .max_by_key(|source| {
                        source
                            .dimensions
                            .as_ref()
                            .and_then(|dimensions| dimensions.width)
                    })
                    .unwrap_or(original);
                    let dimensions = selected.dimensions.unwrap_or_default();
                    Some(MarkdownMedia {
                        id: image.id?,
                        mime_type: image.mimetype,
                        url: selected.url?,
                        width: dimensions.width,
                        height: dimensions.height,
                    })
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn when_media_has_no_signed_preview_then_normalization_should_fall_back_without_inventing_metadata()
     {
        for preview_url in [
            "https://preview.redd.it/image.png?width=640",
            "https://preview.redd.it/image.png?s=",
            "https://example.com/image.png?s=signature",
            "http://preview.redd.it/image.png?s=signature",
        ] {
            let content: Content = serde_json::from_value(json!({
                "markdown": "Before ![image](asset) after.",
                "richtextMedia": [
                    {"__typename": "ImageAsset", "status": "VALID", "id": "asset", "url": "https://i.redd.it/image.png", "xlarge": {"url": preview_url, "dimensions": {"width": 640, "height": 320}}},
                    {"__typename": "ImageAsset", "status": "FAILED", "id": "failed", "url": "https://i.redd.it/failed.png"},
                    {"__typename": "VideoAsset"},
                    {"__typename": "ImageAsset", "status": "VALID", "id": "missing-url"},
                    {"__typename": "ImageAsset", "status": "VALID", "url": "https://i.redd.it/missing-id.png"}
                ]
            })).unwrap();

            let markdown = content.into_markdown();

            assert_eq!(
                markdown,
                Some(Markdown {
                    content: "Before ![image](asset) after.".into(),
                    media: vec![MarkdownMedia {
                        id: "asset".into(),
                        mime_type: None,
                        url: "https://i.redd.it/image.png".into(),
                        width: None,
                        height: None,
                    }],
                })
            );
        }
    }

    #[test]
    fn when_content_or_media_is_absent_then_normalization_should_preserve_null_and_empty_markdown()
    {
        for (wire, expected) in [
            (json!({}), None),
            (json!({"markdown": null}), None),
            (json!({"markdown": ""}), Some("".into())),
            (
                json!({"markdown": "text", "richtextMedia": null}),
                Some("text".into()),
            ),
            (
                json!({"markdown": "text", "richtextMedia": []}),
                Some("text".into()),
            ),
        ] {
            let content: Content = serde_json::from_value(wire).unwrap();

            let markdown = content.into_markdown();

            assert_eq!(markdown, expected);
        }
    }
}
