use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;

/// Defaults API responses to no-store and prevents caching of errors.
pub async fn api_cache_policy(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let status = response.status();

    if status.is_client_error() || status.is_server_error() {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    } else {
        response
            .headers_mut()
            .entry(header::CACHE_CONTROL)
            .or_insert(HeaderValue::from_static("no-store"));
    }

    response
}

#[cfg(test)]
mod tests {
    use super::api_cache_policy;
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::middleware;
    use axum::routing::get;
    use tower::ServiceExt;

    #[tokio::test]
    async fn when_resource_sets_cache_policy_then_middleware_should_preserve_it() {
        let app = Router::new()
            .route(
                "/resource",
                get(|| async {
                    (
                        [(header::CACHE_CONTROL, "public, max-age=60, s-maxage=300")],
                        "resource",
                    )
                }),
            )
            .layer(middleware::from_fn(api_cache_policy));
        let request = Request::builder()
            .uri("/resource")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=60, s-maxage=300"
        );
    }

    #[tokio::test]
    async fn when_resource_has_no_cache_policy_then_middleware_should_disable_caching() {
        let app = Router::new()
            .route("/resource", get(|| async { "resource" }))
            .layer(middleware::from_fn(api_cache_policy));
        let request = Request::builder()
            .uri("/resource")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn when_error_has_public_cache_policy_then_middleware_should_disable_caching() {
        for status in [StatusCode::NOT_FOUND, StatusCode::INTERNAL_SERVER_ERROR] {
            let app = Router::new()
                .route(
                    "/resource",
                    get(move || async move {
                        (status, [(header::CACHE_CONTROL, "public, max-age=60")])
                    }),
                )
                .layer(middleware::from_fn(api_cache_policy));
            let request = Request::builder()
                .uri("/resource")
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();

            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
    }
}
