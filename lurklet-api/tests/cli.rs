use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use axum::Router;
use axum::http::{HeaderMap, StatusCode, header};
use axum::routing::get;
use serde_json::Value;
use std::future::IntoFuture;
use std::net::TcpListener;
use std::time::Duration;
use tokio::net::TcpListener as AsyncTcpListener;
use tokio::task;

#[test]
fn when_openapi_export_is_requested_then_command_should_emit_only_the_contract() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut command = Command::new(cargo_bin!("lurklet-api"));
    command
        .arg("--bind")
        .arg(listener.local_addr().unwrap().to_string())
        .arg("export-openapi")
        .env("LURKLET_STATIC_DIR", "/missing-static-directory")
        .timeout(Duration::from_secs(10))
        .env_remove("RUST_LOG")
        .env_remove("LURKLET_BIND");

    let output = command.assert().success().get_output().clone();
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(output.stderr.is_empty());
    assert_eq!(document["openapi"], "3.2.0");
    assert_eq!(
        document["paths"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["/api/health"]
    );
    assert_eq!(
        document["paths"]["/api/health"]["get"]["operationId"],
        "get_health"
    );
}

#[test]
fn when_log_filter_is_invalid_then_command_should_fail_without_echoing_it() {
    let mut command = Command::new(cargo_bin!("lurklet-api"));
    command
        .arg("export-openapi")
        .timeout(Duration::from_secs(10))
        .env("RUST_LOG", "private_target=invalid_level")
        .env_remove("LURKLET_BIND");

    let output = command.assert().failure().get_output().clone();

    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"Invalid RUST_LOG configuration\n");
}

#[test]
fn when_listener_cannot_bind_then_default_logs_should_report_failure_without_details() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let mut command = Command::new(cargo_bin!("lurklet-api"));
    command
        .env("LURKLET_BIND", &address)
        .env(
            "LURKLET_STATIC_DIR",
            concat!(env!("CARGO_MANIFEST_DIR"), "/static"),
        )
        .env_remove("RUST_LOG")
        .timeout(Duration::from_secs(10));

    let output = command.assert().failure().get_output().clone();
    let logs = String::from_utf8(output.stderr).unwrap();

    assert!(output.stdout.is_empty());
    assert!(logs.contains("ERROR"));
    assert!(logs.contains("API execution failed; operator action is required"));
    assert!(!logs.contains(&address));
    assert!(!logs.contains("API failure details"));
}

#[test]
fn when_openapi_export_flag_is_used_then_command_should_reject_it() {
    let mut command = Command::new(cargo_bin!("lurklet-api"));
    command
        .arg("--export-openapi")
        .env_remove("LURKLET_BIND")
        .timeout(Duration::from_secs(10));

    let output = command.assert().failure().get_output().clone();

    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unexpected argument")
    );
}

#[tokio::test]
async fn when_health_check_is_requested_then_command_should_probe_directly_and_succeed_silently() {
    let listener = AsyncTcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/api/health",
        get(move |headers: HeaderMap| async move {
            assert_eq!(headers[header::HOST], address.to_string());
            StatusCode::OK
        }),
    );
    let server = tokio::spawn(axum::serve(listener, app).into_future());
    let mut command = Command::new(cargo_bin!("lurklet-api"));
    command
        .arg("health-check")
        .env("LURKLET_BIND", format!("0.0.0.0:{}", address.port()))
        .env("LURKLET_STATIC_DIR", "/missing-static-directory")
        .env("RUST_LOG", "trace")
        .env("HTTP_PROXY", "http://127.0.0.1:9")
        .env("http_proxy", "http://127.0.0.1:9")
        .env("HTTPS_PROXY", "http://127.0.0.1:9")
        .env("https_proxy", "http://127.0.0.1:9")
        .env("ALL_PROXY", "http://127.0.0.1:9")
        .env("all_proxy", "http://127.0.0.1:9")
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .timeout(Duration::from_secs(5));

    let output = task::spawn_blocking(move || command.assert().success().get_output().clone())
        .await
        .unwrap();
    server.abort();

    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn when_health_check_fails_then_command_should_exit_unsuccessfully_without_sensitive_details() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut command = Command::new(cargo_bin!("lurklet-api"));
    command
        .arg("health-check")
        .env("LURKLET_BIND", address.to_string())
        .env("RUST_LOG", "trace")
        .timeout(Duration::from_secs(5));

    let output = command.assert().failure().get_output().clone();

    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"Health check failed\n");
}

#[test]
fn when_static_index_is_missing_then_server_should_fail_before_binding() {
    let directory = assert_fs::TempDir::new().unwrap();
    let mut command = Command::new(cargo_bin!("lurklet-api"));
    command
        .arg("--static-dir")
        .arg(directory.path())
        .env("LURKLET_BIND", "127.0.0.1:0")
        .env("RUST_LOG", "debug")
        .timeout(Duration::from_secs(5));

    let output = command.assert().failure().get_output().clone();

    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("static index.html is missing")
    );
}
