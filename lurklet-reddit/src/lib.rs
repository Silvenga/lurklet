//! Reddit communication independent of the lurklet API server.

mod backends;
mod builder;
mod capture;
mod client;
mod clock;
mod error;
mod named_reference;
mod profile;
mod reddit_id;
mod state;
mod storage;

pub use backends::{
    GraphQlDecodeError, GraphQlErrorKind, GraphQlOutcome, GraphQlResponse, ListedPost,
    MobileBackend, Post, PostAttachment, RedditBackend, RequestAllowlist, SubredditPage,
    SubredditSort, Thumbnail, TimeRange, VideoFormat, VideoSource,
};
pub use builder::RedditClientBuilder;
pub use capture::{ResponseCapture, ResponseObserver};
pub use client::RedditClient;
pub use clock::{Clock, SystemClock};
pub use error::{
    BuildError, ClockError, Error, ResponseError, RetryAfter, StorageError, TransportError,
    TransportErrorKind,
};
pub use named_reference::NamedReference;
pub use profile::{AppProfile, DeviceProfile};
pub use reddit_id::RedditId;
pub use state::{
    AccessToken, AnalyticsSession, AnonymousSession, AuthenticationState, ClientState,
    Installation, StateSnapshot,
};
pub use storage::{InMemoryStore, StateStore, Storage};
