use crate::{ClientState, StateSnapshot, StateStore, StorageError};
use tokio::sync::Mutex;

/// Process-local state storage; use a shared Storage handle to coordinate clients.
#[derive(Debug, Default)]
pub struct InMemoryStore {
    snapshot: Mutex<StateSnapshot>,
}

impl InMemoryStore {
    /// Initializes storage with caller-restored state.
    pub fn with_state(state: ClientState) -> Self {
        Self {
            snapshot: Mutex::new(StateSnapshot { revision: 0, state }),
        }
    }
}

impl StateStore for InMemoryStore {
    async fn load(&self) -> Result<StateSnapshot, StorageError> {
        Ok(self.snapshot.lock().await.clone())
    }

    async fn compare_exchange(
        &self,
        expected_revision: u64,
        state: ClientState,
    ) -> Result<Option<StateSnapshot>, StorageError> {
        let mut snapshot = self.snapshot.lock().await;
        if snapshot.revision != expected_revision {
            return Ok(None);
        }
        let revision = snapshot
            .revision
            .checked_add(1)
            .ok_or(StorageError::RevisionExhausted)?;
        *snapshot = StateSnapshot { revision, state };
        Ok(Some(snapshot.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn when_a_revision_is_stale_then_compare_exchange_should_preserve_newer_state() {
        let storage = InMemoryStore::default();
        let mut first = ClientState::default();
        first.authentication.loid = Some("first".into());
        let mut second = ClientState::default();
        second.authentication.loid = Some("second".into());

        let accepted = storage.compare_exchange(0, first).await.unwrap().unwrap();
        let rejected = storage.compare_exchange(0, second).await.unwrap();

        assert!(rejected.is_none());
        assert_eq!(storage.load().await.unwrap(), accepted);
    }
}
