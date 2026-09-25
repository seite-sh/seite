use super::common::*;

#[test]
fn test_build_reports_all_broken_files_in_one_pass() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Multi", "posts");
    let site = tmp.path().join("site");
    // Two bad frontmatters and one bad shortcode, in different files.
    write_site_file(
        &site,
        "content/posts/2024-01-01-a.md",
        "---\ntitle: A\ndate: notadate\n---\nbody\n",
    );
    write_site_file(
        &site,
        "content/posts/2024-01-02-b.md",
        "---\ntitle: B\ntags: [unclosed\n---\nbody\n",
    );
    write_site_file(
        &site,
        "content/posts/2024-01-03-c.md",
        "---\ntitle: C\n---\n\nIntro.\n\n{{< youtub(id=\"x\") >}}\n",
    );
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let doc = json_stdout(&output);
    let diagnostics = doc["error"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 3, "{doc}");
    let by_file = |name: &str| {
        diagnostics
            .iter()
            .find(|d| d["file"].as_str().unwrap().ends_with(name))
            .unwrap_or_else(|| panic!("no diagnostic for {name}: {doc}"))
    };
    let a = by_file("2024-01-01-a.md");
    assert_eq!(a["code"], "frontmatter-parse");
    assert_eq!(a["line"], 3);
    let b = by_file("2024-01-02-b.md");
    assert_eq!(b["code"], "frontmatter-parse");
    assert!(b["line"].as_u64().unwrap() >= 3, "{b}");
    let c = by_file("2024-01-03-c.md");
    assert_eq!(c["code"], "shortcode-unknown");
    assert_eq!(c["line"], 7, "shortcode line must be a file line: {c}");
    assert_eq!(c["hint"], "did you mean `youtube`?");

    // Human output: each diagnostic on its own compiler-style line, then summary.
    let output = page_cmd().arg("build").current_dir(&site).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("content/posts/2024-01-01-a.md:3:7: error[frontmatter-parse]"),
        "{stderr}"
    );
    assert!(
        stderr.contains("content/posts/2024-01-03-c.md:7:1: error[shortcode-unknown]"),
        "{stderr}"
    );
    assert!(
        stderr.contains("Error: found 3 errors, 0 warnings"),
        "{stderr}"
    );
}

#[test]
fn test_check_reports_every_shortcode_problem_in_a_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Shortcodes", "posts");
    let site = tmp.path().join("site");
    // Two syntax errors and one unknown shortcode in the same file.
    write_site_file(
        &site,
        "content/posts/2024-01-01-many.md",
        "---\ntitle: Many\n---\n\n{{< youtube id=\"x\" >}}\n\n{{< figure(src=) >}}\n\nText.\n\n{{< vimo(id=\"1\") >}}\n",
    );
    for cmd in ["check", "build"] {
        let output = page_cmd()
            .args(["--json", cmd])
            .current_dir(&site)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let doc = json_stdout(&output);
        let diagnostics = doc["error"]["diagnostics"].as_array().unwrap();
        let found: Vec<(String, u64)> = diagnostics
            .iter()
            .map(|d| {
                assert_eq!(d["file"], "content/posts/2024-01-01-many.md", "{doc}");
                (
                    d["code"].as_str().unwrap().to_string(),
                    d["line"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            found,
            vec![
                ("shortcode-syntax".to_string(), 5),
                ("shortcode-syntax".to_string(), 7),
                ("shortcode-unknown".to_string(), 11),
            ],
            "{cmd}: {doc}"
        );
    }
}

#[test]
fn test_build_warns_on_unknown_config_key_and_still_succeeds() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Keys", "posts");
    let site = tmp.path().join("site");
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    let config = config.replacen("[build]\n", "[build]\nminfy = true\n", 1);
    fs::write(site.join("seite.toml"), &config).unwrap();
    let line = config.lines().position(|l| l.starts_with("minfy")).unwrap() + 1;

    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "unknown keys must not fail the build"
    );
    let doc = json_stdout(&output);
    let diagnostics = doc["data"]["diagnostics"].as_array().unwrap();
    let d = diagnostics
        .iter()
        .find(|d| d["code"] == "config-unknown-key")
        .unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(d["severity"], "warning");
    assert_eq!(d["file"], "seite.toml");
    assert_eq!(d["line"], line);
    assert_eq!(d["hint"], "did you mean `minify`?");
    assert!(doc["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains("minfy")));

    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "seite.toml:{line}:1: warning[config-unknown-key]"
        )))
        .stdout(predicate::str::contains("did you mean `minify`?"));
}

#[test]
fn test_build_hugo_closing_tag_hint() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Hugo", "posts");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "content/posts/2024-02-01-hugo.md",
        "---\ntitle: Hugo\n---\n{{< /callout >}}\n",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "content/posts/2024-02-01-hugo.md:4:1: error[shortcode-syntax]",
        ))
        .stderr(predicate::str::contains(
            "close body shortcodes with `{{% end %}}`",
        ));
}

#[test]
fn test_check_template_parse_error_names_template_file_and_line() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Tpl", "posts");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "templates/post.html",
        "{% extends \"base.html\" %}\n{% block content %}\n{% if %}\n{% endblock %}\n",
    );
    let output = page_cmd()
        .args(["--json", "check"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let doc = json_stdout(&output);
    let diagnostics = doc["error"]["diagnostics"].as_array().unwrap();
    let d = diagnostics
        .iter()
        .find(|d| d["code"] == "template-parse")
        .unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(d["severity"], "error");
    assert_eq!(d["file"], "templates/post.html");
    assert_eq!(d["line"], 3);
    assert!(d["column"].is_u64());

    // The build keeps its fallback behaviour but points at the same file/line.
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    let d = doc["data"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "template-parse")
        .unwrap_or_else(|| panic!("{doc}"))
        .clone();
    assert_eq!(d["severity"], "warning");
    assert_eq!(d["file"], "templates/post.html");
    assert_eq!(d["line"], 3);
}

#[test]
fn test_build_render_error_names_source_and_template() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Render", "posts");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "templates/post.html",
        "{% extends \"base.html\" %}\n{% block content %}{{ page.titel }}{% endblock %}\n",
    );
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let doc = json_stdout(&output);
    let diagnostics = doc["error"]["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty(), "{doc}");
    let d = &diagnostics[0];
    assert_eq!(d["code"], "template-render");
    let file = d["file"].as_str().unwrap();
    assert!(
        file.starts_with("content/posts/") && file.ends_with(".md"),
        "{d}"
    );
    let message = d["message"].as_str().unwrap();
    assert!(message.contains("post.html"), "{message}");
    assert!(message.contains("templates/post.html"), "{message}");
    assert!(message.contains("page.titel"), "{message}");
    assert!(!message.contains(site.to_str().unwrap()), "{message}");
    assert_eq!(d["hint"], "did you mean `page.title`?");
}

#[test]
fn test_check_clean_site_exits_zero_without_touching_dist() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Clean", "posts,pages");
    let site = tmp.path().join("site");
    assert!(!site.join("dist").exists());

    page_cmd()
        .arg("check")
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("No problems found"));
    assert!(!site.join("dist").exists(), "check must not create dist/");

    // With an existing build, dist/ is left byte-for-byte alone.
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    fs::write(site.join("dist/marker.txt"), "keep me").unwrap();
    let before = dir_snapshot(&site.join("dist"));
    page_cmd()
        .arg("check")
        .current_dir(&site)
        .assert()
        .success();
    assert_eq!(before, dir_snapshot(&site.join("dist")));
    assert!(!site.join("dist-subdomains").exists());
}

#[test]
fn test_check_strict_fails_on_warnings() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Strict", "posts");
    let site = tmp.path().join("site");
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        config.replacen("[build]\n", "[build]\nminfy = true\n", 1),
    )
    .unwrap();

    page_cmd()
        .arg("check")
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("warning[config-unknown-key]"));
    page_cmd()
        .args(["check", "--strict"])
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains("warning[config-unknown-key]"))
        .stderr(predicate::str::contains("--strict"));
}

#[test]
fn test_check_json_shape() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Json Check", "posts");
    let site = tmp.path().join("site");

    // Clean site: data carries an empty list and zero counts.
    let output = page_cmd()
        .args(["--json", "check"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], true);
    assert_eq!(doc["command"], "check");
    assert_eq!(doc["data"]["diagnostics"], serde_json::json!([]));
    assert_eq!(doc["data"]["summary"]["errors"], 0);
    assert_eq!(doc["data"]["summary"]["warnings"], 0);

    // Broken link (warning) + broken frontmatter (error): error envelope.
    write_site_file(
        &site,
        "content/posts/2024-03-01-links.md",
        "---\ntitle: Links\n---\nSee [gone](/posts/not-here).\n",
    );
    write_site_file(&site, "content/posts/2024-03-02-bad.md", "no frontmatter\n");
    let output = page_cmd()
        .args(["--json", "check"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], false);
    assert_eq!(doc["error"]["summary"]["errors"], 1, "{doc}");
    let diagnostics = doc["error"]["diagnostics"].as_array().unwrap();
    let missing = diagnostics
        .iter()
        .find(|d| d["code"] == "frontmatter-missing")
        .unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(missing["file"], "content/posts/2024-03-02-bad.md");
    assert_eq!(missing["line"], 1);

    // Links are only checked once content renders: fix the error, see the link.
    fs::remove_file(site.join("content/posts/2024-03-02-bad.md")).unwrap();
    let output = page_cmd()
        .args(["--json", "check"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    let diagnostics = doc["data"]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics.iter().any(|d| d["code"] == "broken-link"
            && d["severity"] == "warning"
            && d["message"].as_str().unwrap().contains("/posts/not-here")),
        "{doc}"
    );
    // One per generated page that contains the link (post, index, ...).
    assert!(doc["data"]["summary"]["warnings"].as_u64().unwrap() >= 1);
}

#[test]
fn test_check_strict_resolves_content_root_md_links() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Content Links", "posts,pages");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "content/pages/target.md",
        "---\ntitle: Target\n---\nTarget page.\n",
    );
    write_site_file(
        &site,
        "content/posts/2024-03-01-linker.md",
        "---\ntitle: Linker\n---\nSee [t](/content/pages/target.md).\n",
    );

    // The build resolves the source link...
    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site)
        .assert()
        .success();
    // ...so check must agree.
    page_cmd()
        .args(["check", "--strict"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("No problems found"));
}

#[test]
fn test_check_strict_reports_subdomain_broken_links() {
    let tmp = TempDir::new().unwrap();
    let site = init_subdomain_site(&tmp, "site");
    write_site_file(
        &site,
        "content/docs/guide.md",
        "---\ntitle: Guide\n---\n\nIntro.\n\nSee [bad](/nonexistent-page).\n\n![bad](/missing.png)\n",
    );

    let output = page_cmd()
        .args(["--json", "check", "--strict"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let doc = json_stdout(&output);
    let diagnostics = doc["error"]["diagnostics"].as_array().unwrap();
    let broken = diagnostics
        .iter()
        .find(|d| d["code"] == "broken-link")
        .unwrap_or_else(|| panic!("no broken-link: {doc}"));
    assert_eq!(broken["file"], "content/docs/guide.md", "{doc}");
    assert_eq!(broken["line"], 7, "{doc}");
    assert!(broken["message"]
        .as_str()
        .unwrap()
        .contains("/nonexistent-page"));
    let missing = diagnostics
        .iter()
        .find(|d| d["code"] == "missing-asset")
        .unwrap_or_else(|| panic!("no missing-asset: {doc}"));
    assert_eq!(missing["file"], "content/docs/guide.md", "{doc}");
    assert_eq!(missing["line"], 9, "{doc}");
    assert!(!site.join("dist-subdomains").exists());
}
