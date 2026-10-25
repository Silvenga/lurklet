use crate::mobile::backend::MobileBackend;
use crate::{ClientState, Error, StateStore};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use wreq::header::{HeaderMap, HeaderName, HeaderValue};

fn insert(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
    sensitive: bool,
) -> Result<(), Error> {
    let mut value =
        HeaderValue::from_str(value).map_err(|_| Error::InvalidState { field: name })?;
    value.set_sensitive(sensitive);
    headers.insert(HeaderName::from_static(name), value);
    Ok(())
}

impl<S: StateStore> MobileBackend<S> {
    fn shared_headers(&self, state: &ClientState) -> Result<HeaderMap, Error> {
        let installation = state.installation.as_ref().ok_or(Error::InvalidState {
            field: "installation",
        })?;
        let mut headers = HeaderMap::new();
        insert(
            &mut headers,
            "user-agent",
            self.config.user_agent.as_deref().unwrap_or_default(),
            false,
        )?;
        insert(
            &mut headers,
            "client-vendor-id",
            &installation.id.to_string(),
            true,
        )?;
        insert(
            &mut headers,
            "x-reddit-compression",
            &self.config.reddit_compression,
            false,
        )?;
        if let Some(loid) = &state.authentication.loid {
            insert(&mut headers, "x-reddit-loid", loid, true)?;
        }
        if let Some(rate) = self.config.download_rate_mbps {
            insert(
                &mut headers,
                "x-reddit-qos",
                &format!("down-rate-mbps={rate:.3}"),
                false,
            )?;
        }
        Ok(headers)
    }

    pub(super) fn auth_headers(&self, state: &ClientState) -> Result<HeaderMap, Error> {
        let mut headers = self.shared_headers(state)?;
        let credential = STANDARD.encode(format!("{}:", self.config.client_id));
        insert(
            &mut headers,
            "authorization",
            &format!("Basic {credential}"),
            true,
        )?;
        insert(
            &mut headers,
            "content-type",
            "application/json; charset=utf-8",
            false,
        )?;
        Ok(headers)
    }

    pub(super) fn api_headers(&self, state: &ClientState) -> Result<HeaderMap, Error> {
        let mut headers = self.shared_headers(state)?;
        let session = state
            .authentication
            .session
            .as_ref()
            .ok_or(Error::InvalidState {
                field: "authentication",
            })?;
        if session.expires_at_ms <= self.clock.now_ms()? {
            return Err(Error::InvalidState {
                field: "expired_session",
            });
        }
        if session.access_token.expose_secret().trim().is_empty() {
            return Err(Error::InvalidState {
                field: "access_token",
            });
        }
        let vendor = headers["client-vendor-id"].clone();
        headers.insert("x-reddit-device-id", vendor);
        insert(
            &mut headers,
            "authorization",
            &format!("Bearer {}", session.access_token.expose_secret()),
            true,
        )?;
        let device = &self.config.device;
        insert(
            &mut headers,
            "device-name",
            &format!("{};{}", device.manufacturer, device.model),
            false,
        )?;
        insert(
            &mut headers,
            "x-reddit-dpr",
            &device.display_density.to_string(),
            false,
        )?;
        let width = if device.display_density > 0.0 {
            (f64::from(device.display_width_px) / device.display_density) as u64
        } else {
            u64::from(device.display_width_px)
        };
        insert(&mut headers, "x-reddit-width", &width.to_string(), false)?;
        if self.config.include_height {
            let height = if device.display_density > 0.0 {
                (f64::from(device.display_height_px) / device.display_density) as u64
            } else {
                u64::from(device.display_height_px)
            };
            insert(&mut headers, "x-reddit-height", &height.to_string(), false)?;
        }
        Ok(headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile::backend::testing::{self, NOW, TOKEN_FAKE, TestClock};
    use crate::{AccessToken, AnonymousSession, DeviceProfile, RedditClient};

    #[test]
    fn when_building_api_headers_then_authentication_headers_should_remain_separate() {
        let client = RedditClient::builder()
            .with_clock(TestClock::default())
            .with_download_rate_mbps(Some(12.3456))
            .with_height_header(true)
            .build()
            .unwrap();
        let mut state = testing::state();
        state.authentication.session = Some(AnonymousSession {
            access_token: AccessToken::new(TOKEN_FAKE),
            expires_at_ms: NOW + 60_000,
        });
        state.authentication.loid = Some("opaque-loid".into());
        state.analytics.session_id = Some("local-analytics-id".into());

        let api = client.backend().api_headers(&state).unwrap();
        let auth = client.backend().auth_headers(&state).unwrap();

        assert_eq!(api["authorization"], format!("Bearer {TOKEN_FAKE}"));
        assert_eq!(api["client-vendor-id"], api["x-reddit-device-id"]);
        assert_eq!(api["device-name"], "Google;Pixel 7");
        assert_eq!(api["x-reddit-dpr"], "2.75");
        assert_eq!(api["x-reddit-width"], "392");
        assert_eq!(api["x-reddit-height"], "872");
        assert_eq!(api["x-reddit-qos"], "down-rate-mbps=12.346");
        assert_eq!(api["x-reddit-loid"], "opaque-loid");
        assert!(api["authorization"].is_sensitive());
        assert!(api["x-reddit-device-id"].is_sensitive());
        assert!(api["x-reddit-loid"].is_sensitive());
        assert_eq!(auth["authorization"], "Basic b2hYcG9xclpZdWIxa2c6");
        for name in [
            "device-name",
            "x-reddit-device-id",
            "x-reddit-dpr",
            "x-reddit-width",
            "x-reddit-height",
        ] {
            assert!(!auth.contains_key(name));
        }
        for headers in [&api, &auth] {
            for name in ["x-reddit-session", "x-reddit-p-device-id", "x-dev-ad-id"] {
                assert!(!headers.contains_key(name));
            }
        }
    }

    #[test]
    fn when_density_is_nonpositive_then_dimensions_should_use_pixel_values() {
        for density in [0.0, -1.0] {
            let client = RedditClient::builder()
                .with_clock(TestClock::default())
                .with_device_profile(DeviceProfile {
                    display_density: density,
                    ..DeviceProfile::default()
                })
                .with_height_header(true)
                .build()
                .unwrap();
            let mut state = testing::state();
            state.authentication.session = Some(AnonymousSession {
                access_token: AccessToken::new(TOKEN_FAKE),
                expires_at_ms: NOW + 1,
            });

            let headers = client.backend().api_headers(&state).unwrap();

            assert_eq!(headers["x-reddit-width"], "1080");
            assert_eq!(headers["x-reddit-height"], "2400");
        }
    }

    #[test]
    fn when_stored_identity_contains_a_newline_then_header_building_should_reject_it() {
        let client = RedditClient::builder().build().unwrap();
        let mut state = testing::state();
        state.authentication.loid = Some("invalid\nheader".into());

        let result = client.backend().auth_headers(&state);

        assert!(matches!(
            result,
            Err(Error::InvalidState {
                field: "x-reddit-loid"
            })
        ));
    }
}
