use serde::{Deserialize, Serialize};

/// Supported discussion ordering; Best is the client's default selection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentSort {
    #[default]
    Best,
    Top,
    New,
}
