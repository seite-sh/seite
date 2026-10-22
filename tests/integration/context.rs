use super::common::*;

fn site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Context Site", "posts,docs,pages");
    tmp.path().join("site")
}

#[test]
fn test_context_human_summary_and_directives() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    page_cmd()
        .arg("context")
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "seite {}",
            env!("CARGO_PKG_VERSION")
        )))
        .stdout(predicate::str::contains("Context Site"))
        .stdout(predicate::str::contains("content/posts"))
        .stdout(predicate::str::contains("Theme:"))
        .stdout(predicate::str::contains("Dev server:"))
        // Fresh site: default theme, no impeccable.
        .stdout(predicate::str::contains("ADDON_SUGGESTION: optional"));
}

#[test]
fn test_context_json_shape() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    let (ok, doc) = run_json(&dir, &["context"]);
    assert!(ok, "{doc}");
    let data = &doc["data"];
    assert_eq!(data["in_project"], true);
    assert_eq!(data["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(data["site"]["title"], "Context Site");
    let posts = data["collections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "posts")
        .unwrap();
    assert_eq!(posts["dir"], "content/posts");
    assert_eq!(posts["items"], 1);
    assert_eq!(posts["drafts"], 0);
    assert_eq!(data["theme"]["active"], "default");
    assert_eq!(data["theme"]["customized"], false);
    assert!(data["dev_server"].is_null());
    // init records every agent's stop hook.
    assert!(!data["hooks_installed"].as_array().unwrap().is_empty());
    let names: Vec<&str> = data["directives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["ADDON_SUGGESTION"]);
}

#[test]
fn test_context_directives_for_old_project_without_hooks() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    fs::write(
        dir.join(".seite/config.json"),
        r#"{"version": "0.1.0", "agents": ["claude"]}"#,
    )
    .unwrap();
    // Customized base.html with impeccable installed: no addon suggestion.
    fs::write(
        dir.join("templates/base.html"),
        "{% block content %}{% endblock %}",
    )
    .unwrap();
    fs::create_dir_all(dir.join(".claude/skills/impeccable")).unwrap();

    let output = page_cmd()
        .arg("context")
        .current_dir(&dir)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("UPGRADE_AVAILABLE: project was set up with seite 0.1.0"),
        "{stdout}"
    );
    assert!(stdout.contains("NO_STOP_HOOK: "), "{stdout}");
    assert!(!stdout.contains("ADDON_SUGGESTION"), "{stdout}");
    assert!(stdout.contains("customized"), "{stdout}");
}

#[test]
fn test_context_reports_live_dev_server() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    // Our own test process is certainly alive.
    fs::write(
        dir.join(".seite/dev.json"),
        format!(
            r#"{{"pid": {}, "port": 4321, "url": "http://localhost:4321"}}"#,
            std::process::id()
        ),
    )
    .unwrap();
    let (ok, doc) = run_json(&dir, &["context"]);
    assert!(ok);
    if cfg!(unix) {
        assert_eq!(doc["data"]["dev_server"]["url"], "http://localhost:4321");
    }
}

#[test]
fn test_context_outside_project_degrades() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .arg("context")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("not in a seite project"));
    let (ok, doc) = run_json(tmp.path(), &["context"]);
    assert!(ok);
    assert_eq!(doc["data"]["in_project"], false);
    assert_eq!(doc["data"]["directives"], serde_json::json!([]));
}

#[test]
fn test_context_from_subdirectory_finds_site() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    let (ok, doc) = run_json(&dir.join("content/posts"), &["context"]);
    assert!(ok);
    assert_eq!(doc["data"]["site"]["title"], "Context Site");
}
