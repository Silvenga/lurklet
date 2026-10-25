use crate::commands::CommandOutput;
use crate::output::graphql_output;
use lurklet_reddit::{Error, MobileBackend, RedditClient, StateStore};

pub async fn execute<S: StateStore>(
    client: &RedditClient<MobileBackend<S>>,
    show_secrets: bool,
) -> Result<CommandOutput, Error> {
    let response = client.get_request_allowlist().await?;
    Ok(graphql_output(response, show_secrets))
}
