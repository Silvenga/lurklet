use serde::Serialize;

/// Whether a response completed cleanly, returned partial data, or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphQlOutcome {
    Complete,
    Partial,
    Failed,
}
