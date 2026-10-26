use crate::ResponseCache;
use crate::error::{ApiError, Problem};
use crate::middleware::{
    RateLimit, access_log, api_cache_policy, api_content_security_policy, content_type_options,
    limit_requests, response_compression,
};
use crate::routes;
use axum::Router;
use axum::http::header;
use axum::middleware;
use lurklet_reddit::{MobileBackend, RedditBackend, RedditClient};
use utoipa::OpenApi as _;
use utoipa::openapi::{OpenApi, OpenApiVersion};

#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Lurklet API",
        description = "Internal API contract. API requests share a per-client rate limit, except for /api/health. Excess requests return 429 Problem Details with Retry-After. Unknown API paths return 404 Problem Details unless rate limited."
    ),
    components(schemas(Problem))
)]
struct ApiDoc;

const API_PREFIX: &str = "/api";

/// Builds the API router without starting a listener.
pub fn application<B: RedditBackend + 'static>(
    reddit: RedditClient<B>,
    rate_limit: &RateLimit,
    cache: Option<&ResponseCache>,
) -> Router {
    let (health, _) = routes::health().split_for_parts();
    let health = health.method_not_allowed_fallback(|| async {
        (
            [(header::CACHE_CONTROL, "no-store")],
            ApiError::MethodNotAllowed,
        )
    });

    let (json_routes, _) = routes::get_post()
        .merge(routes::get_post_comments())
        .merge(routes::get_comment_thread())
        .merge(routes::get_subreddit())
        .merge(routes::list_subreddit_posts())
        .split_for_parts();
    let json_routes = json_routes.fallback(|| async { ApiError::NotFound });
    let json_api = data_api(json_routes, reddit.clone(), rate_limit, cache)
        .merge(health)
        .layer(middleware::from_fn(response_compression));
    let (media_routes, _) = routes::get_media().split_for_parts();
    let media_api = data_api(media_routes, reddit, rate_limit, cache);
    let api = json_api
        .merge(media_api)
        .layer(middleware::from_fn(api_cache_policy))
        .layer(api_content_security_policy())
        .layer(content_type_options())
        .layer(middleware::from_fn(access_log));

    // Service nesting reserves /api, /api/, and descendants, including unmatched paths.
    Router::new().nest_service(API_PREFIX, api)
}

/// Builds the API contract without initializing runtime dependencies.
pub fn specification() -> OpenApi {
    let mut routes = routes::health().into_openapi();
    routes.merge(routes::get_post::<MobileBackend>().into_openapi());
    routes.merge(routes::get_post_comments::<MobileBackend>().into_openapi());
    routes.merge(routes::get_comment_thread::<MobileBackend>().into_openapi());
    routes.merge(routes::get_subreddit::<MobileBackend>().into_openapi());
    routes.merge(routes::get_media::<MobileBackend>().into_openapi());
    routes.merge(routes::list_subreddit_posts::<MobileBackend>().into_openapi());
    let mut specification = ApiDoc::openapi().nest(API_PREFIX, routes);
    specification.openapi = OpenApiVersion::Version32;
    specification
}

fn data_api<B: RedditBackend + 'static>(
    routes: Router<RedditClient<B>>,
    reddit: RedditClient<B>,
    rate_limit: &RateLimit,
    cache: Option<&ResponseCache>,
) -> Router {
    let mut api = routes
        .method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed })
        .with_state(reddit);
    if let Some(cache) = cache {
        api = cache.wrap(api);
    }
    api.layer(middleware::from_fn_with_state(
        rate_limit.clone(),
        limit_requests,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RateLimitOptions, StaticHost};
    use assert_fs::prelude::*;
    use axum::Extension;
    use axum::body::{Body, to_bytes};
    use axum::extract::{ConnectInfo, State};
    use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header};
    use axum::response::Html;
    use axum::routing::get;
    use lurklet_reddit::{StateStore, Storage};
    use serde_json::{Value, json};
    use std::net::SocketAddr;
    use tower::ServiceExt;

    fn application() -> Router {
        let reddit = RedditClient::builder().build().unwrap();
        let app = super::application(reddit, &RateLimit::default(), None);
        let peer: SocketAddr = "192.0.2.1:1234".parse().unwrap();
        app.layer(Extension(ConnectInfo(peer)))
    }

    fn assert_browser_policy(headers: &HeaderMap) {
        for (name, expected) in [
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; sandbox; style-src 'unsafe-inline'; media-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ] {
            assert_eq!(
                headers
                    .get_all(&name)
                    .iter()
                    .map(|value| value.to_str().unwrap())
                    .collect::<Vec<_>>(),
                [expected],
                "{name}"
            );
        }
    }

    #[tokio::test]
    async fn when_data_routes_extract_the_client_then_state_should_be_available_with_or_without_caching()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let storage = Storage::default();
        let reddit = RedditClient::builder()
            .with_storage(storage.clone())
            .build()
            .unwrap();

        for cache in [None, Some(&cache)] {
            let routes = Router::new().route(
                "/resource",
                get(|State(_reddit): State<RedditClient>| async { StatusCode::NO_CONTENT }),
            );
            let app = data_api(routes, reddit.clone(), &RateLimit::default(), cache);
            let request = Request::builder()
                .uri("/resource")
                .extension(ConnectInfo("192.0.2.1:1234".parse::<SocketAddr>().unwrap()))
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::NO_CONTENT);
        }
        assert_eq!(storage.store().load().await.unwrap().revision, 0);
    }

    #[tokio::test]
    async fn when_health_is_requested_through_the_api_then_response_should_disable_caching() {
        let app = application();
        let request = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_browser_policy(response.headers());
    }

    #[tokio::test]
    async fn when_client_ip_is_unavailable_then_internal_errors_should_keep_browser_policies() {
        let reddit = RedditClient::builder().build().unwrap();
        let app = super::application(reddit, &RateLimit::default(), None);
        let request = Request::builder()
            .uri("/api/media")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_browser_policy(response.headers());
    }

    #[tokio::test]
    async fn when_an_ip_is_limited_then_health_robots_and_static_routes_should_remain_exempt() {
        let directory = assert_fs::TempDir::new().unwrap();
        directory.child("index.html").write_str("site").unwrap();
        directory
            .child("assets/app.js")
            .write_str("script")
            .unwrap();
        let limiter = RateLimit::new(RateLimitOptions {
            rate_limit_burst: 1.try_into().unwrap(),
            ..RateLimitOptions::default()
        });
        let reddit = RedditClient::builder().build().unwrap();
        let app = super::application(reddit, &limiter, None);
        let peer: SocketAddr = "192.0.2.1:1234".parse().unwrap();
        let app = app
            .fallback_service(StaticHost::new(directory.path()).await.unwrap().router())
            .layer(Extension(ConnectInfo(peer)));
        let request = |path: &str| {
            Request::builder()
                .uri(path)
                .header(header::ACCEPT, "text/html")
                .body(Body::empty())
                .unwrap()
        };
        let _ = app.clone().oneshot(request("/api/missing")).await.unwrap();

        for _ in 0..12 {
            for path in [
                "/api/health",
                "/api/health?probe=1",
                "/robots.txt",
                "/%72obots.txt",
                "/",
                "/assets/app.js",
                "/r/rust",
            ] {
                let response = app.clone().oneshot(request(path)).await.unwrap();

                assert_eq!(response.status(), StatusCode::OK, "{path}");
                assert!(
                    !response.headers().contains_key("x-ratelimit-limit"),
                    "{path}"
                );
            }
        }
        for method in [Method::HEAD, Method::POST] {
            let mut request = request("/api/health");
            *request.method_mut() = method.clone();
            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(
                response.status(),
                if method == Method::HEAD {
                    StatusCode::OK
                } else {
                    StatusCode::METHOD_NOT_ALLOWED
                }
            );
            assert!(!response.headers().contains_key("x-ratelimit-limit"));
        }
        for path in [
            "/api",
            "/api/",
            "/api/health/",
            "/api/robots.txt",
            "/api/media",
        ] {
            let response = app.clone().oneshot(request(path)).await.unwrap();

            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS, "{path}");
            assert_browser_policy(response.headers());
        }
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
            let app = application();
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
            assert_browser_policy(&headers);
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
            let app = application();
            let app = app.fallback(|| async { Html("SPA") });
            let request = Request::builder().uri(path).body(Body::empty()).unwrap();

            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_browser_policy(&headers);
            assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
            assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=10");
            assert!(!headers.contains_key("x-ratelimit-limit"));
            assert!(!headers.contains_key("x-ratelimit-remaining"));
            assert!(!headers.contains_key("x-ratelimit-after"));
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                json!({"type": "about:blank", "title": "Not Found", "status": 404})
            );
        }
    }

    #[tokio::test]
    async fn when_path_is_outside_api_then_response_should_allow_spa_fallback() {
        for path in ["/", "/posts/example", "/apiculture", "/openapi.json"] {
            let app = application();
            let app = app.fallback(|| async { Html("SPA") });
            let request = Request::builder().uri(path).body(Body::empty()).unwrap();

            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::OK, "{path}");
            assert!(!headers.contains_key(header::CACHE_CONTROL));
            assert!(!headers.contains_key(header::CONTENT_SECURITY_POLICY));
            assert!(!headers.contains_key(header::X_CONTENT_TYPE_OPTIONS));
            assert_eq!(body.as_ref(), b"SPA");
        }
    }

    #[test]
    fn when_openapi_is_generated_then_contract_should_document_api_routes() {
        let specification = specification();

        let document = serde_json::to_value(specification).unwrap();

        insta::assert_json_snapshot!("openapi", document);
    }

    #[tokio::test]
    async fn when_compression_is_requested_then_media_and_static_hosting_should_bypass_the_json_layer()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        directory
            .child("index.html")
            .write_str("<html>Static content remains outside dynamic API compression.</html>")
            .unwrap();
        let cache_directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(cache_directory.path()).await.unwrap();
        let media = "A complete media fixture that is larger than the compression threshold.";
        let seed = cache.wrap(Router::new().route(
            "/api/media",
            get(move || async move {
                (
                    [
                        (header::CONTENT_TYPE, "video/mp4"),
                        (header::CACHE_CONTROL, "public, max-age=300"),
                    ],
                    media,
                )
            }),
        ));
        let response = seed
            .oneshot(
                Request::builder()
                    .uri("/api/media")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        to_bytes(response.into_body(), 4096).await.unwrap();
        let reddit = RedditClient::builder().build().unwrap();
        let app = super::application(reddit, &RateLimit::default(), Some(&cache))
            .fallback_service(StaticHost::new(directory.path()).await.unwrap().router())
            .layer(Extension(ConnectInfo(
                "192.0.2.1:1234".parse::<SocketAddr>().unwrap(),
            )));

        for accept in [
            "gzip, br, deflate, zstd",
            "gzip;q=0, br;q=0, deflate;q=0, zstd;q=0, identity;q=0",
        ] {
            for (path, status) in [
                ("/api/media", StatusCode::OK),
                ("/api/media?missingUrl=true", StatusCode::BAD_REQUEST),
            ] {
                let response = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(path)
                            .header(header::ACCEPT_ENCODING, accept)
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();

                assert_eq!(response.status(), status, "{path}: {accept}");
                assert_browser_policy(response.headers());
                assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
                if status == StatusCode::OK {
                    assert_eq!(response.headers()[header::CONTENT_TYPE], "video/mp4");
                    assert_eq!(response.headers()["x-lurklet-cache"], "hit");
                    assert_eq!(
                        to_bytes(response.into_body(), 4096).await.unwrap(),
                        media.as_bytes()
                    );
                } else {
                    assert_eq!(
                        response.headers()[header::CONTENT_TYPE],
                        "application/problem+json"
                    );
                    assert_eq!(
                        response.headers()[header::CACHE_CONTROL],
                        "public, max-age=10"
                    );
                }
            }
        }
        let json = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/missing")
                    .header(header::ACCEPT_ENCODING, "gzip")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let site = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::ACCEPT_ENCODING, "gzip, br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(json.status(), StatusCode::NOT_FOUND);
        assert_browser_policy(json.headers());
        assert_eq!(json.headers()[header::CONTENT_ENCODING], "gzip");
        assert_eq!(json.headers()[header::CACHE_CONTROL], "public, max-age=10");
        assert_eq!(site.status(), StatusCode::OK);
        assert!(!site.headers().contains_key(header::CONTENT_ENCODING));
    }

    #[tokio::test]
    async fn when_health_encoding_is_unacceptable_then_problem_details_should_remain_uncacheable() {
        let app = application();

        for method in [Method::GET, Method::HEAD, Method::POST] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method.clone())
                        .uri("/api/health")
                        .header(
                            header::ACCEPT_ENCODING,
                            "gzip;q=0, br;q=0, deflate;q=0, zstd;q=0, identity;q=0",
                        )
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::NOT_ACCEPTABLE, "{method}");
            assert_browser_policy(&headers);
            assert_eq!(headers[header::CACHE_CONTROL], "no-store");
            assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
            assert!(!headers.contains_key(header::CONTENT_ENCODING));
            if method == Method::HEAD {
                assert!(body.is_empty());
            } else {
                assert_eq!(
                    serde_json::from_slice::<Value>(&body).unwrap(),
                    json!({"type": "about:blank", "title": "Not Acceptable", "status": 406})
                );
            }
        }
    }

    #[test]
    fn when_comment_contracts_are_generated_then_defaults_and_recursive_references_should_be_resolvable()
     {
        let specification = specification();

        let document = serde_json::to_value(specification).unwrap();

        for path in ["/api/posts/{id}/comments", "/api/comments/{id}/thread"] {
            assert_eq!(
                document["paths"][path]["get"]["parameters"][1],
                json!({
                    "name": "sort", "in": "query", "required": false,
                    "schema": {"type": "string", "default": "best", "enum": ["best", "top", "new"]}
                })
            );
        }
        assert_eq!(
            document["components"]["schemas"]["CommentItem"]["oneOf"][0]["properties"]["replies"]["items"]
                ["$ref"],
            "#/components/schemas/CommentItem"
        );
        assert_eq!(
            document["paths"]["/api/posts/{id}/comments"]["get"]["parameters"][2]["name"],
            "cursor"
        );
        assert_eq!(
            document["paths"]["/api/comments/{id}/thread"]["get"]["parameters"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let mut pending = vec![&document];
        while let Some(value) = pending.pop() {
            match value {
                Value::Object(object) => {
                    if let Some(Value::String(reference)) = object.get("$ref") {
                        assert!(
                            document
                                .pointer(reference.strip_prefix('#').unwrap())
                                .is_some(),
                            "{reference}"
                        );
                    }
                    pending.extend(object.values());
                }
                Value::Array(array) => pending.extend(array),
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
            }
        }
    }

    #[tokio::test]
    async fn when_data_input_is_invalid_then_the_mounted_endpoint_should_return_problem_details() {
        let app = application();
        for uri in [
            "/api/posts/%FF",
            "/api/subreddits/rust/posts?sort=invalid",
            "/api/subreddits/%FF",
            "/api/media",
            "/api/posts/post/comments?sort=invalid",
            "/api/comments/anchor/thread?sort=invalid",
        ] {
            let request = Request::builder().uri(uri).body(Body::empty()).unwrap();

            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
            assert_browser_policy(response.headers());
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/problem+json"
            );
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=10"
            );
        }
    }

    #[tokio::test]
    async fn when_cache_is_enabled_then_api_hits_should_be_limited_and_health_should_bypass_caching()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let fixture = cache.wrap(Router::new().route(
            "/api/posts/18t5968",
            get(|| async { ([(header::CACHE_CONTROL, "public, max-age=3600")], "cached") }),
        ));
        let response = fixture
            .oneshot(
                Request::builder()
                    .uri("/api/posts/18t5968")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()["x-lurklet-cache"], "miss");
        to_bytes(response.into_body(), 4096).await.unwrap();
        let limiter = RateLimit::new(RateLimitOptions {
            rate_limit_burst: 1.try_into().unwrap(),
            ..RateLimitOptions::default()
        });
        let reddit = RedditClient::builder().build().unwrap();
        let app = super::application(reddit, &limiter, Some(&cache));
        for _ in 0..12 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/health")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert!(!response.headers().contains_key("x-lurklet-cache"));
            assert!(!response.headers().contains_key("x-ratelimit-limit"));
        }
        let app = app.layer(Extension(ConnectInfo(
            "192.0.2.1:1234".parse::<SocketAddr>().unwrap(),
        )));

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/posts/18t5968")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let second = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/posts/18t5968")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let health = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(first.status(), StatusCode::OK);
        assert_browser_policy(first.headers());
        assert_eq!(first.headers()["x-lurklet-cache"], "hit");
        assert!(!first.headers().contains_key("x-ratelimit-remaining"));
        assert_eq!(
            to_bytes(first.into_body(), 4096).await.unwrap(),
            b"cached".as_slice()
        );
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_browser_policy(second.headers());
        assert!(!second.headers().contains_key("x-lurklet-cache"));
        assert_eq!(health.status(), StatusCode::OK);
        assert_browser_policy(health.headers());
        assert!(!health.headers().contains_key("x-lurklet-cache"));
        assert_eq!(health.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            serde_json::from_slice::<Value>(&to_bytes(health.into_body(), 4096).await.unwrap())
                .unwrap(),
            json!({"health": "ok"})
        );
    }

    #[tokio::test]
    async fn when_media_was_cached_without_isolation_then_hits_and_stale_responses_should_use_current_browser_policies()
     {
        const SVG: &str =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;

        for (cache_control, outcome) in [
            ("public, max-age=300", "hit"),
            ("public, max-age=0", "stale"),
        ] {
            for old_policy in [None, Some("default-src * 'unsafe-inline'")] {
                let directory = assert_fs::TempDir::new().unwrap();
                let cache = ResponseCache::new(directory.path()).await.unwrap();
                let seed = cache.wrap(Router::new().route(
                    "/api/media",
                    get(move || async move {
                        let mut headers = HeaderMap::from_iter([
                            (
                                header::CONTENT_TYPE,
                                HeaderValue::from_static("image/svg+xml"),
                            ),
                            (
                                header::CACHE_CONTROL,
                                HeaderValue::from_static(cache_control),
                            ),
                        ]);
                        if let Some(policy) = old_policy {
                            headers.append(
                                header::CONTENT_SECURITY_POLICY,
                                HeaderValue::from_static(policy),
                            );
                            headers.append(
                                header::CONTENT_SECURITY_POLICY,
                                HeaderValue::from_static("sandbox allow-scripts allow-same-origin"),
                            );
                            headers.insert(
                                header::X_CONTENT_TYPE_OPTIONS,
                                HeaderValue::from_static("invalid"),
                            );
                        }
                        (headers, SVG)
                    }),
                ));
                let request = Request::builder()
                    .uri("/api/media")
                    .body(Body::empty())
                    .unwrap();
                let response = seed.oneshot(request).await.unwrap();
                to_bytes(response.into_body(), 4096).await.unwrap();
                let reddit = RedditClient::builder().build().unwrap();
                let app = super::application(reddit, &RateLimit::default(), Some(&cache)).layer(
                    Extension(ConnectInfo("192.0.2.1:1234".parse::<SocketAddr>().unwrap())),
                );

                for method in [Method::GET, Method::HEAD] {
                    let request = Request::builder()
                        .method(method.clone())
                        .uri("/api/media")
                        .body(Body::empty())
                        .unwrap();

                    let response = app.clone().oneshot(request).await.unwrap();
                    let status = response.status();
                    let headers = response.headers().clone();
                    let body = to_bytes(response.into_body(), 4096).await.unwrap();

                    assert_eq!(status, StatusCode::OK);
                    assert_browser_policy(&headers);
                    assert_eq!(headers["x-lurklet-cache"], outcome);
                    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml");
                    assert_eq!(headers[header::CACHE_CONTROL], cache_control);
                    assert_eq!(headers[header::CONTENT_LENGTH], SVG.len().to_string());
                    assert_eq!(
                        body.as_ref(),
                        if method == Method::HEAD {
                            b""
                        } else {
                            SVG.as_bytes()
                        }
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn when_cache_is_enabled_then_static_robots_and_unsupported_methods_should_remain_outside_it()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        directory.child("index.html").write_str("site").unwrap();
        let cache_directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(cache_directory.path()).await.unwrap();
        let reddit = RedditClient::builder().build().unwrap();
        let app = super::application(reddit, &RateLimit::default(), Some(&cache));
        let app = app
            .fallback_service(StaticHost::new(directory.path()).await.unwrap().router())
            .layer(Extension(ConnectInfo(
                "192.0.2.1:1234".parse::<SocketAddr>().unwrap(),
            )));

        for (method, path, expected) in [
            (Method::GET, "/robots.txt", StatusCode::OK),
            (Method::GET, "/", StatusCode::OK),
            (Method::GET, "/missing.js", StatusCode::NOT_FOUND),
            (Method::POST, "/api/health", StatusCode::METHOD_NOT_ALLOWED),
            (
                Method::POST,
                "/api/posts/18t5968",
                StatusCode::METHOD_NOT_ALLOWED,
            ),
            (
                Method::POST,
                "/api/subreddits/rust/posts",
                StatusCode::METHOD_NOT_ALLOWED,
            ),
            (
                Method::POST,
                "/api/subreddits/rust",
                StatusCode::METHOD_NOT_ALLOWED,
            ),
            (
                Method::POST,
                "/api/posts/post/comments",
                StatusCode::METHOD_NOT_ALLOWED,
            ),
            (
                Method::POST,
                "/api/comments/anchor/thread",
                StatusCode::METHOD_NOT_ALLOWED,
            ),
            (Method::POST, "/api/media", StatusCode::METHOD_NOT_ALLOWED),
            (Method::GET, "/api/health", StatusCode::OK),
            (Method::HEAD, "/api/health", StatusCode::OK),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), expected, "{path}");
            if expected == StatusCode::METHOD_NOT_ALLOWED && path != "/api/health" {
                assert_eq!(
                    response.headers()[header::CACHE_CONTROL],
                    "public, max-age=10"
                );
            }
            if path.starts_with("/api/") {
                assert_browser_policy(response.headers());
                assert!(!response.headers().contains_key(header::REFERRER_POLICY));
            } else {
                assert_eq!(
                    response.headers()[header::CONTENT_SECURITY_POLICY],
                    "default-src 'self'; style-src-elem 'self' 'unsafe-inline'; style-src-attr 'none'"
                );
                assert_eq!(response.headers()[header::REFERRER_POLICY], "same-origin");
                assert_eq!(
                    response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                    "nosniff"
                );
            }
            assert!(
                !response.headers().contains_key("x-lurklet-cache"),
                "{path}"
            );
        }
    }
}
