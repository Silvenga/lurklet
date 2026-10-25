use lurklet_reddit::{Error, RedditBackend, RedditClient};
use serde_json::{Value, json};

pub async fn execute<B: RedditBackend>(
    client: &RedditClient<B>,
    show_secrets: bool,
) -> Result<Value, Error> {
    let session = client.bootstrap().await?;
    Ok(json!({
        "access_token": if show_secrets { session.access_token.expose_secret() } else { "[REDACTED]" },
        "expires_at_ms": session.expires_at_ms,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arguments::Arguments;
    use clap::Parser;
    use lurklet_reddit::{AccessToken, AnonymousSession, Post};

    struct FakeBackend;

    impl RedditBackend for FakeBackend {
        async fn get_post(&self, _: &str) -> Result<Post, Error> {
            unreachable!("bootstrap must not retrieve posts")
        }

        async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
            Ok(AnonymousSession {
                access_token: AccessToken::new("private-token"),
                expires_at_ms: 1234,
            })
        }
    }

    #[tokio::test]
    async fn when_bootstrap_is_dispatched_then_the_public_operation_should_return_redacted_json() {
        let arguments = Arguments::try_parse_from(["cli", "bootstrap"]).unwrap();
        let client = RedditClient::from_backend(FakeBackend);

        let result = execute(&client, arguments.show_secrets).await.unwrap();

        assert_eq!(
            result,
            json!({"access_token": "[REDACTED]", "expires_at_ms": 1234})
        );
    }

    #[tokio::test]
    async fn when_secret_output_is_requested_then_the_public_result_should_expose_the_token() {
        let arguments = Arguments::try_parse_from(["cli", "bootstrap", "--show-secrets"]).unwrap();
        let client = RedditClient::from_backend(FakeBackend);

        let result = execute(&client, arguments.show_secrets).await.unwrap();

        assert_eq!(result["access_token"], "private-token");
    }
}
