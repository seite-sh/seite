use super::common::*;

// --- workspace CLI ---

#[test]
fn test_workspace_init() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["workspace", "init", "my-ws"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // workspace init creates in cwd, with the name as the config name, not a subdir
    assert!(tmp.path().join("seite-workspace.toml").exists());
    assert!(tmp.path().join("sites").is_dir());
}

#[test]
fn test_workspace_add_and_list() {
    let tmp = TempDir::new().unwrap();

    // Init workspace
    page_cmd()
        .args(["workspace", "init", "ws"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Add a site with a unique name
    page_cmd()
        .args([
            "workspace",
            "add",
            "mysite",
            "--title",
            "My Site",
            "--collections",
            "posts",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    // List should show the site
    page_cmd()
        .args(["workspace", "list"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("mysite"));
}

#[test]
fn test_workspace_status() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["workspace", "init", "ws-status"])
        .current_dir(tmp.path())
        .assert()
        .success();

    page_cmd()
        .args([
            "workspace",
            "add",
            "docs",
            "--title",
            "Docs",
            "--collections",
            "docs,pages",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    page_cmd()
        .args(["workspace", "status"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("docs"));
}

// --- workspace build ---

#[test]
fn test_workspace_build() {
    let tmp = TempDir::new().unwrap();

    // Init workspace
    page_cmd()
        .args(["workspace", "init", "ws-build"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Add a site with a unique name
    page_cmd()
        .args([
            "workspace",
            "add",
            "myblog",
            "--title",
            "My Blog",
            "--collections",
            "posts",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Build all sites in workspace
    page_cmd()
        .args(["build"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Verify output was created
    assert!(tmp.path().join("sites/myblog/dist").exists());
}

#[test]
fn test_workspace_build_with_site_filter() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["workspace", "init", "ws-filter"])
        .current_dir(tmp.path())
        .assert()
        .success();

    page_cmd()
        .args([
            "workspace",
            "add",
            "docs",
            "--title",
            "Docs",
            "--collections",
            "docs",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Build only the docs site
    page_cmd()
        .args(["build", "--site", "docs"])
        .current_dir(tmp.path())
        .assert()
        .success();
}

/// Regression follow-up to #86: `serve --site` in a workspace must bake the
/// live dev address into that site's absolute URLs, not localhost:3000.
#[test]
fn test_workspace_serve_site_bakes_actual_port() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["workspace", "init", "ws-serve"])
        .current_dir(tmp.path())
        .assert()
        .success();

    page_cmd()
        .args([
            "workspace",
            "add",
            "foo",
            "--title",
            "Foo",
            "--collections",
            "posts,pages",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Retry across ephemeral ports (TOCTOU between selection and the child bind).
    for attempt in 0..5 {
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        let output = page_cmd()
            .args([
                "serve",
                "--site",
                "foo",
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
            ])
            .current_dir(tmp.path())
            .write_stdin("stop\n")
            .output()
            .unwrap();

        if !output.status.success() {
            // Retry on a fresh port for a likely race, but surface the real
            // stderr on exhaustion so a genuine failure isn't hidden.
            assert!(
                attempt < 4,
                "serve --site failed to start on 5 ephemeral ports; last stderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            continue;
        }

        let sitemap = fs::read_to_string(tmp.path().join("sites/foo/dist/sitemap.xml")).unwrap();
        assert!(
            sitemap.contains(&format!("http://127.0.0.1:{port}")),
            "workspace --site sitemap should use the actual serve address, got:\n{sitemap}"
        );
        assert!(
            !sitemap.contains("localhost:3000"),
            "workspace --site sitemap must not fall back to localhost:3000"
        );
        return;
    }
}

// --- workspace init and add ---

#[test]
fn test_workspace_init_and_add_site() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["workspace", "init", "ws-test"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Verify workspace file was created
    assert!(tmp.path().join("seite-workspace.toml").exists());
    assert!(tmp.path().join("sites").exists());

    // Add a site (use "mysite" not "blog" — init template has a commented `# name = "blog"` line
    // which trips the naive string-contains check in workspace add)
    page_cmd()
        .args(["workspace", "add", "mysite", "--collections", "posts"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Verify the site was added
    assert!(tmp.path().join("sites/mysite/seite.toml").exists());

    // List sites
    page_cmd()
        .args(["workspace", "list"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("mysite"));
}

/// Helper: init a workspace in `tmp` with one `posts` site per name.
fn init_workspace_with_sites(tmp: &TempDir, sites: &[&str]) {
    page_cmd()
        .args(["workspace", "init", "ws"])
        .current_dir(tmp.path())
        .assert()
        .success();
    for name in sites {
        page_cmd()
            .args([
                "workspace",
                "add",
                name,
                "--title",
                name,
                "--collections",
                "posts",
            ])
            .current_dir(tmp.path())
            .assert()
            .success();
    }
}

#[test]
fn test_workspace_build_strict_json_includes_link_diagnostics() {
    let tmp = TempDir::new().unwrap();
    init_workspace_with_sites(&tmp, &["alpha", "beta"]);
    for site in ["alpha", "beta"] {
        write_site_file(
            &tmp.path().join("sites").join(site),
            "content/posts/2025-01-01-broken.md",
            "---\ntitle: Broken\n---\n\nSee [gone](/posts/not-here).\n",
        );
    }

    let output = page_cmd()
        .args(["--json", "build", "--strict"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], false);
    let diagnostics = doc["error"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("no diagnostics: {doc}"));
    // One per site, with workspace-relative (unambiguous) file paths.
    assert_eq!(diagnostics.len(), 2, "{doc}");
    for (d, site) in diagnostics.iter().zip(["alpha", "beta"]) {
        assert_eq!(d["code"], "broken-link", "{doc}");
        assert_eq!(d["severity"], "error", "{doc}");
        assert_eq!(
            d["file"],
            format!("sites/{site}/content/posts/2025-01-01-broken.md"),
            "{doc}"
        );
        assert_eq!(d["line"], 5, "{doc}");
    }

    // Human output: grouped report per site plus one summary, no repeats.
    let output = page_cmd()
        .args(["build", "--strict"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(all.matches("/posts/not-here").count(), 2, "{all}");
    assert!(all.contains("Error: Build failed"), "{all}");
    assert!(
        all.contains("Site 'alpha'") && all.contains("Site 'beta'"),
        "{all}"
    );
}

/// Add a `minfy` typo under `[build]` in a workspace site; returns its line.
fn add_unknown_key(site: &std::path::Path) -> usize {
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    let config = config.replacen("[build]\n", "[build]\nminfy = true\n", 1);
    fs::write(site.join("seite.toml"), &config).unwrap();
    config.lines().position(|l| l.starts_with("minfy")).unwrap() + 1
}

#[test]
fn test_workspace_build_warns_on_unknown_config_key() {
    let tmp = TempDir::new().unwrap();
    init_workspace_with_sites(&tmp, &["alpha"]);
    let line = add_unknown_key(&tmp.path().join("sites/alpha"));

    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "unknown keys must not fail");
    let doc = json_stdout(&output);
    let diagnostics = doc["data"]["sites"]["alpha"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("{doc}"));
    let d = diagnostics
        .iter()
        .find(|d| d["code"] == "config-unknown-key")
        .unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(d["severity"], "warning");
    assert_eq!(d["file"], "seite.toml", "site-relative within sites.alpha");
    assert_eq!(d["line"], line);
    assert_eq!(d["hint"], "did you mean `minify`?");
    assert!(doc["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains("minfy")));

    page_cmd()
        .arg("build")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "sites/alpha/seite.toml:{line}:1: warning[config-unknown-key]"
        )));
}

#[test]
fn test_workspace_check_reports_each_site() {
    let tmp = TempDir::new().unwrap();
    init_workspace_with_sites(&tmp, &["alpha", "beta"]);
    let line = add_unknown_key(&tmp.path().join("sites/alpha"));
    write_site_file(
        &tmp.path().join("sites/beta"),
        "content/posts/2025-01-01-broken.md",
        "---\ntitle: Broken\n---\n\nSee [gone](/posts/not-here).\n",
    );

    // Whole workspace from its root: both sites, workspace-relative files.
    let output = page_cmd()
        .args(["--json", "check"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "warnings only");
    let doc = json_stdout(&output);
    let diagnostics = doc["data"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("{doc}"));
    assert!(
        diagnostics.iter().any(|d| d["code"] == "config-unknown-key"
            && d["file"] == "sites/alpha/seite.toml"
            && d["line"] == line),
        "{doc}"
    );
    assert!(
        diagnostics.iter().any(|d| d["code"] == "broken-link"
            && d["file"] == "sites/beta/content/posts/2025-01-01-broken.md"
            && d["line"] == 5),
        "{doc}"
    );

    // --site limits the check to one site.
    let output = page_cmd()
        .args(["--json", "--site", "beta", "check", "--strict"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let doc = json_stdout(&output);
    let diagnostics = doc["error"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("{doc}"));
    assert!(diagnostics
        .iter()
        .all(|d| d["code"] != "config-unknown-key"));
    assert!(diagnostics.iter().any(|d| d["code"] == "broken-link"));

    // Unknown site names are an error.
    page_cmd()
        .args(["--site", "nope", "check"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown site 'nope'"));
}

#[test]
fn test_workspace_build_locates_problems_under_the_site_dir() {
    let tmp = TempDir::new().unwrap();
    init_workspace_with_sites(&tmp, &["alpha", "beta"]);
    // The workspace has its own templates/ dir, so a bare
    // `templates/base.html` would point at the wrong file.
    assert!(tmp.path().join("templates").is_dir());
    write_site_file(
        &tmp.path().join("sites/beta"),
        "templates/base.html",
        "<html>{% if %}</html>",
    );
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "template fallback is a warning");
    let doc = json_stdout(&output);
    let warnings: Vec<&str> = doc["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("sites/beta/templates/base.html:1:13: warning[template-parse]")),
        "{warnings:?}"
    );

    // A config syntax error fails the build, located in that site's seite.toml.
    let config_path = tmp.path().join("sites/alpha/seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("[site\n");
    let line = config.lines().count();
    fs::write(&config_path, config).unwrap();
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let doc = json_stdout(&output);
    let d = &doc["error"]["diagnostics"][0];
    assert_eq!(d["code"], "config-invalid", "{doc}");
    assert_eq!(d["file"], "sites/alpha/seite.toml");
    assert_eq!(d["line"], line);
}

#[test]
fn test_workspace_add_checks_real_sites_not_config_text() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["workspace", "init", "ws"])
        .current_dir(tmp.path())
        .assert()
        .success();
    // The scaffold's commented-out example is named "blog"; the workspace
    // itself is named "ws". Neither is a site.
    for name in ["blog", "ws"] {
        page_cmd()
            .args(["workspace", "add", name, "--collections", "posts"])
            .current_dir(tmp.path())
            .assert()
            .success();
        assert!(tmp
            .path()
            .join("sites")
            .join(name)
            .join("seite.toml")
            .is_file());
    }
    // A real duplicate is still refused.
    page_cmd()
        .args(["workspace", "add", "blog", "--collections", "posts"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "site 'blog' already exists in the workspace",
        ));
}
