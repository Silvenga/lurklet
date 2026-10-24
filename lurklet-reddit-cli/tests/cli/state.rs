use super::support::{cached_command, command, state, write_state};
use assert_fs::TempDir;
use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::net::TcpListener;

#[test]
fn when_state_is_corrupt_then_the_cli_should_fail_without_replacing_it() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    write_state(&path, &json!({"unexpected": "private-data"}));
    let before = fs::read(&path).unwrap();

    let output = cached_command(&path).output().unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["error"]["kind"], "state_format");
    assert_eq!(fs::read(path).unwrap(), before);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private-data"));
}

#[test]
fn when_another_process_owns_state_then_the_cli_should_fail_without_making_a_request() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let lock_path = directory.path().join("session.json.lock");
    write_state(&path, &state());
    write_state(&lock_path, &Value::Null);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path)
        .unwrap();
    lock.try_lock().unwrap();

    let output = cached_command(&path).output().unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["error"]["kind"], "state_busy");
}

#[test]
fn when_bootstrap_fails_then_the_cli_should_still_preserve_installation_identity() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("https://{}/token", listener.local_addr().unwrap());

    let output = command()
        .args(["--state-file"])
        .arg(&path)
        .args([
            "--auth-endpoint",
            &endpoint,
            "--request-timeout-ms",
            "100",
            "--connect-timeout-ms",
            "50",
            "bootstrap",
        ])
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["error"]["kind"], "transport");
    assert!(saved["state"]["installation"]["id"].as_str().is_some());
    assert!(saved["state"]["authentication"]["session"].is_null());
}

#[test]
fn when_state_format_is_unsupported_then_the_cli_should_preserve_the_original_file() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let mut original = state();
    original["version"] = 2.into();
    write_state(&path, &original);
    let before = fs::read(&path).unwrap();

    let output = cached_command(&path).output().unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["error"]["kind"], "state_format");
    assert_eq!(fs::read(path).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn when_state_uses_a_symlink_then_atomic_save_should_not_modify_the_target() {
    use std::os::unix::fs::symlink;

    let directory = TempDir::new().unwrap();
    let target = directory.path().join("original.json");
    let path = directory.path().join("session.json");
    write_state(&target, &state());
    let before = fs::read(&target).unwrap();
    symlink(&target, &path).unwrap();

    let output = cached_command(&path).output().unwrap();

    assert!(output.status.success());
    assert_eq!(fs::read(target).unwrap(), before);
    assert!(!fs::symlink_metadata(path).unwrap().file_type().is_symlink());
}
