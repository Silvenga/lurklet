//! Isolate tracing callsite state from concurrent router tests.

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, header};
use lurklet_api::application;
use std::sync::Mutex;
use tower::ServiceExt;
use tracing::Level;
use tracing::instrument::WithSubscriber;

#[tokio::test]
async fn when_requests_are_logged_then_details_should_follow_the_configured_level() {
    for level in [Level::INFO, Level::DEBUG, Level::TRACE] {
        let subscriber = tracing::Dispatch::new(
            tracing_subscriber::fmt()
                .with_max_level(level)
                .with_ansi(false)
                .with_writer(Mutex::new(Vec::<u8>::new()))
                .finish(),
        );

        async {
            let (app, _) = application();
            tracing::info!("Operator event");
            for (method, path) in [
                (Method::GET, "/api/health?query=private-query"),
                (Method::GET, "/api/private-path?query=private-query"),
                (Method::POST, "/api/health?query=private-query"),
            ] {
                let request = Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::AUTHORIZATION, "Bearer private-token")
                    .header(header::COOKIE, "session=private-cookie")
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
        } else {
            assert!(logs.contains("method=GET"), "{logs}");
            assert!(logs.contains("method=POST"), "{logs}");
            assert!(logs.contains("/api/health?query=private-query"), "{logs}");
            assert!(
                logs.contains("/api/private-path?query=private-query"),
                "{logs}"
            );
            for status in [200, 404, 405] {
                assert!(logs.contains(&format!("status={status}")), "{logs}");
            }
            assert!(logs.contains("duration_ms="), "{logs}");
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
