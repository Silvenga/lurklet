use axum::http::{HeaderMap, header};
use mediatype::{MediaType, names};

pub fn is_persistable(headers: &HeaderMap) -> bool {
    !headers.contains_key(header::CONTENT_RANGE)
        && !headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| MediaType::parse(value).ok())
            .is_some_and(|media| media.ty == names::VIDEO && media.subty == names::MP4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResponseCache;
    use crate::response_cache::key::CacheKey;
    use crate::response_cache::store::Store;
    use assert_fs::TempDir;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use std::time::Duration;
    use tower::ServiceExt;

    #[tokio::test]
    async fn when_mp4s_are_already_cached_then_neither_fresh_hits_nor_stale_fallback_should_use_or_replace_them()
     {
        for lifetime in [Duration::ZERO, Duration::from_secs(3600)] {
            let directory = TempDir::new().unwrap();
            let cache = ResponseCache::new(directory.path()).await.unwrap();
            let store = Store::new(directory.path()).await.unwrap();
            let key = CacheKey::new(&"/media?url=opaque".parse().unwrap());
            let mut writer = store.begin(&key).await.unwrap();
            writer.write(b"cached").await.unwrap();
            let metadata = writer
                .publish(
                    HeaderMap::from_iter([
                        (
                            header::CONTENT_TYPE,
                            "VIDEO/MP4; codecs=avc1".parse().unwrap(),
                        ),
                        (
                            header::CACHE_CONTROL,
                            "public, max-age=3600".parse().unwrap(),
                        ),
                    ]),
                    lifetime,
                    6,
                )
                .await
                .unwrap();

            for status in [StatusCode::OK, StatusCode::BAD_GATEWAY] {
                let app = cache.wrap(Router::new().route(
                    "/media",
                    get(move || async move {
                        (
                            status,
                            [
                                (
                                    header::CONTENT_TYPE,
                                    if status == StatusCode::OK {
                                        "video/mp4"
                                    } else {
                                        "application/problem+json"
                                    },
                                ),
                                (header::CACHE_CONTROL, "public, max-age=42"),
                            ],
                            "upstream",
                        )
                    }),
                ));

                let response = app
                    .oneshot(
                        Request::builder()
                            .uri(key.uri())
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();

                assert_eq!(response.status(), status);
                assert_eq!(
                    response.headers()[header::CACHE_CONTROL],
                    "public, max-age=42"
                );
                assert_eq!(
                    to_bytes(response.into_body(), 4096).await.unwrap(),
                    b"upstream".as_slice()
                );
                assert_eq!(
                    store.read(&key).await.unwrap().metadata.content_file,
                    metadata.content_file
                );
            }
        }
    }

    #[tokio::test]
    async fn when_range_requests_encounter_a_cached_file_then_they_should_bypass_hits_and_stale_fallback_without_overwriting_it()
     {
        for lifetime in [Duration::ZERO, Duration::from_secs(3600)] {
            let directory = TempDir::new().unwrap();
            let cache = ResponseCache::new(directory.path()).await.unwrap();
            let store = Store::new(directory.path()).await.unwrap();
            let key = CacheKey::new(&"/media".parse().unwrap());
            let mut writer = store.begin(&key).await.unwrap();
            writer.write(b"cached").await.unwrap();
            let metadata = writer
                .publish(
                    HeaderMap::from_iter([
                        (header::CONTENT_TYPE, "image/png".parse().unwrap()),
                        (
                            header::CACHE_CONTROL,
                            "public, max-age=3600".parse().unwrap(),
                        ),
                    ]),
                    lifetime,
                    6,
                )
                .await
                .unwrap();

            for status in [
                StatusCode::OK,
                StatusCode::PARTIAL_CONTENT,
                StatusCode::RANGE_NOT_SATISFIABLE,
            ] {
                let app = cache.wrap(Router::new().route(
                    "/media",
                    get(move || async move {
                        (
                            status,
                            [
                                (header::CONTENT_TYPE, "image/png"),
                                (header::CACHE_CONTROL, "public, max-age=42"),
                            ],
                            "upstream",
                        )
                    }),
                ));

                let response = app
                    .oneshot(
                        Request::builder()
                            .uri("/media")
                            .header(header::RANGE, "bytes=0-3")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();

                assert_eq!(response.status(), status);
                assert!(!response.headers().contains_key("x-lurklet-cache"));
                assert_eq!(
                    to_bytes(response.into_body(), 4096).await.unwrap(),
                    b"upstream".as_slice()
                );
                assert_eq!(
                    store.read(&key).await.unwrap().metadata.content_file,
                    metadata.content_file
                );
            }
        }
    }
}
