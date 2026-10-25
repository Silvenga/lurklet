use crate::commands::{
    CommandOutput, GetCommentThread, GetMedia, GetPostComments, ListSubredditPosts, bootstrap,
    get_post, get_request_allowlist, get_subreddit,
};
use crate::output::CliError;
use clap::Subcommand;
use lurklet_reddit::{MobileBackend, RedditClient, RedditId, StateStore};

#[derive(Subcommand)]
pub enum Command {
    /// Stream a supported public media URL to a new file without authentication. May make a live request.
    GetMedia(GetMedia),
    /// Reuse a valid anonymous session or acquire one from Reddit. May make a live request.
    Bootstrap,
    /// Fetch the mobile GET allowlist as a POST diagnostic; does not enable GET or cache the list.
    GetRequestAllowlist,
    /// Retrieve a post ID (bare or fullname) as Markdown and attachments. May make a live request.
    GetPost { id: RedditId },
    /// Read a bare community name's metadata or unavailability explanation. May make a live request.
    GetSubreddit { name: String },
    /// Read a post's comments or expand one ordinary placeholder. May make a live request.
    GetPostComments(GetPostComments),
    /// Open a comment thread with ancestor context. Further expansion is disabled. May make a live request.
    GetCommentThread(GetCommentThread),
    /// List one page of a subreddit without fetching individual posts. May make a live request.
    ListSubredditPosts(ListSubredditPosts),
}

impl Command {
    pub async fn execute<S: StateStore>(
        &self,
        client: &RedditClient<MobileBackend<S>>,
        show_secrets: bool,
    ) -> Result<CommandOutput, CliError> {
        Ok(match self {
            Self::GetMedia(arguments) => arguments.execute(client).await?,
            Self::Bootstrap => CommandOutput {
                result: bootstrap::execute(client, show_secrets).await?,
                error: None,
            },
            Self::GetRequestAllowlist => {
                get_request_allowlist::execute(client, show_secrets).await?
            }
            Self::GetPost { id } => get_post::execute(client, id).await?,
            Self::GetSubreddit { name } => get_subreddit::execute(client, name).await?,
            Self::GetPostComments(arguments) => arguments.execute(client).await?,
            Self::GetCommentThread(arguments) => arguments.execute(client).await?,
            Self::ListSubredditPosts(arguments) => arguments.execute(client).await?,
        })
    }
}
