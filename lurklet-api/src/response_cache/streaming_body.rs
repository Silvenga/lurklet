use crate::response_cache::store::Writer;
use axum::body::{Body, BodyDataStream};
use axum::http::HeaderMap;
use futures_util::{StreamExt, stream};
use std::io;
use std::mem;
use std::time::Duration;
use tokio::sync::OwnedMutexGuard;

struct Transfer {
    source: BodyDataStream,
    writer: Option<Writer>,
    headers: HeaderMap,
    fresh_for: Duration,
    expected_length: Option<u64>,
    received: u64,
    guard: Option<OwnedMutexGuard<()>>,
    complete: bool,
}

impl Transfer {
    async fn finish(&mut self) -> Result<(), axum::Error> {
        if self
            .expected_length
            .is_some_and(|length| length != self.received)
        {
            return Err(axum::Error::new(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "response content length mismatch",
            )));
        }
        if let Some(writer) = self.writer.take()
            && let Err(error) = writer
                .publish(mem::take(&mut self.headers), self.fresh_for, self.received)
                .await
        {
            tracing::warn!(
                "Unable to publish a response cache entry; returning the fresh response"
            );
            tracing::debug!(?error, "Response cache publication failure details");
        }
        self.complete = true;
        self.guard.take();
        Ok(())
    }
}

/// Caches a body only as it is consumed, discarding unfinished writes when dropped.
pub fn streaming_body(
    body: Body,
    writer: Writer,
    headers: HeaderMap,
    fresh_for: Duration,
    expected_length: Option<u64>,
    guard: OwnedMutexGuard<()>,
) -> Body {
    let transfer = Transfer {
        source: body.into_data_stream(),
        writer: Some(writer),
        headers,
        fresh_for,
        expected_length,
        received: 0,
        guard: Some(guard),
        complete: false,
    };
    Body::from_stream(stream::try_unfold(transfer, |mut transfer| async move {
        if transfer.complete {
            return Ok(None);
        }
        let chunk = match transfer.source.next().await {
            Some(chunk) => chunk?,
            None => {
                transfer.finish().await?;
                return Ok(None);
            }
        };
        if let Some(writer) = &mut transfer.writer
            && let Err(error) = writer.write(&chunk).await
        {
            tracing::warn!("Unable to write a response cache entry; continuing the fresh response");
            tracing::debug!(?error, "Response cache write failure details");
            transfer.writer = None;
        }
        transfer.received += chunk.len() as u64;
        if transfer
            .expected_length
            .is_some_and(|length| transfer.received >= length)
        {
            // HTTP consumers may stop polling once Content-Length bytes have been delivered.
            // Establish EOF and finish publication before yielding that final chunk.
            while let Some(next) = transfer.source.next().await {
                if !next?.is_empty() {
                    return Err(axum::Error::new(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "response exceeds content length",
                    )));
                }
            }
            transfer.finish().await?;
        }
        Ok::<_, axum::Error>(Some((chunk, transfer)))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response_cache::key::CacheKey;
    use crate::response_cache::store::Store;
    use bytes::Bytes;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn when_a_large_body_has_unknown_length_then_transfer_and_cache_hits_should_stay_chunked()
    {
        let directory = assert_fs::TempDir::new().unwrap();
        let store = Store::new(directory.path()).await.unwrap();
        let key = CacheKey::new(&"/large".parse().unwrap());
        let writer = store.begin(&key).await.unwrap();
        let polled = Arc::new(AtomicUsize::new(0));
        let source = Body::from_stream(stream::iter(0..128).map({
            let polled = polled.clone();
            move |_| {
                polled.fetch_add(1, Ordering::SeqCst);
                Ok::<_, io::Error>(Bytes::from_static(&[42; 16384]))
            }
        }));
        let guard = Arc::new(Mutex::new(())).lock_owned().await;
        let body = streaming_body(
            source,
            writer,
            HeaderMap::new(),
            Duration::from_secs(300),
            None,
            guard,
        );
        let mut chunks = body.into_data_stream();

        assert_eq!(polled.load(Ordering::SeqCst), 0);
        for expected in 1..=128 {
            let chunk = chunks.next().await.unwrap().unwrap();
            assert_eq!(chunk.len(), 16384);
            assert_eq!(polled.load(Ordering::SeqCst), expected);
        }
        assert!(store.read(&key).await.is_none());
        assert!(chunks.next().await.is_none());
        let entry = store.read(&key).await.unwrap();

        assert_eq!(entry.metadata.content_length, 128 * 16384);
        let mut cached = entry.into_response().into_body().into_data_stream();
        let mut received = 0;
        while let Some(chunk) = cached.next().await {
            let chunk = chunk.unwrap();
            assert!(chunk.len() <= 16384);
            assert!(chunk.iter().all(|byte| *byte == 42));
            received += chunk.len();
        }
        assert_eq!(received, 128 * 16384);
    }
}
