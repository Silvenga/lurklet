//! HTTP API routing and its internal OpenAPI contract.

mod agents;
mod app;
mod error;
mod middleware;
mod response_cache;
mod routes;
mod static_host;

pub use app::{agents_service, application, specification};
pub use middleware::{
    ApiCacheOptions, ClientIpSource, QueryPolicy, RateLimit, RateLimitOptions, RateLimits,
};
pub use response_cache::ResponseCache;
pub use static_host::StaticHost;
