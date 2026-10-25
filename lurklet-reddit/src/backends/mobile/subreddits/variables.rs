use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variables<'a> {
    subreddit_name: &'a str,
    include_recap_fields: bool,
    include_devvit_data: bool,
}

impl<'a> Variables<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            subreddit_name: name,
            include_recap_fields: false,
            include_devvit_data: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn when_metadata_is_requested_then_variables_should_send_only_inputs_without_document_defaults()
    {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-info-request.json"
        ))
        .unwrap();

        let variables = serde_json::to_value(Variables::new("rust")).unwrap();

        assert_eq!(variables, fixture["variables"]);
        assert_eq!(
            variables,
            json!({
                "subredditName": "rust",
                "includeRecapFields": false,
                "includeDevvitData": false
            })
        );
    }
}
