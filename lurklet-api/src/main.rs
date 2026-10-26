use crate::session_store::SessionStore;
use anyhow::Context;
use clap::{Parser, Subcommand};
use lurklet_api::{
    ApiCacheOptions, RateLimit, RateLimitOptions, ResponseCache, StaticHost, application,
    specification,
};
use lurklet_reddit::{RedditClient, Storage};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

mod health_check;
mod server;
mod session_store;

#[derive(Parser)]
#[command(version, about)]
struct Arguments {
    /// Address on which to serve HTTP.
    #[arg(long, env = "LURKLET_BIND", default_value = "127.0.0.1:3000")]
    bind: SocketAddr,
    /// Directory containing the built website.
    #[arg(long, env = "LURKLET_STATIC_DIR", default_value = "lurklet-api/static")]
    static_dir: PathBuf,
    /// Save a persistent API response cache (cache-on-error).
    #[arg(long, env = "LURKLET_CACHE_DIR")]
    cache_dir: Option<PathBuf>,
    #[command(flatten)]
    api_cache: ApiCacheOptions,
    /// Restore and save Reddit session state across restarts.
    #[arg(long, env = "LURKLET_STATE_FILE")]
    state_file: Option<PathBuf>,
    #[arg(
        long,
        env = "LURKLET_PROXY",
        hide_env_values = true,
        value_name = "URL",
        help = "Proxy for Reddit requests (http, https, socks5, or socks5h)"
    )]
    proxy: Option<String>,
    /// Total upstream request timeout in seconds. -1 means no limit.
    #[arg(long, env = "LURKLET_REDDIT_REQUEST_TIMEOUT_SECONDS", default_value = "-1", allow_negative_numbers = true,
        value_parser = |value: &str| -> Result<i64, &'static str> {
            match value.parse::<i64>() {
                Ok(value) if value == -1 || value > 0 => Ok(value),
                _ => Err("must be -1 or a positive number of seconds"),
            }
        })]
    reddit_request_timeout_seconds: i64,
    #[command(flatten)]
    rate_limit: RateLimitOptions,
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
                let specification = specification();
                let mut output = io::stdout().lock();
                serde_json::to_writer_pretty(&mut output, &specification)
                    .context("failed to serialize the OpenAPI document")?;
                writeln!(output).context("failed to write the OpenAPI document")?;
                Ok(())
            }
            Some(Command::HealthCheck) => health_check::check(arguments.bind).await,
            None => {
                let storage = Storage::new(SessionStore::new(arguments.state_file).await);
                let reddit = RedditClient::builder()
                    .with_storage(storage)
                    .with_request_timeout(
                        u64::try_from(arguments.reddit_request_timeout_seconds)
                            .ok()
                            .map(Duration::from_secs),
                    );
                let reddit = match arguments.proxy {
                    Some(proxy) => reddit.with_proxy(proxy),
                    None => reddit,
                }
                .build()
                .context("failed to initialize Reddit client")?;
                let rate_limit = RateLimit::new(arguments.rate_limit);
                let cache = match arguments.cache_dir {
                    Some(directory) => Some(
                        ResponseCache::new(directory)
                            .await
                            .context("failed to initialize response cache")?,
                    ),
                    None => None,
                };
                let app = application(reddit, &rate_limit, cache.as_ref(), &arguments.api_cache);
                let static_host = StaticHost::new(&arguments.static_dir)
                    .await
                    .context("failed to initialize static hosting")?;
                let app = app.fallback_service(static_host.router());
                let listener = TcpListener::bind(arguments.bind)
                    .await
                    .context("failed to bind the API listener")?;

                server::serve(listener, app, rate_limit).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use lurklet_api::ClientIpSource;

    #[test]
    fn when_defaults_are_used_then_cli_and_library_quotas_should_match() {
        let arguments = Arguments::try_parse_from(["lurklet-api"]).unwrap();
        let defaults = RateLimitOptions::default();

        assert_eq!(arguments.rate_limit.rate_limit_rps, defaults.rate_limit_rps);
        assert_eq!(
            arguments.rate_limit.rate_limit_burst,
            defaults.rate_limit_burst
        );
        assert!(matches!(
            arguments.rate_limit.client_ip_source,
            ClientIpSource::Peer
        ));
        assert!(arguments.rate_limit.trusted_proxies.is_empty());
        assert!(arguments.cache_dir.is_none());
        assert_eq!(arguments.api_cache.api_cache_max_age_seconds, 3600);
        assert!(arguments.state_file.is_none());
        assert!(arguments.proxy.is_none());
        assert_eq!(arguments.reddit_request_timeout_seconds, -1);
    }

    #[test]
    fn when_api_freshness_is_selected_then_zero_and_positive_values_should_parse() {
        for seconds in [0, 1, 42, 3600] {
            let arguments = Arguments::try_parse_from([
                "lurklet-api",
                "--api-cache-max-age-seconds",
                &seconds.to_string(),
            ])
            .unwrap();

            assert_eq!(arguments.api_cache.api_cache_max_age_seconds, seconds);
        }
    }

    #[test]
    fn when_api_freshness_is_invalid_then_argument_parsing_should_reject_it() {
        for seconds in ["-1", "1.5", "invalid", "18446744073709551616"] {
            let result =
                Arguments::try_parse_from(["lurklet-api", "--api-cache-max-age-seconds", seconds]);

            assert!(result.is_err(), "{seconds}");
        }
    }

    #[test]
    fn when_a_state_file_is_supplied_then_argument_parsing_should_retain_its_path() {
        let arguments =
            Arguments::try_parse_from(["lurklet-api", "--state-file", "session.json"]).unwrap();

        assert_eq!(arguments.state_file, Some(PathBuf::from("session.json")));
    }

    #[test]
    fn when_a_request_timeout_is_selected_then_disabled_and_positive_values_should_parse() {
        for value in [-1, 1, 60] {
            let arguments = Arguments::try_parse_from([
                "lurklet-api",
                "--reddit-request-timeout-seconds",
                &value.to_string(),
            ])
            .unwrap();

            assert_eq!(arguments.reddit_request_timeout_seconds, value);
        }
    }
}
