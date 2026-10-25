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
    use lurklet_reddit::{NamedReference, Post, PostAttachment, VideoFormat, VideoSource};

    #[tokio::test]
    async fn when_a_post_is_requested_then_the_command_should_return_the_backend_neutral_json() {
        let arguments = Arguments::try_parse_from(["cli", "get-post", "18t5968"]).unwrap();
        let Command::GetPost { id } = arguments.command else {
            panic!("expected the post command")
        };
        let post = Post {
            id: "18t5968".into(),
            title: "Example".into(),
            author: None,
            community: Some(NamedReference {
                id: Some("community".into()),
                name: Some("example".into()),
            }),
            created_at: "2023-12-28T21:35:27.507Z".parse().unwrap(),
            score: None,
            comment_count: Some(2),
            is_nsfw: false,
            is_spoiler: true,
            body_markdown: Some("**Markdown**".into()),
            attachments: vec![
                PostAttachment::Link {
                    url: "https://example.invalid/".into(),
                },
                PostAttachment::Video {
                    sources: vec![
                        VideoSource {
                            url: "https://example.invalid/video.mp4".into(),
                            format: VideoFormat::Mp4,
                        },
                        VideoSource {
                            url: "https://example.invalid/video.m3u8".into(),
                            format: VideoFormat::Hls,
                        },
                    ],
                    poster_url: None,
                },
            ],
            crosspost_id: None,
        };
        let client = RedditClient::from_backend(FakeBackend {
            post: Some(("18t5968".into(), Ok(post.clone()))),
            ..FakeBackend::default()
        });

        let output = execute(&client, &id).await.unwrap();

        assert!(output.error.is_none());
        assert_eq!(output.result, json!(post));
        assert_eq!(
            output.result["attachments"],
            json!([
                {"kind": "link", "url": "https://example.invalid/"},
                {"kind": "video", "sources": [
                    {"url": "https://example.invalid/video.mp4", "format": "mp4"},
                    {"url": "https://example.invalid/video.m3u8", "format": "hls"}
                ], "poster_url": null}
            ])
        );
        assert!(output.result.get("graphql").is_none());
        assert_eq!(output.result["created_at"], "2023-12-28T21:35:27.507Z");
        assert_eq!(
            output.result["community"],
            json!({"id": "community", "name": "example"})
        );
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

    #[tokio::test]
    async fn when_the_post_is_unavailable_then_the_command_should_propagate_the_unavailable_error()
    {
        let client = RedditClient::from_backend(FakeBackend {
            post: Some(("18t5968".into(), Err(Error::Unavailable))),
            ..FakeBackend::default()
        });

        let output = execute(&client, &"18t5968".into()).await;

        assert!(matches!(output, Err(Error::Unavailable)));
    }

    #[tokio::test]
    async fn when_the_lookup_fails_then_the_command_should_propagate_the_library_error() {
        let client = RedditClient::from_backend(FakeBackend {
            post: Some(("18t5968".into(), Err(Error::OperationFailed))),
            ..FakeBackend::default()
        });

        let result = execute(&client, &"18t5968".into()).await;

        assert!(matches!(result, Err(Error::OperationFailed)));
    }
}
