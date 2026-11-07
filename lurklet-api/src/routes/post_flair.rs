use lurklet_reddit::{
    FlairPart as RedditFlairPart, FlairTextColor as RedditFlairTextColor,
    PostFlair as RedditPostFlair,
};
use o2o::o2o;
use serde::Serialize;
use utoipa::ToSchema;

/// Subreddit-defined flair with plain-text fallback and unproxied emoji URLs.
#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditPostFlair)]
#[serde(rename_all = "camelCase")]
pub struct PostFlair {
    #[schema(required)]
    text: Option<String>,
    #[schema(required)]
    background_color: Option<String>,
    #[schema(required)]
    #[from(~.map(Into::into))]
    text_color: Option<FlairTextColor>,
    #[from(~.into_iter().map(Into::into).collect())]
    parts: Vec<FlairPart>,
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditFlairTextColor)]
#[serde(rename_all = "camelCase")]
enum FlairTextColor {
    Light,
    Dark,
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditFlairPart | attribute(expect(
    clippy::redundant_field_names,
    reason = "o2o generates explicit field names for enum variant constructors"
)))]
#[serde(tag = "kind", rename_all = "camelCase")]
enum FlairPart {
    Text { text: String },
    Emoji { url: String, alt: String },
}
