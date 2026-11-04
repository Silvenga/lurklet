use crate::commands::CommandOutput;
use lurklet_reddit::{Error, RedditBackend, RedditClient, RedditId};
use serde_json::json;

pub async fn execute<B: RedditBackend>(
    client: &RedditClient<B>,
    id: &RedditId,
) -> Result<CommandOutput, Error> {
    let post = client.get_post(id).await?;
    Ok(CommandOutput {
        result: json!(post),
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arguments::Arguments;
    use crate::commands::Command;
    use crate::commands::testing::FakeBackend;
    use clap::Parser;
    use lurklet_reddit::{Post, PostAttachment};

    #[tokio::test]
    async fn when_a_post_is_requested_then_the_command_should_return_the_backend_neutral_json() {
        let arguments = Arguments::try_parse_from(["cli", "get-post", "18t5968"]).unwrap();
        let Command::GetPost { id } = arguments.command else {
            panic!("expected the post command")
        };
        let post = Post {
            id: "18t5968".into(),
            title: "Example".into(),
            permalink: Some("/r/example/comments/18t5968/example/".into()),
            flair: None,
            author: None,
            community: None,
            created_at: "2023-12-28T21:35:27.507Z".parse().unwrap(),
            score: None,
            comment_count: Some(2),
            is_nsfw: false,
            is_spoiler: true,
            markdown: Some("**Markdown**".into()),
            attachments: vec![PostAttachment::Link {
                url: "https://example.invalid/".into(),
            }],
            crosspost_id: None,
        };
        let client = RedditClient::from_backend(FakeBackend {
            post: Some(("18t5968".into(), Ok(post.clone()))),
            ..FakeBackend::default()
        });

        let output = execute(&client, &id).await.unwrap();

        assert!(output.error.is_none());
        assert_eq!(output.result, json!(post));
    }

    #[tokio::test]
    async fn when_a_fullname_is_supplied_then_the_cli_should_pass_a_normalized_typed_id_to_the_backend()
     {
        let arguments = Arguments::try_parse_from(["cli", "get-post", "t3_18t5968"]).unwrap();
        let Command::GetPost { id } = arguments.command else {
            panic!("expected post command")
        };
        let client = RedditClient::from_backend(FakeBackend {
            post: Some(("18t5968".into(), Err(Error::Unavailable))),
            ..FakeBackend::default()
        });

        let result = execute(&client, &id).await;

        assert_eq!(id, RedditId::from("18t5968"));
        assert!(matches!(result, Err(Error::Unavailable)));
    }
}
