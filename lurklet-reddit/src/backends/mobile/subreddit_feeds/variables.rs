use crate::{SubredditSort, TimeRange};
use serde::Serialize;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variables<'a> {
    subreddit_name: &'a str,
    ad_context_input: AdContext,
    feed_context_input: FeedContext,
    mobile_context_input: MobileContext,
    sort: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    time: Option<TimeRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AdContext {
    distance: Option<i64>,
    layout: &'static str,
    device_ad_id: &'static str,
    client_signal_session_data: BTreeMap<String, ()>,
    feed_slot_index_data: BTreeMap<String, ()>,
    force_ads: BTreeMap<String, ()>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeedContext {
    layout: &'static str,
    navigation_session_id: Uuid,
}

#[derive(Serialize)]
struct MobileContext {
    autoplay: bool,
}

impl<'a> Variables<'a> {
    pub fn new(
        subreddit: &'a str,
        sort: SubredditSort,
        navigation_session_id: Uuid,
        after: Option<&'a str>,
        distance: Option<i64>,
    ) -> Self {
        let (sort, time) = match sort {
            SubredditSort::Hot => ("HOT", None),
            SubredditSort::New => ("NEW", None),
            SubredditSort::Top(range) => ("TOP", Some(range)),
        };
        Self {
            subreddit_name: subreddit,
            ad_context_input: AdContext {
                distance,
                layout: "CLASSIC",
                device_ad_id: "",
                client_signal_session_data: BTreeMap::new(),
                feed_slot_index_data: BTreeMap::new(),
                force_ads: BTreeMap::new(),
            },
            feed_context_input: FeedContext {
                layout: "CLASSIC",
                navigation_session_id,
            },
            mobile_context_input: MobileContext { autoplay: false },
            sort,
            time,
            after,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn when_the_first_page_is_requested_then_variables_should_use_fixed_context_and_query_defaults()
    {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-feed-request.json"
        ))
        .unwrap();
        let navigation = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();

        let variables = serde_json::to_value(Variables::new(
            "rust",
            SubredditSort::Hot,
            navigation,
            None,
            None,
        ))
        .unwrap();

        assert_eq!(variables, fixture["variables"]);
        assert_eq!(variables.as_object().unwrap().len(), 5);
    }

    #[test]
    fn when_a_sort_is_selected_then_only_top_should_send_a_time_range() {
        let navigation = Uuid::new_v4();
        let cases = [
            (SubredditSort::Hot, "HOT", None),
            (SubredditSort::New, "NEW", None),
            (SubredditSort::Top(TimeRange::Hour), "TOP", Some("HOUR")),
            (SubredditSort::Top(TimeRange::Day), "TOP", Some("DAY")),
            (SubredditSort::Top(TimeRange::Week), "TOP", Some("WEEK")),
            (SubredditSort::Top(TimeRange::Month), "TOP", Some("MONTH")),
            (SubredditSort::Top(TimeRange::Year), "TOP", Some("YEAR")),
            (SubredditSort::Top(TimeRange::All), "TOP", Some("ALL")),
        ];

        for (sort, expected_sort, time) in cases {
            let variables =
                serde_json::to_value(Variables::new("rust", sort, navigation, None, None)).unwrap();

            assert_eq!(variables["sort"], expected_sort);
            assert_eq!(variables.get("time"), time.map(|time| json!(time)).as_ref());
            assert!(!variables.as_object().unwrap().contains_key("after"));
        }
    }

    #[test]
    fn when_a_page_is_continued_then_variables_should_preserve_cursor_and_integer_distance() {
        let navigation = Uuid::new_v4();
        let cursor = "opaque+/=% continuation";

        let variables = serde_json::to_value(Variables::new(
            "rust",
            SubredditSort::New,
            navigation,
            Some(cursor),
            Some(4),
        ))
        .unwrap();

        assert_eq!(variables["after"], cursor);
        assert_eq!(variables["adContextInput"]["distance"], 4);
        assert_eq!(
            variables["feedContextInput"]["navigationSessionId"],
            navigation.to_string()
        );
    }
}
