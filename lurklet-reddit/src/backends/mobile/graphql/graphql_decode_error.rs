use serde::Serialize;
use thiserror::Error;

/// A decoding failure without embedding upstream data in its message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Error)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum GraphQlDecodeError {
    #[error("response is not valid JSON")]
    InvalidJson,
    #[error("response is not a recognizable GraphQL envelope")]
    InvalidEnvelope,
    #[error("incremental multipart responses are not supported")]
    UnsupportedFormat,
    #[error("operation result fields could not be decoded")]
    InvalidData,
    #[error("response contains no supported result data")]
    UnsupportedData,
}
