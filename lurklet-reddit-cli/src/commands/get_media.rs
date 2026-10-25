use crate::commands::CommandOutput;
use crate::output::CliError;
use clap::Args;
use futures_util::StreamExt;
use lurklet_reddit::{RedditBackend, RedditClient};
use serde_json::json;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task::spawn_blocking;

#[derive(Args)]
pub struct GetMedia {
    /// Complete URL from a supported Reddit media origin; path and query are opaque.
    url: String,
    /// New destination file; its parent must exist. Existing destinations are never overwritten.
    #[arg(long)]
    output: PathBuf,
}

impl GetMedia {
    pub async fn execute<B: RedditBackend>(
        &self,
        client: &RedditClient<B>,
    ) -> Result<CommandOutput, CliError> {
        let output = self.output.clone();
        let temporary = spawn_blocking(move || {
            match fs::symlink_metadata(&output) {
                Ok(_) => return Err(io::Error::from(io::ErrorKind::AlreadyExists)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            let parent = output
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            NamedTempFile::new_in(parent)
        })
        .await
        .map_err(|_| CliError::Worker)?
        .map_err(|error| CliError::io("create_media_file", error))?;
        let mut media = client.get_media(&self.url).await?;
        let (file, temporary) = temporary.into_parts();
        let mut file = File::from_std(file);
        let mut bytes_written = 0_u64;
        while let Some(chunk) = media.body.next().await {
            let chunk = chunk?;
            file.write_all(&chunk)
                .await
                .map_err(|error| CliError::io("write_media_file", error))?;
            bytes_written += chunk.len() as u64;
        }
        file.flush()
            .await
            .map_err(|error| CliError::io("write_media_file", error))?;
        drop(file);
        let output = self.output.clone();
        spawn_blocking(move || temporary.persist_noclobber(output))
            .await
            .map_err(|_| CliError::Worker)?
            .map_err(|error| CliError::io("publish_media_file", error.error))?;

        Ok(CommandOutput {
            result: json!({
                "kind": media.kind,
                "content_type": media.content_type,
                "content_length": media.content_length,
                "bytes_written": bytes_written,
                "output": self.output,
            }),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arguments::Arguments;
    use crate::commands::Command;
    use crate::commands::testing::{FakeBackend, MediaCall};
    use assert_fs::TempDir;
    use clap::Parser;
    use futures_util::stream;
    use lurklet_reddit::{Error, MediaFile, MediaKind};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::Notify;
    use tokio::time::timeout;

    #[tokio::test]
    async fn when_media_is_downloaded_then_the_command_should_stream_to_a_new_file_and_return_only_json_metadata()
     {
        let directory = TempDir::new().unwrap();
        let output = directory.path().join("asset.bin");
        let arguments = Arguments::try_parse_from([
            "cli",
            "get-media",
            "https://preview.redd.it/file.gif?s=a%2Bb&width=320",
            "--output",
            output.to_str().unwrap(),
        ])
        .unwrap();
        let Command::GetMedia(arguments) = arguments.command else {
            panic!("expected media command")
        };
        let destination = output.clone();
        let body = stream::iter([Ok(b"first".to_vec().into()), Ok(b"second".to_vec().into())])
            .inspect(move |_| assert!(!destination.exists()));
        let client = RedditClient::from_backend(FakeBackend {
            media: Some(MediaCall {
                url: "https://preview.redd.it/file.gif?s=a%2Bb&width=320".into(),
                result: Mutex::new(Some(Ok(MediaFile {
                    content_type: "video/mp4".into(),
                    kind: MediaKind::Mp4,
                    content_length: Some(11),
                    body: Box::pin(body),
                }))),
            }),
            ..FakeBackend::default()
        });

        let result = arguments.execute(&client).await.unwrap();

        assert_eq!(fs::read(&output).unwrap(), b"firstsecond");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert_eq!(
            result.result,
            json!({"kind": "mp4", "content_type": "video/mp4", "content_length": 11, "bytes_written": 11, "output": output})
        );
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn when_opening_media_fails_then_the_command_should_remove_its_temporary_file_without_publishing_a_destination()
     {
        let directory = TempDir::new().unwrap();
        let output = directory.path().join("asset.bin");
        let arguments = GetMedia {
            url: "https://i.redd.it/file".into(),
            output: output.clone(),
        };
        let client = RedditClient::from_backend(FakeBackend {
            media: Some(MediaCall {
                url: arguments.url.clone(),
                result: Mutex::new(Some(Err(Error::MediaStatus {
                    status: 403,
                    retry_after: None,
                }))),
            }),
            ..FakeBackend::default()
        });

        let result = arguments.execute(&client).await;

        assert!(matches!(
            result,
            Err(CliError::Library(Error::MediaStatus { status: 403, .. }))
        ));
        assert!(!output.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn when_a_stream_fails_after_delivering_bytes_then_the_command_should_not_publish_the_partial_file()
     {
        let directory = TempDir::new().unwrap();
        let output = directory.path().join("asset.bin");
        let arguments = GetMedia {
            url: "https://i.redd.it/file".into(),
            output: output.clone(),
        };
        let client = RedditClient::from_backend(FakeBackend {
            media: Some(MediaCall {
                url: arguments.url.clone(),
                result: Mutex::new(Some(Ok(MediaFile {
                    content_type: "image/png".into(),
                    kind: MediaKind::Image,
                    content_length: None,
                    body: Box::pin(stream::iter([
                        Ok(b"partial".to_vec().into()),
                        Err(Error::Unavailable),
                    ])),
                }))),
            }),
            ..FakeBackend::default()
        });

        let result = arguments.execute(&client).await;

        assert!(matches!(result, Err(CliError::Library(Error::Unavailable))));
        assert!(!output.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn when_the_destination_already_exists_then_the_command_should_leave_it_unchanged_without_fetching_media()
     {
        let directory = TempDir::new().unwrap();
        let output = directory.path().join("asset.bin");
        fs::write(&output, b"existing user file").unwrap();
        let arguments = GetMedia {
            url: "https://i.redd.it/file".into(),
            output: output.clone(),
        };
        let client = RedditClient::from_backend(FakeBackend::default());

        let result = arguments.execute(&client).await;

        assert!(matches!(
            result,
            Err(CliError::Io {
                operation: "create_media_file",
                kind: io::ErrorKind::AlreadyExists
            })
        ));
        assert_eq!(fs::read(output).unwrap(), b"existing user file");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn when_a_destination_is_created_during_download_then_publication_should_not_overwrite_it()
     {
        let directory = TempDir::new().unwrap();
        let output = directory.path().join("asset.bin");
        let arguments = GetMedia {
            url: "https://i.redd.it/file".into(),
            output: output.clone(),
        };
        let destination = output.clone();
        let body = stream::once(async move {
            fs::write(destination, b"concurrent user file").unwrap();
            Ok(b"downloaded bytes".to_vec().into())
        });
        let client = RedditClient::from_backend(FakeBackend {
            media: Some(MediaCall {
                url: arguments.url.clone(),
                result: Mutex::new(Some(Ok(MediaFile {
                    content_type: "image/png".into(),
                    kind: MediaKind::Image,
                    content_length: None,
                    body: Box::pin(body),
                }))),
            }),
            ..FakeBackend::default()
        });

        let result = arguments.execute(&client).await;

        assert!(matches!(
            result,
            Err(CliError::Io {
                operation: "publish_media_file",
                kind: io::ErrorKind::AlreadyExists
            })
        ));
        assert_eq!(fs::read(output).unwrap(), b"concurrent user file");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn when_a_download_is_cancelled_then_the_temporary_file_should_be_removed_without_publishing_partial_bytes()
     {
        let directory = TempDir::new().unwrap();
        let output = directory.path().join("asset.bin");
        let arguments = GetMedia {
            url: "https://i.redd.it/file".into(),
            output: output.clone(),
        };
        let reading = Arc::new(Notify::new());
        let signal = reading.clone();
        let body = stream::once(async move {
            signal.notify_one();
            Ok(b"partial".to_vec().into())
        })
        .chain(stream::pending());
        let client = RedditClient::from_backend(FakeBackend {
            media: Some(MediaCall {
                url: arguments.url.clone(),
                result: Mutex::new(Some(Ok(MediaFile {
                    content_type: "video/mp4".into(),
                    kind: MediaKind::Mp4,
                    content_length: None,
                    body: Box::pin(body),
                }))),
            }),
            ..FakeBackend::default()
        });

        let task = tokio::spawn(async move { arguments.execute(&client).await });
        timeout(Duration::from_secs(5), reading.notified())
            .await
            .unwrap();
        task.abort();
        let result = task.await;

        assert!(matches!(result, Err(error) if error.is_cancelled()));
        assert!(!output.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
