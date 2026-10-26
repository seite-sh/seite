use super::common::*;

fn site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Context Site", "posts,docs,pages");
    let dir = tmp.path().join("site");
    // Machines where no zone is detectable get a commented line; pin one so
    // the directive list does not depend on the test machine.
    let toml = fs::read_to_string(dir.join("seite.toml")).unwrap();
    if !toml.contains("\ntimezone = ") {
        let toml = toml.replace("# timezone = ", "timezone = ");
        fs::write(dir.join("seite.toml"), toml).unwrap();
    }
    dir
}

#[test]
fn test_context_timezone_unset_directive() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    let toml = fs::read_to_string(dir.join("seite.toml")).unwrap();
    let toml: String = toml
        .lines()
        .filter(|l| !l.starts_with("timezone = "))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(dir.join("seite.toml"), toml).unwrap();
    let (ok, doc) = run_json(&dir, &["context"]);
    assert!(ok, "{doc}");
    let names: Vec<&str> = doc["data"]["directives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"TIMEZONE_UNSET"), "{names:?}");
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
        .stdout(predicate::str::contains("`seite audit`"))
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
    // Our own test process is certainly alive, and listens on the port.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    fs::write(
        dir.join(".seite/dev.json"),
        format!(
            r#"{{"pid": {}, "port": {port}, "url": "http://localhost:{port}"}}"#,
            std::process::id()
        ),
    )
    .unwrap();
    let (ok, doc) = run_json(&dir, &["context"]);
    assert!(ok);
    assert_eq!(
        doc["data"]["dev_server"]["url"],
        format!("http://127.0.0.1:{port}")
    );
    drop(listener);
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

fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn test_serve_writes_and_removes_dev_json() {
    use std::io::Write;
    use std::process::Stdio;

    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    let dev = dir.join(".seite/dev.json");
    let _ = std::fs::remove_file(&dev);

    // Retry across fresh ports in case one is raced away before bind.
    for attempt in 0..5 {
        let port = free_port();
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("seite"))
            .args(["serve", "--host", "127.0.0.1", "--port", &port.to_string()])
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let start = std::time::Instant::now();
        while !dev.exists()
            && child.try_wait().unwrap().is_none()
            && start.elapsed() < std::time::Duration::from_secs(10)
        {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if !dev.exists() {
            let _ = child.kill();
            let _ = child.wait();
            assert!(attempt < 4, "dev.json never appeared");
            continue;
        }

        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&dev).unwrap()).unwrap();
        assert_eq!(v["pid"], child.id());
        assert_eq!(v["port"], port);
        assert_eq!(v["url"], format!("http://127.0.0.1:{port}"));
        assert!(v["started_at"].is_string());

        child.stdin.take().unwrap().write_all(b"stop\n").unwrap();
        assert!(child.wait().unwrap().success());
        assert!(!dev.exists(), "dev.json should be removed on stop");
        return;
    }
}

#[cfg(unix)]
#[test]
fn test_serve_refuses_second_server_when_dev_json_alive() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    std::fs::create_dir_all(dir.join(".seite")).unwrap();
    // A live pid (ours) and a listening loopback port stand in for a server.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::fs::write(
        dir.join(".seite/dev.json"),
        format!(
            r#"{{"pid":{},"port":{port},"url":"https://evil.example","started_at":"2026-01-01T00:00:00Z"}}"#,
            std::process::id()
        ),
    )
    .unwrap();
    page_cmd()
        .arg("serve")
        .current_dir(&dir)
        .timeout(std::time::Duration::from_secs(20))
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("http://127.0.0.1:{port}")))
        .stdout(predicate::str::contains("evil.example").not());
    drop(listener);
}

#[test]
fn test_context_ignores_planted_dev_json() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    std::fs::create_dir_all(dir.join(".seite")).unwrap();
    // Live pid, but nothing listening on the port: not reported.
    std::fs::write(
        dir.join(".seite/dev.json"),
        format!(
            r#"{{"pid":{},"port":1,"url":"https://evil.example"}}"#,
            std::process::id()
        ),
    )
    .unwrap();
    let (ok, doc) = run_json(&dir, &["context"]);
    assert!(ok);
    assert!(doc["data"]["dev_server"].is_null(), "{doc}");
}
