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
    fn when_builder_overrides_are_supplied_then_the_cli_should_parse_and_apply_them() {
        let flags = "cli bootstrap
            --auth-endpoint https://auth.invalid/token --graphql-endpoint https://content.invalid/
            --client-id custom-client --scope one --scope two
            --app-version 1.2.3 --app-build 42 --android-release 15
            --manufacturer Example --model Model --locale en-GB --display-density -1
            --display-width-px 800 --display-height-px 1200 --user-agent custom-agent
            --emulation-profile okhttp_4.12 --reddit-compression 0 --download-rate-mbps 12.3
            --height-header=false --max-response-bytes 4096
            --request-timeout-ms 1234 --connect-timeout-ms 567 --clock-ms 1000";
        let arguments = Arguments::try_parse_from(flags.split_whitespace()).unwrap();

        let client = arguments.client.builder().build();

        assert!(client.is_ok());
    }

    #[test]
    fn when_an_invalid_override_is_supplied_then_library_validation_should_reject_it() {
        let arguments =
            Arguments::try_parse_from(["cli", "bootstrap", "--request-timeout-ms", "0"]).unwrap();

        let result = arguments.client.builder().build();

        assert!(matches!(
            result,
            Err(BuildError::InvalidConfiguration {
                field: "request_timeout"
            })
        ));
    }
}
