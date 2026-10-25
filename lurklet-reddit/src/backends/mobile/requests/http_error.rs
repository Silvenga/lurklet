use crate::{Error, RetryAfter};
use std::time::Duration;
use wreq::header::HeaderMap;

/// Classifies HTTP status independently of any response-body protocol.
pub fn http_error(status: u16, headers: &HeaderMap) -> Option<Error> {
    let retry_after = retry_after(headers);
    match status {
        200..=299 => None,
        401 => Some(Error::Authentication { status }),
        403 => Some(Error::AccessDenied { status }),
        429 => Some(Error::RateLimited { retry_after }),
        500..=599 => Some(Error::Upstream {
            status,
            retry_after,
        }),
        _ => Some(Error::UnexpectedStatus { status }),
    }
}

/// Parses server retry guidance without scheduling a retry.
pub fn retry_after(headers: &HeaderMap) -> Option<RetryAfter> {
    headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            if let Ok(seconds) = value.parse::<u64>() {
                Some(RetryAfter::Delay(Duration::from_secs(seconds)))
            } else {
                httpdate::parse_http_date(value).ok().map(RetryAfter::At)
            }
        })
}
