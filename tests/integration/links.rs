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

#[test]
fn test_build_strict_reports_subdomain_broken_links() {
    let tmp = TempDir::new().unwrap();
    let site = init_subdomain_site(&tmp, "site");
    write_site_file(
        &site,
        "content/docs/guide.md",
        "---\ntitle: Guide\n---\n\nIntro.\n\nSee [bad](/nonexistent-page).\n\n![bad](/missing.png)\n",
    );

    // Non-strict: warnings in the report and the JSON data.
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    let broken = doc["data"]["broken_links"].as_array().unwrap();
    assert!(
        broken.iter().any(|b| b["target"] == "/nonexistent-page"
            && b["source"] == "content/docs/guide.md"
            && b["line"] == 7),
        "{doc}"
    );
    let missing = doc["data"]["missing_assets"].as_array().unwrap();
    assert!(
        missing.iter().any(|b| b["target"] == "/missing.png"),
        "{doc}"
    );

    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "1 broken internal link, 1 missing asset",
        ))
        .stdout(predicate::str::contains("/nonexistent-page"))
        .stdout(predicate::str::contains("content/docs/guide.md:7"))
        .stdout(predicate::str::contains("/missing.png"));
}

#[test]
fn test_build_md_links_across_subdomains() {
    let tmp = TempDir::new().unwrap();
    let site = init_subdomain_site(&tmp, "site");
    write_site_file(
        &site,
        "content/docs/intro.md",
        "---\ntitle: Intro\n---\nBack to [the post](../posts/2025-01-01-hello.md#top).\n",
    );
    write_site_file(
        &site,
        "content/posts/2025-01-01-hello.md",
        "---\ntitle: Hello\n---\nRead the [intro](../docs/intro.md).\n",
    );

    page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site)
        .assert()
        .success();

    let post = fs::read_to_string(site.join("dist/posts/hello.html")).unwrap();
    assert!(
        post.contains(r#"href="https://docs.example.com/intro""#),
        "main -> subdomain: {post}"
    );
    let doc = fs::read_to_string(site.join("dist-subdomains/docs/intro.html")).unwrap();
    assert!(
        doc.contains(r#"href="https://example.com/posts/hello#top""#),
        "subdomain -> main: {doc}"
    );
}

#[test]
fn test_build_strict_json_includes_link_diagnostics() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Strict Json", "posts");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "content/posts/2025-01-01-broken.md",
        "---\ntitle: Broken\n---\n\nSee [gone](/posts/not-here).\n",
    );

    let output = page_cmd()
        .args(["--json", "build", "--strict"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], false);
    let diagnostics = doc["error"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("no diagnostics: {doc}"));
    let first = &diagnostics[0];
    assert_eq!(first["code"], "broken-link", "{doc}");
    assert_eq!(first["severity"], "error", "{doc}");
    assert_eq!(first["file"], "content/posts/2025-01-01-broken.md", "{doc}");
    assert_eq!(first["line"], 5, "{doc}");
    assert!(first["message"]
        .as_str()
        .unwrap()
        .contains("/posts/not-here"));

    // Human output: the grouped report plus one summary, not the same
    // problem repeated as diagnostic lines.
    let output = page_cmd()
        .args(["build", "--strict"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(all.matches("/posts/not-here").count(), 1, "{all}");
    assert!(
        all.contains("content/posts/2025-01-01-broken.md:5"),
        "{all}"
    );
    assert!(
        all.contains("Error: Build failed: 1 broken internal link"),
        "{all}"
    );
}

// --- link extraction from raw HTML ---

#[test]
fn test_build_checks_links_in_raw_html_of_any_quoting_and_case() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Raw Html", "posts");
    let site = tmp.path().join("site");
    write_site_file(&site, "static/ok.png", "png");
    write_site_file(
        &site,
        "content/posts/2025-01-01-raw.md",
        "---\ntitle: Raw\n---\n\n\
         <IMG SRC=/static/ok.png ALT=fine>\n\
         <IMG SRC=/static/unquoted.png ALT=x>\n\
         <a title=\"a > b\" href='/ghost-single'>x</a>\n\
         <img srcset=\"/static/one.png 1x, /static/two.png 2x\" alt=\"\">\n\
         <LINK REL=\"preload\" AS=\"image\" HREF=\"/static/preloaded.png\">\n\
         <video poster=\"/static/poster.png\"></video>\n\
         <svg><use href=\"/static/sprite.svg\"></use></svg>\n\
         <!-- <a href=\"/commented-out\">x</a> -->\n",
    );
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    let targets = |key: &str| -> Vec<String> {
        let mut t: Vec<String> = doc["data"][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["target"].as_str().unwrap().to_string())
            .collect();
        t.sort();
        t
    };
    assert_eq!(targets("broken_links"), vec!["/ghost-single"], "{doc}");
    assert_eq!(
        targets("missing_assets"),
        vec![
            "/static/one.png",
            "/static/poster.png",
            "/static/preloaded.png",
            "/static/sprite.svg",
            "/static/two.png",
            "/static/unquoted.png",
        ],
        "{doc}"
    );
    // Located in the markdown source, on the line the reference is written.
    let unquoted = doc["data"]["missing_assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["target"] == "/static/unquoted.png")
        .unwrap();
    assert_eq!(unquoted["source"], "content/posts/2025-01-01-raw.md");
    assert_eq!(unquoted["line"], 6);
}

#[test]
fn test_build_reference_link_at_end_of_file_is_located() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Ref Link", "posts");
    let site = tmp.path().join("site");
    // No trailing newline: the href is the very last thing in the file.
    write_site_file(
        &site,
        "content/posts/2025-01-01-ref.md",
        "---\ntitle: Ref\n---\n\nSee [the guide][g].\n\n[g]: /ghost-ref",
    );
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    let doc = json_stdout(&output);
    let broken = doc["data"]["broken_links"].as_array().unwrap();
    assert_eq!(broken.len(), 1, "{doc}");
    assert_eq!(broken[0]["target"], "/ghost-ref");
    assert_eq!(broken[0]["source"], "content/posts/2025-01-01-ref.md");
    assert_eq!(broken[0]["line"], 7);
}

#[test]
fn test_build_strict_names_template_for_template_links() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Template Link", "posts");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "content/posts/2025-01-01-a.md",
        "---\ntitle: A\n---\nbody\n",
    );
    write_site_file(
        &site,
        "templates/post.html",
        "{% extends \"base.html\" %}\n{% block content %}\n<a href=\"/from-template\">x</a>\n{{ page.content | safe }}\n{% endblock %}\n",
    );
    let output = page_cmd()
        .args(["--json", "build", "--strict"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let doc = json_stdout(&output);
    let d = doc["error"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "broken-link")
        .unwrap_or_else(|| panic!("{doc}"))
        .clone();
    assert_eq!(d["file"], "templates/post.html", "{d}");
    assert_eq!(d["line"], 3, "{d}");
    let message = d["message"].as_str().unwrap();
    assert!(message.contains("`/from-template`"), "{message}");
    assert!(
        message.contains("(from template/listing, on page posts/"),
        "{message}"
    );
}

// --- output directory swapping ---

#[cfg(unix)]
#[test]
fn test_build_into_symlinked_output_dir_replaces_contents_in_place() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Symlinked Dist", "posts,pages");
    let site = tmp.path().join("site");
    let real = tmp.path().join("real-dist");
    fs::create_dir_all(real.join("old-dir")).unwrap();
    fs::write(real.join("stale.html"), "stale").unwrap();
    fs::write(real.join("old-dir/x.txt"), "x").unwrap();
    std::os::unix::fs::symlink(&real, site.join("dist")).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();

    let meta = fs::symlink_metadata(site.join("dist")).unwrap();
    assert!(meta.file_type().is_symlink(), "dist must stay a symlink");
    assert!(
        real.join("index.html").is_file(),
        "output written through link"
    );
    assert!(!real.join("stale.html").exists(), "stale files removed");
    assert!(!real.join("old-dir").exists(), "stale dirs removed");
    let index = fs::read_to_string(real.join("index.html")).unwrap();

    // A failed build leaves the linked directory's contents alone.
    write_site_file(
        &site,
        "content/posts/2025-02-02-bad.md",
        "---\ntitle: [unclosed\n---\nbody\n",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .failure();
    assert_eq!(fs::read_to_string(real.join("index.html")).unwrap(), index);
    assert!(fs::symlink_metadata(site.join("dist"))
        .unwrap()
        .file_type()
        .is_symlink());
    let leftovers: Vec<String> = fs::read_dir(&site)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("dist."))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[cfg(unix)]
#[test]
fn test_build_leaves_scratch_dirs_of_running_builds_alone() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Concurrent", "posts,pages");
    let site = tmp.path().join("site");
    // This test process stands in for another seite build that is still running.
    let live = format!("dist.staging-{}", std::process::id());
    fs::create_dir_all(site.join(&live).join("posts")).unwrap();
    fs::create_dir_all(site.join("dist.old-4242424")).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();

    assert!(
        site.join(&live).join("posts").is_dir(),
        "a running build's staging dir must not be deleted"
    );
    assert!(
        !site.join("dist.old-4242424").exists(),
        "a dead build's leftover is cleaned up"
    );
}

#[test]
fn test_build_prunes_output_of_collection_no_longer_on_a_subdomain() {
    let tmp = TempDir::new().unwrap();
    let site = init_subdomain_site(&tmp, "site");
    write_site_file(
        &site,
        "content/docs/guide.md",
        "---\ntitle: Guide\n---\nhi\n",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    assert!(site.join("dist-subdomains/docs/guide.html").is_file());

    // Move docs back onto the main site.
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        config.replace("subdomain = \"docs\"\n", ""),
    )
    .unwrap();
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    assert!(
        !site.join("dist-subdomains/docs").exists(),
        "stale subdomain output must be pruned"
    );
    assert!(site.join("dist/docs/guide.html").is_file());
}

#[test]
fn test_build_subdomain_template_warning_reported_once() {
    let tmp = TempDir::new().unwrap();
    let site = init_subdomain_site(&tmp, "site");
    write_site_file(&site, "content/docs/g.md", "---\ntitle: G\n---\nhi\n");
    write_site_file(&site, "templates/base.html", "<html>{% if %}</html>");
    let output = page_cmd()
        .args(["--json", "build"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    let parse: Vec<&serde_json::Value> = doc["data"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "template-parse")
        .collect();
    // Main site and subdomain both hit it; it is one problem in one file.
    assert_eq!(parse.len(), 1, "{doc}");
    assert_eq!(parse[0]["file"], "templates/base.html");
    assert_eq!(
        doc["data"]["warnings"].as_array().unwrap().len(),
        1,
        "{doc}"
    );
}
