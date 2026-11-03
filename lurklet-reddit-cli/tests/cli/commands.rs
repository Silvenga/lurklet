use super::support::{TOKEN, cached_command, command, state, write_state};
use assert_fs::TempDir;
use serde_json::{Value, json};
use std::fs;

#[test]
fn when_no_command_is_supplied_then_the_cli_should_show_help_without_side_effects() {
    let directory = TempDir::new().unwrap();

    let output = command().current_dir(directory.path()).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("bootstrap"));
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn when_valid_overrides_and_a_cached_session_are_supplied_then_the_cli_should_emit_redacted_json_without_network_access()
 {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let expected = state();
    write_state(&path, &expected);
    let overrides = "--graphql-endpoint https://127.0.0.1:1/
        --client-id custom-client --scope one --scope two
        --app-version 1.2.3 --app-build 42 --android-release 15
        --manufacturer Example --model Model --locale en-GB --display-density -1
        --display-width-px 800 --display-height-px 1200 --user-agent custom-agent
        --emulation-profile okhttp_4.12 --reddit-compression 0 --download-rate-mbps 12.3
        --graphql-accept application/json --graphql-retry-header attempt=0,max=0
        --allowlist-operation-name CustomAllowlist --allowlist-sha256 custom-hash
        --height-header=false --max-response-bytes 4096
        --request-timeout-ms 1234 --connect-timeout-ms 567";

    let output = cached_command(&path)
        .args(overrides.split_whitespace())
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let restored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        result,
        json!({"ok": true, "captures_written": 0, "result": {"access_token": "[REDACTED]", "expires_at_ms": 10000}})
    );
    assert_eq!(restored, expected);
    assert!(String::from_utf8_lossy(&output.stderr).contains("TRACE"));
    for secret in [
        TOKEN,
        "private-fixture-loid",
        "12345678-1234-4234-8234-123456789abc",
    ] {
        assert!(!String::from_utf8_lossy(&output.stdout).contains(secret));
        assert!(!String::from_utf8_lossy(&output.stderr).contains(secret));
    }
}

#[test]
fn when_secret_output_is_enabled_then_only_stdout_should_contain_the_token() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    write_state(&path, &state());

    let output = cached_command(&path)
        .arg("--show-secrets")
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(output.status.success());
    assert_eq!(result["result"]["access_token"], TOKEN);
    assert!(!String::from_utf8_lossy(&output.stderr).contains(TOKEN));
}

#[test]
fn when_logging_configuration_is_invalid_then_the_cli_should_emit_a_structured_error() {
    let output = command()
        .env("RUST_LOG", "invalid[syntax")
        .arg("bootstrap")
        .output()
        .unwrap();

    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["error"]["kind"], "logging");
}

#[test]
fn when_allowlist_header_construction_fails_then_the_cli_should_return_a_structured_error_without_network()
 {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let expected = state();
    write_state(&path, &expected);

    let output = command()
        .arg("--state-file")
        .arg(&path)
        .args([
            "--clock-ms",
            "1000",
            "--graphql-endpoint",
            "https://127.0.0.1:1/",
            "--auth-endpoint",
            "https://127.0.0.1:1/token",
            "--allowlist-operation-name",
            "bad\nheader",
            "--allowlist-sha256",
            "fixture",
            "get-request-allowlist",
        ])
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["error"]["kind"], "invalid_request");
    assert_eq!(result["error"]["field"], "x-apollo-operation-name");
    assert_eq!(result["captures_written"], 0);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
        expected
    );
}

#[test]
fn when_arguments_are_incomplete_then_the_cli_should_reject_them_without_side_effects() {
    let directory = TempDir::new().unwrap();

    let output = command()
        .current_dir(directory.path())
        .arg("get-post")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn when_post_transport_fails_then_the_cli_should_return_nonzero_status_and_preserve_session_state()
{
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let expected = state();
    write_state(&path, &expected);

    let output = command()
        .arg("--state-file")
        .arg(&path)
        .args([
            "--clock-ms",
            "1000",
            "--graphql-endpoint",
            "https://127.0.0.1:1/",
            "--auth-endpoint",
            "https://127.0.0.1:1/token",
            "get-post",
            "18t5968",
        ])
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["kind"], "transport");
    assert_eq!(result["captures_written"], 0);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
        expected
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains(TOKEN));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(TOKEN));
}

#[test]
fn when_a_listing_cursor_is_invalid_then_the_cli_should_return_an_input_error_without_network() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let expected = state();
    write_state(&path, &expected);

    let output = command()
        .arg("--state-file")
        .arg(&path)
        .args([
            "--auth-endpoint",
            "https://127.0.0.1:1/token",
            "list-subreddit-posts",
            "rust",
            "--after",
            "invalid!",
        ])
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["error"]["kind"], "invalid_request");
    assert_eq!(result["error"]["field"], "after");
    assert_eq!(result["captures_written"], 0);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
        expected
    );
}

#[test]
fn when_a_media_url_is_unsupported_then_the_cli_should_ignore_session_state_and_leave_no_file_or_credentials()
 {
    let directory = TempDir::new().unwrap();
    let state_path = directory.path().join("session.json");
    let file_path = directory.path().join("media.bin");
    let capture_path = directory.path().join("captures");
    fs::write(&state_path, "invalid state that media must not read").unwrap();

    let output = command()
        .arg("--state-file")
        .arg(&state_path)
        .arg("--capture-dir")
        .arg(&capture_path)
        .args([
            "get-media",
            "https://example.invalid/file?private=signed",
            "--output",
        ])
        .arg(&file_path)
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(value["error"]["kind"], "invalid_request");
    assert_eq!(value["error"]["field"], "media_url");
    assert_eq!(value["captures_written"], 0);
    assert!(!file_path.exists());
    assert!(!directory.path().join("session.json.lock").exists());
    assert_eq!(
        fs::read_to_string(state_path).unwrap(),
        "invalid state that media must not read"
    );
    assert!(!stderr.contains("Session state is ephemeral"));
    assert!(!stderr.contains("private=signed"));
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("private=signed")
    );
    assert_eq!(fs::read_dir(capture_path).unwrap().count(), 0);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn when_a_comment_cursor_is_invalid_then_the_cli_should_return_nonzero_status_without_network() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let expected = state();
    write_state(&path, &expected);

    let output = command()
        .arg("--state-file")
        .arg(&path)
        .args([
            "--auth-endpoint",
            "https://127.0.0.1:1/token",
            "--graphql-endpoint",
            "https://127.0.0.1:1/",
            "get-post-comments",
            "post",
            "--cursor",
            "invalid!",
        ])
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["error"]["kind"], "invalid_request");
    assert_eq!(result["error"]["field"], "cursor");
    assert_eq!(result["captures_written"], 0);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
        expected
    );
}
