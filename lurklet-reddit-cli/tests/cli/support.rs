use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub const TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJsb2lkIn0.c3ludGhldGlj";

pub fn command() -> Command {
    let mut command = cargo_bin_cmd!("lurklet-reddit-cli");
    command.env_remove("RUST_LOG");
    command
}

pub fn state() -> Value {
    json!({
        "version": 1,
        "state": {
            "installation": {"id": "12345678-1234-4234-8234-123456789abc", "created_at_ms": 1},
            "authentication": {"session": {"access_token": TOKEN, "expires_at_ms": 10000}, "loid": "private-fixture-loid"},
            "analytics": {"session_id": null, "creation_timestamp_ms": null}
        }
    })
}

pub fn write_state(path: &Path, state: &Value) {
    fs::write(path, state.to_string()).unwrap();
}

pub fn cached_command(path: &Path) -> Command {
    let mut command = command();
    command
        .args([
            "--auth-endpoint",
            "https://127.0.0.1:1/token",
            "--clock-ms",
            "1000",
            "--state-file",
        ])
        .arg(path)
        .arg("bootstrap");
    command
}
