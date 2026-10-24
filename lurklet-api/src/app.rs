use crate::error::{ApiError, Problem};
use crate::middleware::{RateLimit, access_log, api_cache_policy, limit_requests};
use crate::routes;
use axum::Router;
use axum::middleware;
use utoipa::OpenApi as _;
use utoipa::openapi::{OpenApi, OpenApiVersion};
use utoipa_axum::router::OpenApiRouter;

#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Lurklet API",
        description = "Internal API contract. API requests share a per-client rate limit, except for /api/health. Excess requests return 429 Problem Details with Retry-After. Unknown API paths return 404 Problem Details unless rate limited. API responses default to Cache-Control: no-store unless a resource explicitly opts into caching."
    ),
    components(schemas(Problem))
)]
struct ApiDoc;

/// Builds the API router and internal specification without starting a listener.
pub fn application(rate_limit: &RateLimit) -> (Router, OpenApi) {
    let (api, specification) = OpenApiRouter::new()
        .merge(routes::health())
        .split_for_parts();

    let api = api
        .fallback(|| async { ApiError::NotFound })
        .method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed })
        .layer(middleware::from_fn_with_state(
            rate_limit.clone(),
            limit_requests,
        ))
        .layer(middleware::from_fn(api_cache_policy))
        .layer(middleware::from_fn(access_log));

    // Service nesting reserves /api, /api/, and descendants, including unmatched paths.
    let prefix = "/api";
    let app = Router::new().nest_service(prefix, api);
    let mut specification = ApiDoc::openapi().nest(prefix, specification);
    specification.openapi = OpenApiVersion::Version32;

    (app, specification)
}

#[cfg(test)]
mod tests {
    use crate::{RateLimit, RateLimitOptions, StaticHost};
    use assert_fs::prelude::*;
    use axum::body::{Body, to_bytes};
    use axum::extract::ConnectInfo;
    use axum::http::{Method, Request, StatusCode, header};
    use axum::response::Html;
    use axum::{Extension, Router};
    use serde_json::{Value, json};
    use std::net::SocketAddr;
    use tower::ServiceExt;
    use utoipa::openapi::OpenApi;

    fn application() -> (Router, OpenApi) {
        let (app, specification) = super::application(&RateLimit::default());
        let peer: SocketAddr = "192.0.2.1:1234".parse().unwrap();
        (app.layer(Extension(ConnectInfo(peer))), specification)
    }

    #[tokio::test]
    async fn when_health_is_requested_through_the_api_then_response_should_disable_caching() {
        let (app, _) = application();
        let request = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn when_an_ip_is_limited_then_health_robots_and_static_routes_should_remain_exempt() {
        let directory = assert_fs::TempDir::new().unwrap();
        directory.child("index.html").write_str("site").unwrap();
        directory
            .child("assets/app.js")
            .write_str("script")
            .unwrap();
        let limiter = RateLimit::new(RateLimitOptions {
            rate_limit_burst: 1.try_into().unwrap(),
            ..RateLimitOptions::default()
        });
        let (app, _) = super::application(&limiter);
        let peer: SocketAddr = "192.0.2.1:1234".parse().unwrap();
        let app = app
            .fallback_service(StaticHost::new(directory.path()).await.unwrap().router())
            .layer(Extension(ConnectInfo(peer)));
        let request = |path: &str| {
            Request::builder()
                .uri(path)
                .header(header::ACCEPT, "text/html")
                .body(Body::empty())
                .unwrap()
        };
        let _ = app.clone().oneshot(request("/api/missing")).await.unwrap();

        for _ in 0..12 {
            for path in [
                "/api/health",
                "/api/health?probe=1",
                "/robots.txt",
                "/%72obots.txt",
                "/",
                "/assets/app.js",
                "/r/rust",
            ] {
                let response = app.clone().oneshot(request(path)).await.unwrap();

                assert_eq!(response.status(), StatusCode::OK, "{path}");
                assert!(
                    !response.headers().contains_key("x-ratelimit-limit"),
                    "{path}"
                );
            }
        }
        for method in [Method::HEAD, Method::POST] {
            let mut request = request("/api/health");
            *request.method_mut() = method.clone();
            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(
                response.status(),
                if method == Method::HEAD {
                    StatusCode::OK
                } else {
                    StatusCode::METHOD_NOT_ALLOWED
                }
            );
            assert!(!response.headers().contains_key("x-ratelimit-limit"));
        }
        for path in ["/api", "/api/", "/api/health/", "/api/robots.txt"] {
            let response = app.clone().oneshot(request(path)).await.unwrap();

            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS, "{path}");
        }
    }

    #[tokio::test]
    async fn when_health_method_is_unsupported_then_response_should_be_problem_details_with_allow()
    {
        for method in [
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ] {
            let (app, _) = application();
            let request = Request::builder()
                .method(method)
                .uri("/api/health")
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
            assert_eq!(headers[header::CACHE_CONTROL], "no-store");
            assert_eq!(headers[header::ALLOW], "GET,HEAD");
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                json!({"type": "about:blank", "title": "Method Not Allowed", "status": 405})
            );
        }
    }

    #[tokio::test]
    async fn when_api_path_is_unknown_then_response_should_never_use_spa_fallback() {
        for path in [
            "/api",
            "/api/",
            "/api//",
            "/api/missing",
            "/api/health/",
            "/api/openapi.json",
            "/api/missing?secret=private",
        ] {
            let (app, _) = application();
            let app = app.fallback(|| async { Html("SPA") });
            let request = Request::builder().uri(path).body(Body::empty()).unwrap();

            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
            assert_eq!(headers[header::CACHE_CONTROL], "no-store");
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                json!({"type": "about:blank", "title": "Not Found", "status": 404})
            );
        }
    }

    #[tokio::test]
    async fn when_path_is_outside_api_then_response_should_allow_spa_fallback() {
        for path in ["/", "/posts/example", "/apiculture", "/openapi.json"] {
            let (app, _) = application();
            let app = app.fallback(|| async { Html("SPA") });
            let request = Request::builder().uri(path).body(Body::empty()).unwrap();

            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::OK, "{path}");
            assert!(!headers.contains_key(header::CACHE_CONTROL));
            assert_eq!(body.as_ref(), b"SPA");
        }
    }

    #[test]
    fn when_openapi_is_generated_then_contract_should_document_api_health() {
        let (_, specification) = application();

        let document = serde_json::to_value(specification).unwrap();

        insta::assert_json_snapshot!("openapi", document);
    }
}
