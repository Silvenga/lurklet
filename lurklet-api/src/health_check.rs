use anyhow::{Context, ensure};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;
use wreq::redirect::Policy;
use wreq::{Client, StatusCode};

/// Checks the local API's response status without consuming the response body.
pub async fn check(mut address: SocketAddr) -> anyhow::Result<()> {
    match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => address.set_ip(Ipv4Addr::LOCALHOST.into()),
        IpAddr::V6(ip) if ip.is_unspecified() => address.set_ip(Ipv6Addr::LOCALHOST.into()),
        IpAddr::V4(_) | IpAddr::V6(_) => {}
    }

    let response = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to initialize the health check client")?
        .get(format!("http://{address}/api/health"))
        .send()
        .await
        .context("failed to request API health")?;
    ensure!(
        response.status() == StatusCode::OK,
        "API health is not HTTP 200"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::http::{HeaderMap, header};
    use axum::routing::get;
    use std::future::IntoFuture;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    #[tokio::test]
    async fn when_health_headers_are_ok_then_check_should_succeed_without_waiting_for_body() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (finish, finished) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let (connection, _) = listener.accept().await.unwrap();
            let mut connection = BufReader::new(connection);
            let mut request_line = String::new();
            connection.read_line(&mut request_line).await.unwrap();
            assert_eq!(request_line, "GET /api/health HTTP/1.1\r\n");
            connection
                .get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\n")
                .await
                .unwrap();
            let _ = finished.await;
        });

        let result = timeout(Duration::from_secs(1), check(address)).await;
        drop(finish);
        server.await.unwrap();

        assert!(result.unwrap().is_ok());
    }

    #[tokio::test]
    async fn when_health_status_is_not_200_then_check_should_fail_without_following_redirects() {
        for status in [
            StatusCode::CREATED,
            StatusCode::NO_CONTENT,
            StatusCode::MOVED_PERMANENTLY,
            StatusCode::FOUND,
            StatusCode::TEMPORARY_REDIRECT,
            StatusCode::PERMANENT_REDIRECT,
            StatusCode::NOT_FOUND,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let app = Router::new()
                .route(
                    "/api/health",
                    get(move || async move { (status, [(header::LOCATION, "/redirected")]) }),
                )
                .route("/redirected", get(|| async { StatusCode::OK }));
            let server = tokio::spawn(axum::serve(listener, app).into_future());

            let result = check(address).await;
            server.abort();

            assert!(result.is_err(), "{status}");
        }
    }

    #[tokio::test]
    async fn when_health_connection_is_refused_then_check_should_fail() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        let result = check(address).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn when_health_headers_never_arrive_then_check_should_time_out() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (finish, finished) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let (connection, _) = listener.accept().await.unwrap();
            let _ = finished.await;
            drop(connection);
        });

        let result = timeout(Duration::from_secs(5), check(address)).await;
        drop(finish);
        server.await.unwrap();

        assert!(result.unwrap().is_err());
    }

    #[tokio::test]
    async fn when_bind_is_ipv6_wildcard_then_check_should_use_ipv6_loopback() {
        let listener = TcpListener::bind("[::1]:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/api/health",
            get(move |headers: HeaderMap| async move {
                assert_eq!(headers[header::HOST], address.to_string());
                StatusCode::OK
            }),
        );
        let server = tokio::spawn(axum::serve(listener, app).into_future());

        let result = check(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            address.port(),
        ))
        .await;
        server.abort();

        assert!(result.is_ok(), "{result:?}");
    }
}
