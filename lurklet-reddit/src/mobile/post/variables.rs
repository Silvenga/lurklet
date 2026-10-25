use serde::Serialize;

#[derive(Serialize)]
pub struct Variables {
    ids: [String; 1],
}

impl Variables {
    pub fn new(id: &str) -> Self {
        Self {
            ids: [format!("t3_{id}")],
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
            "../../../tests/fixtures/mobile/synthetic-post-request.json"
        ))
        .unwrap();

        let variables = serde_json::to_value(Variables::new("18t5968")).unwrap();

        assert_eq!(variables, fixture["variables"]);
        assert_eq!(variables, json!({"ids": ["t3_18t5968"]}));
    }

    #[test]
    fn when_a_different_post_is_requested_then_encoding_should_send_only_its_fullname() {
        let variables = Variables::new("another");

        let encoded = serde_json::to_string(&variables).unwrap();

        assert_eq!(encoded, r#"{"ids":["t3_another"]}"#);
    }
}
