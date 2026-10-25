use anyhow::{anyhow, ensure};
use lurklet_reddit::{ClientState, InMemoryStore, StateSnapshot, StateStore, StorageError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::task::spawn_blocking;

#[derive(Serialize, Deserialize)]
struct StoredSession {
    version: u32,
    state: ClientState,
}

/// In-memory session state with optional, best-effort disk snapshots.
pub struct SessionStore {
    memory: InMemoryStore,
    path: Option<PathBuf>,
}

impl SessionStore {
    /// Restores configured state without creating files or contacting Reddit.
    pub async fn new(path: Option<PathBuf>) -> Self {
        let state = if let Some(path) = path.clone() {
            let result = spawn_blocking(move || -> anyhow::Result<ClientState> {
                let bytes = match fs::read(path) {
                    Ok(bytes) => bytes,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Ok(ClientState::default());
                    }
                    Err(error) => return Err(error.into()),
                };
                let stored: StoredSession = serde_json::from_slice(&bytes)
                    .map_err(|_| anyhow!("invalid session state format"))?;
                ensure!(stored.version == 1, "unsupported session state version");
                Ok(stored.state)
            })
            .await
            .map_err(anyhow::Error::from)
            .and_then(|result| result);
            match result {
                Ok(state) => state,
                Err(error) => {
                    tracing::warn!(
                        "Unable to restore Reddit session state; starting with empty in-memory state"
                    );
                    tracing::debug!(?error, "Session state restore failure details");
                    ClientState::default()
                }
            }
        } else {
            ClientState::default()
        };
        Self {
            memory: InMemoryStore::with_state(state),
            path,
        }
    }
}

impl StateStore for SessionStore {
    async fn load(&self) -> Result<StateSnapshot, StorageError> {
        self.memory.load().await
    }

    async fn compare_exchange(
        &self,
        expected_revision: u64,
        state: ClientState,
    ) -> Result<Option<StateSnapshot>, StorageError> {
        let snapshot = self
            .memory
            .compare_exchange(expected_revision, state)
            .await?;
        if let (Some(path), Some(snapshot)) = (&self.path, &snapshot) {
            let path = path.clone();
            let state = snapshot.state.clone();
            let result = spawn_blocking(move || -> anyhow::Result<()> {
                let parent = path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                fs::create_dir_all(parent)?;
                let mut temporary = NamedTempFile::new_in(parent)?;
                serde_json::to_writer_pretty(&mut temporary, &StoredSession { version: 1, state })?;
                temporary.write_all(b"\n")?;
                temporary.into_temp_path().persist(path)?;
                Ok(())
            })
            .await
            .map_err(anyhow::Error::from)
            .and_then(|result| result);
            if let Err(error) = result {
                tracing::warn!(
                    "Unable to save Reddit session state; continuing with in-memory state"
                );
                tracing::debug!(?error, "Session state save failure details");
            }
        }
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use lurklet_reddit::{RedditClient, Storage};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::task::{JoinSet, yield_now};

    fn state() -> ClientState {
        serde_json::from_value(json!({
            "installation": {
                "id": "00000000-0000-4000-8000-000000000001",
                "created_at_ms": 123
            },
            "authentication": {
                "session": {"access_token": "private-token", "expires_at_ms": u64::MAX},
                "loid": "private-loid"
            },
            "analytics": {"session_id": "private-analytics", "creation_timestamp_ms": 456}
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn when_state_files_are_disabled_then_updates_should_remain_local_to_the_store() {
        let store = SessionStore::new(None).await;
        let state = state();

        let saved = store
            .compare_exchange(0, state.clone())
            .await
            .unwrap()
            .unwrap();
        let other = SessionStore::new(None).await;

        assert_eq!(saved, StateSnapshot { revision: 1, state });
        assert_eq!(store.load().await.unwrap(), saved);
        assert_eq!(other.load().await.unwrap(), StateSnapshot::default());
        assert!(store.path.is_none());
    }

    #[tokio::test]
    async fn when_the_snapshot_is_missing_then_initialization_should_not_create_files() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("nested/session.json");

        let store = SessionStore::new(Some(path)).await;

        assert_eq!(store.load().await.unwrap(), StateSnapshot::default());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn when_state_changes_then_reopening_should_restore_all_fields_and_reuse_the_token() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("nested/session.json");
        let store = SessionStore::new(Some(path.clone())).await;
        let state = state();
        let session = state.authentication.session.clone().unwrap();
        store
            .compare_exchange(0, state.clone())
            .await
            .unwrap()
            .unwrap();
        drop(store);
        let before = fs::read(&path).unwrap();
        let storage = Storage::new(SessionStore::new(Some(path.clone())).await);
        let client = RedditClient::builder()
            .with_storage(storage.clone())
            .with_auth_endpoint("https://127.0.0.1:1/unused")
            .build()
            .unwrap();

        let restored = storage.store().load().await.unwrap();
        let reused = client.bootstrap().await.unwrap();

        assert_eq!(restored.state, state);
        assert_eq!(reused, session);
        assert_eq!(storage.store().load().await.unwrap(), restored);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&before).unwrap(),
            json!({"version": 1, "state": state})
        );
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn when_a_revision_conflicts_then_neither_memory_nor_disk_should_be_replaced() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("session.json");
        let store = SessionStore::new(Some(path.clone())).await;
        let accepted = store.compare_exchange(0, state()).await.unwrap().unwrap();
        let before = fs::read(&path).unwrap();

        let rejected = store
            .compare_exchange(0, ClientState::default())
            .await
            .unwrap();

        assert!(rejected.is_none());
        assert_eq!(store.load().await.unwrap(), accepted);
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[tokio::test]
    async fn when_saved_data_is_invalid_or_unsupported_then_initialization_should_fall_back_without_overwriting_it()
     {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("session.json");

        for bytes in [
            b"private-invalid-json".to_vec(),
            serde_json::to_vec(&json!({"version": 2, "state": state()})).unwrap(),
            serde_json::to_vec(&json!({"version": 1, "state": null})).unwrap(),
        ] {
            fs::write(&path, &bytes).unwrap();

            let store = SessionStore::new(Some(path.clone())).await;

            assert_eq!(store.load().await.unwrap(), StateSnapshot::default());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }

    #[tokio::test]
    async fn when_a_file_cannot_be_read_or_replaced_then_memory_should_remain_usable_and_partials_should_be_removed()
     {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("session.json");
        fs::create_dir_all(&path).unwrap();
        let store = SessionStore::new(Some(path.clone())).await;
        assert_eq!(store.load().await.unwrap(), StateSnapshot::default());

        let saved = store.compare_exchange(0, state()).await.unwrap().unwrap();

        assert_eq!(
            saved,
            StateSnapshot {
                revision: 1,
                state: state()
            }
        );
        assert_eq!(store.load().await.unwrap(), saved);
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn when_a_previous_save_failed_then_the_next_state_change_should_try_saving_again() {
        let directory = TempDir::new().unwrap();
        let parent = directory.path().join("blocked");
        fs::write(&parent, "not a directory").unwrap();
        let path = parent.join("session.json");
        let store = SessionStore::new(Some(path.clone())).await;
        let first = store.compare_exchange(0, state()).await.unwrap().unwrap();
        fs::remove_file(&parent).unwrap();
        let mut next = state();
        next.authentication.loid = Some("updated-loid".into());

        let saved = store
            .compare_exchange(first.revision, next.clone())
            .await
            .unwrap()
            .unwrap();
        let reopened = SessionStore::new(Some(path)).await;

        assert_eq!(saved.revision, first.revision + 1);
        assert_eq!(store.load().await.unwrap().state, next);
        assert_eq!(reopened.load().await.unwrap().state, next);
    }

    #[tokio::test]
    async fn when_two_stores_use_the_same_file_then_both_should_open_without_live_state_synchronization()
     {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("session.json");
        let first = SessionStore::new(Some(path.clone())).await;
        first.compare_exchange(0, state()).await.unwrap().unwrap();
        let second = SessionStore::new(Some(path.clone())).await;
        assert_eq!(second.load().await.unwrap().state, state());
        let mut next = state();
        next.authentication.loid = Some("second-consumer".into());

        second
            .compare_exchange(0, next.clone())
            .await
            .unwrap()
            .unwrap();
        let reopened = SessionStore::new(Some(path)).await;

        assert_eq!(first.load().await.unwrap().state, state());
        assert_eq!(second.load().await.unwrap().state, next);
        assert_eq!(reopened.load().await.unwrap().state, next);
    }

    #[tokio::test]
    async fn when_writes_overlap_then_readers_should_only_observe_complete_snapshots() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("session.json");
        let first = Arc::new(SessionStore::new(Some(path.clone())).await);
        first.compare_exchange(0, state()).await.unwrap().unwrap();
        let second = Arc::new(SessionStore::new(Some(path.clone())).await);
        let mut first_state = state();
        first_state.authentication.loid = Some("first".repeat(1024));
        let mut second_state = state();
        second_state.authentication.loid = Some("second".repeat(1024));
        let mut writes = JoinSet::new();

        for (store, state) in [(first, first_state.clone()), (second, second_state.clone())] {
            writes.spawn(async move {
                for _ in 0..16 {
                    let revision = store.load().await.unwrap().revision;
                    store
                        .compare_exchange(revision, state.clone())
                        .await
                        .unwrap()
                        .unwrap();
                }
            });
        }
        for _ in 0..32 {
            let stored: StoredSession = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(stored.version, 1);
            assert!(
                stored.state == state()
                    || stored.state == first_state
                    || stored.state == second_state
            );
            yield_now().await;
        }
        while let Some(result) = writes.join_next().await {
            result.unwrap();
        }

        let stored: StoredSession = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(stored.state == first_state || stored.state == second_state);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
