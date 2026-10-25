use crate::arguments::Arguments;
use crate::captures::Recorder;
use crate::output::CliError;
use crate::state_file::StateFile;
use lurklet_reddit::{ClientState, Error, InMemoryStore, StateStore, Storage};
use serde_json::{Value, json};
use std::fs;
use tokio::task::spawn_blocking;

pub async fn run(arguments: Arguments) -> Result<Value, CliError> {
    let (state_file, state) = if let Some(path) = arguments.state_file {
        let (file, state) = spawn_blocking(move || StateFile::open(path))
            .await
            .map_err(|_| CliError::Worker)??;
        (Some(file), state)
    } else {
        tracing::warn!(
            "Session state is ephemeral; installation identity will not survive this invocation"
        );
        (None, ClientState::default())
    };
    let storage = Storage::new(InMemoryStore::with_state(state));
    let mut builder = arguments.client.builder().with_storage(storage.clone());
    let recorder = if let Some(directory) = &arguments.capture_dir {
        let directory = directory.clone();
        spawn_blocking(move || fs::create_dir_all(directory))
            .await
            .map_err(|_| CliError::Worker)?
            .map_err(|error| CliError::io("create_capture_directory", error))?;
        let recorder = Recorder::default();
        builder = builder.with_response_observer(recorder.clone());
        Some(recorder)
    } else {
        None
    };
    let client = builder.build()?;
    let operation = arguments
        .command
        .execute(&client, arguments.show_secrets)
        .await;
    let mut secondary_errors = Vec::new();
    if let Some(file) = state_file {
        let snapshot = storage.store().load().await.map_err(Error::from)?;
        let saved = spawn_blocking(move || file.save(snapshot.state))
            .await
            .map_err(|_| CliError::Worker)
            .and_then(|result| result);
        if let Err(error) = saved {
            secondary_errors.push(error.json());
        }
    }
    let mut captured = 0;
    if let (Some(recorder), Some(directory)) = (recorder, arguments.capture_dir) {
        match spawn_blocking(move || recorder.write(&directory))
            .await
            .map_err(|_| CliError::Worker)
            .and_then(|result| result)
        {
            Ok(count) => captured = count,
            Err(error) => secondary_errors.push(error.json()),
        }
    }
    let mut output = match operation {
        Ok(report) => match report.error {
            Some(error) => json!({"ok": false, "error": error, "operation_result": report.result}),
            None if secondary_errors.is_empty() => json!({"ok": true, "result": report.result}),
            None => {
                json!({"ok": false, "error": secondary_errors.remove(0), "operation_result": report.result})
            }
        },
        Err(error) => json!({"ok": false, "error": CliError::Library(error).json()}),
    };
    output["captures_written"] = captured.into();
    if !secondary_errors.is_empty() {
        output["secondary_errors"] = secondary_errors.into();
    }
    Ok(output)
}
