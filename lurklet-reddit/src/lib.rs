//! Reddit communication independent of the lurklet API server.

mod backend;
mod builder;
mod capture;
mod client;
mod clock;
mod error;
mod mobile;
mod profile;
mod state;
mod storage;

pub use backend::RedditBackend;
pub use builder::RedditClientBuilder;
pub use capture::{ResponseCapture, ResponseObserver};
pub use client::RedditClient;
pub use clock::{Clock, SystemClock};
pub use error::{
    BuildError, ClockError, Error, ResponseError, RetryAfter, StorageError, TransportError,
    TransportErrorKind,
};
pub use mobile::MobileBackend;
pub use profile::{AppProfile, DeviceProfile};
pub use state::{
    AccessToken, AnalyticsSession, AnonymousSession, AuthenticationState, ClientState,
    Installation, StateSnapshot,
};
pub use storage::{InMemoryStore, StateStore, Storage};
