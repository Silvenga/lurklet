use chrono::{DateTime, Utc};
use lurklet_reddit::Fetched;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FetchedResponse<T> {
    /// When Lurklet successfully fetched this result from Reddit.
    fetched_at: DateTime<Utc>,
    #[serde(flatten)]
    value: T,
}

impl<T, U: From<T>> From<Fetched<T>> for FetchedResponse<U> {
    fn from(result: Fetched<T>) -> Self {
        Self {
            fetched_at: result.fetched_at,
            value: result.value.into(),
        }
    }
}
