use crate::commands::CommandOutput;
use lurklet_reddit::{Error, RedditBackend, RedditClient};
use serde_json::json;

pub async fn execute<B: RedditBackend>(
    client: &RedditClient<B>,
    name: &str,
) -> Result<CommandOutput, Error> {
    let subreddit = client.get_subreddit(name).await?;
    Ok(CommandOutput {
        result: json!(subreddit),
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
    use lurklet_reddit::{Subreddit, SubredditDetails, UnavailableSubreddit};

    #[tokio::test]
    async fn when_metadata_is_requested_then_the_command_should_forward_the_name_and_return_display_metadata()
     {
        let arguments = Arguments::try_parse_from(["cli", "get-subreddit", "Rust"]).unwrap();
        let Command::GetSubreddit { name } = arguments.command else {
            panic!("expected the subreddit command")
        };
        let client = RedditClient::from_backend(FakeBackend {
            subreddit: Some((
                "Rust".into(),
                Ok(Subreddit::Available(Box::new(SubredditDetails {
                    id: "t5_fixture".into(),
                    name: "rust".into(),
                    title: Some("Example community".into()),
                    created_at: Some("2010-12-02T21:27:18.336Z".parse().unwrap()),
                    short_description: Some("Short description".into()),
                    description_markdown: Some("## About".into()),
                    subscriber_count: Some(123),
                    weekly_active_user_count: Some(20),
                    weekly_contribution_count: Some(7),
                    styles: None,
                    community_type: Some("PUBLIC".into()),
                    is_nsfw: Some(false),
                    is_posting_restricted: Some(false),
                    is_quarantined: Some(false),
                    quarantine_message_markdown: None,
                    interstitial_warning_markdown: None,
                }))),
            )),
            ..FakeBackend::default()
        });

        let output = execute(&client, &name).await.unwrap();

        assert_eq!(
            output.result,
            json!({
                "kind": "available",
                "id": "fixture",
                "name": "rust",
                "title": "Example community",
                "created_at": "2010-12-02T21:27:18.336Z",
                "short_description": "Short description",
                "description_markdown": "## About",
                "subscriber_count": 123,
                "weekly_active_user_count": 20,
                "weekly_contribution_count": 7,
                "styles": null,
                "community_type": "PUBLIC",
                "is_nsfw": false,
                "is_posting_restricted": false,
                "is_quarantined": false,
                "quarantine_message_markdown": null,
                "interstitial_warning_markdown": null
            })
        );
        assert_eq!(output.error, None);
    }

    #[tokio::test]
    async fn when_unavailable_metadata_is_returned_then_the_command_should_succeed_without_inventing_identity()
     {
        let client = RedditClient::from_backend(FakeBackend {
            subreddit: Some((
                "example".into(),
                Ok(Subreddit::Unavailable(UnavailableSubreddit {
                    id: None,
                    forbidden_reason: Some("PRIVATE".into()),
                    ban_title: None,
                    ban_message_markdown: None,
                    quarantine_message_markdown: None,
                    interstitial_warning_markdown: Some("This community is private.".into()),
                })),
            )),
            ..FakeBackend::default()
        });

        let output = execute(&client, "example").await.unwrap();

        assert_eq!(
            output.result,
            json!({
                "kind": "unavailable",
                "id": null,
                "forbidden_reason": "PRIVATE",
                "ban_title": null,
                "ban_message_markdown": null,
                "quarantine_message_markdown": null,
                "interstitial_warning_markdown": "This community is private."
            })
        );
        assert_eq!(output.error, None);
    }

    #[tokio::test]
    async fn when_no_metadata_is_available_then_the_command_should_propagate_the_error() {
        let client = RedditClient::from_backend(FakeBackend {
            subreddit: Some(("example".into(), Err(Error::Unavailable))),
            ..FakeBackend::default()
        });

        let result = execute(&client, "example").await;

        assert!(matches!(result, Err(Error::Unavailable)));
    }
}
