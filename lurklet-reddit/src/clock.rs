use crate::ClockError;
use std::time::{SystemTime, UNIX_EPOCH};

/// Supplies time for session expiration and installation creation.
pub trait Clock: Send + Sync {
    /// Returns milliseconds elapsed since the Unix epoch.
    fn now_ms(&self) -> Result<u64, ClockError>;
}

/// The operating system's wall clock.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ClockError::BeforeEpoch)?;
        u64::try_from(elapsed.as_millis()).map_err(|_| ClockError::Overflow)
    }
}
