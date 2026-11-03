use crate::response_cache::entry::{Entry, Metadata};
use crate::response_cache::key::CacheKey;
use anyhow::ensure;
use axum::http::HeaderMap;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tempfile::{Builder, NamedTempFile, TempPath};
use tokio::fs::File as AsyncFile;
use tokio::io::AsyncWriteExt;
use tokio::task::spawn_blocking;

/// Index-free storage with independently published metadata and immutable content.
#[derive(Clone)]
pub struct Store {
    root: PathBuf,
}

/// A partial file that is removed unless its completed representation is published.
pub struct Writer {
    file: AsyncFile,
    temporary: TempPath,
    metadata_path: PathBuf,
    key: String,
}

impl Store {
    /// Initializes writable cache directories without scanning existing entries.
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

    /// Reads metadata and opens its content without loading the body into memory.
    pub async fn read(&self, key: &CacheKey) -> Option<Entry> {
        let path = self.path(key);
        let expected_key = key.uri().to_string();
        let result = spawn_blocking(move || -> anyhow::Result<Option<Entry>> {
            let file = match File::open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error.into()),
            };
            let metadata: Metadata = serde_json::from_reader(BufReader::new(file))?;
            ensure!(metadata.key == expected_key, "cache entry key mismatch");
            ensure!(
                Path::new(&metadata.content_file).file_name()
                    == Some(metadata.content_file.as_ref()),
                "invalid cache content filename"
            );
            let file = match File::open(path.parent().unwrap().join(&metadata.content_file)) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error.into()),
            };
            ensure!(
                file.metadata()?.len() == metadata.content_length,
                "cache content length mismatch"
            );
            Ok(Some(Entry {
                metadata,
                body: AsyncFile::from_std(file),
            }))
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

    /// Creates a unique partial content file on the cache filesystem.
    pub async fn begin(&self, key: &CacheKey) -> anyhow::Result<Writer> {
        let metadata_path = self.path(key);
        let temporary = self.root.join(".tmp");
        let prefix = format!("{}-", key.digest());
        let key = key.uri().to_string();
        spawn_blocking(move || {
            fs::create_dir_all(metadata_path.parent().unwrap())?;
            let (file, temporary) = Builder::new()
                .prefix(&prefix)
                .suffix(".body")
                .tempfile_in(temporary)?
                .into_parts();
            Ok(Writer {
                file: AsyncFile::from_std(file),
                temporary,
                metadata_path,
                key,
            })
        })
        .await?
    }

    fn path(&self, key: &CacheKey) -> PathBuf {
        self.root
            .join("v1")
            .join(&key.digest()[..2])
            .join(format!("{}.json", key.digest()))
    }

    #[cfg(test)]
    pub fn metadata_path(&self, key: &CacheKey) -> PathBuf {
        self.path(key)
    }
}

impl Writer {
    /// Writes one chunk before the streaming response advances to another.
    pub async fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all(bytes).await
    }

    /// Publishes content first and atomically replaces metadata as the commit point.
    pub async fn publish(
        mut self,
        headers: HeaderMap,
        fresh_for: Duration,
        length: u64,
    ) -> anyhow::Result<Metadata> {
        self.file.flush().await?;
        drop(self.file);
        spawn_blocking(move || {
            let parent = self.metadata_path.parent().unwrap();
            let temporary_directory = self.temporary.parent().unwrap().to_owned();
            let content_file = self
                .temporary
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            let content_path = parent.join(&content_file);
            self.temporary.persist_noclobber(&content_path)?;
            let mut unpublished = TempPath::try_from_path(content_path)?;
            let metadata = Metadata {
                key: self.key,
                stored_at: SystemTime::now(),
                fresh_for,
                headers,
                content_file,
                content_length: length,
            };
            let previous = File::open(&self.metadata_path)
                .ok()
                .and_then(|file| serde_json::from_reader::<_, Metadata>(BufReader::new(file)).ok());
            let mut file = NamedTempFile::new_in(temporary_directory)?;
            serde_json::to_writer(file.as_file_mut(), &metadata)?;
            file.into_temp_path().persist(&self.metadata_path)?;
            unpublished.disable_cleanup(true);
            if let Some(previous) = previous
                && Path::new(&previous.content_file).file_name()
                    == Some(previous.content_file.as_ref())
            {
                let _ = fs::remove_file(parent.join(previous.content_file));
            }
            Ok(metadata)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response_cache::streaming_body::streaming_body;
    use assert_fs::TempDir;
    use axum::body::{Body, to_bytes};
    use axum::http::{HeaderValue, header};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    async fn write(store: &Store, key: &CacheKey, bytes: &[u8]) -> Metadata {
        let mut writer = store.begin(key).await.unwrap();
        writer.write(bytes).await.unwrap();
        writer
            .publish(
                HeaderMap::from_iter([
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("public, max-age=60"),
                    ),
                    (
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("application/octet-stream"),
                    ),
                ]),
                Duration::from_secs(60),
                bytes.len() as u64,
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn when_writing_a_chunk_fails_then_the_client_should_still_receive_the_complete_response()
    {
        let directory = TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/resource".parse().unwrap());
        let original = write(&store, &key, b"old").await;
        let mut writer = store.begin(&key).await.unwrap();
        writer.file = AsyncFile::from_std(File::open(&writer.temporary).unwrap());
        let guard = Arc::new(Mutex::new(())).lock_owned().await;
        let body = streaming_body(
            Body::from("fresh"),
            writer,
            HeaderMap::new(),
            Duration::from_secs(60),
            Some(5),
            guard,
        );

        let received = to_bytes(body, 4096).await.unwrap();

        assert_eq!(received, b"fresh".as_slice());
        assert_eq!(
            store.read(&key).await.unwrap().metadata.content_file,
            original.content_file
        );
        assert_eq!(
            fs::read_dir(directory.path().join(".tmp")).unwrap().count(),
            0
        );
    }

    #[tokio::test]
    async fn when_a_store_is_reopened_then_metadata_and_content_should_survive_separately() {
        let directory = TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/resource".parse().unwrap());
        let mut expected = write(&store, &key, b"complete body").await;
        expected.stored_at -= Duration::from_secs(365 * 24 * 3600);
        expected
            .headers
            .append("x-example", HeaderValue::from_static("first"));
        expected
            .headers
            .append("x-example", HeaderValue::from_static("second"));
        fs::write(store.path(&key), serde_json::to_vec(&expected).unwrap()).unwrap();

        let reopened = Store::new(directory.path()).await.unwrap();
        let actual = reopened.read(&key).await.unwrap();

        assert_eq!(actual.metadata.headers, expected.headers);
        assert_eq!(actual.metadata.stored_at, expected.stored_at);
        assert_eq!(actual.metadata.fresh_for, expected.fresh_for);
        assert!(!actual.is_fresh());
        assert_eq!(
            to_bytes(actual.into_response().into_body(), 4096)
                .await
                .unwrap(),
            b"complete body".as_slice()
        );
        assert_eq!(
            fs::read(
                store
                    .path(&key)
                    .parent()
                    .unwrap()
                    .join(&expected.content_file)
            )
            .unwrap(),
            b"complete body"
        );
        assert!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(store.path(&key)).unwrap())
                .unwrap()
                .get("body")
                .is_none()
        );
    }

    #[tokio::test]
    async fn when_a_partial_is_dropped_then_the_previous_entry_should_survive_and_the_partial_should_be_removed()
     {
        let directory = TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/resource".parse().unwrap());
        write(&store, &key, b"old").await;
        let mut partial = store.begin(&key).await.unwrap();
        partial.write(b"incomplete").await.unwrap();

        drop(partial);
        let actual = store.read(&key).await.unwrap();

        assert_eq!(
            to_bytes(actual.into_response().into_body(), 4096)
                .await
                .unwrap(),
            b"old".as_slice()
        );
        assert_eq!(
            fs::read_dir(directory.path().join(".tmp")).unwrap().count(),
            0
        );
    }

    #[tokio::test]
    async fn when_publication_fails_then_unpublished_content_should_be_removed_without_replacing_the_old_entry()
     {
        let directory = TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/resource".parse().unwrap());
        let original = write(&store, &key, b"old").await;
        let mut writer = store.begin(&key).await.unwrap();
        writer.write(b"new").await.unwrap();
        let blocked = directory.path().join("blocked");
        fs::create_dir_all(&blocked).unwrap();
        writer.metadata_path = blocked;

        let result = writer
            .publish(HeaderMap::new(), Duration::from_secs(60), 3)
            .await;
        let actual = store.read(&key).await.unwrap();

        assert!(result.is_err());
        drop(result);
        assert_eq!(actual.metadata.content_file, original.content_file);
        assert_eq!(
            to_bytes(actual.into_response().into_body(), 4096)
                .await
                .unwrap(),
            b"old".as_slice()
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);
        assert_eq!(
            fs::read_dir(directory.path().join(".tmp")).unwrap().count(),
            0
        );
    }

    #[tokio::test]
    async fn when_entries_are_corrupt_incomplete_or_mismatched_then_reads_should_be_misses() {
        let directory = TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/resource".parse().unwrap());
        let metadata = write(&store, &key, b"old").await;
        fs::write(store.path(&key), b"not json").unwrap();
        assert!(store.read(&key).await.is_none());

        for invalid in [
            Metadata {
                key: "/other".into(),
                ..metadata.clone()
            },
            Metadata {
                content_file: "../outside.body".into(),
                ..metadata.clone()
            },
            Metadata {
                content_file: "missing.body".into(),
                ..metadata.clone()
            },
            Metadata {
                content_length: 100,
                ..metadata.clone()
            },
        ] {
            fs::write(store.path(&key), serde_json::to_vec(&invalid).unwrap()).unwrap();

            assert!(store.read(&key).await.is_none());
        }
    }

    #[tokio::test]
    async fn when_independent_writers_overlap_then_only_published_content_should_be_visible_to_new_readers()
     {
        let directory = TempDir::new().unwrap();
        let first = Store::new(directory.path()).await.unwrap();
        let second = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/resource".parse().unwrap());
        let old_metadata = write(&first, &key, b"initial").await;
        let old = first.read(&key).await.unwrap();
        let mut slow = first.begin(&key).await.unwrap();
        slow.write(b"slow ").await.unwrap();
        let mut fast = second.begin(&key).await.unwrap();
        fast.write(b"fast ").await.unwrap();

        let during_writes = second.read(&key).await.unwrap();
        fast.write(b"complete").await.unwrap();
        let fast_metadata = fast
            .publish(HeaderMap::new(), Duration::from_secs(60), 13)
            .await
            .unwrap();
        let between_publications = first.read(&key).await.unwrap();
        slow.write(b"complete").await.unwrap();
        slow.publish(HeaderMap::new(), Duration::from_secs(60), 13)
            .await
            .unwrap();
        let latest = second.read(&key).await.unwrap();

        for (entry, expected) in [
            (old, b"initial".as_slice()),
            (during_writes, b"initial".as_slice()),
            (between_publications, b"fast complete".as_slice()),
            (latest, b"slow complete".as_slice()),
        ] {
            let length = entry.metadata.content_length;
            let body = to_bytes(entry.into_response().into_body(), 4096)
                .await
                .unwrap();
            assert_eq!(body.as_ref(), expected);
            assert_eq!(length, expected.len() as u64);
        }
        for retired in [old_metadata, fast_metadata] {
            assert!(
                !first
                    .path(&key)
                    .parent()
                    .unwrap()
                    .join(retired.content_file)
                    .exists()
            );
        }
        assert_eq!(
            fs::read_dir(directory.path().join(".tmp")).unwrap().count(),
            0
        );
    }
}
