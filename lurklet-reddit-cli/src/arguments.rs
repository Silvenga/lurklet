use crate::client_options::ClientOptions;
use crate::commands::Command;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, arg_required_else_help = true)]
pub struct Arguments {
    /// Restore and atomically save session state; otherwise state is ephemeral.
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
