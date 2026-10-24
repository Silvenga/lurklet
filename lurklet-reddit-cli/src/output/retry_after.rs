use lurklet_reddit::RetryAfter;
use serde_json::{Value, json};
use std::time::UNIX_EPOCH;

pub fn retry_after_json(value: &Option<RetryAfter>) -> Value {
    match value {
        Some(RetryAfter::Delay(duration)) => json!({"delay_seconds": duration.as_secs()}),
        Some(RetryAfter::At(time)) => {
            let timestamp = match time.duration_since(UNIX_EPOCH) {
                Ok(duration) => i128::from(duration.as_secs()),
                Err(error) => -i128::from(error.duration().as_secs()),
            };
            json!({"at_unix_seconds": timestamp})
        }
        None => Value::Null,
    }
}
