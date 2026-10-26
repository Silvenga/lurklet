use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use lurklet_reddit::{Error, RedditClient};
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::time::{sleep, timeout};

struct ServerProcess(Child);

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn when_proxy_configuration_is_invalid_then_startup_should_fail_without_exposing_credentials() {
    for from_environment in [false, true] {
        for level in ["info", "trace"] {
            let proxy = "unsupported://private-user:private-secret@private-host:1080";
            let mut command = Command::new(cargo_bin!("lurklet-api"));
            command
                .env_clear()
                .env("RUST_LOG", level)
                .timeout(Duration::from_secs(5));
            if from_environment {
                command.env("LURKLET_PROXY", proxy);
            } else {
                command.args(["--proxy", proxy]);
            }

            let output = command.assert().failure().get_output().clone();

            assert!(output.stdout.is_empty());
            let logs = String::from_utf8(output.stderr).unwrap();
            assert!(logs.contains("API execution failed"), "{logs}");
            assert!(!logs.contains("private-"), "{logs}");
        }
    }
}

#[test]
fn when_proxy_credentials_are_in_the_environment_then_help_should_hide_them() {
    let mut command = Command::new(cargo_bin!("lurklet-api"));
    command
        .env_clear()
        .env(
            "LURKLET_PROXY",
            "socks5h://private-user:private-secret@private-host:1080",
        )
        .arg("--help")
        .timeout(Duration::from_secs(5));

    let output = command.assert().success().get_output().clone();

    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("--proxy <URL>"));
    assert!(help.contains("LURKLET_PROXY"));
    assert!(!help.contains("private-"), "{help}");
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn when_a_proxy_is_selected_then_api_requests_should_use_the_configured_precedence() {
    let client = wreq::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    for configuration in ["flag", "environment", "both", "standard"] {
        let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_url = format!("http://user:p%40ss@{}", proxy.local_addr().unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let mut command = ProcessCommand::new(cargo_bin!("lurklet-api"));
        command
            .env_clear()
            .args(["--bind", &address.to_string(), "--static-dir"])
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/static"))
            .env("HTTPS_PROXY", "http://127.0.0.1:9")
            .env("ALL_PROXY", "http://127.0.0.1:9")
            .env("NO_PROXY", "*")
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        match configuration {
            "flag" => {
                command.args(["--proxy", &proxy_url]);
            }
            "environment" => {
                command.env("LURKLET_PROXY", &proxy_url);
            }
            "both" => {
                command
                    .args(["--proxy", &proxy_url])
                    .env("LURKLET_PROXY", "invalid-proxy");
            }
            "standard" => {
                command
                    .env("HTTPS_PROXY", &proxy_url)
                    .env_remove("NO_PROXY");
            }
            _ => unreachable!(),
        }
        let mut server = ServerProcess(command.spawn().unwrap());
        timeout(Duration::from_secs(5), async {
            loop {
                if let Some(status) = server.0.try_wait().unwrap() {
                    let mut stderr = String::new();
                    server
                        .0
                        .stderr
                        .as_mut()
                        .unwrap()
                        .read_to_string(&mut stderr)
                        .unwrap();
                    panic!("server exited before becoming ready: {status}: {stderr}");
                }
                if let Ok(response) = client
                    .get(format!("http://{address}/api/health"))
                    .send()
                    .await
                {
                    assert_eq!(response.status(), wreq::StatusCode::OK);
                    break;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let receive_connect = async {
            let (connection, _) = proxy.accept().await.unwrap();
            let mut connection = BufReader::new(connection);
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                assert_ne!(connection.read_line(&mut line).await.unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
                headers.push_str(&line);
            }
            connection
                .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
            headers
        };

        let (response, headers) = timeout(Duration::from_secs(5), async {
            tokio::join!(
                client
                    .get(format!(
                        "http://{address}/api/media?url=https%3A%2F%2Fi.redd.it%2Fimage.png"
                    ))
                    .send(),
                receive_connect,
            )
        })
        .await
        .unwrap();

        assert_eq!(
            response.unwrap().status(),
            wreq::StatusCode::BAD_GATEWAY,
            "{configuration}"
        );
        assert!(
            headers.starts_with("CONNECT i.redd.it:443 HTTP/1.1\r\n"),
            "{headers}"
        );
        assert!(
            headers
                .lines()
                .any(|line| line.split_once(": ").is_some_and(|(name, value)| {
                    name.eq_ignore_ascii_case("proxy-authorization")
                        && value == "Basic dXNlcjpwQHNz"
                })),
            "{headers}"
        );
    }
}

#[tokio::test]
async fn when_socks_is_configured_then_handshakes_should_use_the_selected_dns_and_authentication() {
    for (scheme, host, authenticate, reject_auth) in [
        ("socks5", "localhost", false, false),
        ("socks5", "localhost", true, false),
        ("socks5h", "localhost", false, false),
        ("socks5h", "localhost", true, false),
        ("socks5h", "proxy-only.invalid", false, false),
        ("socks5", "localhost", true, true),
        ("socks5h", "localhost", true, true),
    ] {
        let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let destination = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = destination.local_addr().unwrap().port();
        let credentials = if authenticate { "user:p%40ss@" } else { "" };
        let client = RedditClient::builder()
            .with_proxy(format!(
                "{scheme}://{credentials}{}",
                proxy.local_addr().unwrap()
            ))
            .with_auth_endpoint(format!("https://{host}:{port}/token"))
            .with_request_timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let handshake = async {
            let (mut connection, _) = proxy.accept().await.unwrap();
            assert_eq!(connection.read_u8().await.unwrap(), 5);
            let mut methods = vec![0; connection.read_u8().await.unwrap().into()];
            connection.read_exact(&mut methods).await.unwrap();
            let method = if authenticate { 2 } else { 0 };
            assert!(methods.contains(&method));
            connection.write_all(&[5, method]).await.unwrap();
            if authenticate {
                assert_eq!(connection.read_u8().await.unwrap(), 1);
                let mut username = vec![0; connection.read_u8().await.unwrap().into()];
                connection.read_exact(&mut username).await.unwrap();
                let mut password = vec![0; connection.read_u8().await.unwrap().into()];
                connection.read_exact(&mut password).await.unwrap();
                assert_eq!(username, b"user");
                assert_eq!(password, b"p@ss");
                connection
                    .write_all(&[1, u8::from(reject_auth)])
                    .await
                    .unwrap();
                if reject_auth {
                    assert_eq!(connection.read(&mut [0]).await.unwrap(), 0);
                    return;
                }
            }
            let mut request = [0; 4];
            connection.read_exact(&mut request).await.unwrap();
            assert_eq!(&request[..3], &[5, 1, 0]);
            if scheme == "socks5h" {
                assert_eq!(request[3], 3);
                let mut domain = vec![0; connection.read_u8().await.unwrap().into()];
                connection.read_exact(&mut domain).await.unwrap();
                assert_eq!(domain, host.as_bytes());
            } else {
                let ip = match request[3] {
                    1 => {
                        let mut octets = [0; 4];
                        connection.read_exact(&mut octets).await.unwrap();
                        IpAddr::V4(Ipv4Addr::from(octets))
                    }
                    4 => {
                        let mut octets = [0; 16];
                        connection.read_exact(&mut octets).await.unwrap();
                        IpAddr::V6(Ipv6Addr::from(octets))
                    }
                    kind => panic!("expected a locally resolved address, got {kind}"),
                };
                assert!(ip.is_loopback());
            }
            assert_eq!(connection.read_u16().await.unwrap(), port);
            connection
                .write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0])
                .await
                .unwrap();
        };

        let (result, ()) = timeout(Duration::from_secs(5), async {
            tokio::join!(client.bootstrap(), handshake)
        })
        .await
        .unwrap();

        assert!(matches!(result, Err(Error::Transport(_))));
        assert!(
            timeout(Duration::from_millis(50), destination.accept())
                .await
                .is_err()
        );
    }
}
