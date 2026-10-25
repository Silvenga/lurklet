use crate::commands::CommandOutput;
use crate::output::{CliError, transport_kind};
use lurklet_reddit::{Error, GraphQlOutcome, GraphQlResponse, ResponseError};
use serde::Serialize;
use serde_json::{Value, json};

pub fn graphql_output<T: Serialize>(
    response: GraphQlResponse<T>,
    show_secrets: bool,
) -> CommandOutput {
    let outcome = response.outcome();
    let error_kinds = response.error_kinds();
    let mut error = if let Some(error) = &response.http_error {
        Some(CliError::Library(error.clone()).json())
    } else if let Some(error) = &response.body_error {
        Some(CliError::Library(Error::Transport(error.clone())).json())
    } else if response.body_truncated {
        Some(CliError::Library(Error::InvalidResponse(ResponseError::BodyTooLarge)).json())
    } else if let Some(error) = response.decode_error {
        Some(json!({"kind": "graphql_decode", "reason": error, "message": error.to_string()}))
    } else if outcome != GraphQlOutcome::Complete {
        Some(
            json!({"kind": "graphql", "message": "GraphQL returned an incomplete or failed result"}),
        )
    } else {
        None
    };
    if let Some(error) = &mut error {
        error["graphql_error_kinds"] = json!(error_kinds);
    }
    let headers: Vec<Value> = response
        .response_headers
        .iter()
        .map(|(name, value)| {
            let sensitive = matches!(
                name.to_ascii_lowercase().as_str(),
                "authorization"
                    | "proxy-authorization"
                    | "cookie"
                    | "set-cookie"
                    | "x-reddit-loid"
                    | "x-reddit-session"
                    | "client-vendor-id"
                    | "x-reddit-device-id"
                    | "x-reddit-p-device-id"
                    | "x-dev-ad-id"
            );
            let value = if sensitive && !show_secrets {
                "[REDACTED]".into()
            } else {
                String::from_utf8_lossy(value).into_owned()
            };
            json!({"name": name, "value": value})
        })
        .collect();
    CommandOutput {
        result: json!({
            "outcome": outcome,
            "data": response.data,
            "http": {"status": response.http_status, "headers": headers},
            "graphql": response.envelope,
            "graphql_error_kinds": error_kinds,
            "body_truncated": response.body_truncated,
            "body_error": response.body_error.as_ref().map(|error| transport_kind(error.kind)),
            "decode_error": response.decode_error,
        }),
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lurklet_reddit::GraphQlDecodeError;

    fn response(envelope: Value) -> GraphQlResponse<Value> {
        GraphQlResponse {
            http_status: 200,
            response_headers: vec![("set-cookie".into(), b"private-cookie".to_vec())],
            data: envelope.get("data").filter(|data| !data.is_null()).cloned(),
            envelope: envelope.as_object().cloned(),
            http_error: None,
            body_error: None,
            body_truncated: false,
            decode_error: None,
        }
    }

    #[test]
    fn when_data_is_partial_then_output_should_report_failure_without_discarding_the_data() {
        let envelope = json!({"data": {"fixture": "retained"}, "errors": [{"message": "fixture", "path": ["fixture"], "extensions": {"code": "INVALID_INPUT"}}]});

        let output = graphql_output(response(envelope.clone()), false);

        assert_eq!(output.result["outcome"], "partial");
        assert_eq!(output.result["data"], envelope["data"]);
        assert_eq!(output.result["graphql"], envelope);
        assert_eq!(
            output.error.unwrap()["graphql_error_kinds"],
            json!(["invalid_input"])
        );
        assert_eq!(output.result["http"]["headers"][0]["value"], "[REDACTED]");
    }

    #[test]
    fn when_http_failure_and_decode_failure_coexist_then_output_should_prioritize_http_status() {
        let mut response = response(Value::Null);
        response.http_status = 403;
        response.http_error = Some(Error::AccessDenied { status: 403 });
        response.decode_error = Some(GraphQlDecodeError::InvalidJson);

        let output = graphql_output(response, false);

        assert_eq!(output.error.unwrap()["kind"], "access_denied");
        assert_eq!(output.result["decode_error"], "invalid_json");
        assert_eq!(output.result["http"]["status"], 403);
    }

    #[test]
    fn when_secret_output_is_explicit_then_response_headers_should_be_available() {
        let output = graphql_output(response(json!({"data": {}})), true);

        assert!(output.error.is_none());
        assert_eq!(
            output.result["http"]["headers"][0]["value"],
            "private-cookie"
        );
    }
}
