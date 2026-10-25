use crate::backends::mobile::graphql::{GraphQlDecodeError, GraphQlErrorKind, GraphQlOutcome};
use crate::{Error, ResponseError, TransportError};
use serde_json::{Map, Value};
use std::fmt;

/// HTTP context, the original envelope, and an optional operation-specific projection.
pub struct GraphQlResponse<T> {
    pub http_status: u16,
    pub response_headers: Vec<(String, Vec<u8>)>,
    pub envelope: Option<Map<String, Value>>,
    pub data: Option<T>,
    pub http_error: Option<Error>,
    pub body_error: Option<TransportError>,
    pub body_truncated: bool,
    pub decode_error: Option<GraphQlDecodeError>,
}

impl<T> GraphQlResponse<T> {
    pub(crate) fn into_data(self) -> Result<T, Error> {
        if let Some(error) = self.http_error {
            return Err(error);
        }
        if let Some(error) = self.body_error {
            return Err(error.into());
        }
        if self.body_truncated {
            return Err(ResponseError::BodyTooLarge.into());
        }
        if self.data.is_none() && !self.errors().is_empty() {
            return Err(Error::OperationFailed);
        }
        if let Some(error) = self.decode_error {
            return Err(match error {
                GraphQlDecodeError::InvalidJson => ResponseError::InvalidJson,
                GraphQlDecodeError::UnsupportedData => ResponseError::UnsupportedData,
                GraphQlDecodeError::InvalidEnvelope
                | GraphQlDecodeError::InvalidData
                | GraphQlDecodeError::UnsupportedFormat => ResponseError::InvalidOperationResponse,
            }
            .into());
        }
        self.data.ok_or(Error::Unavailable)
    }

    /// Returns None for missing data and Some(Null) for explicit null data.
    pub fn raw_data(&self) -> Option<&Value> {
        self.envelope.as_ref()?.get("data")
    }

    /// Returns complete server error entries without narrowing their fields.
    pub fn errors(&self) -> &[Value] {
        self.envelope
            .as_ref()
            .and_then(|envelope| envelope.get("errors"))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Classifies each server error without discarding the original entries.
    pub fn error_kinds(&self) -> Vec<GraphQlErrorKind> {
        self.errors()
            .iter()
            .map(GraphQlErrorKind::from_error)
            .collect()
    }

    /// Distinguishes transport/protocol failure from partial and complete GraphQL results.
    pub fn outcome(&self) -> GraphQlOutcome {
        if !(200..=299).contains(&self.http_status)
            || self.http_error.is_some()
            || self.body_error.is_some()
            || self.body_truncated
            || self.decode_error.is_some()
        {
            GraphQlOutcome::Failed
        } else if !self.errors().is_empty() {
            if self.raw_data().is_some_and(|data| !data.is_null()) {
                GraphQlOutcome::Partial
            } else {
                GraphQlOutcome::Failed
            }
        } else if self.raw_data().is_some() {
            GraphQlOutcome::Complete
        } else {
            GraphQlOutcome::Failed
        }
    }
}

impl<T> fmt::Debug for GraphQlResponse<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GraphQlResponse")
            .field("http_status", &self.http_status)
            .field("outcome", &self.outcome())
            .field("error_count", &self.errors().len())
            .field("decode_error", &self.decode_error)
            .finish_non_exhaustive()
    }
}
