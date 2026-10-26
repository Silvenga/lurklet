use crate::error::ApiError;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use headers::{CacheControl, HeaderMapExt};
use tower::ServiceExt;
use tower_http::compression::Compression;

/// Negotiates streaming JSON compression while keeping negotiation errors in the API format.
pub async fn response_compression(request: Request, next: Next) -> Response {
    let response = Compression::new(next).oneshot(request).await.unwrap();
    if response.status() == StatusCode::NOT_ACCEPTABLE {
        let mut problem = ApiError::NotAcceptable.into_response();
        for value in response.headers().get_all(header::VARY) {
            problem.headers_mut().append(header::VARY, value.clone());
        }
        if response
            .headers()
            .typed_get::<CacheControl>()
            .is_some_and(|policy| policy.no_store())
        {
            problem
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
        if let Some(retry_after) = response.headers().get(header::RETRY_AFTER) {
            problem
                .headers_mut()
                .insert(header::RETRY_AFTER, retry_after.clone());
        }
        return problem;
    }
    response.map(Body::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResponseCache;
    use axum::Router;
    use axum::body::to_bytes;
    use axum::http::Method;
    use axum::middleware;
    use axum::routing::get;
    use brotli::Decompressor;
    use flate2::read::{GzDecoder, ZlibDecoder};
    use futures_util::stream;
    use serde_json::{Value, json};
    use std::fs;
    use std::io::{self, Read};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::Poll;
    use std::time::Duration;
    use tokio::time::timeout;
    use zstd::stream::decode_all;

    const JSON: &str = r#"{"message":"A sufficiently large JSON representation to exercise gzip and Brotli negotiation without depending on the minimum compression threshold."}"#;

    fn decode(bytes: &[u8], encoding: Option<&str>) -> Vec<u8> {
        let mut output = Vec::new();
        match encoding {
            Some("gzip") => {
                GzDecoder::new(bytes).read_to_end(&mut output).unwrap();
            }
            Some("br") => {
                Decompressor::new(bytes, 4096)
                    .read_to_end(&mut output)
                    .unwrap();
            }
            Some("deflate") => {
                ZlibDecoder::new(bytes).read_to_end(&mut output).unwrap();
            }
            Some("zstd") => output = decode_all(bytes).unwrap(),
            None => output.extend_from_slice(bytes),
            Some(other) => panic!("unexpected encoding {other}"),
        }
        output
    }

    fn request(encoding: Option<&str>) -> Request {
        let mut request = Request::builder().uri("/resource");
        if let Some(encoding) = encoding {
            request = request.header(header::ACCEPT_ENCODING, encoding);
        }
        request.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn when_json_is_requested_then_compression_should_negotiate_encodings_and_preserve_response_metadata()
     {
        let app = Router::new()
            .route(
                "/resource",
                get(|| async {
                    (
                        [
                            (header::CONTENT_TYPE, "application/json".to_owned()),
                            (header::CONTENT_LENGTH, JSON.len().to_string()),
                            (header::CACHE_CONTROL, "public, max-age=300".to_owned()),
                            (header::AGE, "17".to_owned()),
                            (header::VARY, "Origin".to_owned()),
                        ],
                        JSON,
                    )
                }),
            )
            .layer(middleware::from_fn(response_compression));

        for (accepted, expected) in [
            (None, None),
            (Some("identity"), None),
            (Some("gzip"), Some("gzip")),
            (Some("br"), Some("br")),
            (Some("gzip, br"), Some("br")),
            (Some("gzip;q=1, br;q=0.5"), Some("gzip")),
            (Some("gzip;q=0.5, br;q=1"), Some("br")),
            (Some("gzip;q=0, br;q=1"), Some("br")),
            (Some("gzip;q=1, br;q=0"), Some("gzip")),
            (Some("gzip;q=0, br;q=0"), None),
            (Some("deflate"), Some("deflate")),
            (Some("zstd"), Some("zstd")),
            (Some("deflate, zstd"), Some("zstd")),
            (Some("gzip, br, deflate, zstd"), Some("zstd")),
            (Some("zstd;q=0.5, deflate;q=1"), Some("deflate")),
            (Some("zstd;q=0, br;q=1"), Some("br")),
            (Some("deflate;q=0, gzip;q=1"), Some("gzip")),
            (Some("compress"), None),
        ] {
            let response = app.clone().oneshot(request(accepted)).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 16_384).await.unwrap();

            assert_eq!(status, StatusCode::OK, "{accepted:?}");
            assert_eq!(
                headers
                    .get(header::CONTENT_ENCODING)
                    .map(|value| value.to_str().unwrap()),
                expected
            );
            assert_eq!(decode(&body, expected), JSON.as_bytes());
            assert_eq!(headers[header::CONTENT_TYPE], "application/json");
            assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=300");
            assert_eq!(headers[header::AGE], "17");
            assert_eq!(
                headers.contains_key(header::CONTENT_LENGTH),
                expected.is_none()
            );
            let vary = headers
                .get_all(header::VARY)
                .iter()
                .flat_map(|value| value.to_str().unwrap().split(','))
                .map(str::trim)
                .collect::<Vec<_>>();
            assert!(vary.contains(&"Origin"));
            assert!(
                vary.iter()
                    .any(|value| value.eq_ignore_ascii_case("accept-encoding"))
            );
        }
    }

    #[tokio::test]
    async fn when_json_is_smaller_than_the_default_threshold_then_it_should_remain_uncompressed() {
        let app = Router::new()
            .route(
                "/resource",
                get(|| async { ([(header::CONTENT_TYPE, "application/json")], "{}") }),
            )
            .layer(middleware::from_fn(response_compression));

        let response = app.oneshot(request(Some("gzip, br"))).await.unwrap();

        assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            b"{}".as_slice()
        );
    }

    #[tokio::test]
    async fn when_an_encoding_is_unacceptable_then_problem_details_should_replace_the_original_body()
     {
        for policy in ["public, max-age=300", "no-store"] {
            let app = Router::new()
                .route(
                    "/resource",
                    get(move || async move {
                        (
                            [
                                (header::CONTENT_TYPE, "application/json"),
                                (header::CACHE_CONTROL, policy),
                                (header::VARY, "Origin"),
                                (header::RETRY_AFTER, "5"),
                            ],
                            JSON,
                        )
                    }),
                )
                .layer(middleware::from_fn(response_compression));

            let response = app
                .oneshot(request(Some(
                    "gzip;q=0, br;q=0, deflate;q=0, zstd;q=0, identity;q=0",
                )))
                .await
                .unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::NOT_ACCEPTABLE);
            assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
            assert_eq!(
                headers[header::CACHE_CONTROL],
                if policy == "no-store" {
                    "no-store"
                } else {
                    "public, max-age=10"
                }
            );
            assert_eq!(headers[header::RETRY_AFTER], "5");
            assert!(!headers.contains_key(header::CONTENT_ENCODING));
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                json!({"type": "about:blank", "title": "Not Acceptable", "status": 406})
            );
            assert!(headers.get_all(header::VARY).iter().any(|value| {
                value
                    .to_str()
                    .unwrap()
                    .eq_ignore_ascii_case("accept-encoding")
            }));
        }
    }

    #[tokio::test]
    async fn when_problem_details_are_compressed_then_status_cache_policy_and_retry_guidance_should_survive()
     {
        let app = Router::new()
            .route(
                "/resource",
                get(|| async { ([(header::RETRY_AFTER, "3")], ApiError::TooManyRequests) }),
            )
            .layer(middleware::from_fn(response_compression));

        for encoding in ["gzip", "br", "deflate", "zstd"] {
            let response = app.clone().oneshot(request(Some(encoding))).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(headers[header::CONTENT_ENCODING], encoding);
            assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
            assert_eq!(headers[header::CACHE_CONTROL], "no-store");
            assert_eq!(headers[header::RETRY_AFTER], "3");
            assert_eq!(
                serde_json::from_slice::<Value>(&decode(&body, Some(encoding))).unwrap(),
                json!({"type": "about:blank", "title": "Too Many Requests", "status": 429})
            );
        }
    }

    #[tokio::test]
    async fn when_clients_choose_different_encodings_then_the_cache_should_reuse_one_identity_representation()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let app = cache
            .wrap(Router::new().route(
                "/resource",
                get({
                    let calls = calls.clone();
                    move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async {
                            (
                                [
                                    (header::CONTENT_TYPE, "application/json"),
                                    (header::CACHE_CONTROL, "public, max-age=300"),
                                ],
                                JSON,
                            )
                        }
                    }
                }),
            ))
            .layer(middleware::from_fn(response_compression));

        for (encoding, outcome) in [
            (Some("gzip"), "miss"),
            (Some("br"), "hit"),
            (Some("deflate"), "hit"),
            (Some("zstd"), "hit"),
            (None, "hit"),
        ] {
            let response = app.clone().oneshot(request(encoding)).await.unwrap();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 16_384).await.unwrap();

            assert_eq!(
                headers
                    .get(header::CONTENT_ENCODING)
                    .map(|value| value.to_str().unwrap()),
                encoding
            );
            assert_eq!(headers["x-lurklet-cache"], outcome);
            assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=300");
            assert!(headers.contains_key(header::AGE));
            assert_eq!(decode(&body, encoding), JSON.as_bytes());
        }
        for encoding in ["gzip", "br", "deflate", "zstd"] {
            let mut head = request(Some(encoding));
            *head.method_mut() = Method::HEAD;

            let response = app.clone().oneshot(head).await.unwrap();

            assert_eq!(response.headers()[header::CONTENT_ENCODING], encoding);
            assert_eq!(response.headers()["x-lurklet-cache"], "hit");
            assert!(
                to_bytes(response.into_body(), 16_384)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn when_a_compressed_stream_is_dropped_then_it_should_not_drain_the_body_or_publish_partial_cache_content()
     {
        struct Dropped(Arc<AtomicUsize>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        for encoding in ["gzip", "br", "deflate", "zstd"] {
            let directory = assert_fs::TempDir::new().unwrap();
            let cache = ResponseCache::new(directory.path()).await.unwrap();
            let polled = Arc::new(AtomicUsize::new(0));
            let dropped = Arc::new(AtomicUsize::new(0));
            let app = cache
                .wrap(Router::new().route(
                    "/resource",
                    get({
                        let polled = polled.clone();
                        let dropped = dropped.clone();
                        move || {
                            let polled = polled.clone();
                            let guard = Dropped(dropped.clone());
                            async move {
                                let source = stream::poll_fn(move |_| {
                                    let _ = &guard;
                                    polled.fetch_add(1, Ordering::SeqCst);
                                    Poll::Pending::<Option<Result<bytes::Bytes, io::Error>>>
                                });
                                (
                                    [
                                        (header::CONTENT_TYPE, "application/json"),
                                        (header::CACHE_CONTROL, "public, max-age=300"),
                                    ],
                                    Body::from_stream(source),
                                )
                            }
                        }
                    }),
                ))
                .layer(middleware::from_fn(response_compression));

            for expected_drops in 1..=2 {
                let response = timeout(
                    Duration::from_secs(5),
                    app.clone().oneshot(request(Some(encoding))),
                )
                .await
                .unwrap()
                .unwrap();

                assert_eq!(response.headers()[header::CONTENT_ENCODING], encoding);
                assert_eq!(response.headers()["x-lurklet-cache"], "miss");
                assert_eq!(polled.load(Ordering::SeqCst), 0);
                drop(response);
                assert_eq!(dropped.load(Ordering::SeqCst), expected_drops);
                assert_eq!(
                    fs::read_dir(directory.path().join(".tmp")).unwrap().count(),
                    0
                );
            }
        }
    }
}
