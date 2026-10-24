use super::bootstrap;
use clap::Subcommand;
use lurklet_reddit::{Error, RedditBackend, RedditClient};
use serde_json::Value;

#[derive(Subcommand)]
pub enum Command {
    /// Reuse a valid anonymous session or acquire one from Reddit. May make a live request.
    Bootstrap,
}

impl Command {
    pub async fn execute<B: RedditBackend>(
        &self,
        client: &RedditClient<B>,
        show_secrets: bool,
    ) -> Result<Value, Error> {
        match self {
            Self::Bootstrap => bootstrap::execute(client, show_secrets).await,
        }
    }
}
