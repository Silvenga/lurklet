use crate::RedditId;
use serde::Serialize;

#[derive(Serialize)]
pub struct Variables {
    ids: [String; 1],
}

impl Variables {
    pub fn new(id: &RedditId) -> Self {
        Self {
            ids: [format!("t3_{}", id.as_str())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn when_a_post_is_requested_then_variables_should_leave_all_selections_to_query_defaults() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-post-request.json"
        ))
        .unwrap();

        let variables = serde_json::to_value(Variables::new(&"18t5968".into())).unwrap();

        assert_eq!(variables, fixture["variables"]);
        assert_eq!(variables, json!({"ids": ["t3_18t5968"]}));
    }

    #[test]
    fn when_a_different_post_is_requested_then_encoding_should_send_only_its_fullname() {
        let variables = Variables::new(&"another".into());

        let encoded = serde_json::to_string(&variables).unwrap();

        assert_eq!(encoded, r#"{"ids":["t3_another"]}"#);
    }

    #[test]
    fn when_an_id_was_constructed_from_a_fullname_then_encoding_should_add_the_post_prefix_only_once()
     {
        for value in ["18t5968", "t3_18t5968"] {
            let id = RedditId::from(value);

            let encoded = serde_json::to_value(Variables::new(&id)).unwrap();

            assert_eq!(encoded, json!({"ids": ["t3_18t5968"]}));
        }
    }
}
