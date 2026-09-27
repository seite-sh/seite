use super::common::*;

// --- Trust Center ---

#[test]
fn test_init_with_trust_collection() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "trustsite",
            "--title",
            "Acme Corp",
            "--description",
            "Security first",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages,trust",
            "--trust-company",
            "Acme Corp",
            "--trust-frameworks",
            "soc2,iso27001",
            "--trust-sections",
            "overview,certifications,subprocessors,faq,disclosure",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let root = tmp.path().join("trustsite");

    // Verify directory structure
    assert!(root.join("content/trust").is_dir());
    assert!(root.join("content/trust/certifications").is_dir());
    assert!(root.join("data/trust").is_dir());

    // Verify data files
    assert!(root.join("data/trust/certifications.yaml").exists());
    assert!(root.join("data/trust/subprocessors.yaml").exists());
    assert!(root.join("data/trust/faq.yaml").exists());

    // Verify content files
    assert!(root.join("content/trust/security-overview.md").exists());
    assert!(root
        .join("content/trust/vulnerability-disclosure.md")
        .exists());
    assert!(root.join("content/trust/certifications/soc2.md").exists());
    assert!(root
        .join("content/trust/certifications/iso27001.md")
        .exists());

    // Verify templates
    assert!(root.join("templates/trust-item.html").exists());
    assert!(root.join("templates/trust-index.html").exists());

    // Verify seite.toml has trust config
    let config = fs::read_to_string(root.join("seite.toml")).unwrap();
    assert!(config.contains("[trust]"));
    assert!(config.contains("company = \"Acme Corp\""));
    assert!(config.contains("soc2"));
    assert!(config.contains("iso27001"));

    // Verify AGENTS.md has brief trust section
    let agents_md = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("## Trust Center"));
    assert!(agents_md.contains("Acme Corp"));

    // Detailed trust content now lives in .claude/rules/trust-center.md
    let trust_rules = fs::read_to_string(root.join(".claude/rules/trust-center.md")).unwrap();
    assert!(trust_rules.starts_with("---\npaths:\n"));
    assert!(trust_rules.contains("Managing Certifications"));
    assert!(trust_rules.contains("Managing Subprocessors"));
    assert!(trust_rules.contains("Managing FAQs"));
    assert!(trust_rules.contains("seite://trust"));
}

#[test]
fn test_build_trust_center() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "site",
            "--title",
            "Trust Test",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "pages,trust",
            "--trust-company",
            "TestCo",
            "--trust-frameworks",
            "soc2",
            "--trust-sections",
            "overview,certifications,subprocessors,faq",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let site_dir = tmp.path().join("site");
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Trust center index page should exist
    assert!(site_dir.join("dist/trust/index.html").exists());

    // Individual trust pages should exist
    assert!(site_dir.join("dist/trust/security-overview.html").exists());
    assert!(site_dir
        .join("dist/trust/certifications/soc2.html")
        .exists());

    // Markdown versions should exist
    assert!(site_dir.join("dist/trust/security-overview.md").exists());

    // Trust center index should contain certification data
    let index = fs::read_to_string(site_dir.join("dist/trust/index.html")).unwrap();
    assert!(index.contains("Trust Center"));
    assert!(index.contains("SOC 2 Type II"));
}

#[test]
fn test_trust_center_no_sections() {
    // Test with minimal trust config — no optional sections
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "site",
            "--title",
            "Minimal Trust",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "trust",
            "--trust-company",
            "MinCo",
            "--trust-frameworks",
            "soc2",
            "--trust-sections",
            "certifications",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let site_dir = tmp.path().join("site");

    // Certifications data and content should exist
    assert!(site_dir.join("data/trust/certifications.yaml").exists());
    assert!(site_dir
        .join("content/trust/certifications/soc2.md")
        .exists());

    // Optional sections should NOT exist
    assert!(!site_dir.join("content/trust/security-overview.md").exists());
    assert!(!site_dir.join("data/trust/subprocessors.yaml").exists());
    assert!(!site_dir.join("data/trust/faq.yaml").exists());

    // Should still build successfully
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[test]
fn test_trust_preset_config() {
    let preset = seite::config::CollectionConfig::preset_trust();
    assert_eq!(preset.name, "trust");
    assert_eq!(preset.label, "Trust Center");
    assert_eq!(preset.url_prefix, "/trust");
    assert!(preset.nested);
    assert!(!preset.has_date);
    assert!(!preset.has_rss);
    assert!(preset.listed);
    assert_eq!(preset.default_template, "trust-item.html");
}

#[test]
fn test_trust_from_preset() {
    assert!(seite::config::CollectionConfig::from_preset("trust").is_some());
}

#[test]
fn test_trust_embedded_doc_exists() {
    let doc = seite::docs::by_slug("trust-center");
    assert!(doc.is_some());
    let doc = doc.unwrap();
    assert_eq!(doc.title, "Trust Center");
    assert!(doc.raw_content.contains("certifications"));
}

#[test]
fn test_init_without_trust_has_no_trust_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Trust", "posts,pages");

    let config = fs::read_to_string(tmp.path().join("site/seite.toml")).unwrap();
    assert!(!config.contains("[trust]"));

    let agents_md = fs::read_to_string(tmp.path().join("site/AGENTS.md")).unwrap();
    assert!(!agents_md.contains("## Trust Center"));
}

#[test]
fn test_password_access_builds_path_worker_and_private_assets() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Private Access", "posts,pages");
    let site_dir = tmp.path().join("site");
    add_collection_line(&site_dir, "posts", "private = true");
    add_collection_line(&site_dir, "posts", "access_group = \"members\"");
    enable_password_access(&site_dir);

    let private_assets = site_dir.join("static/private/members");
    fs::create_dir_all(&private_assets).unwrap();
    fs::write(private_assets.join("notice.txt"), "private asset").unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let dist = site_dir.join("dist");
    let worker = fs::read_to_string(dist.join("_worker.js")).unwrap();
    assert!(worker.contains(r#""prefix":"/posts""#));
    assert!(worker.contains("SEITE_PASSWORD_MEMBERS"));
    assert!(dist.join("private-assets/members/notice.txt").exists());
    assert!(!dist.join("static/private/members/notice.txt").exists());
}

#[test]
fn test_password_access_protects_translated_collection_routes() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Translated Access", "docs,pages");
    let site_dir = tmp.path().join("site");
    add_collection_line(&site_dir, "docs", "private = true");
    add_collection_line(&site_dir, "docs", "access_group = \"staff\"");
    enable_password_access(&site_dir);
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[languages.de]\ntitle = \"Intern\"\n");
    fs::write(config_path, config).unwrap();

    fs::write(
        site_dir.join("content/docs/secret.de.md"),
        "---\ntitle: Geheim\n---\nVertraulich.",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let worker = fs::read_to_string(site_dir.join("dist/_worker.js")).unwrap();
    assert!(worker.contains(r#""prefix":"/de/docs""#));
    assert!(site_dir.join("dist/de/docs/secret.html").exists());
}

#[test]
fn test_password_access_does_not_publish_private_image_variants() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Private Images", "posts,pages");
    let site_dir = tmp.path().join("site");
    add_collection_line(&site_dir, "posts", "private = true");
    add_collection_line(&site_dir, "posts", "access_group = \"members\"");
    enable_password_access(&site_dir);

    let private_dir = site_dir.join("static/private/members");
    fs::create_dir_all(&private_dir).unwrap();
    let image = image::RgbaImage::from_pixel(100, 80, image::Rgba([12, 34, 56, 255]));
    image.save(private_dir.join("confidential.png")).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let dist = site_dir.join("dist");
    assert!(dist
        .join("private-assets/members/confidential.png")
        .exists());
    assert!(!dist
        .join("static/private/members/confidential.webp")
        .exists());
    assert!(!dist
        .join("static/private/members/confidential-48w.webp")
        .exists());
}

#[test]
fn test_private_collection_without_access_keeps_existing_build_behavior() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Private Metadata", "posts,pages");
    let site_dir = tmp.path().join("site");
    add_collection_line(&site_dir, "posts", "private = true");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(!site_dir.join("dist/_worker.js").exists());
}

#[test]
fn test_access_groups_lists_separate_path_password_groups() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Access Groups", "posts,docs,pages");
    let site_dir = tmp.path().join("site");
    add_collection_line(&site_dir, "posts", "private = true");
    add_collection_line(&site_dir, "posts", "access_group = \"members\"");
    add_collection_line(&site_dir, "docs", "private = true");
    add_collection_line(&site_dir, "docs", "access_group = \"staff\"");
    enable_password_access(&site_dir);

    page_cmd()
        .args(["access", "groups"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("members")
                .and(predicate::str::contains("path /posts"))
                .and(predicate::str::contains("staff"))
                .and(predicate::str::contains("path /docs")),
        );
}

#[cfg(unix)]
#[test]
fn test_access_set_password_without_tty_refuses_and_uploads_nothing() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Set Password", "posts,pages");
    let site = tmp.path().join("site");
    add_collection_line(&site, "posts", "private = true");
    add_collection_line(&site, "posts", "access_group = \"members\"");
    enable_password_access(&site);
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        config.replace(
            "target = \"github-pages\"",
            "target = \"cloudflare\"\nproject = \"my-site\"",
        ),
    )
    .unwrap();

    // A wrangler that records any call: nothing may be uploaded.
    let bin = TempDir::new().unwrap();
    let called = bin.path().join("called");
    let wrangler = bin.path().join("wrangler");
    fs::write(
        &wrangler,
        format!("#!/bin/sh\necho \"$@\" >> {}\n", called.display()),
    )
    .unwrap();
    fs::set_permissions(&wrangler, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    // --yes can't stand in for a password either.
    for args in [
        vec!["access", "set-password", "members"],
        vec!["--yes", "access", "set-password", "members"],
    ] {
        page_cmd()
            .args(&args)
            .current_dir(&site)
            .env("PATH", &path)
            .write_stdin("hunter2\nhunter2\n")
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "cannot prompt for a password when not running interactively",
            ))
            .stderr(predicate::str::contains(
                "passwords are never read from flags",
            ));
    }
    assert!(!called.exists(), "wrangler must not be called");
}

#[test]
fn test_password_access_protects_an_entire_subdomain_output() {
    let tmp = TempDir::new().unwrap();
    let site_dir = init_subdomain_site(&tmp, "site");
    add_collection_line(&site_dir, "docs", "private = true");
    add_collection_line(&site_dir, "docs", "access_group = \"docs-team\"");
    enable_password_access(&site_dir);

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let main_worker = fs::read_to_string(site_dir.join("dist/_worker.js")).unwrap();
    assert!(main_worker.contains(r#"LEGACY_PRIVATE_PREFIX = "/static/private""#));
    let worker = fs::read_to_string(site_dir.join("dist-subdomains/docs/_worker.js")).unwrap();
    assert!(worker.contains(r#""prefix":"/""#));
    assert!(worker.contains("SEITE_PASSWORD_DOCS_TEAM"));
}

#[test]
fn test_password_access_rejects_custom_worker_conflict() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Worker Conflict", "posts,pages");
    let site_dir = tmp.path().join("site");
    add_collection_line(&site_dir, "posts", "private = true");
    enable_password_access(&site_dir);
    fs::write(site_dir.join("public/_worker.js"), "export default {};").unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "password access cannot generate _worker.js",
        ));
}

#[test]
fn test_private_collection_hub_renders_but_excluded_from_discovery() {
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "site");
    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "de", "Vertrauen");
    add_collection_line(&site_dir, "trust", "private = true");
    write_plain_trust_sections(&site_dir);

    // German translation of the scaffolded trust item so /de/trust/ has content too.
    let overview = fs::read_to_string(site_dir.join("content/trust/security-overview.md")).unwrap();
    fs::write(
        site_dir.join("content/trust/security-overview.de.md"),
        overview,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();
    let dist = site_dir.join("dist");

    // 1. Hub renders its data-driven sections AND carries noindex — both languages.
    for hub in ["trust/index.html", "de/trust/index.html"] {
        let html = fs::read_to_string(dist.join(hub)).unwrap();
        assert!(
            html.contains("AWS"),
            "{hub}: hub missing subprocessor section"
        );
        assert!(
            html.contains("Where is data stored?"),
            "{hub}: hub missing FAQ section"
        );
        assert!(
            html.contains(r#"<meta name="robots" content="noindex, nofollow">"#),
            "{hub}: missing noindex robots meta"
        );
    }

    // 2. Item pages + .md alternates still build, with noindex.
    for p in [
        "trust/security-overview.html",
        "de/trust/security-overview.html",
    ] {
        let html = fs::read_to_string(dist.join(p)).unwrap();
        assert!(
            html.contains(r#"<meta name="robots" content="noindex, nofollow">"#),
            "{p}: missing noindex robots meta"
        );
    }
    assert!(dist.join("trust/security-overview.md").exists());
    assert!(dist.join("de/trust/security-overview.md").exists());

    // 3. ZERO trust URLs/content in any public discovery surface (default + /de/).
    for f in [
        "llms.txt",
        "llms-full.txt",
        "sitemap.xml",
        "search-index.json",
        "index.html",
        "de/llms.txt",
        "de/llms-full.txt",
        "de/search-index.json",
        "de/index.html",
    ] {
        let content = fs::read_to_string(dist.join(f)).unwrap();
        assert!(!content.contains("/trust"), "{f} leaks a /trust URL");
        assert!(
            !content.contains("Security Overview"),
            "{f} leaks the private page title"
        );
    }
}

#[test]
fn test_private_collection_hub_renders_even_when_unlisted() {
    // private is independent of listed: hub builds even when hidden from the
    // homepage. (listed = false alone would suppress the hub entirely.)
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "site");
    let site_dir = tmp.path().join("site");

    let toml_path = site_dir.join("seite.toml");
    let cfg = fs::read_to_string(&toml_path).unwrap();
    let cfg = cfg.replace(
        "listed = true\nurl_prefix = \"/trust\"",
        "listed = false\nurl_prefix = \"/trust\"",
    );
    fs::write(&toml_path, cfg).unwrap();
    add_collection_line(&site_dir, "trust", "private = true");
    write_plain_trust_sections(&site_dir);

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();
    let dist = site_dir.join("dist");

    // Hub still renders despite listed = false.
    let hub = fs::read_to_string(dist.join("trust/index.html")).unwrap();
    assert!(
        hub.contains("AWS"),
        "hub did not render with listed=false + private=true"
    );
    // Still excluded from discovery.
    assert!(!fs::read_to_string(dist.join("sitemap.xml"))
        .unwrap()
        .contains("/trust"));
    assert!(!fs::read_to_string(dist.join("index.html"))
        .unwrap()
        .contains("/trust"));
}

#[test]
fn test_collection_without_private_appears_in_discovery() {
    // Backward compatibility: absent `private` keeps the collection discoverable.
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "site");
    let site_dir = tmp.path().join("site");
    write_plain_trust_sections(&site_dir);

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();
    let dist = site_dir.join("dist");

    assert!(
        fs::read_to_string(dist.join("sitemap.xml"))
            .unwrap()
            .contains("/trust"),
        "non-private collection should appear in the sitemap"
    );
    assert!(fs::read_to_string(dist.join("llms.txt"))
        .unwrap()
        .contains("/trust"));
}

#[test]
fn test_private_collection_logs_excluded_count() {
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "site");
    let site_dir = tmp.path().join("site");
    add_collection_line(&site_dir, "trust", "private = true");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("private").and(predicate::str::contains("excluded")));
}

#[test]
fn test_private_paginated_collection_hub_renders_but_excluded() {
    // `private` composes with `paginate`: the paginated hub pages render (and
    // carry noindex) while the collection stays out of every discovery surface.
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Paginated Private", "posts,pages");
    let site_dir = tmp.path().join("site");
    add_pagination(&site_dir, "posts", 1);
    add_collection_line(&site_dir, "posts", "private = true");

    // init scaffolds one post; add a second so paginate = 1 yields two pages.
    fs::write(
        site_dir.join("content/posts/2025-02-01-second.md"),
        "---\ntitle: Second Secret Post\ndate: 2025-02-01\ndescription: Gated.\n---\n\nConfidential body.\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();
    let dist = site_dir.join("dist");

    // Both paginated hub pages render, each with noindex.
    for hub in ["posts/index.html", "posts/page/2/index.html"] {
        let html = fs::read_to_string(dist.join(hub)).unwrap();
        assert!(
            html.contains(r#"<meta name="robots" content="noindex, nofollow">"#),
            "{hub}: missing noindex robots meta"
        );
    }

    // The item page still builds, also with noindex.
    let item = fs::read_to_string(dist.join("posts/second.html")).unwrap();
    assert!(item.contains(r#"<meta name="robots" content="noindex, nofollow">"#));

    // Excluded from every public discovery surface.
    for f in [
        "llms.txt",
        "llms-full.txt",
        "sitemap.xml",
        "search-index.json",
        "index.html",
        "feed.xml",
    ] {
        let content = fs::read_to_string(dist.join(f)).unwrap();
        assert!(
            !content.contains("/posts/second"),
            "{f} leaks a private post URL"
        );
        assert!(
            !content.contains("Second Secret Post"),
            "{f} leaks the private post title"
        );
    }
}

#[test]
fn test_subdomain_collection_hub_uses_collection_index_template() {
    // A subdomain collection's root must render with the collection's own index
    // template + context (data-driven hub), not the generic site index template.
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "site");
    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "de", "Vertrauen");
    write_plain_trust_sections(&site_dir);
    add_collection_line(&site_dir, "trust", "subdomain = \"trust\"");
    add_collection_line(
        &site_dir,
        "trust",
        "subdomain_base_url = \"https://trust.example.com\"",
    );

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // The subdomain root (default + per-language) renders the trust hub: the
    // subprocessor table and FAQ come only from trust-index.html, never the
    // generic index template.
    for hub in [
        "dist-subdomains/trust/index.html",
        "dist-subdomains/trust/de/index.html",
    ] {
        let html = fs::read_to_string(site_dir.join(hub)).unwrap();
        assert!(
            html.contains("AWS"),
            "{hub}: hub missing subprocessor table"
        );
        assert!(
            html.contains("Where is data stored?"),
            "{hub}: hub missing FAQ section"
        );
        assert!(
            html.contains("Trust Center"),
            "{hub}: not rendered with trust-index.html (generic index template used)"
        );
    }
}
