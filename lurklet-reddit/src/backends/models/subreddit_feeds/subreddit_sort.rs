use serde::{Deserialize, Serialize};

/// Supported subreddit ordering; only Top accepts a time range.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubredditSort {
    #[default]
    Hot,
    New,
    Top(TimeRange),
}

/// The time window used when listing top posts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeRange {
    Hour,
    Day,
    Week,
    Month,
    Year,
    #[default]
    All,
}
