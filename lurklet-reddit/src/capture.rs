use crate::TransportErrorKind;
use std::fmt;
use std::time::Duration;

/// An opt-in diagnostic response, which can contain credentials and personal data.
pub struct ResponseCapture {
    pub operation: &'static str,
    pub url: String,
    pub status: u16,
    pub http_version: String,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
    pub body_truncated: bool,
    pub body_error: Option<TransportErrorKind>,
    pub elapsed: Duration,
}

impl fmt::Debug for ResponseCapture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseCapture")
            .field("operation", &self.operation)
            .field("status", &self.status)
            .field("body_bytes", &self.body.len())
            .field("body_truncated", &self.body_truncated)
            .field("body_error", &self.body_error)
            .finish_non_exhaustive()
    }
}

/// Receives bounded, decoded HTTP responses. Implementations should enqueue, not block.
pub trait ResponseObserver: Send + Sync {
    /// Receives sensitive response data only when explicitly configured on the builder.
    fn on_response(&self, response: ResponseCapture);
}
