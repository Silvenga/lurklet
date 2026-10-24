use crate::output::CliError;
use lurklet_reddit::ClientState;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredFile {
    version: u32,
    state: ClientState,
}

pub struct StateFile {
    path: PathBuf,
    _lock: File,
}

impl StateFile {
    pub fn open(path: PathBuf) -> Result<(Self, ClientState), CliError> {
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|error| CliError::io("create_directory", error))?;
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|error| CliError::io("open_state_lock", error))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(CliError::StateBusy),
            Err(TryLockError::Error(error)) => return Err(CliError::io("lock_state", error)),
        }
        let state = match File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(8 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| CliError::io("read_state", error))?;
                if bytes.len() > 8 * 1024 * 1024 {
                    return Err(CliError::StateFormat);
                }
                let stored: StoredFile =
                    serde_json::from_slice(&bytes).map_err(|_| CliError::StateFormat)?;
                if stored.version != 1 {
                    return Err(CliError::StateFormat);
                }
                tracing::trace!("Restored existing session state");
                stored.state
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                tracing::trace!("No saved session state; starting with empty state");
                ClientState::default()
            }
            Err(error) => return Err(CliError::io("open_state", error)),
        };
        Ok((Self { path, _lock: lock }, state))
    }

    pub fn save(&self, state: ClientState) -> Result<(), CliError> {
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temporary =
            NamedTempFile::new_in(parent).map_err(|error| CliError::io("create_state", error))?;
        serde_json::to_writer_pretty(&mut temporary, &StoredFile { version: 1, state }).map_err(
            |_| CliError::Io {
                operation: "write_state",
                kind: io::ErrorKind::Other,
            },
        )?;
        temporary
            .write_all(b"\n")
            .map_err(|error| CliError::io("write_state", error))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| CliError::io("sync_state", error))?;
        temporary
            .persist(&self.path)
            .map_err(|error| CliError::io("replace_state", error.error))?;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(|error| CliError::io("sync_state_directory", error))?;
        tracing::trace!("Session state saved atomically");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;

    #[test]
    fn when_state_is_saved_then_reopening_should_restore_it_and_release_the_lock() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("session.json");
        let (file, mut state) = StateFile::open(path.clone()).unwrap();
        state.authentication.loid = Some("private-loid".into());

        file.save(state.clone()).unwrap();
        let locked = StateFile::open(path.clone());
        drop(file);
        let (_, restored) = StateFile::open(path).unwrap();

        assert!(matches!(locked, Err(CliError::StateBusy)));
        assert_eq!(restored, state);
    }
}
