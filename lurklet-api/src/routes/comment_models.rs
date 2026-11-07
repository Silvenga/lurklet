use crate::routes::Markdown;
use chrono::{DateTime, Utc};
use lurklet_reddit::{
    Comment as RedditComment, CommentForest as RedditCommentForest,
    CommentItem as RedditCommentItem, CommentSort as RedditCommentSort,
    CommentState as RedditCommentState, NamedReference as RedditNamedReference,
};
use o2o::o2o;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Deserialize, Serialize, ToSchema, o2o)]
#[map_owned(RedditCommentSort)]
#[serde(rename_all = "camelCase")]
#[schema(default = "best")]
pub enum CommentSort {
    #[default]
    Best,
    Top,
    New,
}

/// Ordered discussion roots, or replacement items for one selected placeholder.
#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditCommentForest)]
#[serde(rename_all = "camelCase")]
pub struct CommentForest {
    #[schema(required)]
    #[from(~.map(Into::into))]
    applied_sort: Option<CommentSort>,
    #[from(~.into_iter().map(Into::into).collect())]
    items: Vec<CommentItem>,
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditCommentItem | attribute(expect(
    clippy::redundant_field_names,
    reason = "o2o generates explicit field names for enum variant constructors"
)))]
#[serde(tag = "kind", rename_all = "camelCase")]
enum CommentItem {
    Comment {
        #[from(~.into())]
        comment: Comment,
        #[schema(no_recursion)]
        #[from(~.into_iter().map(Into::into).collect())]
        replies: Vec<CommentItem>,
    },
    #[serde(rename_all = "camelCase")]
    More {
        /// Opaque cursor for replacing this placeholder; null means no action is available.
        #[schema(required)]
        cursor: Option<String>,
        /// Upstream hint, not an exact count.
        #[schema(required)]
        count_hint: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    ContinueThread {
        /// Anchor for the thread endpoint; null means no action is available.
        #[schema(required)]
        #[from(~.map(|id| id.as_str().to_owned()))]
        comment_id: Option<String>,
    },
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditComment)]
#[serde(rename_all = "camelCase")]
struct Comment {
    #[schema(required)]
    #[from(~.map(|id| id.as_str().to_owned()))]
    id: Option<String>,
    #[from(~.into())]
    state: CommentState,
    #[schema(required)]
    #[from(~.map(Into::into))]
    author: Option<CommentAuthor>,
    #[schema(required)]
    created_at: Option<DateTime<Utc>>,
    /// Untrusted Markdown; embedded media URLs are not proxied.
    #[schema(required)]
    #[from(~.map(Into::into))]
    markdown: Option<Markdown>,
    #[schema(required)]
    score: Option<i64>,
    #[schema(required)]
    is_op: Option<bool>,
    #[schema(required)]
    is_pinned: Option<bool>,
    #[schema(required)]
    initially_collapsed: Option<bool>,
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditCommentState)]
#[serde(rename_all = "camelCase")]
enum CommentState {
    Present,
    Deleted,
    Removed,
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditNamedReference)]
#[serde(rename_all = "camelCase")]
struct CommentAuthor {
    #[schema(required)]
    #[from(~.map(|id| id.as_str().to_owned()))]
    id: Option<String>,
    #[schema(required)]
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn when_a_forest_is_converted_then_comment_states_reply_order_and_actions_should_be_preserved()
    {
        let present = RedditComment {
            id: Some("reply".into()),
            state: RedditCommentState::Present,
            author: Some(RedditNamedReference {
                id: Some("author".into()),
                name: Some("example-user".into()),
            }),
            created_at: Some(DateTime::from_timestamp(0, 0).unwrap()),
            markdown: Some("**Untrusted** Markdown".into()),
            score: Some(-1),
            is_op: Some(true),
            is_pinned: Some(false),
            initially_collapsed: Some(false),
        };
        let deleted = RedditComment {
            id: None,
            state: RedditCommentState::Deleted,
            author: None,
            created_at: None,
            markdown: None,
            score: None,
            is_op: None,
            is_pinned: None,
            initially_collapsed: Some(true),
        };
        let removed = RedditComment {
            id: Some("removed".into()),
            state: RedditCommentState::Removed,
            initially_collapsed: None,
            ..deleted.clone()
        };
        let forest = RedditCommentForest {
            applied_sort: Some(RedditCommentSort::New),
            items: vec![
                RedditCommentItem::Comment {
                    comment: deleted,
                    replies: vec![
                        RedditCommentItem::Comment {
                            comment: present,
                            replies: vec![RedditCommentItem::More {
                                cursor: Some("nested+/=% token".into()),
                                count_hint: Some(5),
                            }],
                        },
                        RedditCommentItem::ContinueThread {
                            comment_id: Some("anchor".into()),
                        },
                    ],
                },
                RedditCommentItem::More {
                    cursor: None,
                    count_hint: None,
                },
                RedditCommentItem::ContinueThread { comment_id: None },
                RedditCommentItem::Comment {
                    comment: removed,
                    replies: Vec::new(),
                },
            ],
        };

        let actual = serde_json::to_value(CommentForest::from(forest)).unwrap();

        assert_eq!(
            actual,
            json!({
                "appliedSort": "new",
                "items": [
                    {
                        "kind": "comment",
                        "comment": {
                            "id": null, "state": "deleted", "author": null, "createdAt": null,
                            "markdown": null, "score": null, "isOp": null, "isPinned": null,
                            "initiallyCollapsed": true
                        },
                        "replies": [
                            {
                                "kind": "comment",
                                "comment": {
                                    "id": "reply", "state": "present",
                                    "author": {"id": "author", "name": "example-user"},
                                    "createdAt": "1970-01-01T00:00:00Z",
                                    "markdown": {"content": "**Untrusted** Markdown", "media": []}, "score": -1,
                                    "isOp": true, "isPinned": false, "initiallyCollapsed": false
                                },
                                "replies": [{"kind": "more", "cursor": "nested+/=% token", "countHint": 5}]
                            },
                            {"kind": "continueThread", "commentId": "anchor"}
                        ]
                    },
                    {"kind": "more", "cursor": null, "countHint": null},
                    {"kind": "continueThread", "commentId": null},
                    {
                        "kind": "comment",
                        "comment": {
                            "id": "removed", "state": "removed", "author": null, "createdAt": null,
                            "markdown": null, "score": null, "isOp": null, "isPinned": null,
                            "initiallyCollapsed": null
                        },
                        "replies": []
                    }
                ]
            })
        );
    }

    #[test]
    fn when_comment_sorts_are_converted_then_each_selection_should_round_trip() {
        for (sort, expected) in [
            (RedditCommentSort::Best, "best"),
            (RedditCommentSort::Top, "top"),
            (RedditCommentSort::New, "new"),
        ] {
            let api = CommentSort::from(sort);

            let serialized = serde_json::to_value(&api).unwrap();
            let restored: RedditCommentSort = api.into();

            assert_eq!(serialized, json!(expected));
            assert_eq!(restored, sort);
        }
    }
}
