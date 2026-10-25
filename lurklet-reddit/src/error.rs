use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;

/// A failure to configure or initialize the client.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BuildError {
    #[error("invalid client configuration: {field}")]
    InvalidConfiguration { field: &'static str },
    #[error("invalid HTTPS endpoint: {field}")]
    InvalidEndpoint { field: &'static str },
    #[error(transparent)]
    Transport(#[from] TransportError),
}

/// A failure while performing a Reddit operation.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Clock(#[from] ClockError),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("Reddit rejected authentication (HTTP {status})")]
    Authentication { status: u16 },
    #[error("Reddit denied access (HTTP {status}); this does not imply an expired token")]
    AccessDenied { status: u16 },
    #[error("Reddit rate limited the request")]
    RateLimited { retry_after: Option<RetryAfter> },
    #[error("Reddit is unavailable (HTTP {status})")]
    Upstream {
        status: u16,
        retry_after: Option<RetryAfter>,
    },
    #[error("unexpected upstream HTTP status: {status}")]
    UnexpectedStatus { status: u16 },
    #[error("invalid upstream response: {0}")]
    InvalidResponse(#[from] ResponseError),
    #[error("invalid stored state: {field}")]
    InvalidState { field: &'static str },
    #[error("unable to construct request field: {field}")]
    InvalidRequest { field: &'static str },
    #[error("the session changed while authentication was in flight")]
    SessionChanged,
}

/// A validated Retry-After value, without assuming when the caller will retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryAfter {
    Delay(Duration),
    At(SystemTime),
}

/// A response validation failure without response contents or credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ResponseError {
    #[error("response body exceeds the configured limit")]
    BodyTooLarge,
    #[error("response is not valid JSON")]
    InvalidJson,
    #[error("response does not match the token schema")]
    InvalidTokenResponse,
    #[error("access token is empty or cannot be used in an HTTP header")]
    InvalidAccessToken,
    #[error("token lifetime is zero or cannot be represented")]
    InvalidExpiry,
    #[error("token expired before acquisition completed")]
    ExpiredOnArrival,
    #[error("response contains an invalid LOID header")]
    InvalidLoid,
}

/// Categories a storage implementation can report to its consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum StorageError {
    #[error("session storage is unavailable")]
    Unavailable,
    #[error("session storage access was denied")]
    PermissionDenied,
    #[error("session storage contains invalid data")]
    InvalidData,
    #[error("session storage revision is exhausted")]
    RevisionExhausted,
}

/// Failure to represent the current time as Unix milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ClockError {
    #[error("clock is before the Unix epoch")]
    BeforeEpoch,
    #[error("clock value exceeds the supported range")]
    Overflow,
}

/// Classification of HTTP transport failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransportErrorKind {
    Timeout,
    Connect,
    Body,
    Request,
    Initialization,
    Other,
}

/// A transport error whose normal formatting does not expose URLs or credentials.
#[derive(Clone, Error)]
#[error("HTTP transport failed ({kind:?})")]
pub struct TransportError {
    pub kind: TransportErrorKind,
    #[source]
    source: Arc<wreq::Error>,
}

impl fmt::Debug for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransportError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl From<wreq::Error> for TransportError {
    fn from(source: wreq::Error) -> Self {
        let kind = if source.is_timeout() {
            TransportErrorKind::Timeout
        } else if source.is_connect() {
            TransportErrorKind::Connect
        } else if source.is_body() || source.is_decode() {
            TransportErrorKind::Body
        } else if source.is_builder() {
            TransportErrorKind::Initialization
        } else if source.is_request() {
            TransportErrorKind::Request
        } else {
            TransportErrorKind::Other
        };
        Self {
            kind,
            source: Arc::new(source),
        }
    }
}
