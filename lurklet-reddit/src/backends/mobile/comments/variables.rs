use crate::CommentSort;
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
    pub fn new(post_id: &str, sort: CommentSort, after: Option<&'a str>) -> Self {
        Self {
            id: format!("t3_{post_id}"),
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
    pub fn new(comment_id: &str, sort: CommentSort) -> Self {
        Self {
            id: format!("t1_{comment_id}"),
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
    use serde_json::{Value, json};

    #[test]
    fn when_initial_comments_are_requested_then_variables_should_use_only_required_and_full_read_choices()
     {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-post-comments-request.json"
        ))
        .unwrap();

        let variables =
            serde_json::to_value(PostVariables::new("example", CommentSort::Best, None)).unwrap();

        assert_eq!(variables, fixture["variables"]);
        assert_eq!(variables.as_object().unwrap().len(), 8);
    }

    #[test]
    fn when_a_placeholder_is_expanded_then_variables_should_keep_the_post_id_and_use_the_native_expansion_count()
     {
        let variables = serde_json::to_value(PostVariables::new(
            "example",
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
    fn when_a_thread_is_opened_then_variables_should_request_ancestor_context_without_a_cursor() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-comment-thread-request.json"
        ))
        .unwrap();

        let variables =
            serde_json::to_value(ThreadVariables::new("anchor", CommentSort::Top)).unwrap();

        assert_eq!(variables, fixture["variables"]);
        assert_eq!(variables.as_object().unwrap().len(), 5);
    }

    #[test]
    fn when_a_sort_is_selected_then_both_operations_should_use_the_same_wire_mapping() {
        for (sort, wire) in [
            (CommentSort::Best, "CONFIDENCE"),
            (CommentSort::Top, "TOP"),
            (CommentSort::New, "NEW"),
        ] {
            let post = serde_json::to_value(PostVariables::new("post", sort, None)).unwrap();
            let thread = serde_json::to_value(ThreadVariables::new("comment", sort)).unwrap();

            assert_eq!(post["sortType"], json!(wire));
            assert_eq!(thread["sortType"], json!(wire));
        }
    }
}
