use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// An access token that is redacted when formatted for debugging.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccessToken(String);

impl AccessToken {
    /// Wraps a token restored by the caller; bootstrap validates it before reuse.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Explicitly exposes the credential for authorized requests or storage.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccessToken([REDACTED])")
    }
}

/// Anonymous credentials with their session-path operational expiration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnonymousSession {
    pub access_token: AccessToken,
    pub expires_at_ms: u64,
}

/// Installation identity, independent of tokens and analytics sessions.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installation {
    pub id: Uuid,
    pub created_at_ms: u64,
}

impl fmt::Debug for Installation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Installation([REDACTED])")
    }
}

/// Authentication credentials and the opaque server-provided LOID.
#[derive(Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticationState {
    pub session: Option<AnonymousSession>,
    pub loid: Option<String>,
}

impl fmt::Debug for AuthenticationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthenticationState([REDACTED])")
    }
}

/// Analytics identity; header generation is intentionally not implemented yet.
#[derive(Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyticsSession {
    pub session_id: Option<String>,
    pub creation_timestamp_ms: Option<u64>,
}

impl fmt::Debug for AnalyticsSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AnalyticsSession([REDACTED])")
    }
}

/// Caller-owned state for one logical logged-out session.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientState {
    pub installation: Option<Installation>,
    pub authentication: AuthenticationState,
    pub analytics: AnalyticsSession,
}

/// A state snapshot with a monotonically increasing revision.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub revision: u64,
    pub state: ClientState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_state_is_formatted_then_tokens_and_identities_should_be_redacted() {
        let state = ClientState {
            installation: Some(Installation {
                id: Uuid::new_v4(),
                created_at_ms: 1,
            }),
            authentication: AuthenticationState {
                session: Some(AnonymousSession {
                    access_token: AccessToken::new("secret-token"),
                    expires_at_ms: 1,
                }),
                loid: Some("private-loid".into()),
            },
            analytics: AnalyticsSession {
                session_id: Some("private-session".into()),
                creation_timestamp_ms: Some(1),
            },
        };

        let formatted = format!("{state:?}");
        let serialized = serde_json::to_string(&state).unwrap();
        let restored: ClientState = serde_json::from_str(&serialized).unwrap();

        for secret in [
            "secret-token",
            "private-loid",
            "private-session",
            &state.installation.as_ref().unwrap().id.to_string(),
        ] {
            assert!(!formatted.contains(secret));
        }
        assert_eq!(state, restored);
    }
}
