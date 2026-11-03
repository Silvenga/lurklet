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
