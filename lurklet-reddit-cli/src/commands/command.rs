use crate::commands::{CommandOutput, bootstrap, get_request_allowlist};
use clap::Subcommand;
use lurklet_reddit::{Error, MobileBackend, RedditClient, StateStore};

#[derive(Subcommand)]
pub enum Command {
    /// Reuse a valid anonymous session or acquire one from Reddit. May make a live request.
    Bootstrap,
    /// Fetch the mobile GET allowlist as a POST diagnostic; does not enable GET or cache the list.
    GetRequestAllowlist,
}

impl Command {
    pub async fn execute<S: StateStore>(
        &self,
        client: &RedditClient<MobileBackend<S>>,
        show_secrets: bool,
    ) -> Result<CommandOutput, Error> {
        match self {
            Self::Bootstrap => Ok(CommandOutput {
                result: bootstrap::execute(client, show_secrets).await?,
                error: None,
            }),
            Self::GetRequestAllowlist => get_request_allowlist::execute(client, show_secrets).await,
        }
    }
}
