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
