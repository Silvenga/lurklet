use crate::backends::mobile::comments::Continuation;
use crate::{
    Comment, CommentForest, CommentItem, CommentSort, CommentState, GraphQlDecodeError,
    NamedReference, RedditId,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Forest {
    applied_sort: Option<String>,
    trees: Vec<Record>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    depth: Option<u32>,
    parent_id: Option<RedditId>,
    node: Option<Node>,
    more: Option<More>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum Node {
    Comment(Box<CommentFields>),
    DeletedComment(DeletedFields),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentFields {
    id: RedditId,
    author_info: Option<NamedReference>,
    created_at: Option<DateTime<Utc>>,
    content: Option<Content>,
    score: Option<i64>,
    is_score_hidden: Option<bool>,
    is_removed: Option<bool>,
    #[serde(rename = "isOP")]
    is_op: Option<bool>,
    is_stickied: Option<bool>,
    is_initially_collapsed: Option<bool>,
}

#[derive(Deserialize)]
struct Content {
    markdown: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeletedFields {
    created_at: Option<DateTime<Utc>>,
    is_initially_collapsed: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct More {
    count: Option<u64>,
    cursor: Option<String>,
    is_too_deep_for_count: Option<bool>,
}

#[derive(Clone, Copy)]
pub enum View<'a> {
    Post {
        post_id: &'a RedditId,
        sort: CommentSort,
        continuation: Option<&'a Continuation>,
    },
    Thread,
}

struct Pending {
    depth: u32,
    item: CommentItem,
}

impl Forest {
    pub fn into_forest(self, view: View<'_>) -> Result<CommentForest, GraphQlDecodeError> {
        let applied_sort = match self.applied_sort.as_deref() {
            Some("CONFIDENCE") => Some(CommentSort::Best),
            Some("TOP") => Some(CommentSort::Top),
            Some("NEW") => Some(CommentSort::New),
            Some(_) | None => None,
        };
        let (base_depth, root_parent) = match view {
            View::Post {
                continuation: Some(cursor),
                ..
            } => (cursor.depth, cursor.parent_id.clone()),
            View::Thread => match self.trees.first() {
                Some(record) => (
                    record.depth.ok_or(GraphQlDecodeError::InvalidData)?,
                    record.parent_id.clone(),
                ),
                None => (0, None),
            },
            View::Post {
                continuation: None, ..
            } => (0, None),
        };
        let mut items = Vec::new();
        // Preorder depth preserves structural parents even when DeletedComment has no ID.
        let mut pending: Vec<Pending> = Vec::new();
        for record in self.trees {
            let depth = record.depth.ok_or(GraphQlDecodeError::InvalidData)?;
            while pending.last().is_some_and(|parent| parent.depth >= depth) {
                attach_last(&mut pending, &mut items)?;
            }
            match pending.last() {
                Some(parent) => {
                    if parent.depth.checked_add(1) != Some(depth) {
                        return Err(GraphQlDecodeError::InvalidData);
                    }
                    let CommentItem::Comment { comment, .. } = &parent.item else {
                        return Err(GraphQlDecodeError::InvalidData);
                    };
                    if let (Some(parent_id), Some(reported_parent)) =
                        (&comment.id, &record.parent_id)
                        && parent_id != reported_parent
                    {
                        return Err(GraphQlDecodeError::InvalidData);
                    }
                }
                None if depth != base_depth || record.parent_id != root_parent => {
                    return Err(GraphQlDecodeError::InvalidData);
                }
                None => {}
            }

            let item = match (record.node, record.more) {
                (Some(Node::Comment(fields)), None) => CommentItem::Comment {
                    comment: Comment {
                        id: Some(fields.id),
                        state: if fields.is_removed == Some(true) {
                            CommentState::Removed
                        } else {
                            CommentState::Present
                        },
                        author: fields.author_info,
                        created_at: fields.created_at,
                        body_markdown: fields.content.and_then(|content| content.markdown),
                        score: if fields.is_score_hidden == Some(true) {
                            None
                        } else {
                            fields.score
                        },
                        is_op: fields.is_op,
                        is_pinned: fields.is_stickied,
                        initially_collapsed: fields.is_initially_collapsed,
                    },
                    replies: Vec::new(),
                },
                (Some(Node::DeletedComment(fields)), None) => CommentItem::Comment {
                    comment: Comment {
                        id: None,
                        state: CommentState::Deleted,
                        author: None,
                        created_at: fields.created_at,
                        body_markdown: None,
                        score: None,
                        is_op: None,
                        is_pinned: None,
                        initially_collapsed: fields.is_initially_collapsed,
                    },
                    replies: Vec::new(),
                },
                (None, Some(more)) => match more.is_too_deep_for_count {
                    Some(true) => CommentItem::ContinueThread {
                        comment_id: match view {
                            View::Post { .. } => record.parent_id,
                            View::Thread => None,
                        },
                    },
                    Some(false) => {
                        let cursor = match (view, more.cursor.filter(|cursor| !cursor.is_empty())) {
                            (View::Post { post_id, sort, .. }, Some(after))
                                if depth == 0 || record.parent_id.is_some() =>
                            {
                                Some(
                                    Continuation {
                                        post_id: post_id.clone(),
                                        sort,
                                        after,
                                        depth,
                                        parent_id: record.parent_id,
                                    }
                                    .encode()
                                    .map_err(|_| GraphQlDecodeError::InvalidData)?,
                                )
                            }
                            (View::Thread, _) | (View::Post { .. }, _) => None,
                        };
                        CommentItem::More {
                            cursor,
                            count_hint: more.count,
                        }
                    }
                    None => CommentItem::More {
                        cursor: None,
                        count_hint: None,
                    },
                },
                (Some(Node::Other), _) => return Err(GraphQlDecodeError::UnsupportedData),
                (Some(Node::Comment(_) | Node::DeletedComment(_)), Some(_)) | (None, None) => {
                    return Err(GraphQlDecodeError::InvalidData);
                }
            };
            pending.push(Pending { depth, item });
        }
        while !pending.is_empty() {
            attach_last(&mut pending, &mut items)?;
        }
        Ok(CommentForest {
            applied_sort,
            items,
        })
    }
}

fn attach_last(
    pending: &mut Vec<Pending>,
    roots: &mut Vec<CommentItem>,
) -> Result<(), GraphQlDecodeError> {
    let finished = pending.pop().expect("pending is not empty");
    if let Some(parent) = pending.last_mut() {
        let CommentItem::Comment { replies, .. } = &mut parent.item else {
            return Err(GraphQlDecodeError::InvalidData);
        };
        replies.push(finished.item);
    } else {
        roots.push(finished.item);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn value() -> Value {
        let response: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-post-comments.json"
        ))
        .unwrap();
        response["data"]["postInfoById"]["commentForest"].clone()
    }

    fn decode(value: Value, view: View<'_>) -> Result<CommentForest, GraphQlDecodeError> {
        let wire: Forest =
            serde_json::from_value(value).map_err(|_| GraphQlDecodeError::InvalidData)?;
        wire.into_forest(view)
    }

    fn post_view(post_id: &RedditId) -> View<'_> {
        View::Post {
            post_id,
            sort: CommentSort::Best,
            continuation: None,
        }
    }

    #[test]
    fn when_captured_forests_are_replayed_then_normalization_should_preserve_branch_context_and_disable_focused_actions()
     {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/captured-comment-forests.json"
        ))
        .unwrap();
        let continuation: Continuation =
            serde_json::from_value(fixture["expansion"]["cursor_context"].clone()).unwrap();
        let cases = [
            ("initial", post_view(&continuation.post_id)),
            (
                "expansion",
                View::Post {
                    post_id: &continuation.post_id,
                    sort: CommentSort::Best,
                    continuation: Some(&continuation),
                },
            ),
            ("thread", View::Thread),
        ];

        for (name, view) in cases {
            let forest = decode(fixture[name]["forest"].clone(), view).unwrap();
            let mut remaining: Vec<_> = forest.items.iter().rev().collect();
            let mut ids = Vec::new();
            let mut cursors = Vec::new();
            let mut thread_links = Vec::new();
            while let Some(item) = remaining.pop() {
                match item {
                    CommentItem::Comment { comment, replies } => {
                        ids.push(comment.id.as_ref().map(RedditId::as_str));
                        remaining.extend(replies.iter().rev());
                    }
                    CommentItem::More { cursor, .. } => cursors.extend(cursor.iter()),
                    CommentItem::ContinueThread { comment_id } => {
                        thread_links.extend(comment_id.iter())
                    }
                }
            }

            assert_eq!(json!(ids), fixture[name]["expected_ids"]);
            if name == "initial" {
                let decoded: Vec<_> = cursors
                    .iter()
                    .map(|token| {
                        Continuation::decode(token, &continuation.post_id, CommentSort::Best)
                            .unwrap()
                    })
                    .collect();
                assert!(decoded.contains(&continuation));
                assert_eq!(decoded.len(), 4);
            } else if name == "thread" {
                assert!(cursors.is_empty());
                assert!(thread_links.is_empty());
                assert_ne!(ids[0], fixture[name]["requested_comment_id"].as_str());
            } else {
                assert_eq!(forest.items.len(), 1);
            }
        }
    }

    #[test]
    fn when_deleted_parents_have_no_id_then_normalization_should_keep_their_children_without_inventing_identity()
     {
        let forest = decode(value(), post_view(&"example".into())).unwrap();

        let CommentItem::Comment {
            comment: parent,
            replies,
        } = &forest.items[0]
        else {
            panic!("expected deleted parent")
        };
        let CommentItem::Comment {
            comment: child,
            replies: child_replies,
        } = &replies[0]
        else {
            panic!("expected child")
        };

        assert_eq!(
            parent,
            &Comment {
                id: None,
                state: CommentState::Deleted,
                author: None,
                created_at: Some("2023-12-28T21:35:27.507Z".parse().unwrap()),
                body_markdown: None,
                score: None,
                is_op: None,
                is_pinned: None,
                initially_collapsed: Some(false),
            }
        );
        assert_eq!(
            child,
            &Comment {
                id: Some("child".into()),
                state: CommentState::Present,
                author: Some(NamedReference {
                    id: Some("author".into()),
                    name: Some("example_author".into())
                }),
                created_at: Some("2023-12-28T21:36:00.123456Z".parse().unwrap()),
                body_markdown: Some("**Reply**".into()),
                score: Some(7),
                is_op: Some(true),
                is_pinned: Some(false),
                initially_collapsed: Some(true),
            }
        );
        assert_eq!(child_replies.len(), 2);
        assert_eq!(forest.items.len(), 3);
        let output = serde_json::to_string(&forest).unwrap();
        assert!(!output.contains("moderationInfo"));
        assert!(!output.contains("script"));
        assert!(!output.contains("childCount"));
    }

    #[test]
    fn when_multiple_continuations_are_returned_then_each_should_retain_its_independent_position_and_context()
     {
        let forest = decode(value(), post_view(&"example".into())).unwrap();
        let CommentItem::Comment { replies, .. } = &forest.items[0] else {
            panic!("expected parent")
        };
        let CommentItem::Comment { replies, .. } = &replies[0] else {
            panic!("expected child")
        };

        let CommentItem::More {
            cursor: Some(nested),
            count_hint,
        } = &replies[0]
        else {
            panic!("expected nested continuation")
        };
        let nested = Continuation::decode(nested, &"example".into(), CommentSort::Best).unwrap();
        let CommentItem::More {
            cursor: Some(root),
            count_hint: root_count,
        } = &forest.items[1]
        else {
            panic!("expected root continuation")
        };
        let root = Continuation::decode(root, &"example".into(), CommentSort::Best).unwrap();

        assert_eq!(
            nested,
            Continuation {
                post_id: "example".into(),
                sort: CommentSort::Best,
                after: "nested-more+/=".into(),
                depth: 2,
                parent_id: Some("child".into())
            }
        );
        assert_eq!(*count_hint, Some(3));
        assert_eq!(*root_count, Some(0));
        assert_eq!(
            (root.depth, root.parent_id, root.after.as_str()),
            (0, None, "root-more")
        );
        assert_eq!(
            replies[1],
            CommentItem::ContinueThread {
                comment_id: Some("child".into())
            }
        );
    }

    #[test]
    fn when_a_nested_fragment_starts_below_zero_then_normalization_should_return_replacement_roots()
    {
        let continuation = Continuation {
            post_id: "example".into(),
            sort: CommentSort::Best,
            after: "nested".into(),
            depth: 2,
            parent_id: Some("parent".into()),
        };
        let wire = json!({"appliedSort": "CONFIDENCE", "trees": [
            {"depth": 2, "parentId": "t1_parent", "node": {"__typename": "DeletedComment"}},
            {"depth": 3, "parentId": "t1_unreturned", "node": {"__typename": "Comment", "id": "t1_child", "content": {"markdown": "Replacement"}}}
        ]});

        let forest = decode(
            wire,
            View::Post {
                post_id: &continuation.post_id,
                sort: CommentSort::Best,
                continuation: Some(&continuation),
            },
        )
        .unwrap();

        assert_eq!(forest.items.len(), 1);
        let CommentItem::Comment { comment, replies } = &forest.items[0] else {
            panic!("expected parent")
        };
        assert_eq!(comment.id, None);
        assert_eq!(replies.len(), 1);
    }

    #[test]
    fn when_a_thread_contains_ancestors_then_normalization_should_not_assign_the_requested_id_or_enable_expansion()
     {
        let response: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-comment-thread.json"
        ))
        .unwrap();

        let forest = decode(
            response["data"]["commentById"]["children"].clone(),
            View::Thread,
        )
        .unwrap();

        let CommentItem::Comment {
            comment: ancestor,
            replies,
        } = &forest.items[0]
        else {
            panic!("expected ancestor")
        };
        let CommentItem::Comment {
            comment: anchor,
            replies,
        } = &replies[0]
        else {
            panic!("expected anchor")
        };
        let CommentItem::Comment {
            comment: descendant,
            replies,
        } = &replies[0]
        else {
            panic!("expected descendant")
        };
        assert_eq!((ancestor.id.as_ref(), anchor.id.as_ref()), (None, None));
        assert_eq!(descendant.id.as_ref().unwrap().as_str(), "descendant");
        assert_eq!(
            replies,
            &vec![
                CommentItem::More {
                    cursor: None,
                    count_hint: Some(2)
                },
                CommentItem::ContinueThread { comment_id: None }
            ]
        );
    }

    #[test]
    fn when_a_thread_uses_a_local_nonzero_root_then_normalization_should_not_assume_post_view_depths()
     {
        let wire = json!({"trees": [
            {"depth": 5, "parentId": "t1_external", "node": {"__typename": "Comment", "id": "t1_anchor"}},
            {"depth": 6, "parentId": "t1_anchor", "node": {"__typename": "DeletedComment"}}
        ]});

        let forest = decode(wire, View::Thread).unwrap();

        assert_eq!(forest.items.len(), 1);
        let CommentItem::Comment { replies, .. } = &forest.items[0] else {
            panic!("expected root")
        };
        assert_eq!(replies.len(), 1);
    }

    #[test]
    fn when_parent_or_depth_metadata_is_inconsistent_then_normalization_should_not_guess_relationships()
     {
        for (depth, parent) in [
            (2, json!("t1_root")),
            (1, json!("t1_wrong")),
            (0, json!("t1_root")),
        ] {
            let wire = json!({"trees": [
                {"depth": 0, "parentId": null, "node": {"__typename": "Comment", "id": "t1_root"}},
                {"depth": depth, "parentId": parent, "node": {"__typename": "Comment", "id": "t1_child"}}
            ]});

            let result = decode(wire, post_view(&"example".into()));

            assert_eq!(result, Err(GraphQlDecodeError::InvalidData));
        }
    }

    #[test]
    fn when_a_fragment_belongs_to_a_different_parent_then_normalization_should_reject_misplacement()
    {
        let cursor = Continuation {
            post_id: "example".into(),
            sort: CommentSort::Best,
            after: "opaque".into(),
            depth: 1,
            parent_id: Some("expected".into()),
        };
        let wire = json!({"trees": [{"depth": 1, "parentId": "t1_other", "node": {"__typename": "Comment", "id": "t1_child"}}]});

        let result = decode(
            wire,
            View::Post {
                post_id: &cursor.post_id,
                sort: CommentSort::Best,
                continuation: Some(&cursor),
            },
        );

        assert_eq!(result, Err(GraphQlDecodeError::InvalidData));
    }

    #[test]
    fn when_a_continuation_has_no_actionable_target_then_normalization_should_keep_a_nonactionable_marker()
     {
        let wire = json!({"trees": [
            {"depth": 0, "node": null, "more": {"count": 0, "cursor": "not-an-id", "isTooDeepForCount": true}},
            {"depth": 0, "node": null, "more": {"count": 2, "cursor": null, "isTooDeepForCount": false}},
            {"depth": 0, "node": null, "more": {"count": 2, "cursor": "not-enough-context"}}
        ]});

        let forest = decode(wire, post_view(&"example".into())).unwrap();

        assert_eq!(
            forest.items,
            vec![
                CommentItem::ContinueThread { comment_id: None },
                CommentItem::More {
                    cursor: None,
                    count_hint: Some(2)
                },
                CommentItem::More {
                    cursor: None,
                    count_hint: None
                },
            ]
        );
    }

    #[test]
    fn when_a_nested_placeholder_lacks_parent_context_then_normalization_should_not_emit_an_unusable_cursor()
     {
        let wire = json!({"trees": [
            {"depth": 0, "parentId": null, "node": {"__typename": "Comment", "id": "t1_root"}},
            {"depth": 1, "parentId": null, "node": null, "more": {"count": 3, "cursor": "opaque", "isTooDeepForCount": false}}
        ]});

        let forest = decode(wire, post_view(&"example".into())).unwrap();

        let CommentItem::Comment { replies, .. } = &forest.items[0] else {
            panic!("expected parent")
        };
        assert_eq!(
            replies,
            &vec![CommentItem::More {
                cursor: None,
                count_hint: Some(3)
            }]
        );
    }

    #[test]
    fn when_a_comment_is_removed_or_its_author_is_unavailable_then_the_record_should_remain_present()
     {
        let forest = decode(value(), post_view(&"example".into())).unwrap();

        let CommentItem::Comment { comment, replies } = &forest.items[2] else {
            panic!("expected removed comment")
        };

        assert_eq!(comment.state, CommentState::Removed);
        assert_eq!(comment.id.as_ref().unwrap().as_str(), "removed");
        assert_eq!(
            comment.author,
            Some(NamedReference {
                id: None,
                name: None
            })
        );
        assert_eq!(comment.score, None);
        assert!(replies.is_empty());
    }

    #[test]
    fn when_a_forest_is_empty_then_normalization_should_return_a_successful_empty_result() {
        let result = decode(
            json!({"appliedSort": "NEW", "trees": []}),
            post_view(&"example".into()),
        )
        .unwrap();

        assert_eq!(
            result,
            CommentForest {
                applied_sort: Some(CommentSort::New),
                items: Vec::new()
            }
        );
    }

    #[test]
    fn when_structural_records_are_missing_then_normalization_should_not_report_an_empty_forest() {
        for wire in [
            json!({}),
            json!({"trees": null}),
            json!({"trees": [null]}),
            json!({"trees": [{"depth": null}]}),
            json!({"trees": [{"depth": 0, "node": null, "more": null}]}),
        ] {
            let result = decode(wire, post_view(&"example".into()));

            assert_eq!(result, Err(GraphQlDecodeError::InvalidData));
        }
    }
}
