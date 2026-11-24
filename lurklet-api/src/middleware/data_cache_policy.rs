use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use clap::Args;

/// Shared freshness configuration for successful API data and media responses.
#[derive(Args, Default)]
pub struct ApiCacheOptions {
    /// Cache lifetime for successful API data (default: 3600 seconds). If set, also applies to media unless overridden.
    #[arg(long, env = "LURKLET_API_CACHE_MAX_AGE_SECONDS")]
    pub api_cache_max_age_seconds: Option<u64>,
    /// Cache lifetime for successful media (default: the explicit API lifetime, otherwise 31536000 seconds).
    #[arg(long, env = "LURKLET_MEDIA_CACHE_MAX_AGE_SECONDS")]
    pub media_cache_max_age_seconds: Option<u64>,
}

impl ApiCacheOptions {
    /// Applies API data freshness before persistent caching.
    pub fn wrap<S: Clone + Send + Sync + 'static>(&self, routes: Router<S>) -> Router<S> {
        Self::wrap_with_age(routes, self.api_cache_max_age_seconds.unwrap_or(3600))
    }

    /// Applies media freshness to complete and partial responses.
    pub fn wrap_media<S: Clone + Send + Sync + 'static>(&self, routes: Router<S>) -> Router<S> {
        let seconds = self
            .media_cache_max_age_seconds
            .or(self.api_cache_max_age_seconds)
            .unwrap_or(31_536_000);
        Self::wrap_with_age(routes, seconds)
    }

    fn wrap_with_age<S: Clone + Send + Sync + 'static>(
        routes: Router<S>,
        seconds: u64,
    ) -> Router<S> {
        let policy = HeaderValue::from_str(&format!("public, max-age={seconds}")).unwrap();
        routes.layer(middleware::from_fn_with_state(policy, Self::apply))
    }

    async fn apply(State(policy): State<HeaderValue>, request: Request, next: Next) -> Response {
        let mut response = next.run(request).await;
        if matches!(
            response.status(),
            StatusCode::OK | StatusCode::PARTIAL_CONTENT
        ) {
            response.headers_mut().insert(header::CACHE_CONTROL, policy);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ApiError;
    use axum::body::Body;
    use axum::routing::get;
    use tower::ServiceExt;

    #[tokio::test]
    async fn when_cache_lifetimes_are_configured_then_media_should_prefer_its_override() {
        for (api, media, data_ttl, media_ttl) in [
            (None, None, 3600, 31_536_000),
            (Some(0), None, 0, 0),
            (None, Some(0), 3600, 0),
            (Some(3600), None, 3600, 3600),
            (Some(42), Some(7), 42, 7),
        ] {
            let options = ApiCacheOptions {
                api_cache_max_age_seconds: api,
                media_cache_max_age_seconds: media,
            };
            let routes = Router::new().route("/", get(|| async {}));
            for (app, ttl) in [
                (options.wrap(routes.clone()), data_ttl),
                (options.wrap_media(routes), media_ttl),
            ] {
                let response = app.oneshot(Request::new(Body::empty())).await.unwrap();

                assert_eq!(
                    response.headers()[header::CACHE_CONTROL],
                    format!("public, max-age={ttl}")
                );
            }
        }
    }

    #[tokio::test]
    async fn when_api_freshness_is_customized_then_error_policies_should_remain_unchanged() {
        let options = ApiCacheOptions {
            api_cache_max_age_seconds: Some(42),
            ..Default::default()
        };
        let routes = Router::new()
            .route("/missing", get(|| async { ApiError::NotFound }))
            .route("/limited", get(|| async { ApiError::TooManyRequests }))
            .route("/failed", get(|| async { ApiError::BadGateway }));
        let app = options.wrap(routes);

        for (path, status, policy) in [
            ("/missing", StatusCode::NOT_FOUND, "public, max-age=10"),
            ("/limited", StatusCode::TOO_MANY_REQUESTS, "no-store"),
            ("/failed", StatusCode::BAD_GATEWAY, "public, max-age=10"),
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), status);
            assert_eq!(response.headers()[header::CACHE_CONTROL], policy);
        }
    }
}
