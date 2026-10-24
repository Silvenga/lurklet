use arguments::Arguments;
use clap::Parser;
use output::CliError;
use serde_json::json;
use std::env::{self, VarError};
use std::io::{self, Write};
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

mod arguments;
mod captures;
mod client_options;
mod commands;
mod output;
mod run;
mod state_file;

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = Arguments::parse();
    let result = async {
        let filter = match env::var("RUST_LOG") {
            Ok(value) => EnvFilter::try_new(value).map_err(|_| CliError::Logging)?,
            Err(VarError::NotPresent) => {
                EnvFilter::new("warn,lurklet_reddit=trace,lurklet_reddit_cli=trace")
            }
            Err(VarError::NotUnicode(_)) => return Err(CliError::Logging),
        };
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(io::stderr)
            .with_ansi(false)
            .try_init()
            .map_err(|_| CliError::Logging)?;
        run::run(arguments).await
    }
    .await;
    let output = result.unwrap_or_else(|error| json!({"ok": false, "error": error.json()}));
    let success = output["ok"] == true;
    let mut stdout = io::stdout().lock();
    if serde_json::to_writer(&mut stdout, &output).is_err() || writeln!(stdout).is_err() {
        tracing::error!("Unable to write the command result to stdout");
        return ExitCode::FAILURE;
    }
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
