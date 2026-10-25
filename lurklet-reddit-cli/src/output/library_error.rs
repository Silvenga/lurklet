use crate::output::retry_after::retry_after_json;
use crate::output::transport_kind;
use lurklet_reddit::{Error, ResponseError};
use serde_json::{Value, json};

pub fn to_json(error: &Error) -> Value {
    match error {
        Error::Storage(reason) => json!({"kind": "storage", "reason": format!("{reason:?}")}),
        Error::Clock(reason) => json!({"kind": "clock", "reason": format!("{reason:?}")}),
        Error::Transport(error) => {
            json!({"kind": "transport", "reason": transport_kind(error.kind)})
        }
        Error::Authentication { status } => json!({"kind": "authentication", "status": status}),
        Error::AccessDenied { status } => json!({"kind": "access_denied", "status": status}),
        Error::RateLimited { retry_after } => {
            json!({"kind": "rate_limited", "status": 429, "retry_after": retry_after_json(retry_after)})
        }
        Error::Upstream {
            status,
            retry_after,
        } => {
            json!({"kind": "upstream", "status": status, "retry_after": retry_after_json(retry_after)})
        }
        Error::UnexpectedStatus { status } => {
            json!({"kind": "unexpected_status", "status": status})
        }
        Error::MediaStatus {
            status,
            retry_after,
        } => {
            json!({"kind": "media_status", "status": status, "retry_after": retry_after_json(retry_after)})
        }
        Error::InvalidResponse(reason) => {
            let reason = match reason {
                ResponseError::BodyTooLarge => "body_too_large",
                ResponseError::InvalidJson => "invalid_json",
                ResponseError::InvalidTokenResponse => "invalid_token_response",
                ResponseError::InvalidAccessToken => "invalid_access_token",
                ResponseError::InvalidExpiry => "invalid_expiry",
                ResponseError::ExpiredOnArrival => "expired_on_arrival",
                ResponseError::InvalidLoid => "invalid_loid",
                ResponseError::InvalidPostResponse => "invalid_post_response",
                ResponseError::InvalidOperationResponse => "invalid_operation_response",
                ResponseError::UnsupportedData => "unsupported_data",
                ResponseError::UnsupportedMediaType => "unsupported_media_type",
                ResponseError::UnsupportedMediaEncoding => "unsupported_media_encoding",
                _ => "unknown",
            };
            json!({"kind": "invalid_response", "reason": reason})
        }
        Error::InvalidState { field } => json!({"kind": "invalid_state", "field": field}),
        Error::InvalidRequest { field } => json!({"kind": "invalid_request", "field": field}),
        Error::SessionChanged => json!({"kind": "session_changed"}),
        Error::OperationFailed => json!({"kind": "operation_failed"}),
        Error::Unavailable => json!({"kind": "unavailable"}),
        Error::NotFound => json!({"kind": "not_found"}),
        _ => json!({"kind": "library"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::CliError;
    use lurklet_reddit::RetryAfter;
    use std::time::Duration;

    #[test]
    fn when_a_post_operation_fails_then_json_should_not_expose_upstream_protocol_details() {
        let error = CliError::Library(Error::OperationFailed);

        let output = error.json();

        assert_eq!(
            output,
            json!({"kind": "operation_failed", "message": "Reddit could not complete the operation"})
        );
    }

    #[test]
    fn when_content_is_unavailable_then_json_should_not_claim_it_was_not_found() {
        let error = CliError::Library(Error::Unavailable);

        let output = error.json();

        assert_eq!(
            output,
            json!({"kind": "unavailable", "message": "the requested content is unavailable"})
        );
    }

    #[test]
    fn when_reddit_reports_a_missing_resource_then_json_should_expose_the_high_level_error() {
        let error = CliError::Library(Error::NotFound);

        let output = error.json();

        assert_eq!(output["kind"], "not_found");
    }

    #[test]
    fn when_an_operation_response_is_invalid_then_json_should_use_the_shared_error_category() {
        let error = CliError::Library(Error::InvalidResponse(
            ResponseError::InvalidOperationResponse,
        ));

        let output = error.json();

        assert_eq!(output["reason"], "invalid_operation_response");
    }

    #[test]
    fn when_a_post_cannot_be_decoded_then_json_should_identify_the_response_failure() {
        let error = CliError::Library(Error::InvalidResponse(ResponseError::InvalidPostResponse));

        let output = error.json();

        assert_eq!(output["reason"], "invalid_post_response");
    }

    #[test]
    fn when_reddit_rate_limits_then_json_should_preserve_retry_after() {
        let error = CliError::Library(Error::RateLimited {
            retry_after: Some(RetryAfter::Delay(Duration::from_secs(60))),
        });

        let output = error.json();

        assert_eq!(output["kind"], "rate_limited");
        assert_eq!(output["status"], 429);
        assert_eq!(output["retry_after"], json!({"delay_seconds": 60}));
    }

    #[test]
    fn when_reddit_denies_access_then_json_should_not_misclassify_it_as_authentication() {
        let error = CliError::Library(Error::AccessDenied { status: 403 });

        let output = error.json();

        assert_eq!(output["kind"], "access_denied");
        assert_eq!(output["status"], 403);
    }

    #[test]
    fn when_a_media_origin_rejects_a_request_then_json_should_report_media_status_without_implying_expired_oauth()
     {
        for status in [401, 403, 429] {
            let error = CliError::Library(Error::MediaStatus {
                status,
                retry_after: Some(RetryAfter::Delay(Duration::from_secs(60))),
            });

            let output = error.json();

            assert_eq!(output["kind"], "media_status");
            assert_eq!(output["status"], status);
            assert_eq!(output["retry_after"], json!({"delay_seconds": 60}));
        }
    }

    #[test]
    fn when_a_media_representation_is_unsupported_then_json_should_report_the_specific_response_failure()
     {
        for (reason, expected) in [
            (
                ResponseError::UnsupportedMediaType,
                "unsupported_media_type",
            ),
            (
                ResponseError::UnsupportedMediaEncoding,
                "unsupported_media_encoding",
            ),
        ] {
            let error = CliError::Library(Error::InvalidResponse(reason));

            let output = error.json();

            assert_eq!(output["kind"], "invalid_response");
            assert_eq!(output["reason"], expected);
        }
    }
}
