use super::MobileBackend;
use crate::{Error, ResponseCapture, ResponseError, RetryAfter, StateStore, TransportError};
use futures_util::{StreamExt, pin_mut};
use std::time::{Duration, Instant};
use wreq::header::HeaderMap;

impl<S: StateStore> MobileBackend<S> {
    pub(super) async fn read_response(
        &self,
        operation: &'static str,
        response: wreq::Response,
        started: Instant,
    ) -> Result<(HeaderMap, Vec<u8>), Error> {
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| {
                if let Ok(seconds) = value.parse::<u64>() {
                    Some(RetryAfter::Delay(Duration::from_secs(seconds)))
                } else {
                    httpdate::parse_http_date(value).ok().map(RetryAfter::At)
                }
            });
        let status_error = match status {
            200..=299 => None,
            401 => Some(Error::Authentication { status }),
            403 => Some(Error::AccessDenied { status }),
            429 => Some(Error::RateLimited { retry_after }),
            500..=599 => Some(Error::Upstream {
                status,
                retry_after,
            }),
            _ => Some(Error::UnexpectedStatus { status }),
        };
        if let Some(error) = &status_error {
            tracing::debug!(status, "Upstream rejected request; no automatic retry");
            if self.observer.is_none() {
                return Err(error.clone());
            }
        }
        let capture = self.observer.as_ref().map(|_| ResponseCapture {
            operation,
            url: response.uri().to_string(),
            status,
            http_version: format!("{:?}", response.version()),
            headers: response
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
                .collect(),
            body: Vec::new(),
            body_truncated: false,
            body_error: None,
            elapsed: Duration::ZERO,
        });
        let headers = response.headers().clone();
        let stream = response.bytes_stream();
        pin_mut!(stream);
        let mut body = Vec::new();
        let mut body_error = None;
        let mut truncated = false;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let keep = chunk
                        .len()
                        .min(self.config.max_response_bytes.saturating_sub(body.len()));
                    body.extend_from_slice(&chunk[..keep]);
                    if keep < chunk.len() {
                        truncated = true;
                        break;
                    }
                }
                Err(error) => {
                    body_error = Some(TransportError::from(error));
                    break;
                }
            }
        }
        if let (Some(observer), Some(mut capture)) = (&self.observer, capture) {
            capture.body = body.clone();
            capture.body_truncated = truncated;
            capture.body_error = body_error.as_ref().map(|error| error.kind);
            capture.elapsed = started.elapsed();
            tracing::trace!(
                status,
                bytes = body.len(),
                truncated,
                "Delivering opt-in response capture"
            );
            observer.on_response(capture);
        }
        // A failed diagnostic body read must not conceal the upstream HTTP failure.
        if let Some(error) = status_error {
            return Err(error);
        }
        if let Some(error) = body_error {
            return Err(error.into());
        }
        if truncated {
            return Err(ResponseError::BodyTooLarge.into());
        }
        Ok((headers, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile::testing::{self, ORIGIN, PATH, TestClock};
    use crate::{RedditClient, ResponseObserver};
    use futures_util::stream;
    use std::io;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<ResponseCapture>>>);

    impl ResponseObserver for Recorder {
        fn on_response(&self, response: ResponseCapture) {
            self.0.lock().unwrap().push(response);
        }
    }

    #[tokio::test]
    async fn when_capture_is_enabled_then_bootstrap_should_deliver_the_success_response() {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        let recorder = Recorder::default();
        let client = RedditClient::builder()
            .with_response_observer(recorder.clone())
            .with_storage(testing::storage())
            .with_clock(TestClock::default())
            .with_mock_server(ORIGIN, &server)
            .build()
            .unwrap();

        let session = client.bootstrap().await.unwrap();

        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].status, 200);
        assert_eq!(captures[0].operation, "bootstrap");
        assert!(!captures[0].body_truncated);
        assert!(captures[0].body_error.is_none());
        assert!(
            String::from_utf8_lossy(&captures[0].body)
                .contains(session.access_token.expose_secret())
        );
        assert!(!format!("{:?}", captures[0]).contains(session.access_token.expose_secret()));
    }

    #[tokio::test]
    async fn when_a_security_block_occurs_then_capture_should_retain_headers_and_body() {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("server", "snooserv")
                    .insert_header("set-cookie", "private-cookie")
                    .set_body_string("blocked by network security"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let recorder = Recorder::default();
        let client = RedditClient::builder()
            .with_storage(testing::storage())
            .with_response_observer(recorder.clone())
            .with_mock_server(ORIGIN, &server)
            .build()
            .unwrap();

        let result = client.bootstrap().await;

        assert!(matches!(result, Err(Error::AccessDenied { status: 403 })));
        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures[0].body, b"blocked by network security");
        assert!(
            captures[0]
                .headers
                .contains(&("set-cookie".into(), b"private-cookie".to_vec()))
        );
        assert!(!format!("{:?}", captures[0]).contains("private-cookie"));
    }

    #[tokio::test]
    async fn when_an_error_body_is_oversized_then_capture_should_not_change_http_classification() {
        let recorder = Recorder::default();
        let client = RedditClient::builder()
            .with_max_response_bytes(4)
            .with_response_observer(recorder.clone())
            .build()
            .unwrap();
        let response = http::Response::builder()
            .status(403)
            .body("long error body")
            .unwrap()
            .into();

        let result = client
            .backend()
            .read_response("bootstrap", response, Instant::now())
            .await;

        assert!(matches!(result, Err(Error::AccessDenied { status: 403 })));
        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures[0].body, b"long");
        assert!(captures[0].body_truncated);
    }

    #[tokio::test]
    async fn when_an_error_body_read_fails_then_capture_should_preserve_status_and_partial_body() {
        let recorder = Recorder::default();
        let client = RedditClient::builder()
            .with_response_observer(recorder.clone())
            .build()
            .unwrap();
        let body = wreq::Body::wrap_stream(stream::iter([
            Ok(b"partial".to_vec()),
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "private transport detail",
            )),
        ]));
        let response = http::Response::builder()
            .status(429)
            .header("retry-after", "60")
            .body(body)
            .unwrap()
            .into();

        let result = client
            .backend()
            .read_response("bootstrap", response, Instant::now())
            .await;

        assert!(
            matches!(result, Err(Error::RateLimited { retry_after: Some(RetryAfter::Delay(delay)) }) if delay == Duration::from_secs(60))
        );
        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures[0].body, b"partial");
        assert!(captures[0].body_error.is_some());
    }

    #[tokio::test]
    async fn when_a_success_body_is_oversized_then_capture_should_report_truncation_and_validation_should_fail()
     {
        let recorder = Recorder::default();
        let client = RedditClient::builder()
            .with_max_response_bytes(4)
            .with_response_observer(recorder.clone())
            .build()
            .unwrap();
        let response = http::Response::builder()
            .status(200)
            .body("long body")
            .unwrap()
            .into();

        let result = client
            .backend()
            .read_response("bootstrap", response, Instant::now())
            .await;

        assert!(matches!(
            result,
            Err(Error::InvalidResponse(ResponseError::BodyTooLarge))
        ));
        assert!(recorder.0.lock().unwrap()[0].body_truncated);
    }
}
