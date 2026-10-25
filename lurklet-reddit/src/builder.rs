use crate::backends::{Config, PersistedQuery};
use crate::{
    AppProfile, BuildError, Clock, DeviceProfile, InMemoryStore, MobileBackend, RedditClient,
    ResponseObserver, StateStore, Storage, SystemClock, TransportError,
};
use std::sync::Arc;
use std::time::Duration;
use url::Url;
use wreq::header::HeaderValue;
use wreq::redirect::Policy;
use wreq::retry;
use wreq_util::{Emulation, Profile};

/// Configuration for constructing a mobile Reddit client.
#[must_use]
pub struct RedditClientBuilder<S: StateStore = InMemoryStore> {
    request_timeout: Duration,
    connect_timeout: Duration,
    config: Config,
    storage: Storage<S>,
    clock: Arc<dyn Clock>,
    emulation_profile: Option<Profile>,
    observer: Option<Arc<dyn ResponseObserver>>,
    #[cfg(test)]
    mock_origins: Vec<(http::Uri, http::Uri)>,
}

impl Default for RedditClientBuilder {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            config: Config::default(),
            storage: Storage::default(),
            clock: Arc::new(SystemClock),
            emulation_profile: None,
            observer: None,
            #[cfg(test)]
            mock_origins: Vec::new(),
        }
    }
}

impl<S: StateStore> RedditClientBuilder<S> {
    /// Enables sensitive response capture, bounded by the response-size limit.
    pub fn with_response_observer(mut self, observer: impl ResponseObserver + 'static) -> Self {
        self.observer = Some(Arc::new(observer));
        self
    }

    /// Supplies caller-owned storage and shared acquisition coordination.
    pub fn with_storage<T: StateStore>(self, storage: Storage<T>) -> RedditClientBuilder<T> {
        RedditClientBuilder {
            request_timeout: self.request_timeout,
            connect_timeout: self.connect_timeout,
            config: self.config,
            storage,
            clock: self.clock,
            emulation_profile: self.emulation_profile,
            observer: self.observer,
            #[cfg(test)]
            mock_origins: self.mock_origins,
        }
    }

    /// Supplies a clock, including deterministic clocks for tests.
    pub fn with_clock(mut self, clock: impl Clock + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    /// Overrides the trusted destination for Basic-authenticated bootstrap requests.
    pub fn with_auth_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.auth_endpoint = endpoint.into();
        self
    }

    /// Overrides the trusted destination for future GraphQL operations.
    pub fn with_graphql_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.graphql_endpoint = endpoint.into();
        self
    }

    /// Overrides the Accept header advertised by GraphQL requests.
    pub fn with_graphql_accept(mut self, value: impl Into<String>) -> Self {
        self.config.graphql_accept = value.into();
        self
    }

    /// Overrides retry header metadata without enabling automatic retries.
    pub fn with_graphql_retry_header(mut self, value: impl Into<String>) -> Self {
        self.config.graphql_retry_header = value.into();
        self
    }

    /// Overrides the paired name and hash for the allowlist diagnostic query.
    pub fn with_request_allowlist_query(
        mut self,
        name: impl Into<String>,
        sha256_hash: impl Into<String>,
    ) -> Self {
        self.config.request_allowlist_query = PersistedQuery {
            name: name.into(),
            sha256_hash: sha256_hash.into(),
        };
        self
    }

    /// Overrides the anonymous OAuth client ID; the password remains empty.
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.config.client_id = client_id.into();
        self
    }

    /// Overrides the default scope list without changing its order.
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.config.scopes = scopes;
        self
    }

    /// Overrides app metadata used by the derived user-agent.
    pub fn with_app_profile(mut self, profile: AppProfile) -> Self {
        self.config.app = profile;
        self
    }

    /// Overrides the stable device and display profile.
    pub fn with_device_profile(mut self, profile: DeviceProfile) -> Self {
        self.config.device = profile;
        self
    }

    /// Overrides the user-agent instead of deriving it from the app and device.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.config.user_agent = Some(user_agent.into());
        self
    }

    /// Enables an opt-in transport preset, or clears it with None, without preset headers.
    pub fn with_emulation_profile(mut self, profile: impl Into<Option<Profile>>) -> Self {
        self.emulation_profile = profile.into();
        self
    }

    /// Overrides Reddit's application-level compression header.
    pub fn with_reddit_compression(mut self, value: impl Into<String>) -> Self {
        self.config.reddit_compression = value.into();
        self
    }

    /// Supplies a measured QoS value, or omits the header when unavailable.
    pub fn with_download_rate_mbps(mut self, value: Option<f64>) -> Self {
        self.config.download_rate_mbps = value;
        self
    }

    /// Explicitly enables the otherwise unresolved general-API height-header branch.
    pub fn with_height_header(mut self, enabled: bool) -> Self {
        self.config.include_height = enabled;
        self
    }

    /// Bounds decompressed response bodies and captures.
    pub fn with_max_response_bytes(mut self, limit: usize) -> Self {
        self.config.max_response_bytes = limit;
        self
    }

    /// Sets the timeout for each HTTP request, including reading its response body.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Sets the timeout for establishing an HTTP connection.
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_mock_server(mut self, origin: &str, server: &wiremock::MockServer) -> Self {
        use http::Uri;

        let origin: Uri = origin.parse().expect("invalid upstream origin");
        assert_eq!(origin.scheme_str(), Some("https"));
        assert!(origin.authority().is_some());
        assert_eq!(origin.path(), "/");
        assert!(origin.query().is_none());
        self.mock_origins
            .push((origin, server.uri().parse().expect("invalid mock origin")));
        self
    }

    /// Builds the client without making requests to Reddit.
    pub fn build(mut self) -> Result<RedditClient<MobileBackend<S>>, BuildError> {
        for (field, endpoint) in [
            ("auth_endpoint", &self.config.auth_endpoint),
            ("graphql_endpoint", &self.config.graphql_endpoint),
        ] {
            let url = Url::parse(endpoint).map_err(|_| BuildError::InvalidEndpoint { field })?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return Err(BuildError::InvalidEndpoint { field });
            }
        }
        for (field, valid) in [
            ("request_timeout", !self.request_timeout.is_zero()),
            ("connect_timeout", !self.connect_timeout.is_zero()),
            ("max_response_bytes", self.config.max_response_bytes > 0),
            (
                "client_id",
                !self.config.client_id.is_empty() && !self.config.client_id.contains(':'),
            ),
            (
                "scopes",
                !self.config.scopes.is_empty() && self.config.scopes.iter().all(|s| !s.is_empty()),
            ),
            (
                "display_density",
                self.config.device.display_density.is_finite(),
            ),
            (
                "download_rate_mbps",
                self.config
                    .download_rate_mbps
                    .is_none_or(|v| v.is_finite() && v >= 0.0),
            ),
        ] {
            if !valid {
                return Err(BuildError::InvalidConfiguration { field });
            }
        }
        let version = self
            .config
            .app
            .version
            .split('+')
            .next()
            .unwrap_or_default();
        let user_agent = self.config.user_agent.clone().unwrap_or_else(|| {
            format!(
                "Reddit/Version {version}/Build {}/Android {}",
                self.config.app.build, self.config.device.android_release
            )
        });
        for (field, value) in [
            ("user_agent", user_agent.as_str()),
            ("app_version", version),
            ("app_build", self.config.app.build.as_str()),
            (
                "android_release",
                self.config.device.android_release.as_str(),
            ),
            ("manufacturer", self.config.device.manufacturer.as_str()),
            ("model", self.config.device.model.as_str()),
            ("locale", self.config.device.locale.as_str()),
            (
                "reddit_compression",
                self.config.reddit_compression.as_str(),
            ),
            ("graphql_accept", self.config.graphql_accept.as_str()),
            (
                "graphql_retry_header",
                self.config.graphql_retry_header.as_str(),
            ),
        ] {
            if value.is_empty() || HeaderValue::from_str(value).is_err() {
                return Err(BuildError::InvalidConfiguration { field });
            }
        }
        self.config.user_agent = Some(user_agent);
        let http = wreq::Client::builder();
        let http = if let Some(profile) = self.emulation_profile {
            tracing::trace!(
                ?profile,
                "Applying explicitly configured transport emulation"
            );
            http.emulation(Emulation::builder().profile(profile).headers(false).build())
        } else {
            tracing::trace!("Using default HTTP transport without emulation");
            http
        };
        let http = http
            .timeout(self.request_timeout)
            .connect_timeout(self.connect_timeout)
            .https_only(true)
            .redirect(Policy::none())
            .retry(retry::Policy::never());

        #[cfg(test)]
        let http = {
            use http::Uri;
            use http::header::HOST;
            use std::io;
            use tower::filter::FilterLayer;

            http.https_only(false).no_proxy().layer(FilterLayer::new(
                move |mut request: http::Request<wreq::Body>| -> Result<_, io::Error> {
                    let target = self
                        .mock_origins
                        .iter()
                        .find(|(origin, _)| {
                            request.uri().scheme() == origin.scheme()
                                && request.uri().authority() == origin.authority()
                        })
                        .map(|(_, target)| target)
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::PermissionDenied,
                                "unregistered upstream origin in offline test",
                            )
                        })?;

                    let mut parts = request.uri().clone().into_parts();
                    parts.scheme = target.scheme().cloned();
                    parts.authority = target.authority().cloned();
                    *request.uri_mut() = Uri::from_parts(parts).map_err(io::Error::other)?;
                    request.headers_mut().remove(HOST);
                    Ok(request)
                },
            ))
        };

        let http = http.build().map_err(TransportError::from)?;

        let client = RedditClient::from_backend(MobileBackend {
            http,
            config: self.config,
            storage: self.storage,
            clock: self.clock,
            observer: self.observer,
        });
        tracing::trace!("Reddit client initialized");
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_a_builder_is_created_then_emulation_should_be_disabled() {
        let builder = RedditClient::builder();

        let profile = builder.emulation_profile;

        assert_eq!(profile, None);
    }

    #[test]
    fn when_emulation_is_explicitly_configured_then_storage_injection_should_preserve_it() {
        let builder = RedditClient::builder().with_emulation_profile(Profile::OkHttp4_12);

        let builder = builder.with_storage(Storage::default());

        assert_eq!(builder.emulation_profile, Some(Profile::OkHttp4_12));
        assert!(builder.build().is_ok());
    }

    #[test]
    fn when_emulation_is_cleared_then_the_builder_should_use_plain_transport() {
        let builder = RedditClient::builder().with_emulation_profile(Profile::OkHttp4_12);

        let builder = builder.with_emulation_profile(None);

        assert_eq!(builder.emulation_profile, None);
        assert!(builder.build().is_ok());
    }

    #[test]
    fn when_configuration_is_invalid_then_build_should_return_a_structured_error() {
        let builders = [
            RedditClient::builder().with_auth_endpoint("http://example.test/token"),
            RedditClient::builder().with_graphql_endpoint("https://user:secret@example.test/"),
            RedditClient::builder().with_user_agent("invalid\nagent"),
            RedditClient::builder().with_scopes(Vec::new()),
            RedditClient::builder().with_client_id("client:password"),
            RedditClient::builder().with_max_response_bytes(0),
            RedditClient::builder().with_download_rate_mbps(Some(f64::NAN)),
            RedditClient::builder().with_graphql_accept("invalid\nheader"),
            RedditClient::builder().with_graphql_retry_header("invalid\nheader"),
        ];

        let results: Vec<_> = builders
            .into_iter()
            .map(RedditClientBuilder::build)
            .collect();

        assert!(results.into_iter().all(|result| matches!(
            result,
            Err(BuildError::InvalidConfiguration { .. } | BuildError::InvalidEndpoint { .. })
        )));
    }

    #[test]
    fn when_app_version_has_a_suffix_then_the_derived_user_agent_should_omit_it() {
        let builder = RedditClient::builder()
            .with_app_profile(AppProfile {
                version: "2026.42.0+debug".into(),
                build: "42".into(),
            })
            .with_device_profile(DeviceProfile {
                android_release: "15".into(),
                ..DeviceProfile::default()
            });

        let client = builder.build().unwrap();

        assert_eq!(
            client.backend().config.user_agent.as_deref(),
            Some("Reddit/Version 2026.42.0/Build 42/Android 15")
        );
    }

    #[test]
    fn when_a_builder_is_created_then_timeouts_should_have_defaults() {
        let builder = RedditClient::builder();

        let timeouts = (builder.request_timeout, builder.connect_timeout);

        assert_eq!(timeouts, (Duration::from_secs(30), Duration::from_secs(10)));
    }

    #[test]
    fn when_timeouts_are_configured_then_the_builder_should_retain_them() {
        let request_timeout = Duration::from_secs(20);
        let connect_timeout = Duration::from_secs(5);

        let builder = RedditClient::builder()
            .with_request_timeout(request_timeout)
            .with_connect_timeout(connect_timeout);

        assert_eq!(
            (builder.request_timeout, builder.connect_timeout),
            (request_timeout, connect_timeout)
        );
    }

    #[tokio::test]
    async fn when_a_client_is_built_then_construction_should_succeed() {
        let builder = RedditClient::builder();

        let result = builder.build();

        assert!(result.is_ok());
    }
}
