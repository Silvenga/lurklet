use crate::{BuildError, MobileBackend, RedditClient};
use std::time::Duration;
use wreq::redirect::Policy;

/// Configuration for constructing a mobile Reddit client.
#[derive(Debug)]
#[must_use]
pub struct RedditClientBuilder {
    request_timeout: Duration,
    connect_timeout: Duration,
    #[cfg(test)]
    mock_origins: Vec<(http::Uri, http::Uri)>,
}

impl Default for RedditClientBuilder {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            #[cfg(test)]
            mock_origins: Vec::new(),
        }
    }
}

impl RedditClientBuilder {
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
    #[tracing::instrument(name = "reddit_client.build", skip_all)]
    pub fn build(self) -> Result<RedditClient, BuildError> {
        let http = wreq::Client::builder()
            .timeout(self.request_timeout)
            .connect_timeout(self.connect_timeout)
            .https_only(true)
            .redirect(Policy::none());

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

        let http = http.build().map_err(|source| BuildError { source })?;

        let client = RedditClient::from_backend(MobileBackend { http });
        tracing::debug!("Reddit client initialized");
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
