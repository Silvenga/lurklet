use thiserror::Error;

/// Failure to initialize the Reddit client's HTTP transport.
#[derive(Debug, Error)]
#[error("failed to initialize the Reddit client's HTTP transport")]
pub struct BuildError {
    #[source]
    pub(crate) source: wreq::Error,
}
