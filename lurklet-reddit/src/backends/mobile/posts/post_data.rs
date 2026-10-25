use crate::backends::mobile::posts::attachments::Attachments;
use crate::{GraphQlDecodeError, NamedReference, Post, RedditId};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostData {
    // A missing root field is malformed; an explicitly null result is unavailable.
    #[serde(deserialize_with = "Option::deserialize")]
    posts_info_by_ids: Option<Vec<Option<PostRecord>>>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum PostRecord {
    #[serde(rename = "SubredditPost", alias = "ProfilePost", alias = "AdPost")]
    Readable(Box<PostFields>),
    #[serde(
        rename = "UnavailablePost",
        alias = "DeletedSubredditPost",
        alias = "DeletedProfilePost"
    )]
    Unavailable,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostFields {
    id: RedditId,
    #[serde(rename = "postTitle")]
    title: String,
    created_at: DateTime<Utc>,
    is_nsfw: bool,
    is_spoiler: bool,
    score: Option<i64>,
    comment_count: Option<u64>,
    #[serde(rename = "authorInfo")]
    author: Option<NamedReference>,
    #[serde(rename = "subreddit")]
    community: Option<NamedReference>,
    content: Option<Content>,
    crosspost_root: Option<Crosspost>,
    #[serde(flatten)]
    attachments: Attachments,
}

#[derive(Deserialize)]
struct Content {
    markdown: Option<String>,
}

#[derive(Deserialize)]
struct Crosspost {
    post: Option<OriginalPost>,
}

#[derive(Deserialize)]
struct OriginalPost {
    id: Option<RedditId>,
}

impl PostData {
    pub fn into_post(self) -> Result<Option<Post>, GraphQlDecodeError> {
        let Some(posts) = self.posts_info_by_ids else {
            return Ok(None);
        };
        let post = posts
            .into_iter()
            .next()
            .ok_or(GraphQlDecodeError::InvalidData)?;
        Ok(match post {
            Some(PostRecord::Readable(fields)) => Some(Post::from(*fields)),
            Some(PostRecord::Unavailable) | None => None,
        })
    }
}

impl From<PostFields> for Post {
    fn from(fields: PostFields) -> Self {
        let attachments = fields
            .attachments
            .into_attachments(fields.crosspost_root.is_some());
        Self {
            id: fields.id,
            title: fields.title,
            author: fields.author,
            community: fields.community,
            created_at: fields.created_at,
            score: fields.score,
            comment_count: fields.comment_count,
            is_nsfw: fields.is_nsfw,
            is_spoiler: fields.is_spoiler,
            body_markdown: fields.content.and_then(|content| content.markdown),
            attachments,
            crosspost_id: fields
                .crosspost_root
                .and_then(|root| root.post)
                .and_then(|post| post.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn decode_post(data: &Value) -> Result<Option<Post>, GraphQlDecodeError> {
        PostData::deserialize(data)
            .map_err(|_| GraphQlDecodeError::InvalidData)?
            .into_post()
    }

    fn data() -> Value {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-post.json"
        ))
        .unwrap();
        fixture["data"].clone()
    }

    fn expected() -> Post {
        Post {
            id: "18t5968".into(),
            title: "Example title".into(),
            author: Some(NamedReference {
                id: Some("author".into()),
                name: Some("example_author".into()),
            }),
            community: Some(NamedReference {
                id: Some("community".into()),
                name: Some("example".into()),
            }),
            created_at: "2023-12-28T21:35:27.507Z".parse().unwrap(),
            score: Some(42),
            comment_count: Some(3),
            is_nsfw: false,
            is_spoiler: false,
            body_markdown: Some("Hello **world**.".into()),
            attachments: Vec::new(),
            crosspost_id: None,
        }
    }

    #[test]
    fn when_a_text_post_is_returned_then_projection_should_expose_only_the_consumer_model() {
        let data = data();

        let post = decode_post(&data).unwrap().unwrap();

        assert_eq!(post, expected());
        insta::assert_json_snapshot!(post, @r#"
        {
          "id": "18t5968",
          "title": "Example title",
          "author": {
            "id": "author",
            "name": "example_author"
          },
          "community": {
            "id": "community",
            "name": "example"
          },
          "created_at": "2023-12-28T21:35:27.507Z",
          "score": 42,
          "comment_count": 3,
          "is_nsfw": false,
          "is_spoiler": false,
          "body_markdown": "Hello **world**.",
          "attachments": [],
          "crosspost_id": null
        }
        "#);
    }

    #[test]
    fn when_readable_post_variants_share_fields_then_serde_should_produce_the_same_consumer_model()
    {
        for typename in ["SubredditPost", "ProfilePost", "AdPost"] {
            let mut data = data();
            data["postsInfoByIds"][0]["__typename"] = typename.into();

            let post = decode_post(&data).unwrap().unwrap();

            assert_eq!(post, expected());
        }
    }

    #[test]
    fn when_timestamps_use_native_offsets_then_serde_should_normalize_them_to_utc() {
        for timestamp in [
            "2023-12-28T21:35:27.507000+0000",
            "2023-12-28T21:35:27.507+00:00",
            "2023-12-28T21:35:27.507Z",
            "2023-12-29T03:05:27.507+0530",
        ] {
            let mut data = data();
            data["postsInfoByIds"][0]["createdAt"] = timestamp.into();

            let post = decode_post(&data).unwrap().unwrap();

            assert_eq!(post, expected());
        }
    }

    #[test]
    fn when_dates_have_submillisecond_precision_then_serde_should_preserve_it_in_the_public_date() {
        let mut data = data();
        data["postsInfoByIds"][0]["createdAt"] = "2023-12-28T21:35:27.507123+0000".into();

        let post = decode_post(&data).unwrap().unwrap();

        assert_eq!(post.created_at.timestamp_subsec_micros(), 507123);
        assert_eq!(
            serde_json::to_value(&post).unwrap()["created_at"],
            "2023-12-28T21:35:27.507123Z"
        );
    }

    #[test]
    fn when_dates_are_malformed_then_serde_should_reject_them_without_a_custom_parser() {
        let mut data = data();
        data["postsInfoByIds"][0]["createdAt"] = "not a date".into();

        let result = decode_post(&data);

        assert_eq!(result, Err(GraphQlDecodeError::InvalidData));
    }

    #[test]
    fn when_query_defaults_return_unused_fields_then_serde_should_not_couple_the_model_to_them() {
        let mut data = data();
        data["postsInfoByIds"][0]["awardings"] = json!({"unrecognized": [true, 123]});
        data["postsInfoByIds"][0]["subreddit"]["communityStatus"] = json!(["unused"]);
        data["postsInfoByIds"][0]["moderationInfo"] = json!({"unexpected": "unused"});

        let post = decode_post(&data).unwrap().unwrap();

        assert_eq!(post, expected());
    }

    #[test]
    fn when_optional_fields_are_absent_then_projection_should_not_invent_content_or_counts() {
        let mut data = data();
        let fields = data["postsInfoByIds"][0].as_object_mut().unwrap();
        for field in [
            "authorInfo",
            "subreddit",
            "score",
            "commentCount",
            "content",
        ] {
            fields.remove(field);
        }
        let mut expected = expected();
        expected.author = None;
        expected.community = None;
        expected.score = None;
        expected.comment_count = None;
        expected.body_markdown = None;

        let post = decode_post(&data).unwrap().unwrap();

        assert_eq!(post, expected);
    }

    #[test]
    fn when_an_author_is_unavailable_then_the_post_should_remain_readable() {
        let mut data = data();
        data["postsInfoByIds"][0]["authorInfo"] =
            json!({"__typename": "UnavailableRedditor", "name": null});
        let mut expected = expected();
        expected.author = Some(NamedReference {
            id: None,
            name: None,
        });

        let post = decode_post(&data).unwrap().unwrap();

        assert_eq!(post, expected);
    }

    #[test]
    fn when_required_fields_are_absent_then_projection_should_fail_without_defaults() {
        for field in ["id", "postTitle", "createdAt", "isNsfw", "isSpoiler"] {
            let mut data = data();
            data["postsInfoByIds"][0]
                .as_object_mut()
                .unwrap()
                .remove(field);

            let result = decode_post(&data);

            assert_eq!(result, Err(GraphQlDecodeError::InvalidData));
        }
    }

    #[test]
    fn when_no_readable_post_is_returned_then_projection_should_return_none_without_inventing_a_reason()
     {
        for posts in [
            Value::Null,
            json!([null]),
            json!([{"__typename": "DeletedSubredditPost"}]),
            json!([{"__typename": "DeletedProfilePost"}]),
            json!([{"__typename": "UnavailablePost"}]),
        ] {
            let data = json!({"postsInfoByIds": posts});

            let result = decode_post(&data).unwrap();

            assert_eq!(result, None);
        }
    }

    #[test]
    fn when_the_post_result_is_malformed_then_projection_should_not_report_unavailable_content() {
        for data in [
            json!({}),
            json!({"postsInfoByIds": {}}),
            json!({"postsInfoByIds": []}),
            json!({"postsInfoByIds": [{"__typename": "SubredditPost"}]}),
        ] {
            let result = decode_post(&data);

            assert_eq!(result, Err(GraphQlDecodeError::InvalidData));
        }
    }

    #[test]
    fn when_a_crosspost_has_an_original_then_projection_should_keep_only_its_reference() {
        let mut data = data();
        data["postsInfoByIds"][0]["isSelfPost"] = false.into();
        data["postsInfoByIds"][0]["crosspostRoot"] = json!({"post": {"id": "t3_original", "postTitle": "Not the outer title", "media": {"typeHint": "IMAGE", "still": {"source": {"url": "https://example.invalid/original.png"}}}}});
        let mut expected = expected();
        expected.crosspost_id = Some("original".into());

        let post = decode_post(&data).unwrap().unwrap();

        assert_eq!(post, expected);
    }

    #[test]
    fn when_a_crosspost_original_is_unavailable_then_the_outer_post_should_remain_readable() {
        let mut data = data();
        data["postsInfoByIds"][0]["crosspostRoot"] = json!({"post": null});

        let post = decode_post(&data).unwrap().unwrap();

        assert_eq!(post, expected());
    }

    #[test]
    fn when_markdown_contains_markup_then_projection_should_leave_rendering_to_the_consumer() {
        let mut data = data();
        let markdown = "**Text**\n\n<script>untrusted</script>";
        data["postsInfoByIds"][0]["content"]["markdown"] = markdown.into();
        let mut expected = expected();
        expected.body_markdown = Some(markdown.into());

        let post = decode_post(&data).unwrap().unwrap();

        assert_eq!(post, expected);
    }
}
