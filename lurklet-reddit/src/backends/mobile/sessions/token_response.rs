use serde::Deserialize;

/// The token endpoint's response body, separate from acquisition and storage policy.
#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
    pub expiry_ts: Option<u64>,
}
