use crate::TransportError;
use wreq::header::HeaderMap;

/// HTTP metadata and bounded body bytes, including incomplete-read information.
pub struct HttpResponse {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
    pub body_truncated: bool,
    pub body_error: Option<TransportError>,
}
