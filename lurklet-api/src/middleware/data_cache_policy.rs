use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use clap::Args;

/// Shared freshness configuration for successful API data and media responses.
#[derive(Args)]
pub struct ApiCacheOptions {
    /// Cache successful API data and media responses for this many seconds; zero makes them immediately stale.
    #[arg(
        long,
        env = "LURKLET_API_CACHE_MAX_AGE_SECONDS",
        default_value_t = Self::default().api_cache_max_age_seconds
    )]
    pub api_cache_max_age_seconds: u64,
}

impl Default for ApiCacheOptions {
    fn default() -> Self {
        Self {
            api_cache_max_age_seconds: 3600,
        }
    }
}

impl ApiCacheOptions {
    /// Applies freshness before persistent caching, leaving error policies unchanged.
    pub fn wrap<S: Clone + Send + Sync + 'static>(&self, routes: Router<S>) -> Router<S> {
        let policy = HeaderValue::from_str(&format!(
            "public, max-age={}",
            self.api_cache_max_age_seconds
        ))
        .unwrap();
        routes.layer(middleware::from_fn_with_state(policy, Self::apply))
    }

    async fn apply(State(policy): State<HeaderValue>, request: Request, next: Next) -> Response {
        let mut response = next.run(request).await;
        if response.status() == StatusCode::OK {
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
    async fn when_api_freshness_is_customized_then_error_policies_should_remain_unchanged() {
        let options = ApiCacheOptions {
            api_cache_max_age_seconds: 42,
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
