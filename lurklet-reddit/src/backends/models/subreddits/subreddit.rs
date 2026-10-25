use crate::RedditId;
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Community metadata, including explanations when the community cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Subreddit {
    Available(Box<SubredditDetails>),
    Unavailable(UnavailableSubreddit),
}

/// Displayable community metadata, independent of the upstream API.
/// Markdown is untrusted; consumers must restrict embedded HTML and URL schemes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubredditDetails {
    pub id: RedditId,
    pub name: String,
    pub title: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub short_description: Option<String>,
    /// Long-form prose, not structured sidebar widgets or rules.
    pub description_markdown: Option<String>,
    pub subscriber_count: Option<u64>,
    /// Weekly activity, not a count of currently online users.
    pub weekly_active_user_count: Option<u64>,
    pub weekly_contribution_count: Option<u64>,
    pub styles: Option<SubredditStyles>,
    /// The reported type, retaining unfamiliar values without inference from restriction flags.
    pub community_type: Option<String>,
    pub is_nsfw: Option<bool>,
    pub is_posting_restricted: Option<bool>,
    pub is_quarantined: Option<bool>,
    pub quarantine_message_markdown: Option<String>,
    pub interstitial_warning_markdown: Option<String>,
}

/// Returned appearance values; asset URLs are opaque and no variant is preferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubredditStyles {
    pub icon_url: Option<String>,
    pub legacy_icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub mobile_banner_url: Option<String>,
    pub legacy_banner_url: Option<String>,
    pub primary_color: Option<String>,
    pub legacy_primary_color: Option<String>,
}

/// Reported unavailability, without inventing identity fields or enabling opt-in behavior.
/// Markdown is untrusted; consumers must restrict embedded HTML and URL schemes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnavailableSubreddit {
    pub id: Option<RedditId>,
    /// The reported reason, including values unknown to this client.
    pub forbidden_reason: Option<String>,
    pub ban_title: Option<String>,
    pub ban_message_markdown: Option<String>,
    pub quarantine_message_markdown: Option<String>,
    pub interstitial_warning_markdown: Option<String>,
}
