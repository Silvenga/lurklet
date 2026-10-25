use crate::error::Problem;
use axum::Json;
use axum::http::header;
use axum::response::IntoResponse;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Serialize, ToSchema)]
struct Health {
    health: HealthStatus,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
enum HealthStatus {
    Ok,
}

/// Builds health routes and their OpenAPI documentation.
pub fn router() -> OpenApiRouter {
    OpenApiRouter::new().routes(routes!(health))
}

#[utoipa::path(
    get,
    path = "/health",
    operation_id = "get_health",
    summary = "Check API liveness without contacting upstream services",
    responses(
        (status = 200, description = "The API is serving requests", body = Health,
            headers(("Cache-Control" = String, description = "Always no-store"))),
        (status = 405, description = "The method is not supported", body = Problem,
            content_type = "application/problem+json",
            headers(
                ("Allow" = String, description = "GET,HEAD"),
                ("Cache-Control" = String, description = "Always no-store")
            ))
    )
)]
async fn health() -> impl IntoResponse {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(Health {
            health: HealthStatus::Ok,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn when_health_is_requested_then_response_should_report_ok() {
        let app: Router = router().into();
        let request = Request::builder()
            .uri("/health")
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
    async fn when_health_is_requested_with_head_then_response_should_have_no_body() {
        let app: Router = router().into();
        let request = Request::builder()
            .method(Method::HEAD)
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 4096).await.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert!(body.is_empty());
    }
}
