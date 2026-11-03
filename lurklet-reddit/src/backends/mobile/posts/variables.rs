use crate::RedditId;
use serde::Serialize;

#[derive(Serialize)]
pub struct Variables {
    ids: [String; 1],
}

impl Variables {
    pub fn new(id: &RedditId) -> Self {
        Self {
            ids: [format!("t3_{}", id.as_str())],
        }
    }
}
