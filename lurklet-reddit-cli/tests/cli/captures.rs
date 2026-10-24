use super::support::{cached_command, state, write_state};
use assert_fs::TempDir;
use serde_json::Value;
use std::fs;

#[test]
fn when_capture_is_enabled_for_a_cached_result_then_no_response_capture_should_be_invented() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let capture_dir = directory.path().join("captures");
    write_state(&path, &state());

    let output = cached_command(&path)
        .arg("--capture-dir")
        .arg(&capture_dir)
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(output.status.success());
    assert_eq!(result["captures_written"], 0);
    assert_eq!(fs::read_dir(capture_dir).unwrap().count(), 0);
}

#[test]
fn when_a_capture_directory_exists_then_the_cli_should_accept_its_default_permissions() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("session.json");
    let captures = directory.path().join("captures");
    write_state(&path, &state());
    fs::create_dir_all(&captures).unwrap();

    let output = cached_command(&path)
        .arg("--capture-dir")
        .arg(&captures)
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(output.status.success());
    assert_eq!(result["captures_written"], 0);
    assert_eq!(fs::read_dir(captures).unwrap().count(), 0);
}
