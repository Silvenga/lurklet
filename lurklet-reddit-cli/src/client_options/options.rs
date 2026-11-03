use super::api::ApiOptions;
use super::app::AppOptions;
use super::device::DeviceOptions;
use super::transport::TransportOptions;
use clap::Args;
use lurklet_reddit::{Clock, ClockError, RedditClient, RedditClientBuilder};

#[derive(Args, Default)]
pub struct ClientOptions {
    #[command(flatten)]
    api: ApiOptions,
    #[command(flatten)]
    app: AppOptions,
    #[command(flatten)]
    device: DeviceOptions,
    #[command(flatten)]
    transport: TransportOptions,
    /// Freeze the operational clock at a Unix millisecond timestamp for debugging.
    #[arg(long, global = true)]
    clock_ms: Option<u64>,
}

struct FixedClock(u64);

impl Clock for FixedClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(self.0)
    }
}

impl ClientOptions {
    pub fn builder(self) -> RedditClientBuilder {
        let builder = self.api.apply(RedditClient::builder());
        let builder = self.app.apply(builder);
        let builder = self.device.apply(builder);
        let mut builder = self.transport.apply(builder);
        if let Some(value) = self.clock_ms {
            builder = builder.with_clock(FixedClock(value));
        }
        builder
    }
}

#[cfg(test)]
mod tests {
    use crate::arguments::Arguments;
    use clap::Parser;
    use lurklet_reddit::BuildError;

    #[test]
    fn when_an_override_is_invalid_then_its_library_error_should_identify_the_forwarded_field() {
        for (option, value, expected) in [
            (
                "--auth-endpoint",
                "http://auth.invalid/token",
                "auth_endpoint",
            ),
            (
                "--graphql-endpoint",
                "http://content.invalid/",
                "graphql_endpoint",
            ),
            ("--client-id", "client:password", "client_id"),
            ("--scope", "", "scopes"),
            ("--app-version", "bad\nversion", "user_agent"),
            ("--app-build", "bad\nbuild", "user_agent"),
            ("--android-release", "bad\nrelease", "user_agent"),
            ("--user-agent", "bad\nagent", "user_agent"),
            ("--manufacturer", "bad\nmanufacturer", "manufacturer"),
            ("--model", "bad\nmodel", "model"),
            ("--locale", "", "locale"),
            ("--display-density", "NaN", "display_density"),
            ("--reddit-compression", "bad\nheader", "reddit_compression"),
            ("--download-rate-mbps", "NaN", "download_rate_mbps"),
            ("--graphql-accept", "bad\naccept", "graphql_accept"),
            (
                "--graphql-retry-header",
                "bad\nretry",
                "graphql_retry_header",
            ),
            ("--max-response-bytes", "0", "max_response_bytes"),
            ("--request-timeout-ms", "0", "request_timeout"),
            ("--connect-timeout-ms", "0", "connect_timeout"),
        ] {
            let arguments = Arguments::try_parse_from(["cli", "bootstrap", option, value]).unwrap();

            let result = arguments.client.builder().build();

            match result {
                Err(
                    BuildError::InvalidConfiguration { field }
                    | BuildError::InvalidEndpoint { field },
                ) => assert_eq!(field, expected, "{option}"),
                _ => panic!("{option} was not forwarded to library validation"),
            }
        }
    }
}
