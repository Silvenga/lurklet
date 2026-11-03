use axum::http::{HeaderValue, header};
use tower_http::set_header::SetResponseHeaderLayer;

/// Restricts resource loading to the document's origin while allowing inline style elements.
pub fn content_security_policy() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; style-src-elem 'self' 'unsafe-inline'; style-src-attr 'none'",
        ),
    )
}

/// Isolates API documents while preserving inline SVG styles and direct media playback.
pub fn api_content_security_policy() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; sandbox; style-src 'unsafe-inline'; media-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    )
}
