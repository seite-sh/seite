use super::common::*;

// --- internal link checking ---

#[test]
fn test_build_link_check_warns_broken_links() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Link Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create a post with a broken internal link
    fs::write(
        site_dir.join("content/posts/2025-01-01-broken-links.md"),
        "---\ntitle: Broken Links\ndate: 2025-01-01\n---\n\n[Missing](/posts/nonexistent)\n",
    )
    .unwrap();

    // Build without --strict: should succeed but warn
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("broken internal link"));
}

#[test]
fn test_build_link_check_strict_fails_on_broken_links() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Strict Link Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create a post with a broken internal link
    fs::write(
        site_dir.join("content/posts/2025-01-01-bad-link.md"),
        "---\ntitle: Bad Link\ndate: 2025-01-01\n---\n\n[Nope](/does/not/exist)\n",
    )
    .unwrap();

    // Build with --strict: should fail
    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("broken internal link"));
}

#[test]
fn test_build_link_check_passes_with_valid_links() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Valid Links", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create a post that links to the homepage and existing content
    fs::write(
        site_dir.join("content/posts/2025-01-01-good-links.md"),
        "---\ntitle: Good Links\ndate: 2025-01-01\n---\n\n[Home](/)\n",
    )
    .unwrap();

    // Build with --strict: should succeed (no broken links)
    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[test]
fn test_build_link_check_strict_with_cross_collection_links() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Cross Links", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    // Create a doc
    fs::write(
        site_dir.join("content/docs/guide.md"),
        "---\ntitle: Guide\n---\n\nA helpful guide.\n",
    )
    .unwrap();

    // Create a post that links to the doc
    fs::write(
        site_dir.join("content/posts/2025-01-01-with-doc-link.md"),
        "---\ntitle: Post With Doc Link\ndate: 2025-01-01\n---\n\n[Guide](/docs/guide)\n",
    )
    .unwrap();

    // Build with --strict: should succeed
    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[test]
fn test_build_link_check_reports_broken_target() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Target Report", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create a post with a specific broken link
    fs::write(
        site_dir.join("content/posts/2025-01-01-specific-broken.md"),
        "---\ntitle: Specific Broken\ndate: 2025-01-01\n---\n\n[Ghost](/ghost-page)\n",
    )
    .unwrap();

    // The output should include the broken URL
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("/ghost-page"));
}

#[test]
fn test_build_link_checker_accepts_public_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Public Links", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Place a file in public/
    fs::write(site_dir.join("public/favicon.ico"), "icon").unwrap();

    // Create a post that links to the public file
    fs::write(
        site_dir.join("content/posts/2025-01-01-public-link.md"),
        "---\ntitle: Public Link\ndate: 2025-01-01\n---\n\n[icon](/favicon.ico)\n",
    )
    .unwrap();

    // Build with --strict: should succeed (favicon.ico is in dist/ from public/)
    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[test]
fn test_build_subdomain_rewrites_cross_links() {
    let tmp = TempDir::new().unwrap();
    let site_dir = init_subdomain_site(&tmp, "crosslink1");

    // Create a post that links to the docs subdomain collection
    fs::write(
        site_dir.join("content/posts/2025-01-01-hello.md"),
        "---\ntitle: Hello\ndescription: A post\n---\nCheck the [docs](/docs/setup) for info.\n",
    )
    .unwrap();

    // Create a doc in the subdomain collection
    fs::write(
        site_dir.join("content/docs/setup.md"),
        "---\ntitle: Setup\ndescription: Setup guide\n---\nSetup instructions.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The post on the main site should have the /docs/setup link rewritten to absolute
    let post_html = fs::read_to_string(site_dir.join("dist/posts/hello.html")).unwrap();
    assert!(
        post_html.contains("https://docs.example.com/setup"),
        "main site should rewrite /docs/setup to https://docs.example.com/setup, got: {}",
        &post_html[..post_html.len().min(500)]
    );
    assert!(
        !post_html.contains("href=\"/docs/setup\""),
        "main site should NOT have unrewritten /docs/setup link"
    );
}

#[test]
fn test_build_subdomain_reverse_links() {
    let tmp = TempDir::new().unwrap();
    let site_dir = init_subdomain_site(&tmp, "crosslink2");

    // Create a post on the main site
    fs::write(
        site_dir.join("content/posts/2025-01-01-hello.md"),
        "---\ntitle: Hello\ndescription: A post\n---\nHello world!\n",
    )
    .unwrap();

    // Create a doc in the subdomain that links to a post
    fs::write(
        site_dir.join("content/docs/guide.md"),
        "---\ntitle: Guide\ndescription: A guide\n---\nSee the [blog post](/posts/hello) for more.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The doc on the subdomain should have /posts/hello rewritten to absolute main-site URL
    let doc_html = fs::read_to_string(site_dir.join("dist-subdomains/docs/guide.html")).unwrap();
    assert!(
        doc_html.contains("https://example.com/posts/hello"),
        "subdomain site should rewrite /posts/hello to https://example.com/posts/hello, got: {}",
        &doc_html[..doc_html.len().min(500)]
    );
}

// --- link/asset diagnostics, .md link rewriting, staged output ---

#[test]
fn test_build_broken_link_reports_source_path_and_line() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Link Source", "posts,docs,pages");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/2025-01-01-agents.md"),
        "---\ntitle: Agents\ndate: 2025-01-01\n---\n\nIntro.\n\nSee [missing](/docs/does-not-exist).\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("/docs/does-not-exist"))
        .stdout(predicate::str::contains(
            "content/posts/2025-01-01-agents.md:8",
        ))
        .stdout(predicate::str::contains("posts/agents.html").not());
}

#[test]
fn test_build_missing_image_reported_and_strict_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Missing Asset", "posts,pages");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/2025-01-01-pic.md"),
        "---\ntitle: Pic\ndate: 2025-01-01\n---\n\n![img](/static/nope.png)\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("missing asset"))
        .stdout(predicate::str::contains("/static/nope.png"))
        .stdout(predicate::str::contains(
            "content/posts/2025-01-01-pic.md:6",
        ));

    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains("1 missing asset"));

    // Once the image exists the strict build passes.
    fs::create_dir_all(site.join("static")).unwrap();
    fs::write(site.join("static/nope.png"), b"png").unwrap();
    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site)
        .assert()
        .success();
}

#[test]
fn test_build_rewrites_relative_md_links() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Md Links", "posts,docs,pages");
    let site = tmp.path().join("site");
    fs::create_dir_all(site.join("content/docs/guides")).unwrap();
    fs::write(
        site.join("content/docs/getting-started.md"),
        "---\ntitle: Getting Started\n---\n\n## Install\n\nNext: [deep dive](guides/deep.md).\n",
    )
    .unwrap();
    fs::write(
        site.join("content/docs/guides/deep.md"),
        "---\ntitle: Deep\n---\n\nBack to [install](../getting-started.md#install).\n",
    )
    .unwrap();
    fs::write(
        site.join("content/posts/2025-01-01-intro.md"),
        "---\ntitle: Intro\ndate: 2025-01-01\n---\n\nRead [the docs](../docs/getting-started.md) or the [raw copy](/docs/getting-started.md).\n",
    )
    .unwrap();

    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site)
        .assert()
        .success();

    let post = fs::read_to_string(site.join("dist/posts/intro.html")).unwrap();
    assert!(post.contains(r#"href="/docs/getting-started""#), "{post}");
    assert!(
        post.contains(r#"href="/docs/getting-started.md""#),
        "root-relative .md links to published copies stay as written"
    );
    let getting_started = fs::read_to_string(site.join("dist/docs/getting-started.html")).unwrap();
    assert!(getting_started.contains(r#"href="/docs/guides/deep""#));
    let deep = fs::read_to_string(site.join("dist/docs/guides/deep.html")).unwrap();
    assert!(deep.contains(r#"href="/docs/getting-started#install""#));
}

#[test]
fn test_build_rewrites_md_links_with_base_path() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Md Base", "posts,docs,pages");
    let site = tmp.path().join("site");
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    let config = config
        .lines()
        .map(|l| {
            if l.starts_with("base_url") {
                "base_url = \"https://example.com/repo\"".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(site.join("seite.toml"), config).unwrap();
    fs::write(
        site.join("content/docs/setup.md"),
        "---\ntitle: Setup\n---\n\nSetup.\n",
    )
    .unwrap();
    fs::write(
        site.join("content/posts/2025-01-01-a.md"),
        "---\ntitle: A\ndate: 2025-01-01\n---\n\n[setup](../docs/setup.md#go)\n",
    )
    .unwrap();

    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site)
        .assert()
        .success();
    let post = fs::read_to_string(site.join("dist/posts/a.html")).unwrap();
    assert!(post.contains(r#"href="/repo/docs/setup#go""#), "{post}");
}

#[test]
fn test_build_unresolved_md_link_reported_with_line() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Md Broken", "posts,pages");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/2025-01-01-a.md"),
        "---\ntitle: A\ndate: 2025-01-01\n---\n\nText.\n\n[other](./no-such-post.md)\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("./no-such-post.md"))
        .stdout(predicate::str::contains("content/posts/2025-01-01-a.md:8"));

    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site)
        .assert()
        .failure();
}

#[test]
fn test_build_failure_keeps_previous_output() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Keep Dist", "posts,pages");
    let site = tmp.path().join("site");

    page_cmd()
        .args(["build"])
        .current_dir(&site)
        .assert()
        .success();
    let index_before = fs::read_to_string(site.join("dist/index.html")).unwrap();

    fs::write(
        site.join("content/posts/2025-02-02-bad.md"),
        "---\ntitle: [unclosed\n---\nbody\n",
    )
    .unwrap();
    page_cmd()
        .args(["build"])
        .current_dir(&site)
        .assert()
        .failure();

    assert_eq!(
        fs::read_to_string(site.join("dist/index.html")).unwrap(),
        index_before,
        "a failed build must not touch the previous output"
    );
    let leftovers: Vec<String> = fs::read_dir(&site)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("dist."))
        .collect();
    assert!(
        leftovers.is_empty(),
        "staging dir left behind: {leftovers:?}"
    );
}

#[test]
fn test_build_cleans_leftover_staging_dir() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Leftover", "posts,pages");
    let site = tmp.path().join("site");
    fs::create_dir_all(site.join("dist.staging-4242424/posts")).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site)
        .assert()
        .success();

    assert!(!site.join("dist.staging-4242424").exists());
    assert!(site.join("dist/index.html").exists());
}

#[test]
fn test_build_json_broken_links_include_source_and_line() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Json Links", "posts,pages");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/2025-01-01-a.md"),
        "---\ntitle: A\ndate: 2025-01-01\n---\n\n[ghost](/ghost-page)\n\n![x](/static/ghost.png)\n",
    )
    .unwrap();

    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    let broken = doc["data"]["broken_links"].as_array().unwrap();
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0]["target"], "/ghost-page");
    assert_eq!(broken[0]["kind"], "page");
    assert_eq!(broken[0]["source"], "content/posts/2025-01-01-a.md");
    assert_eq!(broken[0]["line"], 6);
    assert_eq!(
        broken[0]["locations"][0]["source"],
        "content/posts/2025-01-01-a.md"
    );
    assert_eq!(broken[0]["locations"][0]["page"], "posts/a.html");

    let missing = doc["data"]["missing_assets"].as_array().unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0]["target"], "/static/ghost.png");
    assert_eq!(missing[0]["line"], 8);
}
