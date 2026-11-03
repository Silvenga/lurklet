use crate::client_options::ClientOptions;
use crate::commands::Command;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, arg_required_else_help = true)]
pub struct Arguments {
    /// Restore and atomically save session state; otherwise state is ephemeral. Ignored for get-media.
    #[arg(long, global = true)]
    pub state_file: Option<PathBuf>,
    /// Capture response headers and bodies, including any credentials they contain.
    #[arg(long, global = true)]
    pub capture_dir: Option<PathBuf>,
    /// Include credentials in JSON results. Never enables credential logging.
    #[arg(long, global = true)]
    pub show_secrets: bool,
    #[command(flatten)]
    pub client: ClientOptions,
    #[command(subcommand)]
    pub command: Command,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter;

    #[test]
    fn when_command_arguments_are_incomplete_or_unsupported_then_parsing_should_reject_them() {
        let cases: &[&[&str]] = &[
            &[
                "get-request-allowlist",
                "--allowlist-operation-name",
                "OnlyName",
            ],
            &["get-request-allowlist", "--allowlist-sha256", "OnlyHash"],
            &["get-subreddit"],
            &["get-subreddit", "rust", "space"],
            &["get-subreddit", "rust", "--opt-in"],
            &["get-subreddit", "rust", "--cursor", "opaque"],
            &["get-post"],
            &["get-post", "one", "two"],
            &["list-subreddit-posts"],
            &["list-subreddit-posts", "rust", "--limit", "10"],
            &["list-subreddit-posts", "rust", "--sort", "awarded"],
            &["get-post-comments"],
            &["get-post-comments", "post", "--sort", "random"],
            &["get-post-comments", "post", "--count", "100"],
            &["get-comment-thread", "anchor", "--cursor", "opaque"],
            &["get-media"],
            &["get-media", "https://example.invalid/file"],
            &["get-media", "--output", "file.bin"],
        ];

        for arguments in cases {
            let result =
                Arguments::try_parse_from(iter::once("cli").chain(arguments.iter().copied()));

            assert!(result.is_err(), "{arguments:?}");
        }
    }
}
