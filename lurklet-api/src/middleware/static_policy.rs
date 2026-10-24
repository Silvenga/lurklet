use crate::routes::robots;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use percent_encoding::percent_decode_str;
use tower::ServiceExt;

/// Protects reserved paths and applies static response cache policies.
pub async fn static_policy(request: Request, next: Next) -> Response {
    let path = percent_decode_str(request.uri().path()).decode_utf8();
    let blocked = path.as_ref().map_or(true, |path| {
        path.contains(['\\', '\0'])
            || path.split('/').any(|part| part.starts_with('.'))
            || path.trim_start_matches('/') == "api"
            || path.trim_start_matches('/').starts_with("api/")
            || path.ends_with(".br")
            || path.ends_with(".gz")
    });
    let immutable = path.as_ref().is_ok_and(|path| {
        path.starts_with("/assets/")
            && mime_guess::from_path(&**path).first_raw() != Some("text/html")
    });
    let robots_alias = path
        .as_ref()
        .is_ok_and(|path| path.trim_start_matches('/') == "robots.txt")
        && request.uri().path() != "/robots.txt";
    let response = if blocked {
        StatusCode::NOT_FOUND.into_response()
    } else if robots_alias {
        get(robots).oneshot(request).await.unwrap()
    } else {
        next.run(request).await
    };
    let policy = if response.status().is_client_error() || response.status().is_server_error() {
        "no-store"
    } else if immutable {
        "public, max-age=31536000, immutable"
    } else {
        "public, no-cache"
    };
    let vary = if response
        .headers()
        .get(header::VARY)
        .is_some_and(|value| value == "Accept")
    {
        "Accept, Accept-Encoding"
    } else {
        "Accept-Encoding"
    };
    (
        [(header::CACHE_CONTROL, policy), (header::VARY, vary)],
        response,
    )
        .into_response()
}
