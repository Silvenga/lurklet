//! HTTP API routing and its internal OpenAPI contract.

mod app;
mod error;
mod middleware;
mod response_cache;
mod routes;
mod static_host;

pub use app::application;
pub use middleware::{ClientIpSource, RateLimit, RateLimitOptions};
pub use response_cache::ResponseCache;
pub use static_host::StaticHost;
