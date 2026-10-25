use axum::extract::{OriginalUri, Request};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use tracing::Instrument;

/// Records access details at debug and request/response metadata only at trace.
pub async fn access_log(request: Request, next: Next) -> Response {
    let uri = request
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| request.uri(), |uri| &uri.0);
    let span = tracing::debug_span!("http.request", method = %request.method(), path = uri.path_and_query().map_or("/", |path| path.as_str()));
    async move {
        let started = Instant::now();
        tracing::trace!(headers = ?request.headers(), version = ?request.version(), "HTTP request metadata");
        let response = next.run(request).await;
        tracing::debug!(status = response.status().as_u16(), duration_ms = %format_args!("{:.3}", started.elapsed().as_secs_f64() * 1000.0), "HTTP request completed");
        tracing::trace!(headers = ?response.headers(), version = ?response.version(), "HTTP response metadata");
        response
    }.instrument(span).await
}
