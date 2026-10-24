use anyhow::Context;
use clap::{Parser, Subcommand};
use lurklet_api::application;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::process::ExitCode;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

mod health_check;
mod server;

#[derive(Parser)]
#[command(version, about)]
struct Arguments {
    /// Address on which to serve HTTP.
    #[arg(long, env = "LURKLET_BIND", default_value = "127.0.0.1:3000")]
    bind: SocketAddr,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Write the internal OpenAPI document to stdout without starting the server.
    #[command(name = "export-openapi")]
    ExportOpenApi,
    /// Check the local API's HTTP status without reading its response body.
    HealthCheck,
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = Arguments::parse();
    let is_health_check = matches!(arguments.command, Some(Command::HealthCheck));
    if !is_health_check {
        let filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env();
        let Ok(filter) = filter else {
            let _ = writeln!(io::stderr().lock(), "Invalid RUST_LOG configuration");
            return ExitCode::FAILURE;
        };
        if tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(io::stderr)
            .with_ansi(false)
            .try_init()
            .is_err()
        {
            let _ = writeln!(io::stderr().lock(), "Unable to initialize logging");
            return ExitCode::FAILURE;
        }
    }

    let result: anyhow::Result<()> = async {
        match arguments.command {
            Some(Command::ExportOpenApi) => {
                let (_, specification) = application();
                let mut output = io::stdout().lock();
                serde_json::to_writer_pretty(&mut output, &specification)
                    .context("failed to serialize the OpenAPI document")?;
                writeln!(output).context("failed to write the OpenAPI document")?;
                Ok(())
            }
            Some(Command::HealthCheck) => health_check::check(arguments.bind).await,
            None => {
                let (app, _) = application();
                let listener = TcpListener::bind(arguments.bind)
                    .await
                    .context("failed to bind the API listener")?;

                server::serve(listener, app).await
            }
        }
    }
    .await;

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if is_health_check {
                let _ = writeln!(io::stderr().lock(), "Health check failed");
            } else {
                tracing::error!("API execution failed; operator action is required");
                tracing::debug!(error = ?error, "API failure details");
            }
            ExitCode::FAILURE
        }
    }
}
