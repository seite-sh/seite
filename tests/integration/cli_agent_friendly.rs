use super::common::*;

/// Regression test for #86: `seite serve` must bake the address it actually
/// serves on into absolute URLs, not the hardcoded default localhost:3000.
#[test]
fn test_serve_bakes_actual_port_into_absolute_urls() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Serve URLs", "posts,pages");
    let site_dir = tmp.path().join("site");

    // There's an unavoidable TOCTOU gap between picking a free ephemeral port
    // and `serve` binding it, so a busy/parallel host can steal it (serve uses
    // an explicit --port and errors rather than auto-incrementing). Retry across
    // a few fresh ports before giving up.
    for attempt in 0..5 {
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        // `stop` on stdin makes the interactive REPL exit after the initial build.
        let output = page_cmd()
            .args(["serve", "--host", "127.0.0.1", "--port", &port.to_string()])
            .current_dir(&site_dir)
            .write_stdin("stop\n")
            .output()
            .unwrap();

        if !output.status.success() {
            // Most likely the port was raced away between selection and bind, so
            // retry on a fresh one — but surface the real stderr once we exhaust
            // the attempts, so a genuine failure isn't hidden as a "port race".
            assert!(
                attempt < 4,
                "serve failed to start on 5 ephemeral ports; last stderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            continue;
        }

        // The sitemap always emits absolute URLs derived from base_url.
        let sitemap = fs::read_to_string(site_dir.join("dist/sitemap.xml")).unwrap();
        assert!(
            sitemap.contains(&format!("http://127.0.0.1:{port}")),
            "sitemap should use the actual serve address, got:\n{sitemap}"
        );
        assert!(
            !sitemap.contains("localhost:3000"),
            "sitemap must not fall back to the hardcoded localhost:3000"
        );
        return;
    }
}

// =========================================================================
// Completions
// =========================================================================

#[test]
fn test_completions_bash_outputs_script() {
    page_cmd()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicates::str::contains("seite"));
}

#[test]
fn test_completions_zsh_outputs_script() {
    page_cmd()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicates::str::contains("seite"));
}

#[test]
fn test_completions_fish_outputs_script() {
    page_cmd()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicates::str::contains("seite"));
}

#[test]
fn test_completions_stdout_is_clean() {
    // The completion script must be usable with shell redirection (seite completions bash > file.sh).
    // No informational messages should appear on stdout — only the generated script.
    let output = page_cmd()
        .args(["completions", "bash"])
        .output()
        .expect("failed to run completions");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should look like a shell script, not a human-readable message
    assert!(
        stdout.contains("complete") || stdout.contains("compgen") || stdout.contains("_seite"),
        "stdout should contain bash completion code, got: {stdout}"
    );
    // Update-check or advisory messages on stdout would corrupt the script
    assert!(
        !stdout.contains("A new version"),
        "update notification must not appear on stdout"
    );
}

#[test]
fn test_init_non_interactive_with_minimal_flags() {
    let tmp = TempDir::new().unwrap();
    // No --title / --description / --collections and no TTY: defaults apply.
    page_cmd()
        .args(["init", "agentsite", "--deploy-target", "netlify"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let toml = fs::read_to_string(tmp.path().join("agentsite/seite.toml")).unwrap();
    assert!(toml.contains("title = \"agentsite\""), "{toml}");
    assert!(toml.contains("target = \"netlify\""), "{toml}");
    assert!(tmp.path().join("agentsite/content/posts").is_dir());
    assert!(tmp.path().join("agentsite/content/pages").is_dir());
}

#[test]
fn test_init_missing_deploy_target_names_flag() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["init", "site", "--title", "T", "--collections", "posts"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing --deploy-target"))
        .stderr(predicate::str::contains(
            "github-pages, cloudflare, netlify",
        ));
    assert!(
        !tmp.path().join("site").exists(),
        "nothing should be created"
    );
}

#[test]
fn test_init_missing_name_names_argument() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["init", "--deploy-target", "github-pages"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing <NAME> argument"));
}

#[test]
fn test_init_rejects_unknown_collection() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "site",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,blogg",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unknown collection preset 'blogg'",
        ));
}

#[test]
fn test_init_tree_output_is_aligned() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "site",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\n  ├── seite.toml"))
        .stdout(predicate::str::contains("\n  │   ├── posts/"))
        .stdout(predicate::str::contains("\n  │   └── pages/"))
        .stdout(predicate::str::contains("\n  └── static/"));
}

#[test]
fn test_init_json_reports_project() {
    let tmp = TempDir::new().unwrap();
    let output = page_cmd()
        .args([
            "--json",
            "init",
            "site",
            "--deploy-target",
            "cloudflare",
            "--collections",
            "posts,docs",
        ])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], true);
    assert_eq!(doc["command"], "init");
    assert_eq!(doc["data"]["deploy_target"], "cloudflare");
    assert_eq!(
        doc["data"]["collections"],
        serde_json::json!(["posts", "docs"])
    );
    assert_eq!(
        doc["data"]["agents"],
        serde_json::json!(["claude", "codex", "opencode", "cursor"])
    );
}

#[test]
fn test_build_json_reports_counts() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Json Build", "posts,pages");
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(tmp.path().join("site"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], true);
    assert_eq!(doc["command"], "build");
    let data = &doc["data"];
    assert_eq!(data["collections"]["posts"], 1);
    assert!(data["pages_written"].as_u64().unwrap() >= 1);
    assert!(data["output_dir"].as_str().unwrap().ends_with("dist"));
    assert!(data["duration_ms"].is_u64());
    assert!(
        !data["timings_ms"].as_object().unwrap().is_empty(),
        "timings belong in the JSON data"
    );
    assert!(data["broken_links"].as_array().unwrap().is_empty());
}

#[test]
fn test_build_json_failure_reports_error() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Json Fail", "posts");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/bad.md"),
        "---\ntitle: ok\ndate: notadate\n---\nbody\n",
    )
    .unwrap();
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(!output.status.success(), "failed build must exit non-zero");
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], false);
    assert_eq!(doc["command"], "build");
    let message = doc["error"]["message"].as_str().unwrap();
    assert!(message.contains("1 error"), "{message}");
    assert!(doc["error"]["chain"].is_array());
    let diagnostics = doc["error"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1, "{doc}");
    assert_eq!(diagnostics[0]["code"], "frontmatter-parse");
    assert_eq!(diagnostics[0]["file"], "content/posts/bad.md");
    assert_eq!(diagnostics[0]["line"], 3);
}

#[test]
fn test_build_frontmatter_error_has_no_empty_cause() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Cause", "posts");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/bad.md"),
        "---\ntitle: ok\ndate: notadate\n---\nbody\n",
    )
    .unwrap();
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "content/posts/bad.md:3:7: error[frontmatter-parse]",
        ))
        // The cause is already part of the message; no blank/duplicate chain.
        .stderr(predicate::str::contains("Caused by").not());
}

#[test]
fn test_new_json_returns_path() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Json New", "posts");
    let site = tmp.path().join("site");
    let output = page_cmd()
        .args(["--json", "new", "post", "Agent Written"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], true);
    assert_eq!(doc["data"]["collection"], "posts");
    assert_eq!(doc["data"]["slug"], "agent-written");
    assert_eq!(doc["data"]["url"], "/posts/agent-written");
    let path = doc["data"]["path"].as_str().unwrap();
    assert!(std::path::Path::new(path).is_file(), "{path} should exist");
}

#[test]
fn test_serve_json_is_rejected() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Json Serve", "posts");
    let output = page_cmd()
        .args(["--json", "serve"])
        .current_dir(tmp.path().join("site"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], false);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--json is not supported"));
}

#[test]
fn test_build_without_verbose_prints_no_timings() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Quiet", "posts");
    let site = tmp.path().join("site");
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("Built"))
        .stdout(predicate::str::contains("Timings").not());
    page_cmd()
        .args(["--verbose", "build"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("Timings:"));
}

#[test]
fn test_build_update_check_suppressed_under_ci() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Nag", "posts");
    // Fresh cache claiming a much newer release: without the opt-outs this
    // would print the "new version" notice.
    let home = tmp.path().join("home");
    fs::create_dir_all(home.join(".seite")).unwrap();
    fs::write(
        home.join(".seite/update-cache.json"),
        format!(
            "{{\"last_check\":\"{}\",\"latest_version\":\"999.0.0\"}}",
            chrono::Utc::now().to_rfc3339()
        ),
    )
    .unwrap();
    page_cmd()
        .arg("build")
        .current_dir(tmp.path().join("site"))
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("CI", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("new version").not())
        .stderr(predicate::str::contains("new version").not());
}

#[test]
fn test_config_flag_runs_in_config_directory() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Config Flag", "posts");
    page_cmd()
        .args(["--config", "site/seite.toml", "build"])
        .current_dir(tmp.path())
        .assert()
        .success();
    assert!(tmp.path().join("site/dist/index.html").exists());
    assert!(!tmp.path().join("dist").exists());
}

#[test]
fn test_config_flag_rejects_custom_file_names() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Config Flag", "posts");
    fs::copy(
        tmp.path().join("site/seite.toml"),
        tmp.path().join("site/other.toml"),
    )
    .unwrap();
    page_cmd()
        .args(["--config", "site/other.toml", "build"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "custom config file names are not supported",
        ));
    page_cmd()
        .args(["--config", "site/missing/seite.toml", "build"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("--config file not found"));
}

#[test]
fn test_upgrade_check_json_reports_pending_changes() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade Json", "posts");
    let site = tmp.path().join("site");
    // Remove the version stamp so the project looks pre-tracking.
    fs::remove_file(site.join(".seite/config.json")).unwrap();
    let output = page_cmd()
        .args(["--json", "upgrade", "--check"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "--check exits 1 when upgrades are pending"
    );
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], false);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("needs upgrading"));
}

#[test]
fn test_serve_no_repl_stays_up() {
    assert_serve_stays_up(&["--no-repl"]);
}

#[test]
fn test_serve_stays_up_when_stdin_closed() {
    assert_serve_stays_up(&[]);
}
