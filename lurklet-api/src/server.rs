use anyhow::Context;
use axum::Router;
use lurklet_api::RateLimit;
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::{MissedTickBehavior, interval};

/// Serves HTTP until shutdown is requested, then drains in-flight requests.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    rate_limit: RateLimit,
) -> anyhow::Result<()> {
    #[cfg(unix)]
    let shutdown = {
        use tokio::signal::unix::{SignalKind, signal};

        let mut interrupt =
            signal(SignalKind::interrupt()).context("failed to register the interrupt handler")?;
        let mut terminate = signal(SignalKind::terminate())
            .context("failed to register the termination handler")?;

        async move {
            tokio::select! {
                _ = interrupt.recv() => {},
                _ = terminate.recv() => {},
            }
            tracing::info!("API shutdown requested");
        }
    };
    #[cfg(not(unix))]
    let shutdown = async {
        if tokio::signal::ctrl_c().await.is_err() {
            tracing::error!("Unable to receive shutdown signals; stopping the API");
        }
        tracing::info!("API shutdown requested");
    };

    let address = listener
        .local_addr()
        .context("failed to read the API listener address")?;
    tracing::info!("API server started");
    tracing::debug!(%address, "API listener bound");
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .into_future();
    tokio::pin!(server);
    let mut cleanup = interval(Duration::from_secs(60));
    cleanup.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            result = &mut server => {
                result.context("API server failed")?;
                break;
            }
            _ = cleanup.tick() => rate_limit.retain_recent(),
        }
    }
    tracing::info!("API server stopped");

    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use axum::routing::get;
    use std::process::{self, Command};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::sync::Notify;
    use tokio::time::{sleep, timeout};

    #[tokio::test]
    async fn when_sigterm_is_received_then_in_flight_requests_should_finish_before_server_stops() {
        let started = Arc::new(Notify::new());
        let finish = Arc::new(Notify::new());
        let app = Router::new().route(
            "/slow",
            get({
                let started = Arc::clone(&started);
                let finish = Arc::clone(&finish);
                move || {
                    let started = Arc::clone(&started);
                    let finish = Arc::clone(&finish);
                    async move {
                        started.notify_one();
                        finish.notified().await;
                        "finished"
                    }
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(serve(listener, app, RateLimit::default()));
        let mut connection = TcpStream::connect(address).await.unwrap();
        connection
            .write_all(b"GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        timeout(Duration::from_secs(5), started.notified())
            .await
            .unwrap();

        // The active request guarantees that this process has installed its signal handlers.
        let signal = Command::new("kill")
            .arg("-TERM")
            .arg(process::id().to_string())
            .status()
            .unwrap();
        timeout(Duration::from_secs(5), async {
            while let Ok(connection) = TcpStream::connect(address).await {
                drop(connection);
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        assert!(signal.success());
        assert!(!server.is_finished());

        finish.notify_one();
        let mut response = String::new();
        timeout(
            Duration::from_secs(5),
            connection.read_to_string(&mut response),
        )
        .await
        .unwrap()
        .unwrap();
        let result = timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
        assert!(response.ends_with("finished"), "{response}");
        assert!(result.is_ok(), "{result:?}");
    }
}
