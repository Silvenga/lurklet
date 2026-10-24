use clap::Args;
use lurklet_reddit::RedditClientBuilder;
use serde_json::Value;
use std::time::Duration;
use wreq_util::Profile;

#[derive(Args, Default)]
pub struct TransportOptions {
    /// Opt into a wreq profile, e.g. okhttp_4.12. No preset is used by default.
    #[arg(long, global = true, value_parser = |value: &str| serde_json::from_value::<Profile>(Value::String(value.into())).map_err(|_| "unknown emulation profile"))]
    emulation_profile: Option<Profile>,
    #[arg(long, global = true)]
    reddit_compression: Option<String>,
    #[arg(long, global = true)]
    download_rate_mbps: Option<f64>,
    /// Limit decompressed response and capture size in bytes.
    #[arg(long, global = true)]
    max_response_bytes: Option<usize>,
    #[arg(long, global = true)]
    request_timeout_ms: Option<u64>,
    #[arg(long, global = true)]
    connect_timeout_ms: Option<u64>,
}

impl TransportOptions {
    pub fn apply(self, mut builder: RedditClientBuilder) -> RedditClientBuilder {
        if let Some(value) = self.emulation_profile {
            builder = builder.with_emulation_profile(value);
        }
        if let Some(value) = self.reddit_compression {
            builder = builder.with_reddit_compression(value);
        }
        if let Some(value) = self.download_rate_mbps {
            builder = builder.with_download_rate_mbps(Some(value));
        }
        if let Some(value) = self.max_response_bytes {
            builder = builder.with_max_response_bytes(value);
        }
        if let Some(value) = self.request_timeout_ms {
            builder = builder.with_request_timeout(Duration::from_millis(value));
        }
        if let Some(value) = self.connect_timeout_ms {
            builder = builder.with_connect_timeout(Duration::from_millis(value));
        }
        builder
    }
}
