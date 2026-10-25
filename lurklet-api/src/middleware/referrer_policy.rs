use axum::http::{HeaderValue, header};
use tower_http::set_header::SetResponseHeaderLayer;

/// Limits browser referrer disclosure to same-origin requests.
pub fn referrer_policy() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
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
    async fn when_the_layer_is_applied_then_responses_should_limit_referrers_to_the_same_origin() {
        let app = Router::new()
            .route("/resource", get(|| async { "resource" }))
            .layer(referrer_policy());
        let request = Request::builder()
            .uri("/resource")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.headers()[header::REFERRER_POLICY], "same-origin");
    }
}
