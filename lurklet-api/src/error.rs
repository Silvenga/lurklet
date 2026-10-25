use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use headers::{HeaderMapExt, RetryAfter};
use lurklet_reddit::{Error as RedditError, RetryAfter as RedditRetryAfter};
use serde::Serialize;
use std::collections::BTreeMap;
use thiserror::Error;
use utoipa::openapi::header::HeaderBuilder;
use utoipa::openapi::{Content, Ref, RefOr, Response as OpenApiResponse, ResponseBuilder};
use utoipa::{IntoResponses, ToSchema};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("bad request")]
    BadRequest,
    #[error("resource not found")]
    NotFound,
    #[error("method not allowed")]
    MethodNotAllowed,
    #[error("too many requests")]
    TooManyRequests,
    #[error("internal server error")]
    InternalServerError,
    #[error("bad gateway")]
    BadGateway,
    #[error("service unavailable")]
    ServiceUnavailable { retry_after: Option<RetryAfter> },
}

impl From<RedditError> for ApiError {
    fn from(error: RedditError) -> Self {
        tracing::debug!(?error, "Reddit operation failed");
        match error {
            RedditError::Unavailable | RedditError::NotFound => Self::NotFound,
            RedditError::RateLimited { retry_after }
            | RedditError::Upstream { retry_after, .. } => Self::ServiceUnavailable {
                retry_after: retry_after.map(|retry| match retry {
                    RedditRetryAfter::Delay(delay) => RetryAfter::delay(delay),
                    RedditRetryAfter::At(time) => RetryAfter::date(time),
                }),
            },
            RedditError::Storage(_)
            | RedditError::Clock(_)
            | RedditError::InvalidState { .. }
            | RedditError::InvalidRequest { .. }
            | RedditError::SessionChanged => Self::InternalServerError,
            _ => Self::BadGateway,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    r#type: &'static str,
    title: &'static str,
    #[schema(minimum = 100, maximum = 599)]
    status: u16,
}

impl IntoResponses for ApiError {
    fn responses() -> BTreeMap<String, RefOr<OpenApiResponse>> {
        [
            StatusCode::BAD_REQUEST,
            StatusCode::NOT_FOUND,
            StatusCode::METHOD_NOT_ALLOWED,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
        ]
        .into_iter()
        .map(|status| {
            let mut response = ResponseBuilder::new()
                .description(status.canonical_reason().unwrap())
                .content(
                    "application/problem+json",
                    Content::new(Some(Ref::from_schema_name(Problem::name()))),
                );
            let header = match status {
                StatusCode::METHOD_NOT_ALLOWED => Some(("Allow", "Supported HTTP methods")),
                StatusCode::TOO_MANY_REQUESTS => {
                    Some(("Retry-After", "Seconds until the client may retry"))
                }
                StatusCode::SERVICE_UNAVAILABLE => Some((
                    "Retry-After",
                    "Upstream retry delay or HTTP date, when supplied",
                )),
                _ => None,
            };
            if let Some((name, description)) = header {
                response = response.header(
                    name,
                    HeaderBuilder::new().description(Some(description)).build(),
                );
            }
            (status.as_u16().to_string(), response.build().into())
        })
        .collect()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let cache_policy = match &self {
            Self::TooManyRequests => "no-store",
            Self::BadRequest
            | Self::NotFound
            | Self::MethodNotAllowed
            | Self::InternalServerError
            | Self::BadGateway
            | Self::ServiceUnavailable { .. } => "public, max-age=10",
        };
        let (status, title, retry_after) = match self {
            Self::BadRequest => (StatusCode::BAD_REQUEST, "Bad Request", None),
            Self::NotFound => (StatusCode::NOT_FOUND, "Not Found", None),
            Self::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed", None),
            Self::TooManyRequests => (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests", None),
            Self::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                None,
            ),
            Self::BadGateway => (StatusCode::BAD_GATEWAY, "Bad Gateway", None),
            Self::ServiceUnavailable { retry_after } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Service Unavailable",
                retry_after,
            ),
        };

        let mut response = (
            status,
            [
                (header::CONTENT_TYPE, "application/problem+json"),
                (header::CACHE_CONTROL, cache_policy),
            ],
            Json(Problem {
                r#type: "about:blank",
                title,
                status: status.as_u16(),
            }),
        )
            .into_response();
        if let Some(retry_after) = retry_after {
            response.headers_mut().typed_insert(retry_after);
        }
        response
    }
}
