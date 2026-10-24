use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use serde_json::Value;
use std::net::TcpListener;
use std::time::Duration;

#[test]
fn when_openapi_export_is_requested_then_command_should_emit_only_the_contract() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut command = Command::new(cargo_bin!("lurklet-api"));
    command
        .arg("--bind")
        .arg(listener.local_addr().unwrap().to_string())
        .arg("export-openapi")
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
