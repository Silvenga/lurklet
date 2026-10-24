use crate::{AppProfile, DeviceProfile};

pub(crate) struct Config {
    pub auth_endpoint: String,
    pub graphql_endpoint: String,
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
