use crate::{RedditId, Subreddit, SubredditDetails, SubredditStyles, UnavailableSubreddit};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Number;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubredditData {
    // A missing root field is malformed; an explicitly null result is unavailable.
    #[serde(deserialize_with = "Option::deserialize")]
    subreddit_info_by_name: Option<SubredditRecord>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum SubredditRecord {
    Subreddit(Box<SubredditFields>),
    UnavailableSubreddit(UnavailableFields),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubredditFields {
    id: RedditId,
    name: String,
    title: Option<String>,
    created_at: Option<DateTime<Utc>>,
    public_description_text: Option<String>,
    description: Option<Content>,
    subscribers_count: Option<SubscriberCount>,
    community_stats: Option<CommunityStats>,
    styles: Option<Styles>,
    #[serde(rename = "type")]
    community_type: Option<String>,
    is_nsfw: Option<bool>,
    is_posting_restricted: Option<bool>,
    is_quarantined: Option<bool>,
    quarantine_message: Option<Content>,
    interstitial_warning_message: Option<Content>,
}

#[derive(Deserialize)]
struct Content {
    markdown: Option<String>,
}

// Some responses encode integral subscriber counts in exponent notation.
#[derive(Deserialize)]
#[serde(try_from = "Number")]
struct SubscriberCount(u64);

impl TryFrom<Number> for SubscriberCount {
    type Error = &'static str;

    fn try_from(value: Number) -> Result<Self, Self::Error> {
        value
            .as_u64()
            .or_else(|| {
                value
                    .as_f64()
                    .filter(|count| {
                        *count >= 0.0 && *count < u64::MAX as f64 && count.fract() == 0.0
                    })
                    .map(|count| count as u64)
            })
            .map(Self)
            .ok_or("expected a nonnegative integral subscriber count")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommunityStats {
    weekly_active_users_count: Option<u64>,
    weekly_contributions_count: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Styles {
    icon: Option<String>,
    legacy_icon: Option<LegacyIcon>,
    banner_background_image: Option<String>,
    mobile_banner_image: Option<String>,
    legacy_banner_background_image: Option<String>,
    primary_color: Option<String>,
    legacy_primary_color: Option<String>,
}

#[derive(Deserialize)]
struct LegacyIcon {
    url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnavailableFields {
    id: Option<RedditId>,
    forbidden_reason: Option<String>,
    ban_title: Option<String>,
    ban_message_content: Option<Content>,
    quarantine_message: Option<Content>,
    interstitial_warning_message: Option<Content>,
}

impl SubredditData {
    pub fn into_subreddit(self) -> Option<Subreddit> {
        self.subreddit_info_by_name.map(|record| match record {
            SubredditRecord::Subreddit(fields) => {
                let (weekly_active_user_count, weekly_contribution_count) = fields
                    .community_stats
                    .map(|stats| {
                        (
                            stats.weekly_active_users_count,
                            stats.weekly_contributions_count,
                        )
                    })
                    .unwrap_or((None, None));
                Subreddit::Available(Box::new(SubredditDetails {
                    id: fields.id,
                    name: fields.name,
                    title: fields.title,
                    created_at: fields.created_at,
                    short_description: fields.public_description_text,
                    description_markdown: fields.description.and_then(|content| content.markdown),
                    subscriber_count: fields.subscribers_count.map(|count| count.0),
                    weekly_active_user_count,
                    weekly_contribution_count,
                    styles: fields.styles.map(|styles| SubredditStyles {
                        icon_url: styles.icon,
                        legacy_icon_url: styles.legacy_icon.and_then(|icon| icon.url),
                        banner_url: styles.banner_background_image,
                        mobile_banner_url: styles.mobile_banner_image,
                        legacy_banner_url: styles.legacy_banner_background_image,
                        primary_color: styles.primary_color,
                        legacy_primary_color: styles.legacy_primary_color,
                    }),
                    community_type: fields.community_type,
                    is_nsfw: fields.is_nsfw,
                    is_posting_restricted: fields.is_posting_restricted,
                    is_quarantined: fields.is_quarantined,
                    quarantine_message_markdown: fields
                        .quarantine_message
                        .and_then(|content| content.markdown),
                    interstitial_warning_markdown: fields
                        .interstitial_warning_message
                        .and_then(|content| content.markdown),
                }))
            }
            SubredditRecord::UnavailableSubreddit(fields) => {
                Subreddit::Unavailable(UnavailableSubreddit {
                    id: fields.id,
                    forbidden_reason: fields.forbidden_reason,
                    ban_title: fields.ban_title,
                    ban_message_markdown: fields
                        .ban_message_content
                        .and_then(|content| content.markdown),
                    quarantine_message_markdown: fields
                        .quarantine_message
                        .and_then(|content| content.markdown),
                    interstitial_warning_markdown: fields
                        .interstitial_warning_message
                        .and_then(|content| content.markdown),
                })
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn data() -> Value {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-info.json"
        ))
        .unwrap();
        fixture["data"].clone()
    }

    fn decode(data: Value) -> Result<Option<Subreddit>, serde_json::Error> {
        Ok(serde_json::from_value::<SubredditData>(data)?.into_subreddit())
    }

    #[test]
    fn when_a_community_is_returned_then_projection_should_preserve_display_metadata_and_all_style_variants()
     {
        let wire = data();

        let subreddit = decode(wire).unwrap().unwrap();

        assert_eq!(
            subreddit,
            Subreddit::Available(Box::new(SubredditDetails {
                id: "fixture".into(),
                name: "rust".into(),
                title: Some("Example community".into()),
                created_at: Some("2010-12-02T21:27:18.336Z".parse().unwrap()),
                short_description: Some("A short plain-text description.".into()),
                description_markdown: Some(
                    "## About\n\nRead the [guide](https://example.invalid/guide).".into()
                ),
                subscriber_count: Some(12345),
                weekly_active_user_count: Some(234),
                weekly_contribution_count: Some(56),
                styles: Some(SubredditStyles {
                    icon_url: Some(
                        "https://styles.redditmedia.com/fixture/icon.png?sig=a%2Bb&v=1".into()
                    ),
                    legacy_icon_url: Some(
                        "https://styles.redditmedia.com/fixture/legacy-icon.png?sig=legacy".into()
                    ),
                    banner_url: Some(
                        "https://styles.redditmedia.com/fixture/banner.png?s=banner".into()
                    ),
                    mobile_banner_url: Some(
                        "https://styles.redditmedia.com/fixture/mobile.png?s=mobile".into()
                    ),
                    legacy_banner_url: Some(
                        "https://styles.redditmedia.com/fixture/legacy.png?s=legacy".into()
                    ),
                    primary_color: Some("#123456".into()),
                    legacy_primary_color: Some("#654321".into()),
                }),
                community_type: Some("PUBLIC".into()),
                is_nsfw: Some(false),
                is_posting_restricted: Some(false),
                is_quarantined: Some(false),
                quarantine_message_markdown: Some("A **quarantine** notice.".into()),
                interstitial_warning_markdown: Some("An interstitial warning.".into()),
            }))
        );
    }

    #[test]
    fn when_optional_metadata_is_missing_or_null_then_projection_should_not_invent_defaults() {
        let minimal = json!({"subredditInfoByName": {
            "__typename": "Subreddit", "id": "t5_minimal", "name": "minimal"
        }});
        let mut nulls = minimal.clone();
        for field in data()["subredditInfoByName"].as_object().unwrap().keys() {
            if !["__typename", "id", "name"].contains(&field.as_str()) {
                nulls["subredditInfoByName"][field] = Value::Null;
            }
        }
        let expected = Some(Subreddit::Available(Box::new(SubredditDetails {
            id: "minimal".into(),
            name: "minimal".into(),
            title: None,
            created_at: None,
            short_description: None,
            description_markdown: None,
            subscriber_count: None,
            weekly_active_user_count: None,
            weekly_contribution_count: None,
            styles: None,
            community_type: None,
            is_nsfw: None,
            is_posting_restricted: None,
            is_quarantined: None,
            quarantine_message_markdown: None,
            interstitial_warning_markdown: None,
        })));

        let missing = decode(minimal).unwrap();
        let null = decode(nulls).unwrap();

        assert_eq!(missing, expected);
        assert_eq!(null, expected);
    }

    #[test]
    fn when_nested_display_fields_are_null_then_projection_should_preserve_their_absence() {
        let mut wire = data();
        wire["subredditInfoByName"]["styles"] = json!({"legacyIcon": {"url": null}});
        wire["subredditInfoByName"]["communityStats"] = json!({"weeklyActiveUsersCount": null});
        wire["subredditInfoByName"]["description"] = json!({"markdown": null, "richtext": "{}"});
        let mut expected = decode(data()).unwrap();
        let Some(Subreddit::Available(fields)) = &mut expected else {
            panic!("expected available metadata")
        };
        fields.styles = Some(SubredditStyles {
            icon_url: None,
            legacy_icon_url: None,
            banner_url: None,
            mobile_banner_url: None,
            legacy_banner_url: None,
            primary_color: None,
            legacy_primary_color: None,
        });
        fields.weekly_active_user_count = None;
        fields.weekly_contribution_count = None;
        fields.description_markdown = None;

        let actual = decode(wire).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn when_types_flags_or_counts_are_reported_then_projection_should_preserve_values_without_inference()
     {
        let mut wire = data();
        let record = &mut wire["subredditInfoByName"];
        record["type"] = json!("FUTURE_COMMUNITY_TYPE");
        record["isNsfw"] = json!(true);
        record["isPostingRestricted"] = json!(true);
        record["isQuarantined"] = json!(true);
        record["subscribersCount"] = json!(0);
        record["communityStats"] =
            json!({"weeklyActiveUsersCount": 0, "weeklyContributionsCount": 0});
        record["description"]["markdown"] = json!("");
        let mut expected = decode(data()).unwrap();
        let Some(Subreddit::Available(fields)) = &mut expected else {
            panic!("expected available metadata")
        };
        fields.community_type = Some("FUTURE_COMMUNITY_TYPE".into());
        fields.is_nsfw = Some(true);
        fields.is_posting_restricted = Some(true);
        fields.is_quarantined = Some(true);
        fields.subscriber_count = Some(0);
        fields.weekly_active_user_count = Some(0);
        fields.weekly_contribution_count = Some(0);
        fields.description_markdown = Some(String::new());

        let actual = decode(wire).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn when_creation_dates_include_fractional_seconds_and_offsets_then_projection_should_normalize_to_utc()
     {
        for (date, utc) in [
            (
                "2010-12-02T21:27:18.336000+0000",
                "2010-12-02T21:27:18.336Z",
            ),
            (
                "2010-12-02T21:27:18.336000+0530",
                "2010-12-02T15:57:18.336Z",
            ),
        ] {
            let mut wire = data();
            wire["subredditInfoByName"]["createdAt"] = json!(date);

            let actual = serde_json::to_value(decode(wire).unwrap()).unwrap();

            assert_eq!(actual["created_at"], utc);
        }
    }

    #[test]
    fn when_subscriber_counts_use_exponent_notation_then_projection_should_retain_the_integer_value()
     {
        for (number, expected) in [
            ("2.7989396e+07", 27_989_396),
            ("27989396", 27_989_396),
            ("0.0", 0),
            ("18446744073709551615", u64::MAX),
        ] {
            let mut wire = data();
            wire["subredditInfoByName"]["subscribersCount"] = serde_json::from_str(number).unwrap();

            let actual = serde_json::to_value(decode(wire).unwrap()).unwrap();

            assert_eq!(actual["subscriber_count"], expected);
        }
    }

    #[test]
    fn when_subscriber_counts_cannot_be_represented_as_integers_then_projection_should_reject_them_without_rounding()
     {
        for number in [
            "-1",
            "-1.0",
            "1.5",
            "1e20",
            "18446744073709551616.0",
            "\"123\"",
        ] {
            let mut wire = data();
            wire["subredditInfoByName"]["subscribersCount"] = serde_json::from_str(number).unwrap();

            let actual = decode(wire);

            assert!(actual.is_err());
        }
    }

    #[test]
    fn when_unused_fields_change_shape_then_projection_should_ignore_them() {
        let mut wire = data();
        for field in [
            "prefixedName",
            "path",
            "wiki",
            "devvit",
            "featureVariants",
            "modPermissions",
            "isSubscribed",
            "welcomePage",
        ] {
            wire["subredditInfoByName"][field] = json!({"unmodeled": [1, null, "future"]});
        }
        wire["subredditInfoByName"]["description"]["richtext"] = json!(["not a rich-text parser"]);
        let expected = decode(data()).unwrap();

        let actual = decode(wire).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn when_unavailability_has_explanations_then_projection_should_preserve_them_as_data() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-unavailable.json"
        ))
        .unwrap();

        let actual = decode(fixture["data"].clone()).unwrap();

        assert_eq!(
            actual,
            Some(Subreddit::Unavailable(UnavailableSubreddit {
                id: Some("unavailable".into()),
                forbidden_reason: Some("BANNED".into()),
                ban_title: Some("Example ban title".into()),
                ban_message_markdown: Some("Example **ban** explanation.".into()),
                quarantine_message_markdown: Some("Example quarantine explanation.".into()),
                interstitial_warning_markdown: Some("Example warning.".into()),
            }))
        );
    }

    #[test]
    fn when_an_unavailable_reason_is_missing_or_unfamiliar_then_projection_should_not_invent_identity()
     {
        for reason in [
            None,
            Some("PRIVATE"),
            Some("QUARANTINED"),
            Some("UNVERIFIED_AGE"),
            Some("FUTURE_REASON"),
        ] {
            let wire = json!({"subredditInfoByName": {
                "__typename": "UnavailableSubreddit", "forbiddenReason": reason
            }});

            let actual = decode(wire).unwrap();

            assert_eq!(
                actual,
                Some(Subreddit::Unavailable(UnavailableSubreddit {
                    id: None,
                    forbidden_reason: reason.map(Into::into),
                    ban_title: None,
                    ban_message_markdown: None,
                    quarantine_message_markdown: None,
                    interstitial_warning_markdown: None,
                }))
            );
        }
    }

    #[test]
    fn when_the_root_is_explicitly_null_then_projection_should_return_no_metadata() {
        let wire = json!({"subredditInfoByName": null});

        let actual = decode(wire).unwrap();

        assert_eq!(actual, None);
    }

    #[test]
    fn when_response_structure_or_required_identity_is_invalid_then_projection_should_reject_it() {
        for wire in [
            json!({}),
            json!({"subredditInfoByName": []}),
            json!({"subredditInfoByName": {"__typename": "FutureSubredditType"}}),
            json!({"subredditInfoByName": {"__typename": "Subreddit", "id": "t5_example"}}),
            json!({"subredditInfoByName": {"__typename": "Subreddit", "name": "example"}}),
            json!({"subredditInfoByName": {"__typename": "UnavailableSubreddit", "id": 123}}),
        ] {
            let result = decode(wire);

            assert!(result.is_err());
        }
    }
}
