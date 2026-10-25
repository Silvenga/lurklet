use crate::backends::mobile::continuations;
use crate::{Error, SubredditSort};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Continuation {
    pub subreddit: String,
    pub sort: SubredditSort,
    pub navigation_session_id: Uuid,
    pub after: String,
    pub distance: Option<i64>,
}

impl Continuation {
    pub fn decode(token: &str, subreddit: &str, sort: SubredditSort) -> Result<Self, Error> {
        let continuation: Self = continuations::decode(token, "after")?;
        if continuation.subreddit != subreddit || continuation.sort != sort {
            return Err(Error::InvalidRequest { field: "after" });
        }
        Ok(continuation)
    }

    pub fn encode(&self) -> Result<String, Error> {
        continuations::encode(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeRange;

    #[test]
    fn when_continuation_is_encoded_then_decoding_should_restore_all_page_context() {
        let expected = Continuation {
            subreddit: "rust".into(),
            sort: SubredditSort::Top(TimeRange::Day),
            navigation_session_id: Uuid::new_v4(),
            after: "upstream+/=% cursor".into(),
            distance: Some(4),
        };

        let token = expected.encode().unwrap();
        let restored =
            Continuation::decode(&token, "rust", SubredditSort::Top(TimeRange::Day)).unwrap();

        assert_eq!(restored, expected);
        assert!(
            token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        );
    }

    #[test]
    fn when_continuation_has_the_wrong_context_then_decoding_should_reject_it() {
        let token = Continuation {
            subreddit: "rust".into(),
            sort: SubredditSort::Hot,
            navigation_session_id: Uuid::new_v4(),
            after: "next".into(),
            distance: None,
        }
        .encode()
        .unwrap();

        for (subreddit, sort) in [("other", SubredditSort::Hot), ("rust", SubredditSort::New)] {
            let result = Continuation::decode(&token, subreddit, sort);

            assert!(matches!(
                result,
                Err(Error::InvalidRequest { field: "after" })
            ));
        }
    }

    #[test]
    fn when_a_cursor_is_malformed_then_decoding_should_return_an_input_error() {
        for token in ["", "not a token!", "bnVsbA"] {
            let result = Continuation::decode(token, "rust", SubredditSort::Hot);

            assert!(matches!(
                result,
                Err(Error::InvalidRequest { field: "after" })
            ));
        }
    }
}
