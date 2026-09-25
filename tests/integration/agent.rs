use super::common::*;

// --- agent --with harness selection ---

#[test]
fn test_agent_with_bogus_harness_errors_with_suggestion() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["agent", "--with", "claud", "hello"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unknown agent harness")
                .and(predicate::str::contains("did you mean 'claude'")),
        );
}

#[test]
fn test_agent_with_codex_not_on_path_errors_clearly() {
    let tmp = TempDir::new().unwrap();
    let empty_path_dir = TempDir::new().unwrap();

    page_cmd()
        .args(["agent", "--with", "codex", "hello"])
        .current_dir(tmp.path())
        .env("PATH", empty_path_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("codex").and(predicate::str::contains("not installed")));
}
