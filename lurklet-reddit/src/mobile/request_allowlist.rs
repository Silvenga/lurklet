use serde::{Deserialize, Serialize};

/// The server's advertised query names; retrieving this does not enable GET requests.
#[derive(Debug, Serialize)]
pub struct RequestAllowlist {
    pub client_name: String,
    pub queries: Vec<String>,
}

#[derive(Deserialize)]
pub struct AllowlistData {
    #[serde(
        rename = "zeroRttClientAllowedQueries",
        deserialize_with = "Option::deserialize"
    )]
    allowlist: Option<AllowlistFields>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AllowlistFields {
    client_name: String,
    queries: Vec<Query>,
}

#[derive(Deserialize)]
struct Query {
    name: String,
}

impl From<AllowlistData> for Option<RequestAllowlist> {
    fn from(data: AllowlistData) -> Self {
        data.allowlist.map(|allowlist| RequestAllowlist {
            client_name: allowlist.client_name,
            queries: allowlist
                .queries
                .into_iter()
                .map(|query| query.name)
                .collect(),
        })
    }
}
