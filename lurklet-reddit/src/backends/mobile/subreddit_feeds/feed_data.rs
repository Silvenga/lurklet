use crate::backends::mobile::subreddit_feeds::cells::{CellGroup, GroupResult};
use crate::{GraphQlDecodeError, ListedPost};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct FeedData {
    #[serde(rename = "subredditV3", deserialize_with = "Option::deserialize")]
    subreddit: Option<SubredditData>,
}

#[derive(Deserialize)]
struct SubredditData {
    #[serde(deserialize_with = "Option::deserialize")]
    elements: Option<Elements>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Elements {
    dist: Option<i64>,
    page_info: PageInfo,
    edges: Vec<Option<Edge>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    #[serde(deserialize_with = "Option::deserialize")]
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
struct Edge {
    node: Option<FeedElement>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum FeedElement {
    CellGroup(CellGroup),
    #[serde(other)]
    Other,
}

pub struct FeedPageData {
    pub posts: Vec<ListedPost>,
    pub end_cursor: Option<String>,
    pub distance: Option<i64>,
}

impl FeedData {
    pub fn into_page(self) -> Result<Option<FeedPageData>, GraphQlDecodeError> {
        let Some(elements) = self.subreddit.and_then(|subreddit| subreddit.elements) else {
            return Ok(None);
        };
        let mut posts = Vec::new();
        let mut unsupported = 0;
        for edge in elements.edges {
            match edge.and_then(|edge| edge.node) {
                Some(FeedElement::CellGroup(group)) => match group.into_post() {
                    GroupResult::Post(post) => posts.push(*post),
                    GroupResult::Ignored => {}
                    GroupResult::Unsupported => unsupported += 1,
                },
                Some(FeedElement::Other) | None => unsupported += 1,
            }
        }
        let end_cursor = elements
            .page_info
            .end_cursor
            .filter(|cursor| !cursor.is_empty());
        if unsupported > 0 {
            tracing::debug!(unsupported, "Skipped unsupported feed elements");
            if posts.is_empty() && end_cursor.is_none() {
                return Err(GraphQlDecodeError::UnsupportedData);
            }
        }
        Ok(Some(FeedPageData {
            posts,
            end_cursor,
            distance: elements.dist,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn data() -> Value {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-feed.json"
        ))
        .unwrap();
        fixture["data"].clone()
    }

    fn decode(data: &Value) -> Result<Option<FeedPageData>, GraphQlDecodeError> {
        FeedData::deserialize(data)
            .map_err(|_| GraphQlDecodeError::InvalidData)?
            .into_page()
    }

    #[test]
    fn when_a_feed_contains_mixed_elements_then_projection_should_preserve_organic_order_without_hydration()
     {
        let data = data();

        let page = decode(&data).unwrap().unwrap();

        assert_eq!(
            page.posts
                .iter()
                .map(|post| post.id.as_str())
                .collect::<Vec<_>>(),
            ["first", "second", "incomplete"]
        );
        assert_eq!(page.end_cursor.as_deref(), Some("upstream-page-2+/="));
        assert_eq!(page.distance, Some(4));
        insta::assert_json_snapshot!(page.posts[0], @r#"
        {
          "id": "first",
          "title": "First post",
          "thumbnail": {
            "url": "https://example.invalid/preview.png",
            "obfuscated_url": "https://example.invalid/blurred.png",
            "is_obfuscated": true,
            "width": 100,
            "height": 75
          },
          "score": 42,
          "comment_count": 3,
          "author_name": "example_author",
          "created_at": "2023-12-28T21:35:27.507Z",
          "is_nsfw": true,
          "is_spoiler": true,
          "is_pinned": true
        }
        "#);
        assert_eq!(page.posts[1].author_name, None);
        assert_eq!(page.posts[1].is_nsfw, Some(false));
        assert_eq!(page.posts[2].title, None);
        assert_eq!(page.posts[2].is_nsfw, None);
        let output = serde_json::to_string(&page.posts).unwrap();
        for excluded in [
            "previewTextCell",
            "not-the-discussion",
            "not_another_row",
            "advertisement",
            "unknown_ad_status",
        ] {
            assert!(!output.contains(excluded));
        }
    }

    #[test]
    fn when_a_page_has_no_organic_posts_but_a_cursor_then_projection_should_keep_the_continuation()
    {
        for edges in [
            json!([]),
            json!([{"node": {"__typename": "FutureUnit"}}]),
            json!([{"node": {"__typename": "CellGroup", "groupId": "t3_ad", "adPayload": {}}}]),
        ] {
            let data = json!({"subredditV3": {"elements": {"dist": 3, "pageInfo": {"endCursor": "next"}, "edges": edges}}});

            let page = decode(&data).unwrap().unwrap();

            assert!(page.posts.is_empty());
            assert_eq!(page.end_cursor.as_deref(), Some("next"));
        }
    }

    #[test]
    fn when_an_unsupported_only_page_has_no_cursor_then_projection_should_not_claim_an_empty_subreddit()
     {
        let data = json!({"subredditV3": {"elements": {"pageInfo": {"endCursor": null}, "edges": [{"node": {"__typename": "FutureUnit"}}]}}});

        let result = decode(&data);

        assert!(matches!(result, Err(GraphQlDecodeError::UnsupportedData)));
    }

    #[test]
    fn when_a_successful_page_has_a_null_or_empty_cursor_then_projection_should_report_no_continuation()
     {
        for cursor in [Value::Null, json!("")] {
            let data = json!({"subredditV3": {"elements": {"pageInfo": {"endCursor": cursor}, "edges": []}}});

            let page = decode(&data).unwrap().unwrap();

            assert_eq!(page.end_cursor, None);
            assert_eq!(page.distance, None);
            assert!(page.posts.is_empty());
        }
    }

    #[test]
    fn when_page_metadata_is_missing_then_projection_should_not_treat_it_as_exhaustion() {
        for elements in [
            json!({"edges": []}),
            json!({"edges": [], "pageInfo": null}),
            json!({"edges": [], "pageInfo": {}}),
            json!({"pageInfo": {"endCursor": null}}),
        ] {
            let data = json!({"subredditV3": {"elements": elements}});

            let result = decode(&data);

            assert!(matches!(result, Err(GraphQlDecodeError::InvalidData)));
        }
    }

    #[test]
    fn when_the_feed_field_is_null_then_projection_should_preserve_unavailability() {
        for data in [
            json!({"subredditV3": null}),
            json!({"subredditV3": {"elements": null}}),
        ] {
            let result = decode(&data).unwrap();

            assert!(result.is_none());
        }
    }
}
