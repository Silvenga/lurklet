use axum::http::{HeaderValue, header};
use tower_http::set_header::SetResponseHeaderLayer;

/// Prevents browsers from MIME-sniffing responses.
pub fn content_type_options() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    )
}
