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

    #[tokio::test]
    async fn when_an_api_response_has_existing_policies_then_the_layer_should_replace_them_with_isolation()
     {
        let app = Router::new()
            .route(
                "/resource",
                get(|| async {
                    (
                        [
                            (header::CONTENT_SECURITY_POLICY, "default-src *"),
                            (
                                header::CONTENT_SECURITY_POLICY,
                                "script-src 'unsafe-inline'",
                            ),
                        ],
                        "resource",
                    )
                }),
            )
            .layer(api_content_security_policy());
        let request = Request::builder()
            .uri("/resource")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(
            response
                .headers()
                .get_all(header::CONTENT_SECURITY_POLICY)
                .iter()
                .map(|value| value.to_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "default-src 'none'; sandbox; style-src 'unsafe-inline'; media-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
            ]
        );
    }
}
