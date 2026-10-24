use crate::{ClientState, StateSnapshot, StorageError};
use std::future::Future;

/// Atomic storage for one logical session; implementations must never reuse revisions.
pub trait StateStore: Send + Sync {
    /// Reads a consistent snapshot of all session state.
    fn load(&self) -> impl Future<Output = Result<StateSnapshot, StorageError>> + Send;

    /// Replaces state only at the expected revision, returning None on conflict.
    fn compare_exchange(
        &self,
        expected_revision: u64,
        state: ClientState,
    ) -> impl Future<Output = Result<Option<StateSnapshot>, StorageError>> + Send;
}
