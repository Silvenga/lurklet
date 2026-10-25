use crate::RedditId;
use serde::{Deserialize, Serialize};

/// Identity and display name for an entity whose full details can be retrieved separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedReference {
    pub id: Option<RedditId>,
    pub name: Option<String>,
}
