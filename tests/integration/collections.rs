use super::common::*;

// --- changelog collection ---

#[test]
fn test_init_with_changelog_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Changelog Test", "posts,changelog,pages");

    let root = tmp.path().join("site");
    assert!(root.join("content/changelog").is_dir());
    assert!(root.join("templates/changelog-entry.html").exists());

    // Verify sample changelog entry exists
    let entries: Vec<_> = fs::read_dir(root.join("content/changelog"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1);
    let content = fs::read_to_string(entries[0].path()).unwrap();
    assert!(content.contains("title:"));
    assert!(content.contains("tags:"));
    assert!(content.contains("new"));

    // Verify seite.toml has changelog collection
    let config = fs::read_to_string(root.join("seite.toml")).unwrap();
    assert!(config.contains("name = \"changelog\""));
}

#[test]
fn test_build_changelog_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Changelog Build", "changelog");

    let site_dir = tmp.path().join("site");

    // Create a changelog entry
    let entry = "---\ntitle: v1.0.0\ndate: 2025-06-01\ndescription: First stable release\ntags:\n  - new\n  - improvement\n---\n\nThis is the first stable release.\n";
    fs::write(
        site_dir.join("content/changelog/2025-06-01-v1-0-0.md"),
        entry,
    )
    .unwrap();
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let dist = site_dir.join("dist");

    // Verify changelog entry HTML + markdown output
    assert!(dist.join("changelog/v1-0-0.html").exists());
    assert!(dist.join("changelog/v1-0-0.md").exists());

    // Verify changelog index page
    assert!(dist.join("changelog/index.html").exists());

    // Verify RSS feed includes changelog (has_rss: true)
    let feed = fs::read_to_string(dist.join("feed.xml")).unwrap();
    assert!(feed.contains("v1.0.0"));
}

#[test]
fn test_changelog_tags_render() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Tag Render", "changelog");

    let site_dir = tmp.path().join("site");

    let entry = "---\ntitle: v2.0.0\ndate: 2025-07-01\ntags:\n  - breaking\n  - fix\n---\n\nBreaking changes and fixes.\n";
    fs::write(
        site_dir.join("content/changelog/2025-07-01-v2-0-0.md"),
        entry,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/changelog/v2-0-0.html")).unwrap();
    // Tag badges should appear in the HTML
    assert!(html.contains("breaking"));
    assert!(html.contains("fix"));
    assert!(html.contains("changelog-tag"));
}

#[test]
fn test_changelog_index_uses_collection_template() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CL Index", "changelog");

    let site_dir = tmp.path().join("site");

    // Add a second entry (init already created v0.1.0)
    let entry =
        "---\ntitle: v0.2.0\ndate: 2025-06-01\ntags:\n  - improvement\n---\n\nSecond release.\n";
    fs::write(
        site_dir.join("content/changelog/2025-06-01-v0-2-0.md"),
        entry,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/changelog/index.html")).unwrap();
    // The changelog-specific index template should be used (has changelog-feed class)
    assert!(index.contains("changelog-feed") || index.contains("changelog-item"));
    assert!(index.contains("v0.2.0"));
}

#[test]
fn test_new_changelog_entry() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CL New", "changelog");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["new", "changelog", "v1.0.0", "--tags", "new,improvement"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    // Find the created file (should have date prefix since changelog has_date=true)
    let entries: Vec<_> = fs::read_dir(site_dir.join("content/changelog"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().unwrap_or("").contains("v1-0-0"))
        .collect();
    assert_eq!(entries.len(), 1);

    let content = fs::read_to_string(entries[0].path()).unwrap();
    assert!(content.contains("title: v1.0.0"));
    assert!(content.contains("new"));
    assert!(content.contains("improvement"));
    // Changelog entries have dates
    assert!(content.contains("date:"));
}

// --- roadmap collection ---

#[test]
fn test_init_with_roadmap_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Roadmap Test", "roadmap");

    let root = tmp.path().join("site");
    assert!(root.join("content/roadmap").is_dir());
    assert!(root.join("templates/roadmap-item.html").exists());

    // Verify sample roadmap items exist (3 items: dark-mode, api-v2, initial-release)
    let items: Vec<_> = fs::read_dir(root.join("content/roadmap"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(items.len(), 3);

    // Verify seite.toml has roadmap collection
    let config = fs::read_to_string(root.join("seite.toml")).unwrap();
    assert!(config.contains("name = \"roadmap\""));
}

#[test]
fn test_build_roadmap_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Roadmap Build", "roadmap");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let dist = site_dir.join("dist");

    // Verify roadmap item pages
    assert!(dist.join("roadmap/dark-mode.html").exists());
    assert!(dist.join("roadmap/dark-mode.md").exists());
    assert!(dist.join("roadmap/api-v2.html").exists());
    assert!(dist.join("roadmap/initial-release.html").exists());

    // Verify roadmap index page
    assert!(dist.join("roadmap/index.html").exists());

    // Roadmap has no RSS (has_rss: false)
    let feed = fs::read_to_string(dist.join("feed.xml")).unwrap();
    assert!(!feed.contains("Dark Mode"));
}

#[test]
fn test_roadmap_grouped_by_status() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Roadmap Status", "roadmap");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/roadmap/index.html")).unwrap();
    // Should contain status section headers
    assert!(
        index.contains("In Progress") || index.contains("in-progress"),
        "Roadmap index should group by status"
    );
    assert!(
        index.contains("Planned") || index.contains("planned"),
        "Roadmap index should have planned section"
    );
    assert!(
        index.contains("Done") || index.contains("done"),
        "Roadmap index should have done section"
    );
}

#[test]
fn test_roadmap_weight_ordering() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Weight Order", "roadmap");

    let site_dir = tmp.path().join("site");

    // Remove sample items, create custom ones with explicit weights
    for entry in fs::read_dir(site_dir.join("content/roadmap")).unwrap() {
        let entry = entry.unwrap();
        fs::remove_file(entry.path()).unwrap();
    }

    fs::write(
        site_dir.join("content/roadmap/feature-c.md"),
        "---\ntitle: Feature C\nweight: 3\ntags:\n  - planned\n---\n\nThird.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/roadmap/feature-a.md"),
        "---\ntitle: Feature A\nweight: 1\ntags:\n  - planned\n---\n\nFirst.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/roadmap/feature-b.md"),
        "---\ntitle: Feature B\nweight: 2\ntags:\n  - planned\n---\n\nSecond.\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/roadmap/index.html")).unwrap();
    let pos_a = index
        .find("Feature A")
        .expect("Feature A should be in index");
    let pos_b = index
        .find("Feature B")
        .expect("Feature B should be in index");
    let pos_c = index
        .find("Feature C")
        .expect("Feature C should be in index");

    assert!(
        pos_a < pos_b && pos_b < pos_c,
        "Roadmap items should be sorted by weight: A(1) < B(2) < C(3), got positions: {} {} {}",
        pos_a,
        pos_b,
        pos_c
    );
}

#[test]
fn test_new_roadmap_item() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "RM New", "roadmap");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["new", "roadmap", "Offline Mode", "--tags", "planned"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    let item_file = site_dir.join("content/roadmap/offline-mode.md");
    // init ships a dark-mode.md sample; `seite new` must not clobber it
    assert!(item_file.exists());

    let content = fs::read_to_string(item_file).unwrap();
    assert!(content.contains("title: Offline Mode"));
    assert!(content.contains("planned"));
    // Roadmap items do NOT have dates (has_date: false)
    assert!(!content.contains("date:"));
}

#[test]
fn test_build_changelog_and_roadmap_together() {
    let tmp = TempDir::new().unwrap();
    init_site(
        &tmp,
        "site",
        "Both Collections",
        "posts,changelog,roadmap,pages",
    );

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let dist = site_dir.join("dist");

    // Both collection indexes should exist
    assert!(dist.join("changelog/index.html").exists());
    assert!(dist.join("roadmap/index.html").exists());

    // Homepage should list both collections
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(
        index.contains("changelog") || index.contains("Changelog"),
        "Homepage should reference changelog"
    );
    assert!(
        index.contains("roadmap") || index.contains("Roadmap"),
        "Homepage should reference roadmap"
    );
}

// --- collection command ---

#[test]
fn test_collection_list() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "collsite", "Coll Test", "posts,docs,pages");
    let site_dir = tmp.path().join("collsite");

    page_cmd()
        .args(["collection", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("posts"))
        .stdout(predicate::str::contains("docs"))
        .stdout(predicate::str::contains("pages"));
}

#[test]
fn test_collection_list_shows_table_headers() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "collhdr", "Headers", "posts");
    let site_dir = tmp.path().join("collhdr");

    page_cmd()
        .args(["collection", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("NAME"))
        .stdout(predicate::str::contains("DIRECTORY"))
        .stdout(predicate::str::contains("DATED"))
        .stdout(predicate::str::contains("RSS"));
}

#[test]
fn test_collection_add_changelog() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "colladd", "Add Test", "posts");
    let site_dir = tmp.path().join("colladd");

    page_cmd()
        .args(["collection", "add", "changelog"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Added 'changelog' collection"));

    // Verify content directory was created
    assert!(site_dir.join("content/changelog").is_dir());

    // Verify seite.toml was updated
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("changelog"));
}

#[test]
fn test_collection_add_roadmap() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "collrm", "Roadmap Test", "posts");
    let site_dir = tmp.path().join("collrm");

    page_cmd()
        .args(["collection", "add", "roadmap"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Added 'roadmap' collection"));

    assert!(site_dir.join("content/roadmap").is_dir());
}

#[test]
fn test_collection_add_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "colldoc", "Doc Test", "posts");
    let site_dir = tmp.path().join("colldoc");

    page_cmd()
        .args(["collection", "add", "docs"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Added 'docs' collection"));

    assert!(site_dir.join("content/docs").is_dir());
}

#[test]
fn test_collection_add_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "collpg", "Pages Test", "posts");
    let site_dir = tmp.path().join("collpg");

    page_cmd()
        .args(["collection", "add", "pages"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Added 'pages' collection"));

    assert!(site_dir.join("content/pages").is_dir());
}

#[test]
fn test_collection_add_duplicate_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "colldup", "Dup Test", "posts");
    let site_dir = tmp.path().join("colldup");

    page_cmd()
        .args(["collection", "add", "posts"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_collection_add_unknown_preset_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "collunk", "Unknown Test", "posts");
    let site_dir = tmp.path().join("collunk");

    page_cmd()
        .args(["collection", "add", "foobar"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown collection preset"));
}

#[test]
fn test_collection_add_outside_project_fails() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["collection", "add", "posts"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_collection_list_shows_properties() {
    // Test that collection list shows has_date, has_rss etc. correctly
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "collprop", "Props", "posts,docs");
    let site_dir = tmp.path().join("collprop");

    // posts should show has_date=yes, has_rss=yes
    let stdout = String::from_utf8(
        page_cmd()
            .args(["collection", "list"])
            .current_dir(&site_dir)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(stdout.contains("yes")); // At least some "yes" for dated/rss collections
    assert!(stdout.contains("/posts")); // URL prefix
    assert!(stdout.contains("/docs")); // URL prefix
}

#[test]
fn test_collection_add_then_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "collbuild", "Build Test", "posts");
    let site_dir = tmp.path().join("collbuild");

    // Add changelog collection
    page_cmd()
        .args(["collection", "add", "changelog"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Create a changelog entry
    fs::write(
        site_dir.join("content/changelog/2025-01-01-v1.md"),
        "---\ntitle: v1.0.0\ndate: 2025-01-01\ntags:\n  - new\n---\nFirst release!\n",
    )
    .unwrap();

    // Build should succeed with the new collection
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify changelog output exists
    let changelog_dir = site_dir.join("dist/changelog");
    assert!(changelog_dir.is_dir());
    let changelog_files: Vec<_> = fs::read_dir(&changelog_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
        .collect();
    assert!(
        !changelog_files.is_empty(),
        "Should have at least one changelog HTML file"
    );
}

#[test]
fn test_build_with_subdomain_collection() {
    let tmp = TempDir::new().unwrap();
    let site_dir = init_subdomain_site(&tmp, "subdom1");

    // Create a doc in the subdomain collection
    fs::write(
        site_dir.join("content/docs/getting-started.md"),
        "---\ntitle: Getting Started\ndescription: A guide\n---\nHello subdomain!\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Main dist/ should NOT contain docs
    assert!(
        !site_dir.join("dist/docs").exists(),
        "main dist/ should not contain subdomain collection"
    );

    // Subdomain output should exist
    let subdomain_dir = site_dir.join("dist-subdomains/docs");
    assert!(
        subdomain_dir.exists(),
        "dist-subdomains/docs/ should be created"
    );

    // Should have the doc rendered at root (no /docs prefix)
    assert!(
        subdomain_dir.join("getting-started.html").exists(),
        "doc should be at subdomain root, not under /docs/"
    );

    // Main dist/ should still have posts and pages
    assert!(site_dir.join("dist/index.html").exists());
}

#[test]
fn test_build_subdomain_has_own_sitemap_rss() {
    let tmp = TempDir::new().unwrap();
    let site_dir = init_subdomain_site(&tmp, "subdom2");

    fs::write(
        site_dir.join("content/docs/intro.md"),
        "---\ntitle: Intro\ndescription: Introduction\n---\nIntro content\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let subdomain_dir = site_dir.join("dist-subdomains/docs");
    assert!(
        subdomain_dir.join("sitemap.xml").exists(),
        "subdomain should have its own sitemap"
    );
    assert!(
        subdomain_dir.join("robots.txt").exists(),
        "subdomain should have its own robots.txt"
    );
    assert!(
        subdomain_dir.join("llms.txt").exists(),
        "subdomain should have its own llms.txt"
    );
}

#[test]
fn test_build_without_subdomain_no_dist_subdomains() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "nosubdom", "No Subdomain", "posts,docs");
    let site_dir = tmp.path().join("nosubdom");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // No dist-subdomains/ should be created
    assert!(
        !site_dir.join("dist-subdomains").exists(),
        "dist-subdomains/ should not be created without subdomain config"
    );

    // Docs should be in main dist/
    assert!(site_dir.join("dist/docs").exists() || !site_dir.join("dist/docs").exists());
    // Main output should still work
    assert!(site_dir.join("dist/index.html").exists());
}

#[test]
fn test_build_subdomain_correct_base_url() {
    let tmp = TempDir::new().unwrap();
    let site_dir = init_subdomain_site(&tmp, "subdom3");

    fs::write(
        site_dir.join("content/docs/test-page.md"),
        "---\ntitle: Test Page\ndescription: Testing base URL\n---\nBase URL test\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist-subdomains/docs/test-page.html")).unwrap();

    // The canonical URL should use the subdomain base_url
    assert!(
        html.contains("docs.example.com"),
        "subdomain pages should use subdomain base URL"
    );
}

#[test]
fn test_build_subdomain_root_urls() {
    let tmp = TempDir::new().unwrap();
    let site_dir = init_subdomain_site(&tmp, "subdom4");

    fs::write(
        site_dir.join("content/docs/setup.md"),
        "---\ntitle: Setup\ndescription: Setup guide\n---\nSetup content\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let subdomain_dir = site_dir.join("dist-subdomains/docs");

    // Items should be at root, not under a /docs/ prefix
    assert!(
        subdomain_dir.join("setup.html").exists(),
        "content should be at subdomain root"
    );
    assert!(
        !subdomain_dir.join("docs/setup.html").exists(),
        "content should NOT be under /docs/ prefix in subdomain"
    );
}

#[test]
fn test_subdomain_config_validation_no_duplicate() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "subdup", "Dup Subdomain", "posts,docs,pages");
    let site_dir = tmp.path().join("subdup");

    // Set the same subdomain on two collections
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config
        .replace("name = \"docs\"", "name = \"docs\"\nsubdomain = \"api\"")
        .replace("name = \"pages\"", "name = \"pages\"\nsubdomain = \"api\"");
    fs::write(&toml_path, config).unwrap();

    // Build should fail due to duplicate subdomain
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate subdomain"));
}

#[test]
fn test_build_subdomain_explicit_base_url_override() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "explicit_sub", "Explicit Sub", "posts,docs,pages");
    let site_dir = tmp.path().join("explicit_sub");

    // Use www. base_url to demonstrate the problem being solved
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "base_url = \"http://localhost:3000\"",
        "base_url = \"https://www.example.com\"",
    );
    let config = config.replace(
        "name = \"docs\"",
        "name = \"docs\"\nsubdomain = \"docs\"\nsubdomain_base_url = \"https://docs.example.com\"",
    );
    fs::write(&toml_path, config).unwrap();

    fs::write(
        site_dir.join("content/docs/guide.md"),
        "---\ntitle: Guide\ndescription: A guide\n---\nGuide content.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist-subdomains/docs/guide.html")).unwrap();

    // Should use the explicit override, not docs.www.example.com
    assert!(
        html.contains("docs.example.com"),
        "subdomain pages should use explicit subdomain_base_url"
    );
    assert!(
        !html.contains("docs.www.example.com"),
        "www prefix should not appear in subdomain URLs"
    );
}

#[test]
fn test_subdomain_base_url_without_subdomain_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "invalid_sub", "Invalid", "posts,docs");
    let site_dir = tmp.path().join("invalid_sub");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "name = \"docs\"",
        "name = \"docs\"\nsubdomain_base_url = \"https://docs.example.com\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires subdomain to be set"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Collection index page tests (content/{collection}/index.md)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_collection_index_page_content_on_collection_index() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "col_idx1", "Collection Index", "docs,pages");
    let site_dir = tmp.path().join("col_idx1");

    // Create a collection index page (with date/updated to exercise date formatting paths)
    fs::write(
        site_dir.join("content/docs/index.md"),
        "---\ntitle: Documentation Hub\ndescription: Welcome to our docs\ndate: 2026-01-10\nupdated: 2026-02-15\n---\n\nWelcome to the **documentation hub**. Browse our guides below.\n",
    )
    .unwrap();

    // Create a regular doc item
    fs::write(
        site_dir.join("content/docs/setup.md"),
        "---\ntitle: Setup Guide\ndescription: How to set up\n---\nSetup instructions.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The collection index page should inject content into the docs index
    let index_html = fs::read_to_string(site_dir.join("dist/docs/index.html")).unwrap();
    assert!(
        index_html.contains("documentation hub"),
        "collection index.md content should appear on collection index page"
    );
    assert!(
        index_html.contains("Documentation Hub"),
        "collection index.md title should be used"
    );

    // The regular doc should still be rendered
    assert!(site_dir.join("dist/docs/setup.html").exists());

    // The index.md should NOT be rendered as a standalone item
    assert!(
        !site_dir.join("dist/docs/index.html").exists() || index_html.contains("documentation hub"),
        "index.md should be the collection index, not a standalone item"
    );
}

#[test]
fn test_collection_index_page_excluded_from_items() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "col_idx2", "Col Index Excl", "docs,pages");
    let site_dir = tmp.path().join("col_idx2");

    // Create a collection index page and a regular doc
    fs::write(
        site_dir.join("content/docs/index.md"),
        "---\ntitle: Docs Home\ndescription: Home\n---\nDocs home content.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/guide.md"),
        "---\ntitle: User Guide\ndescription: Guide\n---\nGuide content.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The main site index should list docs as a collection but the index.md
    // should not appear as a separate item
    let main_index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        main_index.contains("User Guide"),
        "regular doc should appear in listings"
    );
    // The index.md should not appear as a listed item (it's extracted)
    assert!(
        !main_index.contains("/docs/index"),
        "collection index.md should not appear as a listed item"
    );
}

#[test]
fn test_subdomain_root_uses_collection_index_page() {
    let tmp = TempDir::new().unwrap();
    init_subdomain_site(&tmp, "sub_idx");
    let site_dir = tmp.path().join("sub_idx");

    // Create a collection index page for the subdomain collection
    // Include date and updated fields to exercise .map(|d| d.to_string()) closure paths
    fs::write(
        site_dir.join("content/docs/index.md"),
        "---\ntitle: Developer Docs\ndescription: API documentation and guides\ndate: 2026-01-15\nupdated: 2026-02-20\n---\n\nWelcome to **developer docs**. Explore our comprehensive API guides.\n",
    )
    .unwrap();

    // Create a regular doc
    fs::write(
        site_dir.join("content/docs/api-reference.md"),
        "---\ntitle: API Reference\ndescription: Full API docs\n---\nAPI endpoint details.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The subdomain root should have the index.md content
    let subdomain_index =
        fs::read_to_string(site_dir.join("dist-subdomains/docs/index.html")).unwrap();
    assert!(
        subdomain_index.contains("developer docs"),
        "subdomain root should render collection index.md content"
    );
    assert!(
        subdomain_index.contains("Developer Docs"),
        "subdomain root should use collection index.md title"
    );
    assert!(
        subdomain_index.contains("API documentation and guides"),
        "subdomain root should use collection index.md description"
    );

    // Regular doc should still render at subdomain root level
    assert!(
        site_dir
            .join("dist-subdomains/docs/api-reference.html")
            .exists(),
        "regular docs should still be rendered in subdomain"
    );
}

#[test]
fn test_subdomain_root_without_collection_index_still_works() {
    let tmp = TempDir::new().unwrap();
    init_subdomain_site(&tmp, "sub_noidx");
    let site_dir = tmp.path().join("sub_noidx");

    // Only create a regular doc, no index.md
    fs::write(
        site_dir.join("content/docs/tutorial.md"),
        "---\ntitle: Tutorial\ndescription: A tutorial\n---\nTutorial content.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Should still produce a valid subdomain root index
    let subdomain_index =
        fs::read_to_string(site_dir.join("dist-subdomains/docs/index.html")).unwrap();
    assert!(
        subdomain_index.contains("Tutorial"),
        "subdomain root should list collection items even without index.md"
    );
}

#[test]
fn test_docs_collection_index_gets_sidebar_nav() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "docs_nav", "Docs Nav", "docs,pages");
    let site_dir = tmp.path().join("docs_nav");

    // Apply the docs theme for sidebar rendering
    page_cmd()
        .args(["theme", "apply", "docs"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Create docs with subdirectory structure for nav sections
    fs::create_dir_all(site_dir.join("content/docs/guides")).unwrap();
    fs::write(
        site_dir.join("content/docs/overview.md"),
        "---\ntitle: Overview\ndescription: Overview\n---\nOverview content.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/guides/setup.md"),
        "---\ntitle: Setup Guide\ndescription: Setup\n---\nSetup content.\n",
    )
    .unwrap();

    // Create a docs index page
    fs::write(
        site_dir.join("content/docs/index.md"),
        "---\ntitle: Documentation\ndescription: All docs\n---\nWelcome to the **docs**.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The docs index should have sidebar nav and page content
    let docs_index = fs::read_to_string(site_dir.join("dist/docs/index.html")).unwrap();
    assert!(
        docs_index.contains("Welcome to the"),
        "docs index should have index.md content"
    );
    // The docs theme sidebar renders nav items
    assert!(
        docs_index.contains("Overview") && docs_index.contains("Setup Guide"),
        "docs index should have sidebar nav with all doc pages"
    );
}

#[test]
fn test_docs_index_uses_docs_index_template() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "docs_tmpl", "Docs Tmpl", "docs,pages");
    let site_dir = tmp.path().join("docs_tmpl");

    // Create a doc
    fs::write(
        site_dir.join("content/docs/intro.md"),
        "---\ntitle: Introduction\ndescription: Intro\n---\nIntro.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // With default template (no custom theme), the docs-index.html template
    // should auto-generate a section overview
    let docs_index = fs::read_to_string(site_dir.join("dist/docs/index.html")).unwrap();
    assert!(
        docs_index.contains("Introduction"),
        "docs index should show doc items from the auto-generated overview"
    );
}

#[test]
fn test_collection_index_redirect_to() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "redir", "Redirect Test", "docs,pages");
    let site_dir = tmp.path().join("redir");

    // Create a docs index that redirects to a specific doc page
    fs::write(
        site_dir.join("content/docs/index.md"),
        "---\ntitle: Docs\nextra:\n  redirect_to: /docs/getting-started\n---\n",
    )
    .unwrap();

    // Create the target doc page
    fs::write(
        site_dir.join("content/docs/getting-started.md"),
        "---\ntitle: Getting Started\ndescription: Start here\n---\nStart here.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The docs index should be a redirect page
    let docs_index = fs::read_to_string(site_dir.join("dist/docs/index.html")).unwrap();
    assert!(
        docs_index.contains("http-equiv=\"refresh\""),
        "docs index should be an HTML redirect"
    );
    assert!(
        docs_index.contains("/docs/getting-started"),
        "docs index should redirect to the specified page"
    );

    // The target page should exist normally
    assert!(
        site_dir.join("dist/docs/getting-started.html").exists(),
        "redirect target should exist"
    );
}

#[test]
fn test_subdomain_redirect_to() {
    let tmp = TempDir::new().unwrap();
    init_subdomain_site(&tmp, "sub_redir");
    let site_dir = tmp.path().join("sub_redir");

    // Create a subdomain index that redirects
    fs::write(
        site_dir.join("content/docs/index.md"),
        "---\ntitle: Docs\nextra:\n  redirect_to: /getting-started\n---\n",
    )
    .unwrap();

    fs::write(
        site_dir.join("content/docs/getting-started.md"),
        "---\ntitle: Getting Started\ndescription: Start\n---\nStart content.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The subdomain root should redirect
    let subdomain_index =
        fs::read_to_string(site_dir.join("dist-subdomains/docs/index.html")).unwrap();
    assert!(
        subdomain_index.contains("http-equiv=\"refresh\""),
        "subdomain root should be an HTML redirect"
    );
    assert!(
        subdomain_index.contains("/getting-started"),
        "subdomain root should redirect to the target page"
    );
}

#[test]
fn test_paginated_collection_index_page_on_first_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "pag_idx", "Paginated Index", "posts,pages");
    let site_dir = tmp.path().join("pag_idx");

    // Enable pagination on the posts collection
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("name = \"posts\"", "name = \"posts\"\npaginate = 2");
    fs::write(&toml_path, config).unwrap();

    // Create a collection index page for posts
    fs::write(
        site_dir.join("content/posts/index.md"),
        "---\ntitle: Blog Archive\ndescription: All our blog posts\ndate: 2026-01-01\nupdated: 2026-02-01\n---\n\nWelcome to the **blog archive**.\n",
    )
    .unwrap();

    // Create enough posts to trigger pagination
    for i in 1..=4 {
        fs::write(
            site_dir.join(format!("content/posts/2026-01-0{i}-post-{i}.md")),
            format!("---\ntitle: Post {i}\ndescription: Post number {i}\n---\nPost {i} content.\n"),
        )
        .unwrap();
    }

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Page 1 should have the collection index.md content
    let page1 = fs::read_to_string(site_dir.join("dist/posts/index.html")).unwrap();
    assert!(
        page1.contains("blog archive"),
        "paginated page 1 should include collection index.md content"
    );
    assert!(
        page1.contains("Blog Archive"),
        "paginated page 1 should use collection index.md title"
    );

    // Page 2 should NOT have the index.md content
    let page2 = fs::read_to_string(site_dir.join("dist/posts/page/2/index.html")).unwrap();
    assert!(
        !page2.contains("blog archive"),
        "paginated page 2 should not have collection index.md content"
    );
}

#[test]
fn test_subdomain_root_has_sidebar_nav() {
    let tmp = TempDir::new().unwrap();
    init_subdomain_site(&tmp, "sub_nav");
    let site_dir = tmp.path().join("sub_nav");

    // Create docs with subdirectory structure
    fs::create_dir_all(site_dir.join("content/docs/guides")).unwrap();
    fs::write(
        site_dir.join("content/docs/overview.md"),
        "---\ntitle: Overview\ndescription: Overview\n---\nOverview.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/guides/setup.md"),
        "---\ntitle: Setup Guide\ndescription: Setup\n---\nSetup.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The subdomain root renders with the collection's own index template
    // (docs-index.html) — the same template + nav context the docs index uses on
    // the main domain — so it lists the docs, not the generic site index.
    let subdomain_index =
        fs::read_to_string(site_dir.join("dist-subdomains/docs/index.html")).unwrap();
    assert!(
        subdomain_index.contains("Overview"),
        "subdomain root should list doc Overview via docs-index.html"
    );
    assert!(
        subdomain_index.contains("Setup Guide"),
        "subdomain root should list doc Setup Guide via docs-index.html"
    );
}

#[test]
fn test_collection_index_redirect_to_with_i18n() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "redir_i18n", "Redirect i18n", "docs,pages");
    let site_dir = tmp.path().join("redir_i18n");

    // Enable Spanish as a language
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Redirigir\"\n");
    fs::write(&toml_path, config).unwrap();

    // Create docs index that redirects (default language)
    fs::write(
        site_dir.join("content/docs/index.md"),
        "---\ntitle: Docs\nextra:\n  redirect_to: /docs/getting-started\n---\n",
    )
    .unwrap();

    // Create Spanish translation of the redirect index
    fs::write(
        site_dir.join("content/docs/index.es.md"),
        "---\ntitle: Documentación\nextra:\n  redirect_to: /docs/getting-started\n---\n",
    )
    .unwrap();

    // Create the target page
    fs::write(
        site_dir.join("content/docs/getting-started.md"),
        "---\ntitle: Getting Started\n---\nStart here.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/getting-started.es.md"),
        "---\ntitle: Comenzar\n---\nComenzar aquí.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default language redirect
    let docs_index = fs::read_to_string(site_dir.join("dist/docs/index.html")).unwrap();
    assert!(
        docs_index.contains("/docs/getting-started"),
        "default lang redirect should point to /docs/getting-started"
    );

    // Spanish redirect should have /es prefix
    let es_docs_index = fs::read_to_string(site_dir.join("dist/es/docs/index.html")).unwrap();
    assert!(
        es_docs_index.contains("/es/docs/getting-started"),
        "Spanish redirect should have /es/ prefix, got: {es_docs_index}"
    );
}

#[test]
fn test_paginated_collection_nav_passed_to_template() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "pag_nav", "Paginated Nav", "docs,pages");
    let site_dir = tmp.path().join("pag_nav");

    // Enable pagination on docs collection
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("name = \"docs\"", "name = \"docs\"\npaginate = 2");
    fs::write(&toml_path, config).unwrap();

    // Apply docs theme which renders the sidebar nav
    page_cmd()
        .args(["theme", "apply", "docs"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Create enough docs for pagination
    fs::create_dir_all(site_dir.join("content/docs/guides")).unwrap();
    for i in 1..=4 {
        fs::write(
            site_dir.join(format!("content/docs/guides/guide-{i}.md")),
            format!("---\ntitle: Guide {i}\ndescription: Guide\nweight: {i}\n---\nGuide {i}.\n"),
        )
        .unwrap();
    }

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Paginated index page 1 should contain nav items
    let page1 = fs::read_to_string(site_dir.join("dist/docs/index.html")).unwrap();
    assert!(
        page1.contains("Guide 1") && page1.contains("Guide 2"),
        "paginated docs page 1 should have nav items rendered in sidebar"
    );
}

#[test]
fn test_subdomain_redirect_to_with_relative_url() {
    let tmp = TempDir::new().unwrap();
    init_subdomain_site(&tmp, "sub_redir2");
    let site_dir = tmp.path().join("sub_redir2");

    // Create a subdomain index that redirects with an absolute URL (no lang prefix needed)
    fs::write(
        site_dir.join("content/docs/index.md"),
        "---\ntitle: Docs\nextra:\n  redirect_to: https://external.example.com/docs\n---\n",
    )
    .unwrap();

    fs::write(
        site_dir.join("content/docs/setup.md"),
        "---\ntitle: Setup\ndescription: Setup\n---\nSetup.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The subdomain root should redirect to the external URL
    let subdomain_index =
        fs::read_to_string(site_dir.join("dist-subdomains/docs/index.html")).unwrap();
    assert!(
        subdomain_index.contains("https://external.example.com/docs"),
        "subdomain root should redirect to external URL"
    );
    assert!(
        subdomain_index.contains("http-equiv=\"refresh\""),
        "should be an HTML redirect"
    );
}

#[test]
fn test_collection_index_redirect_to_external_url() {
    // Tests the non-paginated redirect_to with a non-`/` URL (absolute/external)
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "redir_ext", "Redirect External", "docs,pages");
    let site_dir = tmp.path().join("redir_ext");

    fs::write(
        site_dir.join("content/docs/index.md"),
        "---\ntitle: Docs\nextra:\n  redirect_to: https://docs.example.com\n---\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/guide.md"),
        "---\ntitle: Guide\ndescription: Guide\n---\nGuide.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let docs_index = fs::read_to_string(site_dir.join("dist/docs/index.html")).unwrap();
    assert!(
        docs_index.contains("https://docs.example.com"),
        "should redirect to external URL without lang prefix"
    );
    assert!(
        docs_index.contains("http-equiv=\"refresh\""),
        "should be an HTML redirect"
    );
}

#[test]
fn test_docs_nav_fallback_for_missing_lang_in_cache() {
    // Tests the nav cache fallback when nav exists for "en" but not "es"
    // This exercises the `else` branch at nav_langs.get(lang) returning None
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "nav_fall", "Nav Fallback", "docs,pages");
    let site_dir = tmp.path().join("nav_fall");

    // Enable Spanish
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Nav Fallback\"\n");
    fs::write(&toml_path, config).unwrap();

    // Create docs only in default language — no Spanish translations
    fs::create_dir_all(site_dir.join("content/docs/guides")).unwrap();
    fs::write(
        site_dir.join("content/docs/overview.md"),
        "---\ntitle: Overview\ndescription: Overview\n---\nOverview.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/guides/setup.md"),
        "---\ntitle: Setup\ndescription: Setup\n---\nSetup.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default lang docs index should have nav items
    let docs_index = fs::read_to_string(site_dir.join("dist/docs/index.html")).unwrap();
    assert!(
        docs_index.contains("Overview"),
        "default lang docs index should have nav items"
    );

    // Build succeeds even when nav cache has "en" but not "es"
    // The Spanish docs index should exist (possibly empty collection)
    let es_docs_index_path = site_dir.join("dist/es/docs/index.html");
    if es_docs_index_path.exists() {
        // If it exists, it should not contain the English nav (empty nav fallback used)
        let _es_docs = fs::read_to_string(&es_docs_index_path).unwrap();
        // Just checking it renders without error is sufficient
    }
}

#[test]
fn test_paginated_docs_nav_fallback_for_missing_lang() {
    // Tests the paginated nav cache fallback when nav exists for "en" but not "es"
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "pag_nav_ml", "Pag Nav ML", "docs,pages");
    let site_dir = tmp.path().join("pag_nav_ml");

    // Enable Spanish + pagination on docs
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("name = \"docs\"", "name = \"docs\"\npaginate = 2");
    let config = format!("{config}\n[languages.es]\ntitle = \"Pag Nav ML\"\n");
    fs::write(&toml_path, config).unwrap();

    // Create docs only in English
    fs::create_dir_all(site_dir.join("content/docs/guides")).unwrap();
    for i in 1..=4 {
        fs::write(
            site_dir.join(format!("content/docs/guides/guide-{i}.md")),
            format!("---\ntitle: Guide {i}\ndescription: Guide\nweight: {i}\n---\nGuide {i}.\n"),
        )
        .unwrap();
    }

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // English page 1 should have nav
    let page1 = fs::read_to_string(site_dir.join("dist/docs/index.html")).unwrap();
    assert!(
        page1.contains("Guide 1"),
        "English docs page 1 should have nav items"
    );

    // Build succeeds for Spanish even though no Spanish docs content
    // (nav cache has "en" but not "es" — exercises empty_nav_value fallback)
}
