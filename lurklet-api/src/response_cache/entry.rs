use axum::body::Body;
use axum::http::HeaderMap;
use axum::response::Response;
use headers::{Age, ContentLength, HeaderMapExt};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

/// Metadata for one immutable content file, published only after completion.
#[derive(Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub key: String,
    pub stored_at: SystemTime,
    pub fresh_for: Duration,
    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,
    pub content_file: String,
    pub content_length: u64,
}

/// An opened representation that remains readable if another process replaces it.
pub struct Entry {
    pub metadata: Metadata,
    pub body: File,
}

impl Entry {
    /// Checks freshness without extending the persisted lifetime.
    pub fn is_fresh(&self) -> bool {
        self.metadata
            .stored_at
            .elapsed()
            .is_ok_and(|age| age < self.metadata.fresh_for)
    }

    /// Streams the content with its original policy, current age, and completed length.
    pub fn into_response(self) -> Response {
        let age = self.metadata.stored_at.elapsed().unwrap_or_default();
        let mut response = Response::new(Body::from_stream(ReaderStream::new(self.body)));
        *response.headers_mut() = self.metadata.headers;
        response
            .headers_mut()
            .typed_insert(Age::from_secs(age.as_secs()));
        response
            .headers_mut()
            .typed_insert(ContentLength(self.metadata.content_length));
        response
    }
}
