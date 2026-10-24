use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("resource not found")]
    NotFound,
    #[error("method not allowed")]
    MethodNotAllowed,
}

#[derive(Serialize, ToSchema)]
pub struct Problem {
    r#type: &'static str,
    title: &'static str,
    #[schema(minimum = 100, maximum = 599)]
    status: u16,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, title) = match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "Not Found"),
            Self::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed"),
        };

        (
            status,
            [
                (header::CONTENT_TYPE, "application/problem+json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            Json(Problem {
                r#type: "about:blank",
                title,
                status: status.as_u16(),
            }),
        )
            .into_response()
    }
}
