use serde::Serialize;
use serde_json::Value;

/// Known server error meanings; the original error object remains authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphQlErrorKind {
    InvalidInput,
    PersistedQueryNotFound,
    NotAuthenticated,
    Other,
}

impl GraphQlErrorKind {
    /// Recognizes known codes without interpreting every NOT_FOUND as an unknown query.
    pub fn from_error(error: &Value) -> Self {
        match error.pointer("/extensions/code").and_then(Value::as_str) {
            Some("INVALID_INPUT") => Self::InvalidInput,
            Some("NOT_AUTHENTICATED") => Self::NotAuthenticated,
            Some("NOT_FOUND") => {
                let message = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim();
                if message.eq_ignore_ascii_case("PersistedQueryNotFound")
                    || message
                        .trim_end_matches('.')
                        .eq_ignore_ascii_case("Persisted query not found")
                {
                    Self::PersistedQueryNotFound
                } else {
                    Self::Other
                }
            }
            _ => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn when_not_found_refers_to_content_then_classification_should_not_report_an_unknown_query() {
        let query =
            json!({"message": "PersistedQueryNotFound", "extensions": {"code": "NOT_FOUND"}});
        let post = json!({"message": "Post not found", "extensions": {"code": "NOT_FOUND"}});

        let results = [
            GraphQlErrorKind::from_error(&query),
            GraphQlErrorKind::from_error(&post),
        ];

        assert_eq!(
            results,
            [
                GraphQlErrorKind::PersistedQueryNotFound,
                GraphQlErrorKind::Other
            ]
        );
    }
}
