use crate::mobile::graphql::{GraphQlDecodeError, GraphQlResponse};
use crate::mobile::http_error::http_error;
use crate::mobile::http_response::HttpResponse;
use serde_json::Value;

/// Decodes the envelope independently of HTTP status, then projects only the requested data.
pub fn decode_response<T>(
    response: HttpResponse,
    project: impl FnOnce(&Value) -> Result<Option<T>, GraphQlDecodeError>,
) -> GraphQlResponse<T> {
    let multipart = response
        .headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("multipart/mixed")
        });
    let mut decoded = GraphQlResponse {
        http_status: response.status,
        http_error: http_error(response.status, &response.headers),
        response_headers: response
            .headers
            .iter()
            .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
            .collect(),
        envelope: None,
        data: None,
        body_error: response.body_error,
        body_truncated: response.body_truncated,
        decode_error: None,
    };
    if decoded.body_error.is_some() || decoded.body_truncated {
        tracing::debug!(
            status = decoded.http_status,
            "Skipping GraphQL decoding because the response body is incomplete"
        );
        return decoded;
    }
    if multipart {
        tracing::debug!(
            status = decoded.http_status,
            "GraphQL multipart response is unsupported"
        );
        decoded.decode_error = Some(GraphQlDecodeError::UnsupportedFormat);
        return decoded;
    }
    let envelope = match serde_json::from_slice(&response.body) {
        Ok(Value::Object(envelope)) => envelope,
        Ok(_) => {
            tracing::debug!(
                status = decoded.http_status,
                "GraphQL response is not an object"
            );
            decoded.decode_error = Some(GraphQlDecodeError::InvalidEnvelope);
            return decoded;
        }
        Err(_) => {
            tracing::debug!(status = decoded.http_status, "GraphQL response is not JSON");
            decoded.decode_error = Some(GraphQlDecodeError::InvalidJson);
            return decoded;
        }
    };
    decoded.envelope = Some(envelope);
    let envelope = decoded.envelope.as_ref().expect("envelope was stored");
    if envelope
        .get("errors")
        .is_some_and(|errors| !errors.is_null() && !errors.is_array())
        || (!envelope.contains_key("data") && decoded.errors().is_empty())
    {
        decoded.decode_error = Some(GraphQlDecodeError::InvalidEnvelope);
        return decoded;
    }
    if let Some(data) = decoded.raw_data().filter(|data| !data.is_null()) {
        match project(data) {
            Ok(data) => decoded.data = data,
            Err(error) => decoded.decode_error = Some(error),
        }
    }
    tracing::trace!(status = decoded.http_status, outcome = ?decoded.outcome(), "Decoded GraphQL response");
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, GraphQlErrorKind, GraphQlOutcome};
    use serde_json::json;
    use wreq::header::{HeaderMap, HeaderValue};

    fn response(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            headers: HeaderMap::new(),
            body: body.as_bytes().into(),
            body_error: None,
            body_truncated: false,
        }
    }

    fn identity(data: &Value) -> Result<Option<Value>, GraphQlDecodeError> {
        Ok(Some(data.clone()))
    }

    #[test]
    fn when_data_is_missing_or_null_then_decoding_should_preserve_the_distinction() {
        let missing = response(200, r#"{"errors":[{"message":"failure"}]}"#);
        let null = response(200, r#"{"data":null,"errors":[{"message":"failure"}]}"#);

        let missing = decode_response(missing, identity);
        let null = decode_response(null, identity);

        assert_eq!(missing.raw_data(), None);
        assert_eq!(null.raw_data(), Some(&Value::Null));
        assert_eq!(missing.outcome(), GraphQlOutcome::Failed);
        assert_eq!(null.outcome(), GraphQlOutcome::Failed);
    }

    #[test]
    fn when_partial_data_contains_errors_then_all_envelope_fields_should_survive() {
        let body = json!({
            "data": {"fixture": [1, null, 1]},
            "errors": [{"message": "private-fixture", "path": ["fixture", 1], "locations": [{"line": 3, "column": 4}], "extensions": {"code": "INVALID_INPUT", "unknown": true}}],
            "extensions": {"traceID": "fixture-trace"}, "futureField": {"retained": true}
        });

        let decoded = decode_response(response(200, &body.to_string()), identity);

        assert_eq!(decoded.outcome(), GraphQlOutcome::Partial);
        assert_eq!(decoded.envelope.as_ref(), body.as_object());
        assert_eq!(decoded.data.as_ref(), body.get("data"));
        assert_eq!(decoded.error_kinds(), [GraphQlErrorKind::InvalidInput]);
        assert!(!format!("{decoded:?}").contains("private-fixture"));
    }

    #[test]
    fn when_json_errors_have_an_http_failure_then_both_classifications_should_be_retained() {
        let body =
            r#"{"errors":[{"message":"Invalid input","extensions":{"code":"INVALID_INPUT"}}]}"#;
        let mut http = response(400, body);
        http.headers
            .insert("x-ratelimit-remaining", HeaderValue::from_static("0"));

        let decoded = decode_response(http, identity);

        assert!(matches!(
            decoded.http_error,
            Some(Error::UnexpectedStatus { status: 400 })
        ));
        assert_eq!(decoded.error_kinds(), [GraphQlErrorKind::InvalidInput]);
        assert_eq!(decoded.outcome(), GraphQlOutcome::Failed);
        assert!(
            decoded
                .response_headers
                .contains(&("x-ratelimit-remaining".into(), b"0".to_vec()))
        );
    }

    #[test]
    fn when_a_block_page_is_not_json_then_decoding_should_not_hide_the_http_failure() {
        let decoded = decode_response(response(403, "<html>blocked</html>"), identity);

        assert!(matches!(
            decoded.http_error,
            Some(Error::AccessDenied { status: 403 })
        ));
        assert_eq!(decoded.decode_error, Some(GraphQlDecodeError::InvalidJson));
        assert_eq!(decoded.outcome(), GraphQlOutcome::Failed);
    }

    #[test]
    fn when_a_multipart_response_arrives_then_decoding_should_not_treat_any_part_as_complete() {
        let mut http = response(200, r#"{"data":{"fixture":true}}"#);
        http.headers.insert(
            "content-type",
            HeaderValue::from_static("Multipart/Mixed; boundary=fixture"),
        );

        let decoded = decode_response(http, identity);

        assert_eq!(
            decoded.decode_error,
            Some(GraphQlDecodeError::UnsupportedFormat)
        );
        assert!(decoded.envelope.is_none());
        assert!(decoded.data.is_none());
    }

    #[test]
    fn when_projection_fails_then_the_original_data_should_remain_available() {
        let body = r#"{"data":{"unexpected":"retained"},"extensions":{"traceID":"fixture"}}"#;

        let decoded = decode_response::<()>(response(200, body), |_| {
            Err(GraphQlDecodeError::InvalidData)
        });

        assert_eq!(decoded.decode_error, Some(GraphQlDecodeError::InvalidData));
        assert_eq!(decoded.raw_data(), Some(&json!({"unexpected": "retained"})));
        assert_eq!(decoded.outcome(), GraphQlOutcome::Failed);
    }

    #[test]
    fn when_the_body_is_truncated_then_a_valid_json_prefix_should_not_be_exposed_as_complete() {
        let mut http = response(200, r#"{"data":{"fixture":true}}"#);
        http.body_truncated = true;

        let decoded = decode_response(http, identity);

        assert_eq!(decoded.outcome(), GraphQlOutcome::Failed);
        assert!(decoded.envelope.is_none());
        assert!(decoded.body_truncated);
    }

    #[test]
    fn when_the_envelope_is_unrecognizable_then_decoding_should_fail_without_validating_a_schema() {
        for body in ["[]", "{}", r#"{"data":{},"errors":"failure"}"#] {
            let decoded = decode_response(response(200, body), identity);

            assert_eq!(
                decoded.decode_error,
                Some(GraphQlDecodeError::InvalidEnvelope)
            );
            assert_eq!(decoded.outcome(), GraphQlOutcome::Failed);
        }
    }

    #[test]
    fn when_nullable_or_unknown_fields_are_returned_then_decoding_should_preserve_them() {
        let body = r#"{"data":null,"errors":null,"extensions":{"future":123}}"#;

        let decoded = decode_response(response(200, body), identity);

        assert_eq!(decoded.outcome(), GraphQlOutcome::Complete);
        assert_eq!(decoded.raw_data(), Some(&Value::Null));
        assert_eq!(
            decoded.envelope.unwrap()["extensions"],
            json!({"future": 123})
        );
    }

    #[tokio::test]
    async fn when_body_reading_fails_then_decoding_should_preserve_http_and_transport_failures() {
        use crate::TransportError;
        use futures_util::stream;
        use std::io;

        let body = wreq::Body::wrap_stream(stream::iter([Err::<Vec<u8>, _>(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "fixture",
        ))]));
        let raw: wreq::Response = http::Response::builder().body(body).unwrap().into();
        let error = raw.bytes().await.unwrap_err();
        let mut http = response(429, "partial");
        http.body_error = Some(TransportError::from(error));

        let decoded = decode_response(http, identity);

        assert!(matches!(
            decoded.http_error,
            Some(Error::RateLimited { .. })
        ));
        assert!(decoded.body_error.is_some());
        assert!(decoded.decode_error.is_none());
        assert!(decoded.envelope.is_none());
        assert_eq!(decoded.outcome(), GraphQlOutcome::Failed);
    }
}
