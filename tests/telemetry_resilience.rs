use std::process::Command;

/// With telemetry pointed at an unroutable endpoint, a normal command must still
/// succeed quickly and exit 0 (telemetry is detached + best-effort).
#[test]
fn command_succeeds_when_telemetry_endpoint_is_dead() {
    let out = Command::new(env!("CARGO_BIN_EXE_seite"))
        .arg("--version")
        .env("SEITE_TELEMETRY_ENDPOINT", "http://127.0.0.1:0/api/event")
        .env("SEITE_TELEMETRY", "1")
        .env_remove("DO_NOT_TRACK")
        .env_remove("CI")
        .output()
        .expect("run seite");
    assert!(out.status.success(), "seite --version should exit 0");
}
