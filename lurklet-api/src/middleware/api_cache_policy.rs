use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;

/// Defaults API responses to no-store and removes cacheable per-client quota headers.
pub async fn api_cache_policy(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));

    let uncacheable = response
        .headers()
        .get_all(header::CACHE_CONTROL)
        .iter()
        .any(|value| {
            value.to_str().is_ok_and(|value| {
                value
                    .split(',')
                    .any(|directive| directive.trim().eq_ignore_ascii_case("no-store"))
            })
        });
    if !uncacheable {
        // A shared cache must not replay one client's quota snapshot to another.
        for name in [
            "x-ratelimit-limit",
            "x-ratelimit-remaining",
            "x-ratelimit-after",
        ] {
            response.headers_mut().remove(name);
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::middleware;
    use axum::routing::get;
    use tower::ServiceExt;

    #[tokio::test]
    async fn when_a_response_can_be_cached_then_quota_headers_should_not_be_replayed() {
        for (status, policy, retain_quota) in [
            (StatusCode::OK, "public, max-age=60", false),
            (StatusCode::OK, "private, max-age=60", false),
            (StatusCode::OK, "no-cache", false),
            (StatusCode::OK, "no-store", true),
            (StatusCode::OK, "PUBLIC, NO-STORE", true),
            (StatusCode::NOT_FOUND, "public, max-age=10", false),
            (StatusCode::SERVICE_UNAVAILABLE, "public, max-age=10", false),
            (StatusCode::SERVICE_UNAVAILABLE, "no-store", true),
            (StatusCode::TOO_MANY_REQUESTS, "no-store", true),
        ] {
            let app = Router::new()
                .route(
                    "/cached",
                    get(move || async move {
                        (
                            status,
                            [
                                ("cache-control", policy),
                                ("x-ratelimit-limit", "10"),
                                ("x-ratelimit-remaining", "9"),
                                ("x-ratelimit-after", "1"),
                            ],
                            "cached",
                        )
                    }),
                )
                .layer(middleware::from_fn(api_cache_policy));
            let request = Request::builder()
                .uri("/cached")
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();

            assert_eq!(response.status(), status);
            assert_eq!(response.headers()[header::CACHE_CONTROL], policy);
            for name in [
                "x-ratelimit-limit",
                "x-ratelimit-remaining",
                "x-ratelimit-after",
            ] {
                assert_eq!(
                    response.headers().contains_key(name),
                    retain_quota,
                    "{policy}: {name}"
                );
            }
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
            .layer(middleware::from_fn(api_cache_policy));
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
            .layer(middleware::from_fn(api_cache_policy));
        let request = Request::builder()
            .uri("/resource")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn when_error_has_public_cache_policy_then_middleware_should_preserve_it() {
        for status in [StatusCode::NOT_FOUND, StatusCode::INTERNAL_SERVER_ERROR] {
            let app = Router::new()
                .route(
                    "/resource",
                    get(move || async move {
                        (status, [(header::CACHE_CONTROL, "public, max-age=10")])
                    }),
                )
                .layer(middleware::from_fn(api_cache_policy));
            let request = Request::builder()
                .uri("/resource")
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();

            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=10"
            );
        }
    }
}
