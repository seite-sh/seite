use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn page_cmd() -> Command {
    Command::cargo_bin("page").unwrap()
}

/// Helper to init a site with given collections
fn init_site(tmp: &TempDir, name: &str, title: &str, collections: &str) {
    page_cmd()
        .args([
            "init",
            name,
            "--title",
            title,
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            collections,
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
}

// --- init command ---

#[test]
fn test_init_creates_project_structure() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "mysite",
            "--title",
            "My Site",
            "--description",
            "A test site",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created new site in 'mysite'"));

    let root = tmp.path().join("mysite");
    assert!(root.join("page.toml").exists());
    assert!(root.join("content/posts").is_dir());
    assert!(root.join("content/pages").is_dir());
    assert!(root.join("templates/base.html").exists());
    assert!(root.join("templates/index.html").exists());
    assert!(root.join("templates/post.html").exists());
    assert!(root.join("templates/page.html").exists());
    assert!(root.join("static").is_dir());

    // Verify page.toml content
    let config_content = fs::read_to_string(root.join("page.toml")).unwrap();
    assert!(config_content.contains("title = \"My Site\""));
    assert!(config_content.contains("[[collections]]"));

    // Verify sample post exists
    let posts: Vec<_> = fs::read_dir(root.join("content/posts"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(posts.len(), 1);
    let post_content = fs::read_to_string(posts[0].path()).unwrap();
    assert!(post_content.contains("title: Hello World"));
}

#[test]
fn test_init_with_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mysite", "Doc Site", "posts,docs,pages");

    let root = tmp.path().join("mysite");
    assert!(root.join("content/docs").is_dir());
    assert!(root.join("templates/doc.html").exists());

    let config_content = fs::read_to_string(root.join("page.toml")).unwrap();
    assert!(config_content.contains("name = \"docs\""));
}

#[test]
fn test_init_fails_if_dir_exists() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir(tmp.path().join("existing")).unwrap();

    page_cmd()
        .args([
            "init",
            "existing",
            "--title",
            "Test",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

// --- build command ---

#[test]
fn test_build_produces_output() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Build Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Build it
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Built"));

    // Verify output files
    let dist = site_dir.join("dist");
    assert!(dist.join("index.html").exists());
    assert!(dist.join("feed.xml").exists());
    assert!(dist.join("sitemap.xml").exists());

    // The hello-world post should be rendered
    assert!(dist.join("posts/hello-world.html").exists());

    // Verify index.html has content
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(index.contains("Build Test"));
    assert!(index.contains("Hello World"));

    // Verify post HTML
    let post = fs::read_to_string(dist.join("posts/hello-world.html")).unwrap();
    assert!(post.contains("Hello World"));
    assert!(post.contains("page")); // from the body text
}

#[test]
fn test_build_excludes_drafts_by_default() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Draft Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create a draft post
    let draft =
        "---\ntitle: Secret Draft\ndraft: true\ndate: 2025-01-01\n---\n\nThis is a draft.\n";
    fs::write(
        site_dir.join("content/posts/2025-01-01-secret-draft.md"),
        draft,
    )
    .unwrap();

    // Build without --drafts
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(!site_dir
        .join("dist/posts/secret-draft.html")
        .exists());

    // Build with --drafts
    page_cmd()
        .args(["build", "--drafts"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir
        .join("dist/posts/secret-draft.html")
        .exists());
}

#[test]
fn test_build_with_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Docs Build", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    // Create a doc
    let doc = "---\ntitle: Getting Started\n---\n\nWelcome to the docs.\n";
    fs::write(
        site_dir.join("content/docs/getting-started.md"),
        doc,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir
        .join("dist/docs/getting-started.html")
        .exists());
}

#[test]
fn test_nested_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Nested Docs", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    // Create nested doc
    fs::create_dir_all(site_dir.join("content/docs/guides")).unwrap();
    let doc = "---\ntitle: Setup Guide\n---\n\nHow to set up.\n";
    fs::write(site_dir.join("content/docs/guides/setup.md"), doc).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir
        .join("dist/docs/guides/setup.html")
        .exists());
}

// --- new command ---

#[test]
fn test_new_post_creates_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "New Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["new", "post", "My Test Post", "--tags", "rust,testing"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    // Find the created file (there should be 2: hello-world + our new post)
    let posts: Vec<_> = fs::read_dir(site_dir.join("content/posts"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .unwrap_or("")
                .contains("my-test-post")
        })
        .collect();
    assert_eq!(posts.len(), 1);

    let content = fs::read_to_string(posts[0].path()).unwrap();
    assert!(content.contains("title: My Test Post"));
    assert!(content.contains("rust"));
    assert!(content.contains("testing"));
}

#[test]
fn test_new_page_creates_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Page Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["new", "page", "About Me"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    let page_file = site_dir.join("content/pages/about-me.md");
    assert!(page_file.exists());

    let content = fs::read_to_string(page_file).unwrap();
    assert!(content.contains("title: About Me"));
}

#[test]
fn test_new_doc_creates_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Doc New Test", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["new", "doc", "Getting Started"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    let doc_file = site_dir.join("content/docs/getting-started.md");
    assert!(doc_file.exists());

    let content = fs::read_to_string(doc_file).unwrap();
    assert!(content.contains("title: Getting Started"));
    // Docs should NOT have a date field at all
    assert!(!content.contains("date:"));
}

#[test]
fn test_new_unknown_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Unknown Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["new", "widget", "My Widget"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown collection"));
}

// --- RSS and sitemap ---

#[test]
fn test_rss_feed_valid() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "RSS Test", "posts,pages");

    page_cmd()
        .arg("build")
        .current_dir(tmp.path().join("site"))
        .assert()
        .success();

    let feed = fs::read_to_string(tmp.path().join("site/dist/feed.xml")).unwrap();
    assert!(feed.contains("<rss"));
    assert!(feed.contains("<channel>"));
    assert!(feed.contains("<title>RSS Test</title>"));
    assert!(feed.contains("Hello World"));
}

#[test]
fn test_rss_excludes_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "RSS Docs Test", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    // Create a doc
    let doc = "---\ntitle: My Doc\n---\n\nDoc content.\n";
    fs::write(site_dir.join("content/docs/my-doc.md"), doc).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let feed = fs::read_to_string(site_dir.join("dist/feed.xml")).unwrap();
    assert!(feed.contains("Hello World")); // posts are in RSS
    assert!(!feed.contains("My Doc")); // docs are NOT in RSS
}

#[test]
fn test_sitemap_valid() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Sitemap Test", "posts,pages");

    page_cmd()
        .arg("build")
        .current_dir(tmp.path().join("site"))
        .assert()
        .success();

    let sitemap = fs::read_to_string(tmp.path().join("site/dist/sitemap.xml")).unwrap();
    assert!(sitemap.contains("<urlset"));
    assert!(sitemap.contains("<loc>"));
    assert!(sitemap.contains("hello-world"));
}

#[test]
fn test_index_shows_listed_only() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Index Test", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    // Create a doc and a page
    let doc = "---\ntitle: A Doc\n---\n\nDoc.\n";
    fs::write(site_dir.join("content/docs/a-doc.md"), doc).unwrap();

    let pg = "---\ntitle: About\n---\n\nAbout page.\n";
    fs::write(site_dir.join("content/pages/about.md"), pg).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Posts and docs are listed
    assert!(index.contains("Hello World"));
    assert!(index.contains("A Doc"));
    // Pages collection is not listed (listed: false)
    assert!(!index.contains("About"));
}

// --- homepage as special page ---

#[test]
fn test_build_homepage_with_index_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Home Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create content/pages/index.md as the homepage
    let homepage = "---\ntitle: Welcome Home\n---\n\nThis is **hero content** for the homepage.\n";
    fs::write(site_dir.join("content/pages/index.md"), homepage).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Homepage content should appear
    assert!(index.contains("hero content"));
    assert!(index.contains("homepage-content"));
    // Collection listing should still work
    assert!(index.contains("Hello World"));

    // Homepage markdown should also be output
    assert!(site_dir.join("dist/index.md").exists());
}

#[test]
fn test_build_homepage_without_index_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Home Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Collection listing should work as before
    assert!(index.contains("Hello World"));
    // No homepage-content div since there's no index.md
    assert!(!index.contains("homepage-content"));
}

// --- multi-language (i18n) ---

/// Helper: add `[languages.es]` to a site's page.toml
fn add_language(site_dir: &std::path::Path, lang: &str, title: &str) {
    let toml_path = site_dir.join("page.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config.push_str(&format!(
        "\n[languages.{lang}]\ntitle = \"{title}\"\n"
    ));
    fs::write(&toml_path, config).unwrap();
}

#[test]
fn test_i18n_translated_pages_urls() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "i18n Test", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Prueba i18n");

    // Create a page in default language and its Spanish translation
    let about_en = "---\ntitle: About\n---\n\nAbout us.\n";
    fs::write(site_dir.join("content/pages/about.md"), about_en).unwrap();

    let about_es = "---\ntitle: Acerca de\n---\n\nSobre nosotros.\n";
    fs::write(site_dir.join("content/pages/about.es.md"), about_es).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // English version at root
    assert!(site_dir.join("dist/about.html").exists());
    // Spanish version under /es/
    assert!(site_dir.join("dist/es/about.html").exists());

    // Verify content
    let en_html = fs::read_to_string(site_dir.join("dist/about.html")).unwrap();
    assert!(en_html.contains("About us."));

    let es_html = fs::read_to_string(site_dir.join("dist/es/about.html")).unwrap();
    assert!(es_html.contains("Sobre nosotros."));
}

#[test]
fn test_i18n_per_language_index() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Index i18n", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Índice i18n");

    // Create a Spanish post
    let es_post = "---\ntitle: Hola Mundo\ndate: 2025-01-15\n---\n\nContenido en español.\n";
    fs::write(
        site_dir.join("content/posts/2025-01-15-hola-mundo.es.md"),
        es_post,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default index should have Hello World (English) but NOT Hola Mundo
    let index_en = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index_en.contains("Hello World"));
    assert!(!index_en.contains("Hola Mundo"));

    // Spanish index should have Hola Mundo but NOT Hello World
    let index_es = fs::read_to_string(site_dir.join("dist/es/index.html")).unwrap();
    assert!(index_es.contains("Hola Mundo"));
    assert!(!index_es.contains("Hello World"));
    // Spanish index uses the Spanish site title
    assert!(index_es.contains("Índice i18n"));
}

#[test]
fn test_i18n_sitemap_alternates() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Sitemap i18n", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Mapa del sitio");

    // Create a page with translation
    let about_en = "---\ntitle: About\n---\n\nAbout us.\n";
    fs::write(site_dir.join("content/pages/about.md"), about_en).unwrap();
    let about_es = "---\ntitle: Acerca de\n---\n\nSobre nosotros.\n";
    fs::write(site_dir.join("content/pages/about.es.md"), about_es).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let sitemap = fs::read_to_string(site_dir.join("dist/sitemap.xml")).unwrap();
    // Sitemap should contain xhtml namespace for multilingual
    assert!(sitemap.contains("xmlns:xhtml"));
    // Should have hreflang alternate links
    assert!(sitemap.contains("xhtml:link"));
    assert!(sitemap.contains("hreflang"));
    // Should contain x-default for index
    assert!(sitemap.contains("x-default"));
}

#[test]
fn test_i18n_per_language_rss() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "RSS i18n", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "RSS i18n ES");

    // Create a Spanish post
    let es_post = "---\ntitle: Hola Mundo\ndate: 2025-01-15\n---\n\nContenido.\n";
    fs::write(
        site_dir.join("content/posts/2025-01-15-hola-mundo.es.md"),
        es_post,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default feed should exist and contain Hello World
    let feed_en = fs::read_to_string(site_dir.join("dist/feed.xml")).unwrap();
    assert!(feed_en.contains("Hello World"));
    assert!(!feed_en.contains("Hola Mundo"));

    // Spanish feed should exist
    let feed_es = fs::read_to_string(site_dir.join("dist/es/feed.xml")).unwrap();
    assert!(feed_es.contains("Hola Mundo"));
    assert!(!feed_es.contains("Hello World"));
}

#[test]
fn test_i18n_discovery_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Discovery i18n", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Discovery ES");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default language discovery files at root
    assert!(site_dir.join("dist/llms.txt").exists());
    assert!(site_dir.join("dist/llms-full.txt").exists());

    // Per-language discovery files
    assert!(site_dir.join("dist/es/llms.txt").exists());
    assert!(site_dir.join("dist/es/llms-full.txt").exists());
}

#[test]
fn test_i18n_backward_compat_single_language() {
    // A site without [languages] should work exactly like before
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Compat Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // No /es/ or other language directories
    let dist = site_dir.join("dist");
    assert!(dist.join("index.html").exists());
    assert!(dist.join("feed.xml").exists());
    assert!(dist.join("sitemap.xml").exists());

    // Sitemap should NOT have xhtml namespace
    let sitemap = fs::read_to_string(dist.join("sitemap.xml")).unwrap();
    assert!(!sitemap.contains("xmlns:xhtml"));
}

#[test]
fn test_new_with_lang_flag() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Lang New Test", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Prueba");

    page_cmd()
        .args(["new", "page", "About", "--lang", "es"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    // File should have .es.md suffix
    assert!(site_dir.join("content/pages/about.es.md").exists());
}

#[test]
fn test_new_with_invalid_lang() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Invalid Lang", "posts,pages");

    let site_dir = tmp.path().join("site");
    // Don't add any languages

    page_cmd()
        .args(["new", "page", "About", "--lang", "fr"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown language"));
}

// --- theme command ---

#[test]
fn test_theme_list() {
    page_cmd()
        .args(["theme", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("default"))
        .stdout(predicate::str::contains("brutalist"))
        .stdout(predicate::str::contains("bento"));
}

#[test]
fn test_theme_apply_brutalist() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Brutalist Test", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "apply", "brutalist"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let base = std::fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(base.contains("fffef0"), "brutalist theme should have cream background");
    assert!(base.contains("ffe600"), "brutalist theme should have yellow accent");
}

#[test]
fn test_theme_apply_bento() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Bento Test", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "apply", "bento"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let base = std::fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(base.contains("border-radius: 20px"), "bento theme should have rounded cards");
    assert!(base.contains("5046e5"), "bento theme should have indigo accent");
}

#[test]
fn test_theme_apply_dark_revised() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Dark Test", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let base = std::fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(base.contains("0a0a0a"), "dark theme should use true black background");
    assert!(base.contains("8b5cf6"), "dark theme should use violet accent");
}

#[test]
fn test_theme_create_requires_page_toml() {
    // `page theme create` without a page.toml in the directory should fail gracefully
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["theme", "create", "dark glassmorphism"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

// --- search index ---

#[test]
fn test_build_generates_search_index() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Search Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // search-index.json must exist
    let index_path = site_dir.join("dist/search-index.json");
    assert!(index_path.exists());

    let index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).unwrap()).unwrap();

    // The scaffolded "Hello World" post should appear
    let arr = index.as_array().unwrap();
    assert!(!arr.is_empty());
    let titles: Vec<&str> = arr
        .iter()
        .map(|e| e["title"].as_str().unwrap_or(""))
        .collect();
    assert!(titles.contains(&"Hello World"), "expected Hello World in search index");
}

#[test]
fn test_build_search_index_schema() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Schema Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let raw = fs::read_to_string(site_dir.join("dist/search-index.json")).unwrap();
    let index: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let entry = &index[0];

    // All required fields must be present
    assert!(entry.get("title").is_some());
    assert!(entry.get("url").is_some());
    assert!(entry.get("collection").is_some());
    assert!(entry.get("tags").is_some());
    assert!(entry.get("lang").is_some());
}

#[test]
fn test_build_search_index_excludes_drafts() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Draft Search Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create a draft post
    let draft =
        "---\ntitle: Secret Draft\ndate: 2025-01-01\ndraft: true\n---\n\nNot published.\n";
    fs::write(
        site_dir.join("content/posts/2025-01-01-secret.md"),
        draft,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let raw = fs::read_to_string(site_dir.join("dist/search-index.json")).unwrap();
    assert!(
        !raw.contains("Secret Draft"),
        "draft post should not appear in search index"
    );
}

#[test]
fn test_build_search_index_multilingual() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Multilingual Search", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Búsqueda multilingüe");

    let es_post = "---\ntitle: Hola Mundo\ndate: 2025-01-15\n---\n\nContenido.\n";
    fs::write(
        site_dir.join("content/posts/2025-01-15-hola-mundo.es.md"),
        es_post,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default language index at root
    let root_index_path = site_dir.join("dist/search-index.json");
    assert!(root_index_path.exists());
    let root_raw = fs::read_to_string(&root_index_path).unwrap();
    assert!(root_raw.contains("Hello World"));
    assert!(!root_raw.contains("Hola Mundo"));

    // Per-language index for Spanish
    let es_index_path = site_dir.join("dist/es/search-index.json");
    assert!(es_index_path.exists());
    let es_raw = fs::read_to_string(&es_index_path).unwrap();
    assert!(es_raw.contains("Hola Mundo"));
    assert!(!es_raw.contains("Hello World"));
}

// --- pagination ---

/// Helper: add `paginate = N` to the [[collections]] entry for `collection_name` in page.toml.
fn add_pagination(site_dir: &std::path::Path, collection_name: &str, page_size: usize) {
    let toml_path = site_dir.join("page.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    // Append paginate to the matching collection block by rewriting the file.
    // Simple approach: find the collection section and append paginate after its last field.
    let updated = config.replace(
        &format!("name = \"{collection_name}\""),
        &format!("name = \"{collection_name}\"\npaginate = {page_size}"),
    );
    fs::write(&toml_path, updated).unwrap();
}

/// Helper: create a dated post file in a site's posts directory.
fn create_post(site_dir: &std::path::Path, date: &str, slug: &str, title: &str) {
    let content = format!(
        "---\ntitle: \"{title}\"\ndate: {date}\n---\n\nContent of {title}.\n"
    );
    fs::write(
        site_dir.join(format!("content/posts/{date}-{slug}.md")),
        content,
    )
    .unwrap();
}

#[test]
fn test_build_pagination_generates_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Paginate Test", "posts");
    let site_dir = tmp.path().join("site");

    // Create 5 posts with paginate = 2 → 3 pages (2, 2, 1 items)
    for i in 1..=5 {
        create_post(&site_dir, &format!("2025-01-{:02}", i), &format!("post-{i}"), &format!("Post {i}"));
    }
    add_pagination(&site_dir, "posts", 2);

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Page 1: dist/posts/index.html
    assert!(site_dir.join("dist/posts/index.html").exists());
    // Page 2: dist/posts/page/2/index.html
    assert!(site_dir.join("dist/posts/page/2/index.html").exists());
    // Page 3: dist/posts/page/3/index.html
    assert!(site_dir.join("dist/posts/page/3/index.html").exists());
    // No page 4 (only 5 items / 2 per page = 3 pages)
    assert!(!site_dir.join("dist/posts/page/4/index.html").exists());
}

#[test]
fn test_build_pagination_context_in_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Paginate Context Test", "posts");
    let site_dir = tmp.path().join("site");

    for i in 1..=4 {
        create_post(&site_dir, &format!("2025-02-{:02}", i), &format!("post-{i}"), &format!("Post {i}"));
    }
    add_pagination(&site_dir, "posts", 2);

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Page 2 should contain pagination nav (4 items, 2 per page → 2 pages)
    let page2 = fs::read_to_string(site_dir.join("dist/posts/page/2/index.html")).unwrap();
    // prev_url points back to page 1 (Tera HTML-encodes / as &#x2F;)
    assert!(
        page2.contains("&#x2F;posts&#x2F;") || page2.contains("/posts/"),
        "page 2 should link back to page 1"
    );
    assert!(page2.contains("Page 2 of"), "page 2 should show page count in pagination nav");
    // Page 2 is not the last page (init creates a hello-world post, so 5 total → 3 pages)
    assert!(page2.contains("class=\"pagination\""), "page 2 should have pagination nav");
}

#[test]
fn test_build_no_pagination_without_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Paginate Test", "posts");
    let site_dir = tmp.path().join("site");

    for i in 1..=5 {
        create_post(&site_dir, &format!("2025-03-{:02}", i), &format!("post-{i}"), &format!("Post {i}"));
    }
    // No add_pagination call

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Without paginate config, no paginated collection index is generated
    assert!(!site_dir.join("dist/posts/page").exists());
}

// --- asset pipeline ---

/// Helper: write a CSS file into a site's static directory.
fn write_static_css(site_dir: &std::path::Path, name: &str, content: &str) {
    fs::create_dir_all(site_dir.join("static")).unwrap();
    fs::write(site_dir.join("static").join(name), content).unwrap();
}

/// Helper: append lines to page.toml [build] section.
fn set_build_option(site_dir: &std::path::Path, key: &str, value: &str) {
    let toml_path = site_dir.join("page.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config = config.replace(
        "[build]",
        &format!("[build]\n{key} = {value}"),
    );
    fs::write(&toml_path, config).unwrap();
}

#[test]
fn test_build_minify_css() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Minify Test", "posts");
    let site_dir = tmp.path().join("site");

    write_static_css(
        &site_dir,
        "style.css",
        "/* header styles */\nbody {\n    color : red ;\n    background : blue ;\n}\n",
    );
    set_build_option(&site_dir, "minify", "true");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let css = fs::read_to_string(site_dir.join("dist/static/style.css")).unwrap();
    assert!(!css.contains("/* header"), "minified CSS should not contain comments");
    assert!(!css.contains("    "), "minified CSS should not have indentation");
    assert!(css.contains("color:red"), "minified CSS should collapse whitespace around colon");
}

#[test]
fn test_build_fingerprint_writes_manifest() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Fingerprint Test", "posts");
    let site_dir = tmp.path().join("site");

    write_static_css(&site_dir, "main.css", "body { color: red; }");
    set_build_option(&site_dir, "fingerprint", "true");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Manifest must exist
    let manifest_path = site_dir.join("dist/asset-manifest.json");
    assert!(manifest_path.exists(), "asset-manifest.json should be generated");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    assert!(manifest.contains("/static/main.css"), "manifest should map original path");
    // Fingerprinted file should exist alongside original
    let dist_static = site_dir.join("dist/static");
    let has_fingerprinted = fs::read_dir(&dist_static)
        .unwrap()
        .any(|e| {
            e.ok()
                .and_then(|e| e.file_name().into_string().ok())
                .map(|n| n.starts_with("main.") && n.ends_with(".css") && n != "main.css")
                .unwrap_or(false)
        });
    assert!(has_fingerprinted, "fingerprinted CSS file should exist in dist/static");
}

#[test]
fn test_build_no_minify_by_default() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Minify Test", "posts");
    let site_dir = tmp.path().join("site");

    let original_css = "/* keep this comment */\nbody {\n    color: red;\n}\n";
    write_static_css(&site_dir, "style.css", original_css);
    // No set_build_option — minify defaults to false

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let css = fs::read_to_string(site_dir.join("dist/static/style.css")).unwrap();
    assert!(css.contains("/* keep"), "CSS should be unmodified when minify is false");
}
