//! Reddit communication independent of the lurklet API server.

mod backend;
mod builder;
mod client;
mod error;
mod mobile;

pub use backend::RedditBackend;
pub use builder::RedditClientBuilder;
pub use client::RedditClient;
pub use error::BuildError;
pub use mobile::MobileBackend;
