use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::str::FromStr;

/// A bare Reddit ID, normalized from a fullname when constructed, parsed, or deserialized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String")]
pub struct RedditId(String);

impl RedditId {
    /// Returns the bare ID for use with another client operation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RedditId {
    fn from(value: String) -> Self {
        if let Some((kind, id)) = value.split_once('_')
            && kind.strip_prefix('t').is_some_and(|kind| {
                !kind.is_empty() && kind.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            Self(id.to_owned())
        } else {
            Self(value)
        }
    }
}

impl From<&str> for RedditId {
    fn from(value: &str) -> Self {
        Self::from(value.to_owned())
    }
}

impl FromStr for RedditId {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn when_an_id_is_parsed_then_it_should_use_the_same_normalization_as_deserialization() {
        for value in ["example", "t1_example", "t3_example", "t5_example"] {
            let parsed: RedditId = value.parse().unwrap();
            let deserialized: RedditId = serde_json::from_value(json!(value)).unwrap();

            assert_eq!(parsed, deserialized);
            assert_eq!(parsed.as_str(), "example");
        }
    }

    #[test]
    fn when_fullnames_are_deserialized_then_ids_should_share_one_normalization_rule() {
        for value in [
            "example",
            "t1_example",
            "t2_example",
            "t3_example",
            "t5_example",
        ] {
            let id: RedditId = serde_json::from_value(json!(value)).unwrap();

            assert_eq!(id.as_str(), "example");
            assert_eq!(serde_json::to_value(&id).unwrap(), json!("example"));
        }
    }

    #[test]
    fn when_an_id_is_not_a_fullname_then_normalization_should_leave_it_unchanged() {
        for value in ["example", "other_example"] {
            let id = RedditId::from(value);

            assert_eq!(id.as_str(), value);
        }
    }

    #[test]
    fn when_ids_are_nested_in_options_and_lists_then_serde_should_apply_the_same_normalization() {
        let input = json!(["t2_author", null, "t5_community"]);

        let ids: Vec<Option<RedditId>> = serde_json::from_value(input).unwrap();

        assert_eq!(
            ids,
            vec![Some("author".into()), None, Some("community".into())]
        );
    }
}
