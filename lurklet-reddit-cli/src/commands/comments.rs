use crate::commands::CommandOutput;
use clap::{Args, ValueEnum};
use lurklet_reddit::{CommentSort, Error, RedditBackend, RedditClient};
use serde_json::json;

#[derive(Args)]
pub struct GetPostComments {
    /// Bare post ID.
    post_id: String,
    #[arg(long, value_enum, default_value = "best")]
    sort: Sort,
    /// Opaque token for one placeholder, bound to the same post and sort.
    #[arg(long)]
    cursor: Option<String>,
}

#[derive(Args)]
pub struct GetCommentThread {
    /// Bare comment ID; this can be the anchor from a continue-thread marker.
    comment_id: String,
    #[arg(long, value_enum, default_value = "best")]
    sort: Sort,
}

#[derive(Clone, Copy, ValueEnum)]
enum Sort {
    Best,
    Top,
    New,
}

impl From<Sort> for CommentSort {
    fn from(sort: Sort) -> Self {
        match sort {
            Sort::Best => Self::Best,
            Sort::Top => Self::Top,
            Sort::New => Self::New,
        }
    }
}

impl GetPostComments {
    pub async fn execute<B: RedditBackend>(
        &self,
        client: &RedditClient<B>,
    ) -> Result<CommandOutput, Error> {
        let forest = client
            .get_post_comments(&self.post_id, self.sort.into(), self.cursor.as_deref())
            .await?;
        Ok(CommandOutput {
            result: json!(forest),
            error: None,
        })
    }
}

impl GetCommentThread {
    pub async fn execute<B: RedditBackend>(
        &self,
        client: &RedditClient<B>,
    ) -> Result<CommandOutput, Error> {
        let thread = client
            .get_comment_thread(&self.comment_id, self.sort.into())
            .await?;
        Ok(CommandOutput {
            result: json!(thread),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arguments::Arguments;
    use crate::commands::Command;
    use crate::commands::testing::{CommentsCall, FakeBackend, ThreadCall};
    use clap::Parser;
    use lurklet_reddit::{CommentForest, CommentItem, CommentThread};
    use serde_json::Value;

    #[tokio::test]
    async fn when_comments_are_dispatched_then_the_command_should_return_the_public_forest_and_opaque_actions()
     {
        for (arguments, sort, cursor) in [
            (
                vec!["cli", "get-post-comments", "post"],
                CommentSort::Best,
                None,
            ),
            (
                vec![
                    "cli",
                    "get-post-comments",
                    "post",
                    "--sort",
                    "top",
                    "--cursor",
                    "opaque",
                ],
                CommentSort::Top,
                Some("opaque"),
            ),
            (
                vec!["cli", "get-post-comments", "post", "--sort", "new"],
                CommentSort::New,
                None,
            ),
        ] {
            let args = Arguments::try_parse_from(arguments).unwrap();
            let Command::GetPostComments(args) = args.command else {
                panic!("expected comments command")
            };
            let expected = CommentForest {
                applied_sort: Some(sort),
                items: vec![CommentItem::More {
                    cursor: Some("next-opaque".into()),
                    count_hint: Some(2),
                }],
            };
            let client = RedditClient::from_backend(FakeBackend {
                comments: Some(CommentsCall {
                    post_id: "post".into(),
                    sort,
                    cursor: cursor.map(String::from),
                    result: Ok(expected.clone()),
                }),
                ..FakeBackend::default()
            });

            let output = args.execute(&client).await.unwrap();

            assert!(output.error.is_none());
            assert_eq!(output.result, json!(expected));
            assert_eq!(
                output.result["items"],
                json!([{"kind": "more", "cursor": "next-opaque", "count_hint": 2}])
            );
        }
    }

    #[tokio::test]
    async fn when_a_thread_is_dispatched_then_the_command_should_expose_context_without_expansion_tokens()
     {
        let args =
            Arguments::try_parse_from(["cli", "get-comment-thread", "anchor", "--sort", "new"])
                .unwrap();
        let Command::GetCommentThread(args) = args.command else {
            panic!("expected thread command")
        };
        let expected = CommentThread {
            post_id: Some("post".into()),
            forest: CommentForest {
                applied_sort: Some(CommentSort::New),
                items: vec![CommentItem::More {
                    cursor: None,
                    count_hint: Some(2),
                }],
            },
        };
        let client = RedditClient::from_backend(FakeBackend {
            thread: Some(ThreadCall {
                comment_id: "anchor".into(),
                sort: CommentSort::New,
                result: Ok(expected.clone()),
            }),
            ..FakeBackend::default()
        });

        let output = args.execute(&client).await.unwrap();

        assert_eq!(output.result, json!(expected));
        assert_eq!(output.result["forest"]["items"][0]["cursor"], Value::Null);
    }

    #[tokio::test]
    async fn when_comments_are_unavailable_then_the_command_should_preserve_the_error() {
        let args = Arguments::try_parse_from(["cli", "get-post-comments", "post"]).unwrap();
        let Command::GetPostComments(args) = args.command else {
            panic!("expected comments command")
        };
        let client = RedditClient::from_backend(FakeBackend {
            comments: Some(CommentsCall {
                post_id: "post".into(),
                sort: CommentSort::Best,
                cursor: None,
                result: Err(Error::Unavailable),
            }),
            ..FakeBackend::default()
        });

        let output = args.execute(&client).await;

        assert!(matches!(output, Err(Error::Unavailable)));
    }
}
