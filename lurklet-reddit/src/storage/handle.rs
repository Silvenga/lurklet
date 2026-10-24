use crate::{Error, InMemoryStore, StateStore};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::Mutex;

type AcquisitionFailure = Option<(u64, Error)>;

/// Shared storage and acquisition coordination for one logical session.
pub struct Storage<S: StateStore = InMemoryStore> {
    store: Arc<S>,
    pub(crate) completed: Arc<AtomicU64>,
    pub(crate) acquisition: Arc<Mutex<AcquisitionFailure>>,
}

impl<S: StateStore> Storage<S> {
    /// Wraps a caller-provided store; clone this handle when sharing a session.
    pub fn new(store: S) -> Self {
        Self {
            store: Arc::new(store),
            completed: Arc::new(AtomicU64::new(0)),
            acquisition: Arc::new(Mutex::new(None)),
        }
    }

    /// Gives the owner access to the supplied storage implementation.
    pub fn store(&self) -> &S {
        &self.store
    }
}

impl Default for Storage {
    fn default() -> Self {
        Self::new(InMemoryStore::default())
    }
}

impl<S: StateStore> Clone for Storage<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            completed: Arc::clone(&self.completed),
            acquisition: Arc::clone(&self.acquisition),
        }
    }
}
