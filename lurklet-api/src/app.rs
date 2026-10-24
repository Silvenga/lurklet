use crate::error::{ApiError, Problem};
use crate::routes;
use axum::Router;
use axum::extract::{OriginalUri, Request};
use axum::http::{HeaderValue, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use std::time::Duration;
use tower_http::trace::TraceLayer;
use tracing::Span;
use utoipa::OpenApi as _;
use utoipa::openapi::{OpenApi, OpenApiVersion};
use utoipa_axum::router::OpenApiRouter;

#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Lurklet API",
        description = "Internal API contract. Unknown API paths return 404 Problem Details. API responses default to Cache-Control: no-store unless a resource explicitly opts into caching."
    ),
    components(schemas(Problem))
)]
struct ApiDoc;

/// Builds the API router and internal specification without starting a listener.
pub fn application() -> (Router, OpenApi) {
    let (api, specification) = OpenApiRouter::new()
        .merge(routes::health())
        .split_for_parts();

    let api = api
        .fallback(|| async { ApiError::NotFound })
        .method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed })
        .layer(middleware::from_fn(cache_policy))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request| {
                    let uri = request
                        .extensions()
                        .get::<OriginalUri>()
                        .map_or_else(|| request.uri(), |original| &original.0);
                    let path = uri.path_and_query().map_or("/", |path| path.as_str());

                    tracing::debug_span!("http.request", method = %request.method(), path)
                })
                .on_request(|request: &Request, _: &Span| {
                    tracing::trace!(headers = ?request.headers(), version = ?request.version(), "HTTP request metadata");
                })
                .on_response(|response: &Response, duration: Duration, _: &Span| {
                    tracing::debug!(
                        status = response.status().as_u16(),
                        duration_ms = duration.as_secs_f64() * 1000.0,
                        "HTTP request completed"
                    );
                    tracing::trace!(headers = ?response.headers(), version = ?response.version(), "HTTP response metadata");
                })
                .on_failure(()),
        );

    // Service nesting reserves /api, /api/, and descendants, including unmatched paths.
    let prefix = "/api";
    let app = Router::new().nest_service(prefix, api);
    let mut specification = ApiDoc::openapi().nest(prefix, specification);
    specification.openapi = OpenApiVersion::Version32;

    (app, specification)
}

async fn cache_policy(request: Request, next: Next) -> Response {
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
    use super::{application, cache_policy};
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode, header};
    use axum::middleware;
    use axum::response::Html;
    use axum::routing::get;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn when_health_is_requested_then_response_should_report_ok_without_caching() {
        let (app, _) = application();
        let request = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 4096).await.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({"health": "ok"})
        );
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
            .layer(middleware::from_fn(cache_policy));
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
            .layer(middleware::from_fn(cache_policy));
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
                .layer(middleware::from_fn(cache_policy));
            let request = Request::builder()
                .uri("/resource")
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();

            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
    }

    #[test]
    fn when_openapi_is_generated_then_contract_should_document_api_health() {
        let (_, specification) = application();

        let document = serde_json::to_value(specification).unwrap();

        insta::assert_json_snapshot!("openapi", document);
    }
}
