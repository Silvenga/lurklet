use crate::{FlairPart, FlairTextColor, PostFlair};
use serde::Deserialize;

/// The shared flair projection returned by mobile post and feed queries.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Flair {
    text: Option<String>,
    richtext: Option<String>,
    text_color: Option<String>,
    template: Option<Template>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    background_color: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "e")]
enum Part {
    #[serde(rename = "text")]
    Text { t: String },
    #[serde(rename = "emoji")]
    Emoji { u: String, a: String },
}

impl Flair {
    /// Projects supported rich text, retaining the supplied label as a fallback.
    pub fn into_flair(self) -> Option<PostFlair> {
        let text = self.text.filter(|text| !text.is_empty());
        let parts: Vec<_> = self
            .richtext
            .and_then(|value| serde_json::from_str::<Vec<Part>>(&value).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|part| match part {
                Part::Text { t } => FlairPart::Text { text: t },
                Part::Emoji { u, a } => FlairPart::Emoji { url: u, alt: a },
            })
            .collect();
        if text.is_none() && parts.is_empty() {
            return None;
        }
        Some(PostFlair {
            text,
            background_color: self.template.and_then(|template| template.background_color),
            text_color: match self.text_color.as_deref() {
                Some("LIGHT") => Some(FlairTextColor::Light),
                Some("DARK") => Some(FlairTextColor::Dark),
                _ => None,
            },
            parts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::mobile::posts::PostData;
    use crate::backends::mobile::subreddit_feeds::FeedData;
    use serde_json::{Value, json};

    #[test]
    fn when_captured_flair_is_projected_then_listings_and_posts_should_share_the_display_model() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/mobile/captured-flair.json"
        ))
        .unwrap();
        for sample in fixture["samples"].as_array().unwrap() {
            let post = json!({"postsInfoByIds": [{
                "__typename": "SubredditPost", "id": "t3_example", "postTitle": "Example",
                "createdAt": "2026-11-03T00:00:00Z", "isNsfw": false, "isSpoiler": false,
                "flair": sample["post_flair"]
            }]});
            let feed = json!({"subredditV3": {"elements": {
                "pageInfo": {"endCursor": null},
                "edges": [{"node": {"__typename": "CellGroup", "groupId": "t3_example",
                    "adPayload": null, "cells": [{"__typename": "ClassicCell", "flairCell": sample["feed_flair_cell"]}]
                }}]
            }}});

            let post = PostData::deserialize(&post)
                .unwrap()
                .into_post()
                .unwrap()
                .unwrap();
            let page = FeedData::deserialize(&feed)
                .unwrap()
                .into_page()
                .unwrap()
                .unwrap();

            assert_eq!(post.flair, page.posts[0].flair, "{}", sample["name"]);
            assert_eq!(
                serde_json::to_value(post.flair).unwrap(),
                sample["expected_flair"],
                "{}",
                sample["name"]
            );
        }
    }

    #[test]
    fn when_richtext_is_unsupported_or_malformed_then_flair_should_keep_its_plain_text() {
        for richtext in [
            "not json",
            "{}",
            "null",
            "[]",
            r#"[{"e":"text","t":"Partial"},{"e":"future","value":"unknown"}]"#,
            r#"[{"e":"emoji","a":":missing-url:"}]"#,
        ] {
            let value = json!({"text": "Full label", "richtext": richtext, "textColor": "FUTURE"});

            let flair = Flair::deserialize(&value).unwrap().into_flair();

            assert_eq!(
                flair,
                Some(PostFlair {
                    text: Some("Full label".into()),
                    background_color: None,
                    text_color: None,
                    parts: vec![],
                })
            );
        }
    }
}
