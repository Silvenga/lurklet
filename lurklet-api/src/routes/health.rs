use crate::error::Problem;
use axum::Json;
use axum::http::header;
use axum::response::IntoResponse;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct Health {
    health: HealthStatus,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
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
        (status = 200, description = "The API is serving requests", body = Health),
        (status = 405, description = "The method is not supported", body = Problem,
            content_type = "application/problem+json",
            headers(("Allow" = String, description = "GET,HEAD"))),
        (status = 406, description = "No acceptable response encoding", body = Problem,
            content_type = "application/problem+json")
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
