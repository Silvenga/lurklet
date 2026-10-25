use crate::{Error, ResponseCapture, ResponseObserver, TransportError};
use bytes::Bytes;
use futures_util::stream::BoxStream;
use futures_util::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

pub struct MediaStream {
    inner: BoxStream<'static, Result<Bytes, wreq::Error>>,
    capture: Option<(Arc<dyn ResponseObserver>, ResponseCapture)>,
    capture_limit: usize,
    started: Instant,
    finished: bool,
}

impl MediaStream {
    pub fn new(
        response: wreq::Response,
        observer: Option<Arc<dyn ResponseObserver>>,
        capture_limit: usize,
        started: Instant,
    ) -> Self {
        let capture = observer.map(|observer| {
            (
                observer,
                ResponseCapture {
                    operation: "GetMedia".into(),
                    url: response.uri().to_string(),
                    status: response.status().as_u16(),
                    http_version: format!("{:?}", response.version()),
                    headers: response
                        .headers()
                        .iter()
                        .map(|(name, value)| (name.as_str().into(), value.as_bytes().to_vec()))
                        .collect(),
                    body: Vec::new(),
                    body_truncated: false,
                    body_error: None,
                    elapsed: started.elapsed(),
                },
            )
        });
        Self {
            inner: response.bytes_stream().boxed(),
            capture,
            capture_limit,
            started,
            finished: false,
        }
    }

    fn finish_capture(&mut self) {
        if let Some((observer, mut capture)) = self.capture.take() {
            capture.body_truncated |= !self.finished;
            capture.elapsed = self.started.elapsed();
            observer.on_response(capture);
        }
    }
}

impl Stream for MediaStream {
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.finished {
            return Poll::Ready(None);
        }
        match this.inner.as_mut().poll_next(context) {
            Poll::Ready(Some(Ok(chunk))) => {
                if let Some((_, capture)) = &mut this.capture {
                    let keep = chunk
                        .len()
                        .min(this.capture_limit.saturating_sub(capture.body.len()));
                    capture.body.extend_from_slice(&chunk[..keep]);
                    capture.body_truncated |= keep < chunk.len();
                }
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(error))) => {
                let error = TransportError::from(error);
                if let Some((_, capture)) = &mut this.capture {
                    capture.body_error = Some(error.kind);
                }
                this.finished = true;
                this.finish_capture();
                Poll::Ready(Some(Err(error.into())))
            }
            Poll::Ready(None) => {
                this.finished = true;
                this.finish_capture();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for MediaStream {
    fn drop(&mut self) {
        self.finish_capture();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{FutureExt, stream};
    use std::io;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<ResponseCapture>>>);

    impl ResponseObserver for Recorder {
        fn on_response(&self, capture: ResponseCapture) {
            self.0.lock().unwrap().push(capture);
        }
    }

    struct Dropped(Arc<AtomicBool>);

    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn when_consumption_is_cancelled_then_streaming_should_stop_without_prefetching_or_draining_the_body()
     {
        let polls = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = Dropped(dropped.clone());
        let body = wreq::Body::wrap_stream(stream::poll_fn({
            let polls = polls.clone();
            move |_| {
                let _ = &guard;
                polls.fetch_add(1, Ordering::SeqCst);
                Poll::Ready(Some(Ok::<_, io::Error>(Bytes::from_static(b"chunk"))))
            }
        }));
        let response = http::Response::builder()
            .status(200)
            .body(body)
            .unwrap()
            .into();
        let recorder = Recorder::default();
        let mut stream = MediaStream::new(
            response,
            Some(Arc::new(recorder.clone())),
            10,
            Instant::now(),
        );

        assert_eq!(polls.load(Ordering::SeqCst), 0);
        let first = stream.next().await.unwrap().unwrap();
        drop(stream);

        assert_eq!(first, b"chunk".as_slice());
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert!(dropped.load(Ordering::SeqCst));
        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].body, b"chunk");
        assert!(captures[0].body_truncated);
        assert!(captures[0].body_error.is_none());
    }

    #[tokio::test]
    async fn when_a_body_is_pending_then_streaming_should_preserve_backpressure_without_reporting_completion()
     {
        let response = http::Response::builder()
            .status(200)
            .body(wreq::Body::wrap_stream(stream::pending::<
                Result<Bytes, io::Error>,
            >()))
            .unwrap()
            .into();
        let recorder = Recorder::default();
        let mut stream = MediaStream::new(
            response,
            Some(Arc::new(recorder.clone())),
            10,
            Instant::now(),
        );

        let next = stream.next().now_or_never();

        assert!(next.is_none());
        assert!(recorder.0.lock().unwrap().is_empty());
        drop(stream);
        let captures = recorder.0.lock().unwrap();
        assert!(captures[0].body.is_empty());
        assert!(captures[0].body_truncated);
    }

    #[tokio::test]
    async fn when_reading_fails_after_a_chunk_then_the_consumer_and_capture_should_receive_the_failure_once()
     {
        let body = wreq::Body::wrap_stream(stream::iter([
            Ok(Bytes::from_static(b"partial")),
            Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
        ]));
        let response = http::Response::builder()
            .status(200)
            .body(body)
            .unwrap()
            .into();
        let recorder = Recorder::default();
        let mut stream = MediaStream::new(
            response,
            Some(Arc::new(recorder.clone())),
            10,
            Instant::now(),
        );

        let first = stream.next().await.unwrap().unwrap();
        let failure = stream.next().await.unwrap();
        let end = stream.next().await;
        drop(stream);

        assert_eq!(first, b"partial".as_slice());
        let Err(Error::Transport(error)) = failure else {
            panic!("expected the body stream's transport failure")
        };
        assert!(end.is_none());
        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].body, b"partial");
        assert_eq!(captures[0].body_error, Some(error.kind));
    }

    #[tokio::test]
    async fn when_captures_reach_the_limit_then_only_excess_bytes_should_mark_them_truncated() {
        for (limit, expected, truncated) in [
            (3, b"abc".as_slice(), true),
            (6, b"abcdef".as_slice(), false),
        ] {
            let body = wreq::Body::wrap_stream(stream::iter([
                Ok::<_, io::Error>(Bytes::from_static(b"ab")),
                Ok(Bytes::from_static(b"cd")),
                Ok(Bytes::from_static(b"ef")),
            ]));
            let response = http::Response::builder()
                .status(200)
                .body(body)
                .unwrap()
                .into();
            let recorder = Recorder::default();
            let mut stream = MediaStream::new(
                response,
                Some(Arc::new(recorder.clone())),
                limit,
                Instant::now(),
            );

            let mut received = Vec::new();
            while let Some(chunk) = stream.next().await {
                received.extend_from_slice(&chunk.unwrap());
            }
            drop(stream);

            assert_eq!(received, b"abcdef");
            let captures = recorder.0.lock().unwrap();
            assert_eq!(captures.len(), 1);
            assert_eq!(captures[0].body, expected);
            assert_eq!(captures[0].body_truncated, truncated);
            assert!(captures[0].body_error.is_none());
        }
    }

    #[tokio::test]
    async fn when_capture_is_disabled_then_the_buffer_limit_should_not_affect_the_file_stream() {
        let response = http::Response::builder()
            .status(200)
            .body("complete file")
            .unwrap()
            .into();
        let mut stream = MediaStream::new(response, None, 1, Instant::now());

        let chunk = stream.next().await.unwrap().unwrap();
        let end = stream.next().await;

        assert_eq!(chunk, b"complete file".as_slice());
        assert!(end.is_none());
    }
}
