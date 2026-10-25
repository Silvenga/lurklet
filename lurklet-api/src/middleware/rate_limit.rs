use crate::error::ApiError;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use clap::{Args, ValueEnum};
use governor::middleware::StateInformationMiddleware;
use ipnet::{IpNet, Ipv6Net};
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use tower_governor::GovernorError;
use tower_governor::governor::{Governor, GovernorConfig, GovernorConfigBuilder};
use tower_governor::key_extractor::KeyExtractor;

/// API quota and opt-in client-IP forwarding configuration.
#[derive(Args, Clone)]
pub struct RateLimitOptions {
    /// Sustained API requests per second per client; IPv6 clients share a /64.
    #[arg(long, env = "LURKLET_RATE_LIMIT_RPS", default_value = "1")]
    pub rate_limit_rps: NonZeroU32,
    /// Maximum burst of API requests per client.
    #[arg(long, env = "LURKLET_RATE_LIMIT_BURST", default_value = "10")]
    pub rate_limit_burst: NonZeroU32,
    /// Client-IP source. Header sources without trusted proxies trust every peer.
    /// X-Forwarded-For uses the rightmost address; multi-hop ingress must normalize it.
    #[arg(
        long,
        env = "LURKLET_CLIENT_IP_SOURCE",
        value_enum,
        default_value = "peer"
    )]
    pub client_ip_source: ClientIpSource,
    /// Only these socket peers may supply the selected header. Empty trusts all peers.
    #[arg(long, env = "LURKLET_TRUSTED_PROXIES", value_delimiter = ',')]
    pub trusted_proxies: Vec<IpNet>,
}

impl Default for RateLimitOptions {
    fn default() -> Self {
        Self {
            rate_limit_rps: NonZeroU32::new(1).unwrap(),
            rate_limit_burst: NonZeroU32::new(10).unwrap(),
            client_ip_source: ClientIpSource::Peer,
            trusted_proxies: Vec::new(),
        }
    }
}

/// Exactly one source is used; missing or malformed headers fall back to the peer.
#[derive(Clone, Copy, ValueEnum)]
pub enum ClientIpSource {
    Peer,
    XForwardedFor,
    XRealIp,
}

#[derive(Clone)]
struct ClientIpKey {
    source: ClientIpSource,
    trusted_proxies: Vec<IpNet>,
}

impl KeyExtractor for ClientIpKey {
    type Key = IpAddr;

    fn extract<T>(&self, request: &Request<T>) -> Result<IpAddr, GovernorError> {
        let peer = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .ok_or(GovernorError::UnableToExtractKey)?
            .0
            .ip()
            .to_canonical();
        let trusted = self.trusted_proxies.is_empty()
            || self
                .trusted_proxies
                .iter()
                .any(|network| network.contains(&peer));
        let ip = if trusted {
            match self.source {
                ClientIpSource::Peer => peer,
                ClientIpSource::XForwardedFor => {
                    client_ip::rightmost_x_forwarded_for(request.headers()).unwrap_or(peer)
                }
                ClientIpSource::XRealIp => client_ip::x_real_ip(request.headers()).unwrap_or(peer),
            }
        } else {
            peer
        };

        Ok(match ip.to_canonical() {
            IpAddr::V4(ip) => IpAddr::V4(ip),
            IpAddr::V6(ip) => IpAddr::V6(Ipv6Net::new(ip, 64).unwrap().network()),
        })
    }
}

/// Shared in-memory API limiter; cloning retains the same client budgets.
#[derive(Clone)]
pub struct RateLimit {
    config: Arc<GovernorConfig<ClientIpKey, StateInformationMiddleware>>,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self::new(RateLimitOptions::default())
    }
}

impl RateLimit {
    /// Builds a limiter without starting background tasks.
    pub fn new(options: RateLimitOptions) -> Self {
        let period = Duration::from_nanos(
            1_000_000_000_u64.div_ceil(u64::from(options.rate_limit_rps.get())),
        );
        let config = GovernorConfigBuilder::default()
            .period(period)
            .burst_size(options.rate_limit_burst.get())
            .key_extractor(ClientIpKey {
                source: options.client_ip_source,
                trusted_proxies: options.trusted_proxies,
            })
            .use_headers()
            .finish()
            .expect("rate limit period and burst are nonzero");

        Self {
            config: Arc::new(config),
        }
    }

    /// Removes entries whose quota has completely recovered.
    pub fn retain_recent(&self) {
        self.config.limiter().retain_recent();
    }
}

/// Applies the shared per-client quota to the wrapped routes.
pub async fn limit_requests(
    State(limiter): State<RateLimit>,
    request: Request,
    next: Next,
) -> Response {
    Governor::new(next, &limiter.config)
        .error_handler(|error| match error {
            GovernorError::TooManyRequests { wait_time, headers } => {
                let mut response = ApiError::TooManyRequests.into_response();
                if let Some(headers) = headers {
                    response.headers_mut().extend(headers);
                }
                // Governor truncates fractional seconds; round conservatively for clients.
                let retry_after = HeaderValue::from(wait_time.saturating_add(1));
                response
                    .headers_mut()
                    .insert(header::RETRY_AFTER, retry_after.clone());
                response
                    .headers_mut()
                    .insert("x-ratelimit-after", retry_after);
                response
            }
            GovernorError::UnableToExtractKey | GovernorError::Other { .. } => {
                ApiError::InternalServerError.into_response()
            }
        })
        .oneshot(request)
        .await
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, StatusCode};
    use axum::middleware;
    use axum::routing::get;
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::task::JoinSet;
    use tokio::time::sleep;

    fn request(path: &str, peer: &str) -> Request<Body> {
        Request::builder()
            .uri(path)
            .extension(ConnectInfo(peer.parse::<SocketAddr>().unwrap()))
            .body(Body::empty())
            .unwrap()
    }

    fn limiter(burst: u32) -> RateLimit {
        RateLimit::new(RateLimitOptions {
            rate_limit_burst: burst.try_into().unwrap(),
            ..RateLimitOptions::default()
        })
    }

    fn app(limiter: &RateLimit) -> Router {
        Router::new()
            .fallback(|| async { StatusCode::NOT_FOUND })
            .layer(middleware::from_fn_with_state(
                limiter.clone(),
                limit_requests,
            ))
    }

    #[test]
    fn when_a_rate_is_configured_then_quota_should_replenish_at_that_rate_independently_of_burst() {
        let limiter = RateLimit::new(RateLimitOptions {
            rate_limit_rps: 4.try_into().unwrap(),
            rate_limit_burst: 20.try_into().unwrap(),
            ..RateLimitOptions::default()
        });

        let snapshot = limiter
            .config
            .limiter()
            .check_key(&"192.0.2.1".parse().unwrap())
            .unwrap();

        assert_eq!(
            snapshot.quota().replenish_interval(),
            Duration::from_millis(250)
        );
        assert_eq!(snapshot.quota().burst_size().get(), 20);
        assert_eq!(snapshot.remaining_burst_capacity(), 19);
    }

    #[tokio::test]
    async fn when_a_method_is_unsupported_then_it_should_still_consume_the_shared_quota() {
        let app = Router::new()
            .route("/resource", get(|| async { "resource" }))
            .layer(middleware::from_fn_with_state(limiter(1), limit_requests));
        let mut unsupported = request("/resource", "192.0.2.1:1234");
        *unsupported.method_mut() = Method::POST;

        let first = app.clone().oneshot(unsupported).await.unwrap();
        let second = app
            .oneshot(request("/resource", "192.0.2.1:1234"))
            .await
            .unwrap();

        assert_eq!(first.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn when_one_ip_uses_its_burst_then_other_paths_and_ports_should_share_the_budget() {
        let app = app(&limiter(2));
        let first = app
            .clone()
            .oneshot(request("/api/one", "192.0.2.1:1234"))
            .await
            .unwrap();
        let second = app
            .clone()
            .oneshot(request("/api/two", "192.0.2.1:5678"))
            .await
            .unwrap();

        let rejected = app
            .clone()
            .oneshot(request("/api/three?different=query", "192.0.2.1:9999"))
            .await
            .unwrap();
        let other = app
            .oneshot(request("/api/one", "192.0.2.2:1234"))
            .await
            .unwrap();
        let status = rejected.status();
        let headers = rejected.headers().clone();
        let body = to_bytes(rejected.into_body(), 4096).await.unwrap();

        assert_eq!(first.status(), StatusCode::NOT_FOUND);
        assert_eq!(first.headers()["x-ratelimit-limit"], "2");
        assert_eq!(first.headers()["x-ratelimit-remaining"], "1");
        assert_eq!(second.status(), StatusCode::NOT_FOUND);
        assert_eq!(second.headers()["x-ratelimit-remaining"], "0");
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(headers[header::RETRY_AFTER], "1");
        assert_eq!(headers["x-ratelimit-after"], headers[header::RETRY_AFTER]);
        assert_eq!(headers["x-ratelimit-remaining"], "0");
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({
                "type": "about:blank", "title": "Too Many Requests", "status": 429
            })
        );
        assert_eq!(other.status(), StatusCode::NOT_FOUND);
        assert_eq!(other.headers()["x-ratelimit-remaining"], "1");
    }

    #[tokio::test]
    async fn when_requests_are_concurrent_then_cloned_routers_should_share_one_burst() {
        let app = app(&limiter(10));
        let mut requests = JoinSet::new();

        for _ in 0..32 {
            let app = app.clone();
            requests.spawn(async move {
                app.oneshot(request("/api/missing", "192.0.2.1:1234"))
                    .await
                    .unwrap()
                    .status()
            });
        }
        let mut accepted = 0;
        let mut rejected = 0;
        while let Some(result) = requests.join_next().await {
            match result.unwrap() {
                StatusCode::NOT_FOUND => accepted += 1,
                StatusCode::TOO_MANY_REQUESTS => rejected += 1,
                status => panic!("unexpected status: {status}"),
            }
        }

        assert_eq!((accepted, rejected), (10, 22));
    }

    #[tokio::test]
    async fn when_quota_recovers_then_cleanup_should_remove_only_recovered_entries() {
        let limiter = limiter(1);
        let app = app(&limiter);
        let first = app
            .clone()
            .oneshot(request("/api/missing", "192.0.2.1:1234"))
            .await
            .unwrap();

        limiter.retain_recent();
        let active = limiter.config.limiter().len();
        let rejected = app
            .clone()
            .oneshot(request("/api/missing", "192.0.2.1:1234"))
            .await
            .unwrap();
        sleep(Duration::from_millis(2100)).await;
        limiter.retain_recent();
        let recovered = limiter.config.limiter().len();
        let next = app
            .oneshot(request("/api/missing", "192.0.2.1:1234"))
            .await
            .unwrap();

        assert_eq!(first.status(), StatusCode::NOT_FOUND);
        assert_eq!(active, 1);
        assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(recovered, 0);
        assert_eq!(next.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn when_limit_is_exceeded_then_the_handler_should_not_execute_even_for_cache_hits() {
        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route(
                "/cached",
                get({
                    let calls = calls.clone();
                    move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async { "cached response" }
                    }
                }),
            )
            .layer(middleware::from_fn_with_state(limiter(1), limit_requests));

        let accepted = app
            .clone()
            .oneshot(request("/cached", "192.0.2.1:1234"))
            .await
            .unwrap();
        let rejected = app
            .oneshot(request("/cached", "192.0.2.1:1234"))
            .await
            .unwrap();

        assert_eq!(accepted.status(), StatusCode::OK);
        assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn when_forwarding_is_configured_then_only_the_selected_source_and_trusted_peers_should_be_used()
     {
        for (source, networks, peer, headers, expected) in [
            (
                ClientIpSource::Peer,
                vec![],
                "192.0.2.1:1234",
                vec![
                    ("x-forwarded-for", "198.51.100.1"),
                    ("x-real-ip", "198.51.100.2"),
                ],
                "192.0.2.1",
            ),
            (
                ClientIpSource::XForwardedFor,
                vec![],
                "192.0.2.1:1234",
                vec![
                    ("x-forwarded-for", "203.0.113.1, 198.51.100.1"),
                    ("x-forwarded-for", "198.51.100.2, 198.51.100.3"),
                ],
                "198.51.100.3",
            ),
            (
                ClientIpSource::XRealIp,
                vec![],
                "192.0.2.1:1234",
                vec![
                    ("x-real-ip", "198.51.100.2"),
                    ("x-forwarded-for", "198.51.100.1"),
                ],
                "198.51.100.2",
            ),
            (
                ClientIpSource::XRealIp,
                vec!["192.0.2.0/24"],
                "192.0.2.1:1234",
                vec![("x-real-ip", "198.51.100.1")],
                "198.51.100.1",
            ),
            (
                ClientIpSource::XRealIp,
                vec!["192.0.2.0/24"],
                "203.0.113.1:1234",
                vec![("x-real-ip", "198.51.100.1")],
                "203.0.113.1",
            ),
            (
                ClientIpSource::XRealIp,
                vec!["192.0.2.0/24"],
                "[::ffff:192.0.2.1]:1234",
                vec![("x-real-ip", "::ffff:198.51.100.1")],
                "198.51.100.1",
            ),
            (
                ClientIpSource::XForwardedFor,
                vec![],
                "192.0.2.1:1234",
                vec![],
                "192.0.2.1",
            ),
            (
                ClientIpSource::XForwardedFor,
                vec![],
                "192.0.2.1:1234",
                vec![
                    ("x-forwarded-for", "198.51.100.1, invalid"),
                    ("x-real-ip", "198.51.100.2"),
                ],
                "192.0.2.1",
            ),
            (
                ClientIpSource::XRealIp,
                vec![],
                "192.0.2.1:1234",
                vec![("x-real-ip", "198.51.100.1"), ("x-real-ip", "198.51.100.2")],
                "192.0.2.1",
            ),
            (
                ClientIpSource::Peer,
                vec![],
                "[2001:db8:1:2::1234]:1234",
                vec![],
                "2001:db8:1:2::",
            ),
            (
                ClientIpSource::XRealIp,
                vec!["2001:db8::/32"],
                "[2001:db8:1:2::1234]:1234",
                vec![("x-real-ip", "2001:db8:2:3::abcd")],
                "2001:db8:2:3::",
            ),
        ] {
            let extractor = ClientIpKey {
                source,
                trusted_proxies: networks
                    .iter()
                    .map(|network| network.parse().unwrap())
                    .collect(),
            };
            let mut request =
                Request::builder().extension(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
            for (name, value) in headers {
                request = request.header(name, value);
            }

            let actual = extractor.extract(&request.body(()).unwrap()).unwrap();

            assert_eq!(actual, expected.parse::<IpAddr>().unwrap());
        }
    }

    #[test]
    fn when_the_selected_header_is_not_ascii_then_extraction_should_fall_back_to_the_peer() {
        let extractor = ClientIpKey {
            source: ClientIpSource::XForwardedFor,
            trusted_proxies: Vec::new(),
        };
        let mut request = request("/api/missing", "192.0.2.1:1234");
        request
            .headers_mut()
            .insert("x-forwarded-for", HeaderValue::from_bytes(b"\x80").unwrap());
        request
            .headers_mut()
            .insert("x-real-ip", HeaderValue::from_static("198.51.100.1"));

        let ip = extractor.extract(&request).unwrap();

        assert_eq!(ip, "192.0.2.1".parse::<IpAddr>().unwrap());
    }

    #[tokio::test]
    async fn when_addresses_are_equivalent_then_they_should_share_a_budget() {
        for (first, equivalent, different) in [
            (
                "192.0.2.1:1234",
                "[::ffff:192.0.2.1]:5678",
                "192.0.2.2:1234",
            ),
            (
                "[2001:db8:1:2::1]:1234",
                "[2001:db8:1:2::ffff]:5678",
                "[2001:db8:1:3::1]:1234",
            ),
        ] {
            let app = app(&limiter(1));
            let _ = app
                .clone()
                .oneshot(request("/api/missing", first))
                .await
                .unwrap();

            let shared = app
                .clone()
                .oneshot(request("/api/missing", equivalent))
                .await
                .unwrap();
            let separate = app
                .oneshot(request("/api/missing", different))
                .await
                .unwrap();

            assert_eq!(shared.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(separate.status(), StatusCode::NOT_FOUND);
        }
    }

    #[tokio::test]
    async fn when_peer_metadata_is_missing_then_the_limiter_should_fail_closed_with_problem_details()
     {
        let app = app(&limiter(1));
        let request = Request::builder()
            .uri("/api/missing")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
}
