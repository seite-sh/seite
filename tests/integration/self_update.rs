use super::common::*;

// ── self-update command ─────────────────────────────────────────────

#[test]
fn test_self_update_check_when_current() {
    // --check with --target-version set to current version should succeed (already up to date)
    page_cmd()
        .args([
            "self-update",
            "--check",
            "--target-version",
            env!("CARGO_PKG_VERSION"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

// --- self-update ---

#[test]
fn test_self_update_check() {
    // --check with current version should report "already up to date"
    // Uses --target-version to avoid network dependency in CI
    page_cmd()
        .args([
            "self-update",
            "--check",
            "--target-version",
            env!("CARGO_PKG_VERSION"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}
