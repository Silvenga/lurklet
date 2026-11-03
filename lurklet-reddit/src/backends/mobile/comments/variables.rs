use crate::{CommentSort, RedditId};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostVariables<'a> {
    id: String,
    sort_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<&'a str>,
    max_depth: u32,
    count: u32,
    truncate: u32,
    num_parents: Option<u32>,
    include_premium_avatar_treatment: bool,
    include_consistency_token: bool,
}

impl<'a> PostVariables<'a> {
    pub fn new(post_id: &RedditId, sort: CommentSort, after: Option<&'a str>) -> Self {
        Self {
            id: format!("t3_{}", post_id.as_str()),
            sort_type: sort_type(sort),
            after,
            max_depth: 10,
            count: if after.is_some() { 200 } else { 20 },
            truncate: 0,
            num_parents: None,
            include_premium_avatar_treatment: false,
            include_consistency_token: false,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadVariables {
    id: String,
    sort_type: &'static str,
    max_depth: u32,
    count: u32,
    num_parents: u32,
}

impl ThreadVariables {
    pub fn new(comment_id: &RedditId, sort: CommentSort) -> Self {
        Self {
            id: format!("t1_{}", comment_id.as_str()),
            sort_type: sort_type(sort),
            max_depth: 10,
            count: 20,
            num_parents: 3,
        }
    }
}

fn sort_type(sort: CommentSort) -> &'static str {
    match sort {
        CommentSort::Best => "CONFIDENCE",
        CommentSort::Top => "TOP",
        CommentSort::New => "NEW",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn when_a_placeholder_is_expanded_then_variables_should_keep_the_post_id_and_use_the_native_expansion_count()
     {
        let variables = serde_json::to_value(PostVariables::new(
            &"example".into(),
            CommentSort::New,
            Some("opaque+/="),
        ))
        .unwrap();

        assert_eq!(variables["id"], "t3_example");
        assert_eq!(variables["sortType"], "NEW");
        assert_eq!(variables["count"], 200);
        assert_eq!(variables["after"], "opaque+/=");
    }

    #[test]
    fn when_a_sort_is_selected_then_both_operations_should_use_the_same_wire_mapping() {
        for (sort, wire) in [
            (CommentSort::Best, "CONFIDENCE"),
            (CommentSort::Top, "TOP"),
            (CommentSort::New, "NEW"),
        ] {
            let post =
                serde_json::to_value(PostVariables::new(&"post".into(), sort, None)).unwrap();
            let thread =
                serde_json::to_value(ThreadVariables::new(&"comment".into(), sort)).unwrap();

            assert_eq!(post["sortType"], json!(wire));
            assert_eq!(thread["sortType"], json!(wire));
        }
    }

    #[test]
    fn when_typed_ids_are_used_then_comment_requests_should_retain_their_wire_fullnames() {
        let post_id = RedditId::from("t3_post");
        let comment_id = RedditId::from("t1_comment");

        let post =
            serde_json::to_value(PostVariables::new(&post_id, CommentSort::Best, None)).unwrap();
        let thread =
            serde_json::to_value(ThreadVariables::new(&comment_id, CommentSort::Best)).unwrap();

        assert_eq!(post["id"], "t3_post");
        assert_eq!(thread["id"], "t1_comment");
    }
}
