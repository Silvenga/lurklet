use crate::response_cache::entry::Entry;
use crate::response_cache::key::CacheKey;
use anyhow::{Context, ensure};
use rmp_serde::encode;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::task::spawn_blocking;

/// Index-free disk storage shared by overlapping processes on the same filesystem.
#[derive(Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Initializes the cache directories and verifies that they are writable.
    pub async fn new(directory: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = directory.as_ref().to_owned();
        spawn_blocking(move || {
            fs::create_dir_all(root.join(".tmp"))?;
            fs::create_dir_all(root.join("v1"))?;
            let root = fs::canonicalize(root)?;
            drop(NamedTempFile::new_in(root.join(".tmp"))?);
            drop(NamedTempFile::new_in(root.join("v1"))?);
            Ok(Self { root })
        })
        .await?
    }

    /// Reads a committed entry, treating unavailable or invalid records as misses.
    pub async fn read(&self, key: &CacheKey) -> Option<Entry> {
        let path = self.path(key);
        let expected_key = key.uri().to_string();
        let result = spawn_blocking(move || -> anyhow::Result<Option<Entry>> {
            let file = match File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error.into()),
            };
            let entry: Entry = rmp_serde::from_read(BufReader::new(file))?;
            ensure!(
                entry.version == 1 && entry.key == expected_key,
                "cache entry format or key mismatch"
            );
            Ok(Some(entry))
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(|result| result);
        match result {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!("Unable to read a response cache entry; fetching instead");
                tracing::debug!(?error, "Response cache read failure details");
                None
            }
        }
    }

    /// Publishes a complete record with atomic rename and no durability sync.
    pub async fn write(&self, key: &CacheKey, entry: Entry) -> anyhow::Result<()> {
        let path = self.path(key);
        let temporary = self.root.join(".tmp");
        spawn_blocking(move || {
            fs::create_dir_all(path.parent().unwrap())?;
            let mut file = NamedTempFile::new_in(temporary)?;
            encode::write(file.as_file_mut(), &entry)?;
            file.into_temp_path()
                .persist(path)
                .context("failed to publish cache entry")?;
            Ok(())
        })
        .await?
    }

    fn path(&self, key: &CacheKey) -> PathBuf {
        // One byte of the digest shards entries across 256 directories.
        self.root
            .join("v1")
            .join(&key.digest()[..2])
            .join(format!("{}.cache", key.digest()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use axum::http::{HeaderMap, HeaderValue, header};
    use bytes::Bytes;
    use std::time::{Duration, SystemTime};
    use tokio::task::JoinSet;

    fn entry(key: &CacheKey, body: &'static [u8]) -> Entry {
        Entry {
            version: 1,
            key: key.uri().to_string(),
            stored_at: SystemTime::now(),
            fresh_for: Duration::from_secs(60),
            initial_age: Duration::ZERO,
            headers: HeaderMap::from_iter([(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )]),
            body: Bytes::from_static(body),
        }
    }

    #[tokio::test]
    async fn when_a_store_is_reopened_then_entries_and_freshness_should_survive() {
        let directory = TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/api/posts?sort=new".parse().unwrap());
        let mut expected = entry(&key, b"complete body");
        expected.stored_at -= Duration::from_secs(365 * 24 * 3600);
        expected
            .headers
            .append("x-example", HeaderValue::from_static("first"));
        expected
            .headers
            .append("x-example", HeaderValue::from_static("second"));
        store.write(&key, expected.clone()).await.unwrap();
        drop(store);

        let reopened = Store::new(directory.path()).await.unwrap();
        let actual = reopened.read(&key).await.unwrap();

        assert_eq!(actual.body, expected.body);
        assert_eq!(actual.headers, expected.headers);
        assert_eq!(actual.stored_at, expected.stored_at);
        assert_eq!(actual.fresh_for, expected.fresh_for);
        assert!(!actual.is_fresh());
    }

    #[tokio::test]
    async fn when_a_partial_is_abandoned_then_readers_should_keep_using_the_committed_entry() {
        let directory = TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/api/posts".parse().unwrap());
        store.write(&key, entry(&key, b"old")).await.unwrap();
        let partial = NamedTempFile::new_in(directory.path().join(".tmp")).unwrap();
        fs::write(partial.path(), b"incomplete new representation").unwrap();

        let reopened = Store::new(directory.path()).await.unwrap();
        let actual = reopened.read(&key).await.unwrap();

        assert_eq!(actual.body, b"old".as_slice());
        assert!(partial.path().exists());
        assert_eq!(
            fs::read_dir(directory.path().join(".tmp")).unwrap().count(),
            1
        );
    }

    #[tokio::test]
    async fn when_a_write_fails_then_the_previous_entry_should_remain_intact() {
        let directory = TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/api/posts".parse().unwrap());
        store.write(&key, entry(&key, b"old")).await.unwrap();
        fs::rename(
            directory.path().join(".tmp"),
            directory.path().join("unavailable"),
        )
        .unwrap();

        let result = store.write(&key, entry(&key, b"new")).await;
        let actual = store.read(&key).await.unwrap();

        assert!(result.is_err());
        assert_eq!(actual.body, b"old".as_slice());
    }

    #[tokio::test]
    async fn when_entries_are_corrupt_or_mismatched_then_reads_should_be_misses() {
        let directory = TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/api/posts".parse().unwrap());
        store.write(&key, entry(&key, b"old")).await.unwrap();
        fs::write(store.path(&key), b"not messagepack").unwrap();

        assert!(store.read(&key).await.is_none());

        for invalid in [
            Entry {
                version: 255,
                ..entry(&key, b"wrong version")
            },
            Entry {
                key: "/another".to_owned(),
                ..entry(&key, b"wrong key")
            },
        ] {
            store.write(&key, invalid).await.unwrap();

            assert!(store.read(&key).await.is_none());
        }
    }

    #[tokio::test]
    async fn when_independent_stores_replace_an_entry_then_readers_should_only_observe_complete_records()
     {
        let directory = TempDir::new().unwrap();
        let first = Store::new(directory.path()).await.unwrap();
        let second = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/api/posts".parse().unwrap());
        first.write(&key, entry(&key, b"initial")).await.unwrap();
        let mut writers = JoinSet::new();
        for writer in [first.clone(), second] {
            writers.spawn(async move {
                let key = CacheKey::new(&"/api/posts".parse().unwrap());
                for byte in 0..32 {
                    let mut next = entry(&key, b"");
                    next.body = Bytes::from(vec![byte; 65536]);
                    writer.write(&key, next).await.unwrap();
                }
            });
        }

        for _ in 0..64 {
            let actual = first.read(&key).await.unwrap();
            assert!(
                actual.body == b"initial".as_slice()
                    || (actual.body.len() == 65536
                        && actual.body.iter().all(|byte| *byte == actual.body[0]))
            );
        }
        while let Some(result) = writers.join_next().await {
            result.unwrap();
        }

        assert_eq!(first.read(&key).await.unwrap().body.len(), 65536);
        assert_eq!(
            fs::read_dir(directory.path().join(".tmp")).unwrap().count(),
            0
        );
    }
}
