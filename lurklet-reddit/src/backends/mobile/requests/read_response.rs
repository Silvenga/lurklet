use crate::backends::mobile::MobileBackend;
use crate::backends::mobile::requests::HttpResponse;
use crate::{ResponseCapture, StateStore, TransportError};
use futures_util::{StreamExt, pin_mut};
use std::time::Instant;

impl<S: StateStore> MobileBackend<S> {
    /// Reads bounded bytes and preserves failures without interpreting HTTP status or payload.
    pub(in crate::backends::mobile) async fn read_response(
        &self,
        operation: &str,
        response: wreq::Response,
        started: Instant,
    ) -> HttpResponse {
        let capture_metadata = self.observer.as_ref().map(|_| {
            (
                response.uri().to_string(),
                format!("{:?}", response.version()),
            )
        });
        let mut buffered = HttpResponse {
            status: response.status().as_u16(),
            headers: response.headers().clone(),
            body: Vec::new(),
            body_truncated: false,
            body_error: None,
        };
        let stream = response.bytes_stream();
        pin_mut!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let keep = chunk.len().min(
                        self.config
                            .max_response_bytes
                            .saturating_sub(buffered.body.len()),
                    );
                    buffered.body.extend_from_slice(&chunk[..keep]);
                    if keep < chunk.len() {
                        buffered.body_truncated = true;
                        break;
                    }
                }
                Err(error) => {
                    buffered.body_error = Some(TransportError::from(error));
                    break;
                }
            }
        }
        if let (Some(observer), Some((url, http_version))) = (&self.observer, capture_metadata) {
            tracing::trace!(
                status = buffered.status,
                bytes = buffered.body.len(),
                truncated = buffered.body_truncated,
                "Delivering opt-in response capture"
            );
            observer.on_response(ResponseCapture {
                operation: operation.to_owned(),
                url,
                status: buffered.status,
                http_version,
                headers: buffered
                    .headers
                    .iter()
                    .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
                    .collect(),
                body: buffered.body.clone(),
                body_truncated: buffered.body_truncated,
                body_error: buffered.body_error.as_ref().map(|error| error.kind),
                elapsed: started.elapsed(),
            });
        }
        buffered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RedditClient;
    use futures_util::stream;
    use std::io;

    #[tokio::test]
    async fn when_status_is_unsuccessful_then_the_reader_should_preserve_headers_and_body_without_capture()
     {
        let client = RedditClient::builder().build().unwrap();
        let body = r#"{"errors":[{"message":"Invalid input"}]}"#;
        let response = http::Response::builder()
            .status(400)
            .header("content-type", "application/graphql-response+json")
            .header("x-ratelimit-remaining", "0")
            .body(body)
            .unwrap()
            .into();

        let buffered = client
            .backend()
            .read_response("fixture", response, Instant::now())
            .await;

        assert_eq!(buffered.status, 400);
        assert_eq!(
            buffered.headers["content-type"],
            "application/graphql-response+json"
        );
        assert_eq!(buffered.headers["x-ratelimit-remaining"], "0");
        assert_eq!(buffered.body, body.as_bytes());
        assert!(!buffered.body_truncated);
        assert!(buffered.body_error.is_none());
    }

    #[tokio::test]
    async fn when_content_type_is_missing_then_the_reader_should_keep_the_body_unchanged() {
        let client = RedditClient::builder().build().unwrap();
        let response = http::Response::builder()
            .status(403)
            .body("<html>blocked</html>")
            .unwrap()
            .into();

        let buffered = client
            .backend()
            .read_response("fixture", response, Instant::now())
            .await;

        assert_eq!(buffered.status, 403);
        assert!(!buffered.headers.contains_key("content-type"));
        assert_eq!(buffered.body, b"<html>blocked</html>");
    }

    #[tokio::test]
    async fn when_a_body_read_fails_then_the_reader_should_keep_status_headers_and_partial_bytes() {
        let client = RedditClient::builder().build().unwrap();
        let body = wreq::Body::wrap_stream(stream::iter([
            Ok(b"partial".to_vec()),
            Err(io::Error::new(io::ErrorKind::UnexpectedEof, "read failed")),
        ]));
        let response = http::Response::builder()
            .status(429)
            .header("retry-after", "60")
            .body(body)
            .unwrap()
            .into();

        let buffered = client
            .backend()
            .read_response("fixture", response, Instant::now())
            .await;

        assert_eq!(buffered.status, 429);
        assert_eq!(buffered.headers["retry-after"], "60");
        assert_eq!(buffered.body, b"partial");
        assert!(buffered.body_error.is_some());
        assert!(!buffered.body_truncated);
    }

    #[tokio::test]
    async fn when_the_body_reaches_the_limit_then_only_excess_bytes_should_mark_it_truncated() {
        let client = RedditClient::builder()
            .with_max_response_bytes(4)
            .build()
            .unwrap();
        for (body, expected, truncated) in [
            ("", "", false),
            ("1234", "1234", false),
            ("12345", "1234", true),
        ] {
            let response = http::Response::builder()
                .status(200)
                .body(body)
                .unwrap()
                .into();

            let buffered = client
                .backend()
                .read_response("fixture", response, Instant::now())
                .await;

            assert_eq!(buffered.body, expected.as_bytes());
            assert_eq!(buffered.body_truncated, truncated);
            assert!(buffered.body_error.is_none());
        }
    }

    #[tokio::test]
    async fn when_the_body_spans_chunks_then_the_reader_should_enforce_one_total_limit() {
        let client = RedditClient::builder()
            .with_max_response_bytes(4)
            .build()
            .unwrap();
        let body = wreq::Body::wrap_stream(stream::iter([
            Ok::<_, io::Error>(b"12".to_vec()),
            Ok(b"34".to_vec()),
            Ok(b"56".to_vec()),
        ]));
        let response = http::Response::builder()
            .status(400)
            .body(body)
            .unwrap()
            .into();

        let buffered = client
            .backend()
            .read_response("fixture", response, Instant::now())
            .await;

        assert_eq!(buffered.status, 400);
        assert_eq!(buffered.body, b"1234");
        assert!(buffered.body_truncated);
        assert!(buffered.body_error.is_none());
    }
}
