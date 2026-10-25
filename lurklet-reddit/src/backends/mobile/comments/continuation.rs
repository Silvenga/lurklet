use crate::backends::mobile::continuations;
use crate::{CommentSort, Error, RedditId};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Continuation {
    pub post_id: RedditId,
    pub sort: CommentSort,
    pub after: String,
    pub depth: u32,
    pub parent_id: Option<RedditId>,
}

impl Continuation {
    pub fn decode(token: &str, post_id: &RedditId, sort: CommentSort) -> Result<Self, Error> {
        let continuation: Self = continuations::decode(token, "cursor")?;
        if &continuation.post_id != post_id || continuation.sort != sort {
            return Err(Error::InvalidRequest { field: "cursor" });
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
    use crate::SubredditSort;
    use crate::backends::mobile::subreddit_feeds::Continuation as FeedContinuation;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use uuid::Uuid;

    #[test]
    fn when_a_comment_cursor_is_encoded_then_decoding_should_restore_its_branch_context() {
        let expected = Continuation {
            post_id: "post".into(),
            sort: CommentSort::Top,
            after: "opaque+/=%".into(),
            depth: 2,
            parent_id: Some("parent".into()),
        };

        let token = expected.encode().unwrap();
        let restored = Continuation::decode(&token, &"post".into(), CommentSort::Top).unwrap();

        assert_eq!(restored, expected);
        assert!(
            token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        );
    }

    #[test]
    fn when_post_or_sort_changes_then_decoding_should_reject_the_cursor() {
        let token = Continuation {
            post_id: "post".into(),
            sort: CommentSort::Best,
            after: "next".into(),
            depth: 0,
            parent_id: None,
        }
        .encode()
        .unwrap();

        for (id, sort) in [("other", CommentSort::Best), ("post", CommentSort::New)] {
            let result = Continuation::decode(&token, &id.into(), sort);

            assert!(matches!(
                result,
                Err(Error::InvalidRequest { field: "cursor" })
            ));
        }
    }

    #[test]
    fn when_a_feed_cursor_is_supplied_then_comment_decoding_should_reject_the_other_context() {
        let feed = FeedContinuation {
            subreddit: "rust".into(),
            sort: SubredditSort::Hot,
            navigation_session_id: Uuid::new_v4(),
            after: "next".into(),
            distance: None,
        }
        .encode()
        .unwrap();

        let result = Continuation::decode(&feed, &"post".into(), CommentSort::Best);

        assert!(matches!(
            result,
            Err(Error::InvalidRequest { field: "cursor" })
        ));
    }

    #[test]
    fn when_an_existing_cursor_uses_a_string_id_then_typed_context_should_preserve_its_encoding() {
        let encoded = URL_SAFE_NO_PAD.encode(
            br#"{"post_id":"post","sort":"best","after":"next","depth":0,"parent_id":null}"#,
        );
        let post_id = RedditId::from("t3_post");

        let restored = Continuation::decode(&encoded, &post_id, CommentSort::Best).unwrap();
        let reencoded = restored.encode().unwrap();

        assert_eq!(restored.post_id, post_id);
        assert_eq!(reencoded, encoded);
    }
}
