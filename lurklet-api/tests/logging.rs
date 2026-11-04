//! Isolate tracing callsite state from concurrent router tests.

use assert_fs::prelude::*;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, header};
use axum::routing::get;
use lurklet_api::{
    ApiCacheOptions, QueryPolicy, RateLimitOptions, RateLimits, StaticHost, application,
    specification,
};
use lurklet_reddit::RedditClient;
use std::net::SocketAddr;
use std::sync::Mutex;
use tower::ServiceExt;
use tracing::instrument::WithSubscriber;
use tracing::{Instrument, Level};

#[tokio::test]
async fn when_unknown_keys_are_discovered_then_warnings_should_be_deduplicated_bounded_and_exclude_request_values()
 {
    let subscriber = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .with_ansi(false)
            .with_writer(Mutex::new(Vec::<u8>::new()))
            .finish(),
    );
    let routes = Router::new()
        .route("/media/{origin}/{*path}", get(|| async {}))
        .route("/posts/{id}", get(|| async {}));
    let app = Router::new().nest_service(
        "/api",
        QueryPolicy::from_specification(&specification()).wrap(routes),
    );

    async {
        for uri in [
            "/api/media/preview.redd.it/private-path.png?discovered=private-query&s=private-signature",
            "/api/media/preview.redd.it/other-private-path.png?%64iscovered=other-private-query",
            "/api/posts/private-id?discovered=private-query",
        ].into_iter().map(str::to_owned).chain((0..1025).map(|index| format!("/api/posts/private-id?new_key_{index}=private-query"))) {
            let request = Request::builder().uri(uri).body(Body::empty()).unwrap();

            let response = app.clone().oneshot(request)
                .instrument(tracing::debug_span!("request", uri = "private-request-uri"))
                .await.unwrap();

            assert!(response.status().is_success());
        }
    }.with_subscriber(subscriber.clone()).await;
    let logs = String::from_utf8(
        subscriber
            .downcast_ref::<Mutex<Vec<u8>>>()
            .unwrap()
            .lock()
            .unwrap()
            .clone(),
    )
    .unwrap();

    assert_eq!(
        logs.matches("query_key=\"discovered\"").count(),
        2,
        "{logs}"
    );
    assert_eq!(
        logs.matches("Dropped an unknown query key").count(),
        1024,
        "{logs}"
    );
    assert_eq!(
        logs.matches("Dropped-query discovery limit reached")
            .count(),
        1,
        "{logs}"
    );
    assert!(logs.contains("route=/api/media/{origin}/{path}"));
    assert!(logs.contains("route=/api/posts/{id}"));
    assert!(!logs.contains("private-"));
    assert!(!logs.contains("preview.redd.it"));
    assert!(!logs.contains("new_key_1024"));
}

#[tokio::test]
async fn when_requests_are_logged_then_details_should_follow_the_configured_level() {
    let directory = assert_fs::TempDir::new().unwrap();
    directory.child("index.html").write_str("site").unwrap();
    for level in [Level::INFO, Level::DEBUG, Level::TRACE] {
        let subscriber = tracing::Dispatch::new(
            tracing_subscriber::fmt()
                .with_max_level(level)
                .with_ansi(false)
                .with_writer(Mutex::new(Vec::<u8>::new()))
                .finish(),
        );

        async {
            let rate_limits = RateLimits::new(RateLimitOptions {
                rate_limit_burst: 1.try_into().unwrap(),
                ..RateLimitOptions::default()
            });
            let reddit = RedditClient::builder().build().unwrap();
            let app = application(reddit, &rate_limits, None, &ApiCacheOptions::default());
            let app =
                app.fallback_service(StaticHost::new(directory.path()).await.unwrap().router());
            tracing::info!("Operator event");
            for (method, path) in [
                (Method::GET, "/api/health?query=private-query"),
                (Method::GET, "/api/private-path?query=private-query"),
                (Method::GET, "/api/private-path?query=private-query"),
                (Method::POST, "/api/health?query=private-query"),
                (Method::GET, "/private-navigation?query=private-query"),
            ] {
                let request = Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::AUTHORIZATION, "Bearer private-token")
                    .header(header::COOKIE, "session=private-cookie")
                    .header("x-forwarded-for", "198.51.100.123")
                    .extension(ConnectInfo(
                        "192.0.2.123:1234".parse::<SocketAddr>().unwrap(),
                    ))
                    .body(Body::from("private-body"))
                    .unwrap();
                let response = app.clone().oneshot(request).await.unwrap();
                to_bytes(response.into_body(), 4096).await.unwrap();
            }
        }
        .with_subscriber(subscriber.clone())
        .await;
        let logs = String::from_utf8(
            subscriber
                .downcast_ref::<Mutex<Vec<u8>>>()
                .unwrap()
                .lock()
                .unwrap()
                .clone(),
        )
        .unwrap();

        assert!(logs.contains("Operator event"));
        assert!(!logs.contains("private-body"), "{logs}");
        if level == Level::INFO {
            assert!(!logs.contains("private-"), "{logs}");
            assert!(!logs.contains("HTTP request completed"), "{logs}");
            assert!(!logs.contains("192.0.2.123"), "{logs}");
            assert!(!logs.contains("198.51.100.123"), "{logs}");
        } else {
            assert!(logs.contains("method=GET"), "{logs}");
            assert!(logs.contains("method=POST"), "{logs}");
            assert!(
                logs.contains("/private-navigation?query=private-query"),
                "{logs}"
            );
            assert!(logs.contains("/api/health?query=private-query"), "{logs}");
            assert!(
                logs.contains("/api/private-path?query=private-query"),
                "{logs}"
            );
            for status in [200, 404, 405, 429] {
                assert!(logs.contains(&format!("status={status}")), "{logs}");
            }
            assert!(logs.contains("duration_ms="), "{logs}");
            for duration in logs
                .split_whitespace()
                .filter_map(|field| field.strip_prefix("duration_ms="))
            {
                let (whole, fractional) = duration.split_once('.').unwrap();
                assert!(
                    whole.bytes().all(|byte| byte.is_ascii_digit()),
                    "{duration}"
                );
                assert_eq!(fractional.len(), 3, "{duration}");
                assert!(
                    fractional.bytes().all(|byte| byte.is_ascii_digit()),
                    "{duration}"
                );
            }
        }
        for detail in [
            "HTTP request metadata",
            "HTTP response metadata",
            "private-token",
            "private-cookie",
            "headers=",
        ] {
            assert_eq!(
                logs.contains(detail),
                level == Level::TRACE,
                "{level}: {logs}"
            );
        }
    }
}
