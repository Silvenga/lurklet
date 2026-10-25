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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use tower::ServiceExt;

    #[tokio::test]
    async fn when_the_layer_is_applied_then_responses_should_restrict_resources_to_the_same_origin()
    {
        let app = Router::new()
            .route("/resource", get(|| async { "resource" }))
            .layer(content_security_policy());
        let request = Request::builder()
            .uri("/resource")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(
            response.headers()[header::CONTENT_SECURITY_POLICY],
            "default-src 'self'; style-src-elem 'self' 'unsafe-inline'; style-src-attr 'none'"
        );
    }
}
