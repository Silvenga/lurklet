use crate::ResponseCache;
use crate::error::{ApiError, Problem};
use crate::middleware::{RateLimit, access_log, api_cache_policy, limit_requests};
use crate::routes;
use axum::Router;
use axum::middleware;
use utoipa::OpenApi as _;
use utoipa::openapi::{OpenApi, OpenApiVersion};

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
pub fn application(rate_limit: &RateLimit, cache: Option<&ResponseCache>) -> (Router, OpenApi) {
    let (health, health_specification) = routes::health().split_for_parts();
    let health = health.method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed });

    let mut data_api = Router::new()
        .fallback(|| async { ApiError::NotFound })
        .method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed });
    if let Some(cache) = cache {
        data_api = cache.wrap(data_api);
    }
    let data_api = data_api.layer(middleware::from_fn_with_state(
        rate_limit.clone(),
        limit_requests,
    ));

    let api = data_api
        .merge(health)
        .layer(middleware::from_fn(api_cache_policy))
        .layer(middleware::from_fn(access_log));

    // Service nesting reserves /api, /api/, and descendants, including unmatched paths.
    let prefix = "/api";
    let app = Router::new().nest_service(prefix, api);
    let mut specification = ApiDoc::openapi().nest(prefix, health_specification);
    specification.openapi = OpenApiVersion::Version32;

    (app, specification)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RateLimitOptions, StaticHost};
    use assert_fs::prelude::*;
    use axum::Extension;
    use axum::body::{Body, to_bytes};
    use axum::extract::ConnectInfo;
    use axum::http::{Method, Request, StatusCode, header};
    use axum::response::Html;
    use axum::routing::get;
    use serde_json::{Value, json};
    use std::net::SocketAddr;
    use tower::ServiceExt;

    fn application() -> (Router, OpenApi) {
        let (app, specification) = super::application(&RateLimit::default(), None);
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
        let (app, _) = super::application(&limiter, None);
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

    #[tokio::test]
    async fn when_cache_is_enabled_then_api_hits_should_be_limited_and_health_should_bypass_caching()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let fixture = cache.wrap(Router::new().route(
            "/api/resource",
            get(|| async { ([(header::CACHE_CONTROL, "public, max-age=3600")], "cached") }),
        ));
        let response = fixture
            .oneshot(
                Request::builder()
                    .uri("/api/resource")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()["x-lurklet-cache"], "miss");
        let limiter = RateLimit::new(RateLimitOptions {
            rate_limit_burst: 1.try_into().unwrap(),
            ..RateLimitOptions::default()
        });
        let (app, _) = super::application(&limiter, Some(&cache));
        for _ in 0..12 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/health")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert!(!response.headers().contains_key("x-lurklet-cache"));
            assert!(!response.headers().contains_key("x-ratelimit-limit"));
        }
        let app = app.layer(Extension(ConnectInfo(
            "192.0.2.1:1234".parse::<SocketAddr>().unwrap(),
        )));

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/resource")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let second = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/resource")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let health = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()["x-lurklet-cache"], "hit");
        assert!(!first.headers().contains_key("x-ratelimit-remaining"));
        assert_eq!(
            to_bytes(first.into_body(), 4096).await.unwrap(),
            b"cached".as_slice()
        );
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(!second.headers().contains_key("x-lurklet-cache"));
        assert_eq!(health.status(), StatusCode::OK);
        assert!(!health.headers().contains_key("x-lurklet-cache"));
        assert_eq!(health.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            serde_json::from_slice::<Value>(&to_bytes(health.into_body(), 4096).await.unwrap())
                .unwrap(),
            json!({"health": "ok"})
        );
    }

    #[tokio::test]
    async fn when_cache_is_enabled_then_static_robots_and_unsupported_methods_should_remain_outside_it()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        directory.child("index.html").write_str("site").unwrap();
        let cache_directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(cache_directory.path()).await.unwrap();
        let (app, _) = super::application(&RateLimit::default(), Some(&cache));
        let app = app
            .fallback_service(StaticHost::new(directory.path()).await.unwrap().router())
            .layer(Extension(ConnectInfo(
                "192.0.2.1:1234".parse::<SocketAddr>().unwrap(),
            )));

        for (method, path, expected) in [
            (Method::GET, "/robots.txt", StatusCode::OK),
            (Method::GET, "/", StatusCode::OK),
            (Method::GET, "/missing.js", StatusCode::NOT_FOUND),
            (Method::POST, "/api/health", StatusCode::METHOD_NOT_ALLOWED),
            (Method::GET, "/api/health", StatusCode::OK),
            (Method::HEAD, "/api/health", StatusCode::OK),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), expected, "{path}");
            assert!(
                !response.headers().contains_key("x-lurklet-cache"),
                "{path}"
            );
        }
    }
}
