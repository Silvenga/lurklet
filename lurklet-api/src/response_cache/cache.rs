use crate::error::ApiError;
use crate::response_cache::entry::Entry;
use crate::response_cache::key::CacheKey;
use crate::response_cache::store::Store;
use axum::Router;
use axum::body::to_bytes;
use axum::extract::{OriginalUri, Request, State};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use headers::{CacheControl, HeaderMapExt};
use std::path::Path;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex as AsyncMutex;
use weak_table::WeakValueHashMap;

type RefreshLocks = WeakValueHashMap<String, Weak<AsyncMutex<()>>>;

/// Optional persistent API cache with process-local, asynchronous refresh locking.
#[derive(Clone)]
pub struct ResponseCache {
    store: Store,
    locks: Arc<Mutex<RefreshLocks>>,
}

impl ResponseCache {
    /// Initializes writable cache storage without fetching or scanning existing entries.
    pub async fn new(directory: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(Self {
            store: Store::new(directory).await?,
            locks: Arc::new(Mutex::new(WeakValueHashMap::new())),
        })
    }

    /// Caches routed responses while leaving final HEAD handling to Axum.
    pub fn wrap(&self, api: Router) -> Router {
        api.layer(middleware::from_fn_with_state(self.clone(), Self::serve))
    }

    async fn serve(State(cache): State<Self>, request: Request, next: Next) -> Response {
        if !matches!(*request.method(), Method::GET | Method::HEAD) {
            return next.run(request).await;
        }
        let original = request
            .extensions()
            .get::<OriginalUri>()
            .map_or(request.uri(), |uri| &uri.0);
        let key = CacheKey::new(original);

        if let Some(entry) = cache.store.read(&key).await.filter(Entry::is_fresh) {
            return finish(entry.into_response(false), "hit");
        }

        let lock = {
            let mut locks = cache.locks.lock().unwrap();
            locks
                .entry(key.digest().to_owned())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        };
        let _guard = lock.lock().await;
        let previous = match cache.store.read(&key).await {
            Some(entry) if entry.is_fresh() => return finish(entry.into_response(false), "hit"),
            previous => previous,
        };

        let response = next.run(request).await;
        if response.status() != StatusCode::OK {
            return match previous {
                Some(entry) => finish(entry.into_response(true), "stale"),
                None => finish(response, "miss"),
            };
        }

        let policy = response.headers().typed_get::<CacheControl>();
        let lifetime = policy
            .as_ref()
            .filter(|policy| !policy.no_store() && !policy.private())
            .and_then(|policy| {
                policy.s_max_age().or_else(|| policy.max_age()).map(|ttl| {
                    if policy.no_cache() {
                        Duration::ZERO
                    } else {
                        ttl
                    }
                })
            });
        let Some(fresh_for) = lifetime else {
            return finish(response, "miss");
        };

        let (parts, body) = response.into_parts();
        let body = match to_bytes(body, usize::MAX).await {
            Ok(body) => body,
            Err(error) => {
                tracing::debug!(?error, "Unable to collect a cacheable response body");
                return match previous {
                    Some(entry) => finish(entry.into_response(true), "stale"),
                    None => finish(ApiError::InternalServerError.into_response(), "miss"),
                };
            }
        };
        let stored_at = SystemTime::now();
        let entry = Entry {
            version: 1,
            key: key.uri().to_string(),
            stored_at,
            fresh_for,
            initial_age: Duration::ZERO,
            headers: parts.headers,
            body,
        };
        if let Err(error) = cache.store.write(&key, entry.clone()).await {
            tracing::warn!("Unable to write a response cache entry; returning the fresh response");
            tracing::debug!(?error, "Response cache write failure details");
        }
        finish(entry.into_response(false), "miss")
    }
}

fn finish(mut response: Response, outcome: &'static str) -> Response {
    response
        .headers_mut()
        .insert("x-lurklet-cache", HeaderValue::from_static(outcome));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use axum::body::Body;
    use axum::http::{HeaderMap, header};
    use axum::routing::get;
    use bytes::Bytes;
    use futures_util::stream;
    use std::fs;
    use std::future::{Future, pending, poll_fn};
    use std::io;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::Poll;
    use tokio::sync::Notify;
    use tokio::task::{JoinSet, yield_now};
    use tokio::time::{sleep, timeout};
    use tower::ServiceExt;

    fn request(uri: &str) -> Request {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    async fn seed(cache: &ResponseCache, uri: &str, age: Duration) -> Entry {
        let key = CacheKey::new(&uri.parse().unwrap());
        let entry = Entry {
            version: 1,
            key: key.uri().to_string(),
            stored_at: SystemTime::now() - age,
            fresh_for: Duration::from_secs(60),
            initial_age: Duration::ZERO,
            headers: HeaderMap::from_iter([
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=60"),
                ),
            ]),
            body: Bytes::from_static(b"old content"),
        };
        cache.store.write(&key, entry.clone()).await.unwrap();
        entry
    }

    #[tokio::test]
    async fn when_equivalent_queries_are_requested_then_they_should_share_an_entry_without_rewriting_the_request()
     {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let app = cache.wrap(Router::new().route(
            "/resource",
            get({
                let calls = Arc::clone(&calls);
                move |request: Request| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    async move {
                        assert_eq!(request.uri(), "/resource?sort=new&q=a%20b");
                        (
                            [
                                (header::CACHE_CONTROL, "public, max-age=3600"),
                                (header::CONTENT_TYPE, "application/json"),
                            ],
                            "fresh",
                        )
                    }
                }
            }),
        ));

        let miss = app
            .clone()
            .oneshot(request("/resource?sort=new&q=a%20b"))
            .await
            .unwrap();
        let hit = app
            .oneshot(request("/resource?q=a+b&sort=%6Eew"))
            .await
            .unwrap();

        assert_eq!(miss.headers()["x-lurklet-cache"], "miss");
        assert_eq!(hit.headers()["x-lurklet-cache"], "hit");
        assert_eq!(hit.headers()[header::CONTENT_TYPE], "application/json");
        assert_eq!(
            to_bytes(hit.into_body(), 4096).await.unwrap(),
            b"fresh".as_slice()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn when_a_head_request_refreshes_then_get_should_receive_the_complete_cached_body() {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let app = cache.wrap(Router::new().route(
            "/resource",
            get({
                let calls = Arc::clone(&calls);
                move |request: Request| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    async move {
                        assert_eq!(request.method(), Method::HEAD);
                        (
                            [(header::CACHE_CONTROL, "public, max-age=3600")],
                            "complete",
                        )
                    }
                }
            }),
        ));
        let mut head_request = request("/resource");
        *head_request.method_mut() = Method::HEAD;

        let head = app.clone().oneshot(head_request).await.unwrap();
        let get = app.clone().oneshot(request("/resource")).await.unwrap();
        let mut head_request = request("/resource");
        *head_request.method_mut() = Method::HEAD;
        let cached_head = app.oneshot(head_request).await.unwrap();

        assert_eq!(head.headers()["x-lurklet-cache"], "miss");
        assert_eq!(head.headers()[header::CONTENT_LENGTH], "8");
        assert!(to_bytes(head.into_body(), 4096).await.unwrap().is_empty());
        assert_eq!(get.headers()["x-lurklet-cache"], "hit");
        assert_eq!(
            to_bytes(get.into_body(), 4096).await.unwrap(),
            b"complete".as_slice()
        );
        assert_eq!(cached_head.headers()["x-lurklet-cache"], "hit");
        assert_eq!(cached_head.headers()[header::CONTENT_LENGTH], "8");
        assert!(
            to_bytes(cached_head.into_body(), 4096)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn when_an_entry_expires_then_a_success_should_replace_it_before_responding() {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let old = seed(&cache, "/resource", Duration::from_secs(120)).await;
        let app = cache.wrap(Router::new().route(
            "/resource",
            get(|| async {
                (
                    [(header::CACHE_CONTROL, "public, max-age=3600")],
                    "replacement",
                )
            }),
        ));

        let response = app.oneshot(request("/resource")).await.unwrap();
        let key = CacheKey::new(&"/resource".parse().unwrap());
        let stored = cache.store.read(&key).await.unwrap();

        assert_eq!(response.headers()["x-lurklet-cache"], "miss");
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            b"replacement".as_slice()
        );
        assert_eq!(stored.body, b"replacement".as_slice());
        assert!(stored.stored_at > old.stored_at);
        assert!(stored.is_fresh());
    }

    #[tokio::test]
    async fn when_refresh_returns_non_200_then_stale_content_should_be_served_without_refreshing_its_age()
     {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let old = seed(&cache, "/resource", Duration::from_secs(365 * 24 * 3600)).await;
        let calls = Arc::new(AtomicUsize::new(0));

        for status in [
            StatusCode::NO_CONTENT,
            StatusCode::MOVED_PERMANENTLY,
            StatusCode::NOT_MODIFIED,
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::GATEWAY_TIMEOUT,
        ] {
            let app = cache.wrap(Router::new().route(
                "/resource",
                get({
                    let calls = Arc::clone(&calls);
                    move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async move { status }
                    }
                }),
            ));
            for _ in 0..2 {
                let response = app.clone().oneshot(request("/resource")).await.unwrap();

                assert_eq!(response.status(), StatusCode::OK, "{status}");
                assert_eq!(response.headers()["x-lurklet-cache"], "stale");
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                assert_eq!(
                    to_bytes(response.into_body(), 4096).await.unwrap(),
                    b"old content".as_slice()
                );
            }
        }
        let key = CacheKey::new(&"/resource".parse().unwrap());
        assert_eq!(
            cache.store.read(&key).await.unwrap().stored_at,
            old.stored_at
        );
        assert_eq!(calls.load(Ordering::SeqCst), 22);
    }

    #[tokio::test]
    async fn when_no_entry_exists_then_an_error_should_be_returned_and_not_stored() {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let app = cache.wrap(Router::new().route(
            "/resource",
            get(|| async { (StatusCode::NOT_FOUND, "missing") }),
        ));

        let response = app.oneshot(request("/resource")).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()["x-lurklet-cache"], "miss");
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            b"missing".as_slice()
        );
        assert!(
            cache
                .store
                .read(&CacheKey::new(&"/resource".parse().unwrap()))
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn when_the_body_is_incomplete_then_the_previous_entry_should_not_be_replaced() {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let old = seed(&cache, "/resource", Duration::from_secs(120)).await;
        let app = cache.wrap(Router::new().route(
            "/resource",
            get(|| async {
                let stream = stream::iter([
                    Ok(Bytes::from_static(b"partial")),
                    Err(io::Error::other("backend read failed")),
                ]);
                (
                    [(header::CACHE_CONTROL, "public, max-age=60")],
                    Body::from_stream(stream),
                )
            }),
        ));

        let response = app.oneshot(request("/resource")).await.unwrap();

        assert_eq!(response.headers()["x-lurklet-cache"], "stale");
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            old.body
        );
        assert_eq!(
            cache
                .store
                .read(&CacheKey::new(&"/resource".parse().unwrap()))
                .await
                .unwrap()
                .stored_at,
            old.stored_at
        );
    }

    #[tokio::test]
    async fn when_a_write_fails_then_the_fresh_response_should_still_be_returned() {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        seed(&cache, "/resource", Duration::from_secs(120)).await;
        fs::rename(
            directory.path().join(".tmp"),
            directory.path().join("unavailable"),
        )
        .unwrap();
        let app = cache.wrap(Router::new().route(
            "/resource",
            get(|| async { ([(header::CACHE_CONTROL, "public, max-age=60")], "fresh") }),
        ));

        let response = app.oneshot(request("/resource")).await.unwrap();

        assert_eq!(response.headers()["x-lurklet-cache"], "miss");
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            b"fresh".as_slice()
        );
        assert_eq!(
            cache
                .store
                .read(&CacheKey::new(&"/resource".parse().unwrap()))
                .await
                .unwrap()
                .body,
            b"old content".as_slice()
        );
    }

    #[tokio::test]
    async fn when_fresh_content_is_served_then_age_and_custom_headers_should_be_preserved() {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let mut entry = seed(&cache, "/resource", Duration::from_secs(20)).await;
        entry
            .headers
            .append("x-custom", HeaderValue::from_static("first"));
        entry
            .headers
            .append("x-custom", HeaderValue::from_static("second"));
        cache
            .store
            .write(&CacheKey::new(&"/resource".parse().unwrap()), entry)
            .await
            .unwrap();
        let app = cache.wrap(Router::new().route(
            "/resource",
            get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
        ));

        let response = app.oneshot(request("/resource")).await.unwrap();

        assert_eq!(response.headers()["x-lurklet-cache"], "hit");
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=60"
        );
        assert!(
            response.headers()[header::AGE]
                .to_str()
                .unwrap()
                .parse::<u64>()
                .unwrap()
                >= 20
        );
        assert_eq!(
            response
                .headers()
                .get_all("x-custom")
                .iter()
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
    }

    #[tokio::test]
    async fn when_a_response_is_not_shared_cacheable_then_it_should_not_be_stored() {
        for policy in [
            "",
            "no-store",
            "private, max-age=60",
            "public",
            "max-age=invalid",
        ] {
            let directory = TempDir::new().unwrap();
            let cache = ResponseCache::new(directory.path()).await.unwrap();
            let app = cache.wrap(Router::new().route(
                "/resource",
                get(move || async move { ([(header::CACHE_CONTROL, policy)], "uncached") }),
            ));

            let response = app.oneshot(request("/resource")).await.unwrap();

            assert_eq!(response.headers()["x-lurklet-cache"], "miss", "{policy}");
            assert!(
                cache
                    .store
                    .read(&CacheKey::new(&"/resource".parse().unwrap()))
                    .await
                    .is_none(),
                "{policy}"
            );
        }
    }

    #[tokio::test]
    async fn when_a_handler_declares_a_cacheable_response_then_its_headers_should_be_preserved() {
        for name in [header::SET_COOKIE, header::VARY, header::CONNECTION] {
            let directory = TempDir::new().unwrap();
            let cache = ResponseCache::new(directory.path()).await.unwrap();
            let app = cache.wrap(Router::new().route(
                "/resource",
                get(move || {
                    let name = name.clone();
                    async move {
                        (
                            [
                                (header::CACHE_CONTROL, "public, max-age=60"),
                                (name, "example"),
                            ],
                            "cached",
                        )
                    }
                }),
            ));

            let response = app.clone().oneshot(request("/resource")).await.unwrap();
            let hit = app.oneshot(request("/resource")).await.unwrap();

            assert_eq!(response.headers()["x-lurklet-cache"], "miss");
            assert_eq!(hit.headers()["x-lurklet-cache"], "hit");
            assert_eq!(hit.headers()[header::CACHE_CONTROL], "public, max-age=60");
            for (name, value) in response.headers() {
                if name != "x-lurklet-cache" && name != header::AGE {
                    assert_eq!(hit.headers().get(name), Some(value));
                }
            }
        }
    }

    #[tokio::test]
    async fn when_a_fetch_takes_longer_than_the_ttl_then_the_completed_response_should_start_fresh()
    {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let app = cache.wrap(Router::new().route(
            "/resource",
            get({
                let calls = Arc::clone(&calls);
                move || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    async {
                        sleep(Duration::from_millis(1100)).await;
                        ([(header::CACHE_CONTROL, "public, max-age=1")], "fresh")
                    }
                }
            }),
        ));

        let fresh = app.clone().oneshot(request("/resource")).await.unwrap();
        let hit = app.oneshot(request("/resource")).await.unwrap();

        assert_eq!(fresh.headers()["x-lurklet-cache"], "miss");
        assert_eq!(hit.headers()["x-lurklet-cache"], "hit");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn when_a_timestamp_is_ahead_of_the_local_clock_then_the_entry_should_remain_available_for_stale_fallback()
     {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let mut entry = seed(&cache, "/resource", Duration::ZERO).await;
        entry.stored_at = SystemTime::now() + Duration::from_secs(3600);
        cache
            .store
            .write(&CacheKey::new(&"/resource".parse().unwrap()), entry)
            .await
            .unwrap();
        let app =
            cache.wrap(Router::new().route("/resource", get(|| async { StatusCode::NOT_FOUND })));

        let response = app.oneshot(request("/resource")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-lurklet-cache"], "stale");
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            b"old content".as_slice()
        );
    }

    #[tokio::test]
    async fn when_requests_share_a_key_then_waiters_should_read_the_published_entry() {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let app = cache.wrap(Router::new().route(
            "/resource",
            get({
                let calls = Arc::clone(&calls);
                let entered = Arc::clone(&entered);
                let release = Arc::clone(&release);
                move || {
                    let entered = Arc::clone(&entered);
                    let release = Arc::clone(&release);
                    calls.fetch_add(1, Ordering::SeqCst);
                    async move {
                        entered.notify_one();
                        release.notified().await;
                        ([(header::CACHE_CONTROL, "public, max-age=3600")], "fresh")
                    }
                }
            }),
        ));
        let mut requests = JoinSet::new();
        for _ in 0..16 {
            requests.spawn(app.clone().oneshot(request("/resource")));
        }
        timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();

        release.notify_one();
        let mut misses = 0;
        while let Some(result) = timeout(Duration::from_secs(5), requests.join_next())
            .await
            .unwrap()
        {
            let response = result.unwrap().unwrap();
            misses += usize::from(response.headers()["x-lurklet-cache"] == "miss");
            assert_eq!(
                to_bytes(response.into_body(), 4096).await.unwrap(),
                b"fresh".as_slice()
            );
        }

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(misses, 1);
    }

    #[tokio::test]
    async fn when_a_refresh_is_cancelled_then_other_keys_and_subsequent_requests_should_continue() {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let entered = Arc::new(Notify::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let app = cache.wrap(
            Router::new()
                .route(
                    "/resource",
                    get({
                        let entered = Arc::clone(&entered);
                        let calls = Arc::clone(&calls);
                        move || {
                            let first = calls.fetch_add(1, Ordering::SeqCst) == 0;
                            let entered = Arc::clone(&entered);
                            async move {
                                if first {
                                    entered.notify_one();
                                    pending::<()>().await;
                                }
                                ([(header::CACHE_CONTROL, "public, max-age=60")], "fresh")
                            }
                        }
                    }),
                )
                .route(
                    "/other",
                    get(|| async { ([(header::CACHE_CONTROL, "public, max-age=60")], "other") }),
                ),
        );
        let blocked = tokio::spawn(app.clone().oneshot(request("/resource")));
        timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();

        let other = timeout(
            Duration::from_secs(5),
            app.clone().oneshot(request("/other")),
        )
        .await
        .unwrap()
        .unwrap();
        blocked.abort();
        assert!(blocked.await.unwrap_err().is_cancelled());
        let response = timeout(Duration::from_secs(5), app.oneshot(request("/resource")))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(other.status(), StatusCode::OK);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(
            cache
                .locks
                .lock()
                .unwrap()
                .get(CacheKey::new(&"/resource".parse().unwrap()).digest())
                .is_none()
        );
    }

    #[tokio::test]
    async fn when_waiters_are_cancelled_then_weak_lock_entries_should_not_accumulate_indefinitely()
    {
        let mut locks = RefreshLocks::new();
        let shared = locks
            .entry("key".to_owned())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())));
        let owner = Arc::clone(&shared).lock_owned().await;
        let mut waiter = Box::pin(Arc::clone(&shared).lock_owned());
        assert!(poll_fn(|cx| Poll::Ready(waiter.as_mut().poll(cx).is_pending())).await);

        drop(shared);
        drop(owner);
        drop(waiter);
        for key in 0..10_000 {
            locks.insert(key.to_string(), Arc::new(AsyncMutex::new(())));
        }

        assert!(locks.get("key").is_none());
        assert_eq!(locks.iter().count(), 0);
        assert!(locks.capacity() < 256, "{}", locks.capacity());
    }

    #[tokio::test]
    async fn when_refreshes_fail_then_waiting_requests_should_retry_serially() {
        let directory = TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        seed(&cache, "/resource", Duration::from_secs(120)).await;
        let active = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let app = cache.wrap(Router::new().route(
            "/resource",
            get({
                let active = Arc::clone(&active);
                let calls = Arc::clone(&calls);
                move || {
                    let active = Arc::clone(&active);
                    calls.fetch_add(1, Ordering::SeqCst);
                    async move {
                        assert_eq!(active.fetch_add(1, Ordering::SeqCst), 0);
                        yield_now().await;
                        assert_eq!(active.fetch_sub(1, Ordering::SeqCst), 1);
                        StatusCode::NOT_FOUND
                    }
                }
            }),
        ));
        let mut requests = JoinSet::new();

        for _ in 0..8 {
            requests.spawn(app.clone().oneshot(request("/resource")));
        }
        while let Some(response) = requests.join_next().await {
            assert_eq!(
                response.unwrap().unwrap().headers()["x-lurklet-cache"],
                "stale"
            );
        }

        assert_eq!(calls.load(Ordering::SeqCst), 8);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn when_response_policy_requires_refresh_then_cache_reads_should_not_extend_freshness() {
        for policy in [
            "public, max-age=0",
            "public, max-age=3600, s-maxage=0",
            "no-cache, max-age=3600",
        ] {
            let directory = TempDir::new().unwrap();
            let cache = ResponseCache::new(directory.path()).await.unwrap();
            let calls = Arc::new(AtomicUsize::new(0));
            let app = cache.wrap(Router::new().route(
                "/resource",
                get({
                    let calls = Arc::clone(&calls);
                    move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async move { ([(header::CACHE_CONTROL, policy)], "fresh") }
                    }
                }),
            ));

            for _ in 0..2 {
                let response = app.clone().oneshot(request("/resource")).await.unwrap();
                assert_eq!(response.headers()["x-lurklet-cache"], "miss", "{policy}");
            }

            assert_eq!(calls.load(Ordering::SeqCst), 2);
            assert!(
                cache
                    .store
                    .read(&CacheKey::new(&"/resource".parse().unwrap()))
                    .await
                    .is_some()
            );
        }
    }
}
