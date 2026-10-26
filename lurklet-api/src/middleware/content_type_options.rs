use axum::http::{HeaderValue, header};
use tower_http::set_header::SetResponseHeaderLayer;

/// Prevents browsers from MIME-sniffing responses.
pub fn content_type_options() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
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
    async fn when_a_response_has_content_type_options_then_the_layer_should_override_them_with_nosniff()
     {
        let app = Router::new()
            .route(
                "/resource",
                get(|| async { ([(header::X_CONTENT_TYPE_OPTIONS, "invalid")], "resource") }),
            )
            .layer(content_type_options());
        let request = Request::builder()
            .uri("/resource")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(
            response
                .headers()
                .get_all(header::X_CONTENT_TYPE_OPTIONS)
                .iter()
                .map(|value| value.to_str().unwrap())
                .collect::<Vec<_>>(),
            ["nosniff"]
        );
    }
}
