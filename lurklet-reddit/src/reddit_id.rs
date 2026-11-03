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
    fn when_an_id_is_constructed_or_decoded_then_it_should_serialize_as_the_normalized_id() {
        for (value, expected) in [
            ("example", "example"),
            ("t1_example", "example"),
            ("t2_example", "example"),
            ("t3_example", "example"),
            ("t5_example", "example"),
            ("other_example", "other_example"),
        ] {
            let parsed: RedditId = value.parse().unwrap();
            let deserialized: RedditId = serde_json::from_value(json!(value)).unwrap();
            let constructed = RedditId::from(value);

            assert_eq!(parsed, deserialized);
            assert_eq!(parsed, constructed);
            assert_eq!(parsed.as_str(), expected);
            assert_eq!(serde_json::to_value(&parsed).unwrap(), json!(expected));
        }
    }
}
