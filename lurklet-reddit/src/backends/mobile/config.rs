use crate::backends::mobile::graphql::PersistedQuery;
use crate::{AppProfile, DeviceProfile};

pub(crate) struct Config {
    pub auth_endpoint: String,
    pub graphql_endpoint: String,
    pub graphql_accept: String,
    pub graphql_retry_header: String,
    pub request_allowlist_query: PersistedQuery,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub app: AppProfile,
    pub device: DeviceProfile,
    pub user_agent: Option<String>,
    pub reddit_compression: String,
    pub download_rate_mbps: Option<f64>,
    pub include_height: bool,
    pub max_response_bytes: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auth_endpoint: "https://www.reddit.com/auth/v2/oauth/access-token/loid".into(),
            graphql_endpoint: "https://gql-fed.reddit.com/".into(),
            graphql_accept: "multipart/mixed;deferSpec=20220824, application/graphql-response+json, application/json".into(),
            graphql_retry_header: "attempt=0, max=3".into(),
            request_allowlist_query: PersistedQuery {
                name: "GetRequestAllowlist".into(),
                sha256_hash: "421cc25fa66813463ea7169d6e1b93d6a3cd46d929a9e4eda7403de4c491e487".into(),
            },
            client_id: "ohXpoqrZYub1kg".into(),
            scopes: ["*", "email", "pii", "adsread", "adsedit"]
                .map(String::from)
                .into(),
            app: AppProfile::default(),
            device: DeviceProfile::default(),
            user_agent: None,
            reddit_compression: "1".into(),
            download_rate_mbps: None,
            include_height: false,
            max_response_bytes: 1024 * 1024,
        }
    }
}
