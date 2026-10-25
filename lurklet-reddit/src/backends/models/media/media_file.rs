use crate::Error;
use bytes::Bytes;
use futures_util::stream::BoxStream;
use serde::Serialize;
use std::fmt;

/// A fallible file stream. Dropping it cancels consumption; EOF establishes completion.
pub type MediaBody = BoxStream<'static, Result<Bytes, Error>>;

/// A supported file representation, without fetching related resources or buffering its body.
pub struct MediaFile {
    /// The upstream declaration, not a safety guarantee or a format inferred from the URL.
    pub content_type: String,
    pub kind: MediaKind,
    /// Length of the delivered representation when known, not a compressed transfer length.
    pub content_length: Option<u64>,
    /// Reading can fail after the response metadata has been returned.
    pub body: MediaBody,
}

impl fmt::Debug for MediaFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MediaFile")
            .field("content_type", &self.content_type)
            .field("kind", &self.kind)
            .field("content_length", &self.content_length)
            .finish_non_exhaustive()
    }
}

/// A declared representation, not a promise that a consumer can safely render it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Mp4,
}
