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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RetryAfter;
    use futures_util::stream;
    use serde_json::json;
    use std::io;
    use std::time::Duration;

    fn response() -> GraphQlResponse<u8> {
        GraphQlResponse {
            http_status: 200,
            response_headers: Vec::new(),
            envelope: None,
            data: Some(7),
            http_error: None,
            body_error: None,
            body_truncated: false,
            decode_error: None,
        }
    }

    #[test]
    fn when_projected_data_is_available_or_missing_then_conversion_should_distinguish_partial_success_failure_and_unavailability()
     {
        for (data, errors) in [
            (Some(7), json!([])),
            (Some(7), json!([{"message": "private upstream detail"}])),
            (None, json!([])),
            (None, json!([{"message": "private upstream detail"}])),
        ] {
            let has_errors = !errors.as_array().unwrap().is_empty();
            let response = GraphQlResponse {
                data,
                envelope: json!({"data": data, "errors": errors}).as_object().cloned(),
                ..response()
            };

            let result = response.into_data();

            match (data, has_errors) {
                (Some(expected), _) => assert_eq!(result.unwrap(), expected),
                (None, false) => assert!(matches!(result, Err(Error::Unavailable))),
                (None, true) => {
                    assert!(matches!(result, Err(Error::OperationFailed)));
                    assert!(!format!("{result:?}").contains("private upstream detail"));
                }
            }
        }
    }

    #[test]
    fn when_decoding_fails_then_conversion_should_map_the_reason_unless_graphql_already_reports_failure()
     {
        for (decode_error, expected) in [
            (GraphQlDecodeError::InvalidJson, ResponseError::InvalidJson),
            (
                GraphQlDecodeError::UnsupportedData,
                ResponseError::UnsupportedData,
            ),
            (
                GraphQlDecodeError::InvalidEnvelope,
                ResponseError::InvalidOperationResponse,
            ),
            (
                GraphQlDecodeError::InvalidData,
                ResponseError::InvalidOperationResponse,
            ),
            (
                GraphQlDecodeError::UnsupportedFormat,
                ResponseError::InvalidOperationResponse,
            ),
        ] {
            for errors in [json!([]), json!([{"message": "operation failed"}])] {
                let has_errors = !errors.as_array().unwrap().is_empty();
                let response = GraphQlResponse {
                    data: None,
                    envelope: json!({"errors": errors}).as_object().cloned(),
                    decode_error: Some(decode_error),
                    ..response()
                };

                let result = response.into_data();

                if has_errors {
                    assert!(matches!(result, Err(Error::OperationFailed)));
                } else {
                    assert!(
                        matches!(result, Err(Error::InvalidResponse(reason)) if reason == expected)
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn when_failures_overlap_then_http_body_and_truncation_should_take_precedence_over_data_and_decoding()
     {
        let body = wreq::Body::wrap_stream(stream::iter([Err::<Vec<u8>, _>(io::Error::from(
            io::ErrorKind::UnexpectedEof,
        ))]));
        let raw: wreq::Response = http::Response::builder().body(body).unwrap().into();
        let error = TransportError::from(raw.bytes().await.unwrap_err());
        let kind = error.kind;
        let retry_after = RetryAfter::Delay(Duration::from_secs(7));

        for (http_error, body_error, expected) in [
            (
                Some(Error::RateLimited {
                    retry_after: Some(retry_after.clone()),
                }),
                Some(error.clone()),
                "http",
            ),
            (None, Some(error), "body"),
            (None, None, "truncated"),
        ] {
            let response = GraphQlResponse {
                http_status: if http_error.is_some() { 429 } else { 200 },
                http_error,
                body_error,
                body_truncated: true,
                decode_error: Some(GraphQlDecodeError::InvalidJson),
                ..response()
            };

            let result = response.into_data();

            match expected {
                "http" => assert!(
                    matches!(result, Err(Error::RateLimited { retry_after: Some(actual) }) if actual == retry_after)
                ),
                "body" => {
                    assert!(matches!(result, Err(Error::Transport(actual)) if actual.kind == kind))
                }
                "truncated" => assert!(matches!(
                    result,
                    Err(Error::InvalidResponse(ResponseError::BodyTooLarge))
                )),
                _ => unreachable!(),
            }
        }
    }
}
