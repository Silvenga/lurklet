use axum::body::Body;
use axum::http::HeaderMap;
use axum::response::Response;
use bytes::Bytes;
use headers::{Age, HeaderMapExt};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// One complete representation, committed atomically with its freshness metadata.
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub version: u8,
    pub key: String,
    pub stored_at: SystemTime,
    pub fresh_for: Duration,
    // Retained for the existing v1 record layout until the storage-format follow-up.
    pub initial_age: Duration,
    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Entry {
    /// Checks freshness without extending the persisted lifetime.
    pub fn is_fresh(&self) -> bool {
        self.stored_at
            .elapsed()
            .is_ok_and(|elapsed| elapsed < self.fresh_for)
    }

    /// Restores representation headers and accounts for elapsed cache age.
    pub fn into_response(self) -> Response {
        let age = self.stored_at.elapsed().unwrap_or_default();
        let mut response = Response::new(Body::from(self.body));
        *response.headers_mut() = self.headers;
        response
            .headers_mut()
            .typed_insert(Age::from_secs(age.as_secs()));
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header;

    #[test]
    fn when_residence_time_has_fractional_seconds_then_age_should_use_whole_seconds() {
        let stored_at = SystemTime::now() - Duration::from_millis(1250);
        let entry = Entry {
            version: 1,
            key: "/resource".into(),
            stored_at,
            fresh_for: Duration::from_secs(300),
            initial_age: Duration::ZERO,
            headers: HeaderMap::new(),
            body: Bytes::new(),
        };
        let before = stored_at.elapsed().unwrap().as_secs();

        let response = entry.into_response();
        let after = stored_at.elapsed().unwrap().as_secs();
        let age = response.headers()[header::AGE]
            .to_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();

        assert!((before..=after).contains(&age));
    }
}
