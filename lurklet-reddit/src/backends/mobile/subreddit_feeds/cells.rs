use crate::{ListedPost, Thumbnail};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::IgnoredAny;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellGroup {
    group_id: Option<String>,
    #[serde(default, rename = "adPayload")]
    ad_presence: AdPresence,
    cells: Option<Vec<Option<Cell>>>,
}

// Only an explicit null proves this group has no ad payload; missing data does not.
#[derive(Default, Deserialize)]
#[serde(from = "Option<IgnoredAny>")]
enum AdPresence {
    #[default]
    Missing,
    Clear,
    Present,
}

impl From<Option<IgnoredAny>> for AdPresence {
    fn from(value: Option<IgnoredAny>) -> Self {
        match value {
            Some(_) => Self::Present,
            None => Self::Clear,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum Cell {
    #[serde(rename = "ClassicCell")]
    Classic(Box<ClassicCell>),
    #[serde(rename = "ActionCell")]
    Action(ActionCell),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClassicCell {
    title_cell: Option<TitleCell>,
    metadata_cell: Option<MetadataCell>,
    indicators_cell: Option<IndicatorsCell>,
    thumbnail_cell: Option<ThumbnailCell>,
}

#[derive(Deserialize)]
struct TitleCell {
    title: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataCell {
    created_at: Option<DateTime<Utc>>,
    subreddit_name: Option<String>,
    status_indicators: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct IndicatorsCell {
    indicators: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ThumbnailCell {
    image: Option<CellImage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CellImage {
    path: Option<String>,
    obfuscated_path: Option<String>,
    is_obfuscated: Option<bool>,
    size: Option<ImageSize>,
}

#[derive(Deserialize)]
struct ImageSize {
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActionCell {
    score: Option<i64>,
    comment_count: Option<u64>,
    is_score_hidden: Option<bool>,
}

pub enum GroupResult {
    Post(Box<ListedPost>),
    Ignored,
    Unsupported,
}

impl CellGroup {
    pub fn into_post(self) -> GroupResult {
        let Some(id) = self.group_id else {
            return GroupResult::Unsupported;
        };
        if !id.strip_prefix("t3_").is_some_and(|id| !id.is_empty()) {
            return GroupResult::Ignored;
        }
        match self.ad_presence {
            AdPresence::Present => return GroupResult::Ignored,
            AdPresence::Missing => return GroupResult::Unsupported,
            AdPresence::Clear => {}
        }
        let mut post = ListedPost {
            id: id.into(),
            title: None,
            thumbnail: None,
            score: None,
            comment_count: None,
            author_name: None,
            created_at: None,
            is_nsfw: false,
            is_spoiler: false,
            is_pinned: false,
        };
        for cell in self.cells.into_iter().flatten().flatten() {
            match cell {
                Cell::Classic(cell) => {
                    post.title = post.title.or(cell.title_cell.and_then(|title| title.title));
                    if let Some(metadata) = cell.metadata_cell {
                        post.created_at = post.created_at.or(metadata.created_at);
                        post.author_name = post.author_name.or(metadata
                            .subreddit_name
                            .and_then(|byline| byline.strip_prefix("u/").map(String::from)));
                        post.is_pinned |= metadata.status_indicators.is_some_and(|indicators| {
                            indicators.iter().any(|value| value == "PINNED")
                        });
                    }
                    if let Some(indicators) = cell.indicators_cell.and_then(|cell| cell.indicators)
                    {
                        post.is_nsfw |= indicators.iter().any(|value| value == "NSFW");
                        post.is_spoiler |= indicators.iter().any(|value| value == "SPOILER");
                    }
                    if let Some(image) = cell.thumbnail_cell.and_then(|cell| cell.image)
                        && let Some(url) = image.path
                    {
                        let (width, height) = image
                            .size
                            .map(|size| (size.width, size.height))
                            .unwrap_or_default();
                        post.thumbnail = post.thumbnail.or(Some(Thumbnail {
                            url,
                            obfuscated_url: image.obfuscated_path,
                            is_obfuscated: image.is_obfuscated,
                            width,
                            height,
                        }));
                    }
                }
                Cell::Action(cell) => {
                    post.score = if cell.is_score_hidden == Some(true) {
                        None
                    } else {
                        cell.score
                    };
                    post.comment_count = cell.comment_count;
                }
                Cell::Other => {}
            }
        }
        GroupResult::Post(Box::new(post))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn group() -> Value {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-feed.json"
        ))
        .unwrap();
        fixture["data"]["subredditV3"]["elements"]["edges"][1]["node"].clone()
    }

    #[test]
    fn when_listing_indicators_are_missing_null_or_empty_then_flags_should_default_to_false() {
        for cell in [
            json!({"__typename": "ClassicCell"}),
            json!({"__typename": "ClassicCell", "metadataCell": null, "indicatorsCell": null}),
            json!({"__typename": "ClassicCell", "metadataCell": {}, "indicatorsCell": {}}),
            json!({"__typename": "ClassicCell", "metadataCell": {"statusIndicators": null}, "indicatorsCell": {"indicators": null}}),
            json!({"__typename": "ClassicCell", "metadataCell": {"statusIndicators": []}, "indicatorsCell": {"indicators": []}}),
        ] {
            let value = json!({"groupId": "t3_example", "adPayload": null, "cells": [cell]});

            let GroupResult::Post(post) = CellGroup::deserialize(&value).unwrap().into_post()
            else {
                panic!("expected a post")
            };
            let output = serde_json::to_value(&post).unwrap();

            assert_eq!(
                (post.is_nsfw, post.is_spoiler, post.is_pinned),
                (false, false, false)
            );
            for field in ["is_nsfw", "is_spoiler", "is_pinned"] {
                assert_eq!(output[field], json!(false));
            }
            assert_eq!(post.score, None);
            assert_eq!(post.comment_count, None);
        }
    }

    #[test]
    fn when_any_cell_has_positive_indicators_then_other_cells_should_not_clear_the_flags() {
        let positive = json!({"__typename": "ClassicCell", "metadataCell": {"statusIndicators": ["PINNED"]}, "indicatorsCell": {"indicators": ["NSFW", "SPOILER"]}});
        let empty = json!({"__typename": "ClassicCell", "metadataCell": {"statusIndicators": []}, "indicatorsCell": {"indicators": []}});
        for cells in [json!([positive, empty]), json!([empty, positive])] {
            let value = json!({"groupId": "t3_example", "adPayload": null, "cells": cells});

            let GroupResult::Post(post) = CellGroup::deserialize(&value).unwrap().into_post()
            else {
                panic!("expected a post")
            };

            assert_eq!(
                (post.is_nsfw, post.is_spoiler, post.is_pinned),
                (true, true, true)
            );
        }
    }

    #[test]
    fn when_thumbnail_obfuscation_is_unknown_then_listing_defaults_should_not_change_its_nullability()
     {
        let value = json!({"groupId": "t3_example", "adPayload": null, "cells": [
            {"__typename": "ClassicCell", "thumbnailCell": {"image": {"path": "https://example.invalid/preview.png", "isObfuscated": null}}}
        ]});

        let GroupResult::Post(post) = CellGroup::deserialize(&value).unwrap().into_post() else {
            panic!("expected a post")
        };

        assert_eq!(post.thumbnail.unwrap().is_obfuscated, None);
    }

    #[test]
    fn when_cells_change_order_then_projection_should_still_find_them_by_type() {
        let original = group();
        let mut reordered = original.clone();
        reordered["cells"].as_array_mut().unwrap().reverse();

        let GroupResult::Post(expected) = CellGroup::deserialize(&original).unwrap().into_post()
        else {
            panic!("expected a post")
        };
        let GroupResult::Post(actual) = CellGroup::deserialize(&reordered).unwrap().into_post()
        else {
            panic!("expected a post")
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn when_ad_metadata_is_missing_then_the_group_should_not_be_assumed_organic() {
        let mut value = group();
        value.as_object_mut().unwrap().remove("adPayload");

        let result = CellGroup::deserialize(&value).unwrap().into_post();

        assert!(matches!(result, GroupResult::Unsupported));
    }

    #[test]
    fn when_an_ad_uses_a_post_id_then_the_group_should_still_be_excluded() {
        let mut value = group();
        value["adPayload"] = json!({"tracking": "never fetched"});

        let result = CellGroup::deserialize(&value).unwrap().into_post();

        assert!(matches!(result, GroupResult::Ignored));
    }

    #[test]
    fn when_the_group_id_is_not_a_post_then_generic_id_normalization_should_not_make_it_one() {
        for id in ["sortcell", "t5_community", "t3_"] {
            let mut value = group();
            value["groupId"] = id.into();

            let result = CellGroup::deserialize(&value).unwrap().into_post();

            assert!(matches!(result, GroupResult::Ignored));
        }
    }

    #[test]
    fn when_a_thumbnail_is_absent_then_projection_should_not_use_the_outbound_destination_as_an_image()
     {
        let mut value = group();
        value["cells"][2]["thumbnailCell"]["image"] = Value::Null;

        let GroupResult::Post(post) = CellGroup::deserialize(&value).unwrap().into_post() else {
            panic!("expected a post")
        };

        assert_eq!(post.thumbnail, None);
    }

    #[test]
    fn when_the_score_is_hidden_then_projection_should_not_advertise_its_numeric_value() {
        let mut value = group();
        value["cells"][0]["isScoreHidden"] = true.into();

        let GroupResult::Post(post) = CellGroup::deserialize(&value).unwrap().into_post() else {
            panic!("expected a post")
        };

        assert_eq!(post.score, None);
        assert_eq!(post.comment_count, Some(3));
    }
}
