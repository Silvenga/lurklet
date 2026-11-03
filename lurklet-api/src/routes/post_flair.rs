use lurklet_reddit::{
    FlairPart as RedditFlairPart, FlairTextColor as RedditFlairTextColor,
    PostFlair as RedditPostFlair,
};
use serde::Serialize;
use utoipa::ToSchema;

/// Subreddit-defined flair with plain-text fallback and unproxied emoji URLs.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PostFlair {
    #[schema(required)]
    text: Option<String>,
    #[schema(required)]
    background_color: Option<String>,
    #[schema(required)]
    text_color: Option<FlairTextColor>,
    parts: Vec<FlairPart>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
enum FlairTextColor {
    Light,
    Dark,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum FlairPart {
    Text { text: String },
    Emoji { url: String, alt: String },
}

impl From<RedditPostFlair> for PostFlair {
    fn from(flair: RedditPostFlair) -> Self {
        Self {
            text: flair.text,
            background_color: flair.background_color,
            text_color: flair.text_color.map(|color| match color {
                RedditFlairTextColor::Light => FlairTextColor::Light,
                RedditFlairTextColor::Dark => FlairTextColor::Dark,
            }),
            parts: flair
                .parts
                .into_iter()
                .map(|part| match part {
                    RedditFlairPart::Text { text } => FlairPart::Text { text },
                    RedditFlairPart::Emoji { url, alt } => FlairPart::Emoji { url, alt },
                })
                .collect(),
        }
    }
}
