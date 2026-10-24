use super::retry_after::retry_after_json;
use super::transport_kind;
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
        Error::InvalidResponse(reason) => {
            let reason = match reason {
                ResponseError::BodyTooLarge => "body_too_large",
                ResponseError::InvalidJson => "invalid_json",
                ResponseError::InvalidTokenResponse => "invalid_token_response",
                ResponseError::InvalidJwt => "invalid_jwt",
                ResponseError::UnexpectedSubject => "unexpected_subject",
                ResponseError::InvalidExpiry => "invalid_expiry",
                ResponseError::ExpiredOnArrival => "expired_on_arrival",
                ResponseError::InvalidLoid => "invalid_loid",
                _ => "unknown",
            };
            json!({"kind": "invalid_response", "reason": reason})
        }
        Error::InvalidState { field } => json!({"kind": "invalid_state", "field": field}),
        Error::SessionChanged => json!({"kind": "session_changed"}),
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
}
