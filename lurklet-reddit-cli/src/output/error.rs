use super::library_error;
use super::transport_kind;
use lurklet_reddit::{BuildError, Error};
use serde_json::{Value, json};
use std::io;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CliError {
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error(transparent)]
    Library(#[from] Error),
    #[error("{operation} failed ({kind:?})")]
    Io {
        operation: &'static str,
        kind: io::ErrorKind,
    },
    #[error("another process is using the selected state file")]
    StateBusy,
    #[error("state file is malformed or has an unsupported format version")]
    StateFormat,
    #[error("invalid RUST_LOG configuration or unavailable tracing subscriber")]
    Logging,
    #[error("a filesystem worker failed")]
    Worker,
}

impl CliError {
    pub fn io(operation: &'static str, error: io::Error) -> Self {
        Self::Io {
            operation,
            kind: error.kind(),
        }
    }

    pub fn json(&self) -> Value {
        let mut result = match self {
            Self::Build(BuildError::InvalidConfiguration { field }) => {
                json!({"kind": "invalid_configuration", "field": field})
            }
            Self::Build(BuildError::InvalidEndpoint { field }) => {
                json!({"kind": "invalid_endpoint", "field": field})
            }
            Self::Build(BuildError::Transport(error)) => {
                json!({"kind": "transport", "reason": transport_kind(error.kind)})
            }
            Self::Build(_) => json!({"kind": "build"}),
            Self::Library(error) => library_error::to_json(error),
            Self::Io { operation, kind } => {
                json!({"kind": "io", "operation": operation, "reason": format!("{kind:?}")})
            }
            Self::StateBusy => json!({"kind": "state_busy"}),
            Self::StateFormat => json!({"kind": "state_format"}),
            Self::Logging => json!({"kind": "logging"}),
            Self::Worker => json!({"kind": "worker"}),
        };
        result["message"] = self.to_string().into();
        result
    }
}
