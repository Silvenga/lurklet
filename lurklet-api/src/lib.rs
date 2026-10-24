//! HTTP API routing and its internal OpenAPI contract.

mod app;
mod error;
mod middleware;
mod routes;
mod static_host;

pub use app::application;
pub use middleware::{ClientIpSource, RateLimit, RateLimitOptions};
pub use static_host::StaticHost;
