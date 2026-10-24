use crate::output::{CliError, transport_kind};
use lurklet_reddit::{ResponseCapture, ResponseObserver};
use serde_json::json;
use std::fs;
use std::mem;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::Builder;

#[derive(Clone, Default)]
pub struct Recorder {
    responses: Arc<Mutex<Vec<ResponseCapture>>>,
}

impl ResponseObserver for Recorder {
    fn on_response(&self, response: ResponseCapture) {
        self.responses
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(response);
    }
}

impl Recorder {
    pub fn write(&self, directory: &Path) -> Result<usize, CliError> {
        fs::create_dir_all(directory)
            .map_err(|error| CliError::io("create_capture_directory", error))?;
        let responses = mem::take(
            &mut *self
                .responses
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        );
        let count = responses.len();
        for response in responses {
            let temporary = Builder::new()
                .prefix("response-")
                .tempdir_in(directory)
                .map_err(|error| CliError::io("create_capture_directory", error))?;
            let metadata = json!({
                "operation": response.operation,
                "url": response.url,
                "status": response.status,
                "http_version": response.http_version,
                "elapsed_ms": response.elapsed.as_millis(),
                "body_bytes": response.body.len(),
                "body_truncated": response.body_truncated,
                "body_error": response.body_error.map(transport_kind),
                "representation": "decoded response as exposed by wreq, not a wire capture",
            });
            let mut headers = Vec::new();
            for (name, value) in response.headers {
                headers.extend_from_slice(name.as_bytes());
                headers.extend_from_slice(b": ");
                headers.extend_from_slice(&value);
                headers.extend_from_slice(b"\n");
            }
            fs::write(temporary.path().join("response.json"), metadata.to_string())
                .map_err(|error| CliError::io("write_capture", error))?;
            fs::write(temporary.path().join("headers.bin"), headers)
                .map_err(|error| CliError::io("write_capture", error))?;
            fs::write(temporary.path().join("body.bin"), response.body)
                .map_err(|error| CliError::io("write_capture", error))?;
            let _ = temporary.keep();
        }
        tracing::trace!(count, "Response captures written");
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use serde_json::Value;
    use std::time::Duration;

    #[test]
    fn when_a_response_is_recorded_then_capture_files_should_preserve_the_diagnostic_data() {
        let directory = TempDir::new().unwrap();
        let capture_dir = directory.path().join("captures");
        let recorder = Recorder::default();
        recorder.on_response(ResponseCapture {
            operation: "bootstrap",
            url: "https://example.invalid/".into(),
            status: 403,
            http_version: "HTTP/2.0".into(),
            headers: vec![("set-cookie".into(), b"private-cookie".to_vec())],
            body: b"blocked".to_vec(),
            body_truncated: true,
            body_error: None,
            elapsed: Duration::from_millis(42),
        });

        let count = recorder.write(&capture_dir).unwrap();
        let path = fs::read_dir(&capture_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let metadata: Value =
            serde_json::from_slice(&fs::read(path.join("response.json")).unwrap()).unwrap();

        assert_eq!(count, 1);
        assert_eq!(fs::read(path.join("body.bin")).unwrap(), b"blocked");
        assert_eq!(
            fs::read(path.join("headers.bin")).unwrap(),
            b"set-cookie: private-cookie\n"
        );
        assert_eq!(metadata["status"], 403);
        assert_eq!(metadata["body_truncated"], true);
        assert_eq!(recorder.write(&capture_dir).unwrap(), 0);
    }
}
