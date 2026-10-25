use serde::Serialize;

/// The server's advertised query names; retrieving this does not enable GET requests.
#[derive(Debug, Serialize)]
pub struct RequestAllowlist {
    pub client_name: String,
    pub queries: Vec<String>,
}
