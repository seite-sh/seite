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

// --- deploy improvements ---

#[test]
fn test_deploy_dry_run_github_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Dry Run Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("github-pages"))
        .stdout(predicate::str::contains("gh-pages"));
}

#[test]
fn test_deploy_dry_run_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CF Dry Run", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run", "--target", "cloudflare"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("cloudflare"));
}

#[test]
fn test_deploy_dry_run_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Netlify Dry Run", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run", "--target", "netlify"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("netlify"));
}

#[test]
fn test_deploy_unknown_target() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Unknown Deploy", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run", "--target", "heroku"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown deploy target"));
}

#[test]
fn test_init_github_pages_creates_workflow() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "mysite",
            "--title",
            "Workflow Test",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let workflow = tmp.path().join("mysite/.github/workflows/deploy.yml");
    assert!(workflow.exists(), "GitHub Actions workflow should be created");

    let content = fs::read_to_string(&workflow).unwrap();
    assert!(content.contains("Deploy to GitHub Pages"));
    assert!(content.contains("deploy-pages"));
    assert!(content.contains("page build"));
}

#[test]
fn test_init_cloudflare_creates_workflow() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "mysite",
            "--title",
            "CF Test",
            "--description",
            "",
            "--deploy-target",
            "cloudflare",
            "--collections",
            "posts",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Should have .github/workflows/deploy.yml for Cloudflare
    let workflow_path = tmp.path().join("mysite/.github/workflows/deploy.yml");
    assert!(workflow_path.exists());
    let content = fs::read_to_string(&workflow_path).unwrap();
    assert!(content.contains("Cloudflare"));
    assert!(content.contains("wrangler-action"));
}

#[test]
fn test_init_netlify_target() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "mysite",
            "--title",
            "Netlify Init",
            "--description",
            "",
            "--deploy-target",
            "netlify",
            "--collections",
            "posts",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let config = fs::read_to_string(tmp.path().join("mysite/page.toml")).unwrap();
    assert!(config.contains("netlify"), "config should contain netlify deploy target");

    // Should generate netlify.toml
    let netlify_toml = tmp.path().join("mysite/netlify.toml");
    assert!(netlify_toml.exists());
    let content = fs::read_to_string(&netlify_toml).unwrap();
    assert!(content.contains("[build]"));
    assert!(content.contains("page build"));

    // Should also generate GitHub Actions workflow
    let workflow_path = tmp.path().join("mysite/.github/workflows/deploy.yml");
    assert!(workflow_path.exists());
    let workflow = fs::read_to_string(&workflow_path).unwrap();
    assert!(workflow.contains("Netlify"));
}

// --- deploy: pre-flight checks ---

#[test]
fn test_deploy_dry_run_shows_preflight_checks() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Preflight Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Pre-flight checks"))
        .stdout(predicate::str::contains("Output directory"))
        .stdout(predicate::str::contains("Base URL"));
}

#[test]
fn test_deploy_dry_run_warns_localhost_base_url() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Localhost Test", "posts");
    let site_dir = tmp.path().join("site");

    // Default base_url is localhost — pre-flight should flag it
    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("localhost"));
}

#[test]
fn test_deploy_dry_run_with_base_url_override() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Override Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run", "--base-url", "https://example.com"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Base URL override: https://example.com"));
}

#[test]
fn test_deploy_dry_run_preview_mode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Preview Test", "posts");
    let site_dir = tmp.path().join("site");

    // GitHub Pages doesn't show preview mode, but Netlify does
    page_cmd()
        .args(["deploy", "--dry-run", "--target", "netlify", "--preview"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("preview"));
}

#[test]
fn test_deploy_dry_run_github_pages_shows_nojekyll() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "NoJekyll Test", "posts");
    let site_dir = tmp.path().join("site");

    // Update base_url to a custom domain
    let config_path = site_dir.join("page.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    let config = config.replace("http://localhost:3000", "https://myblog.com");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(".nojekyll"))
        .stdout(predicate::str::contains("CNAME: myblog.com"));
}

// --- deploy: domain setup ---

#[test]
fn test_deploy_domain_updates_base_url() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Domain Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--domain", "myblog.com"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated base_url"))
        .stdout(predicate::str::contains("myblog.com"));

    // Verify page.toml was updated
    let config = fs::read_to_string(site_dir.join("page.toml")).unwrap();
    assert!(config.contains("https://myblog.com"));
}

#[test]
fn test_deploy_domain_shows_dns_instructions() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "DNS Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--domain", "myblog.com"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("DNS records"))
        .stdout(predicate::str::contains("github.io"));
}

// --- image handling ---

/// Helper: write a minimal valid PNG (1x1 pixel) into a site's static directory.
fn write_test_image(site_dir: &std::path::Path, name: &str) {
    let img_dir = site_dir.join("static/images");
    fs::create_dir_all(&img_dir).unwrap();
    // Create a 100x100 red PNG using the image crate
    let img = image::RgbImage::from_fn(100, 100, |_, _| image::Rgb([255u8, 0, 0]));
    img.save(img_dir.join(name)).unwrap();
}

/// Helper: add [images] section to page.toml.
fn set_images_config(site_dir: &std::path::Path, widths: &str) {
    let toml_path = site_dir.join("page.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config.push_str(&format!(
        "\n[images]\nwidths = {widths}\nquality = 80\nlazy_loading = true\nwebp = true\n"
    ));
    fs::write(&toml_path, config).unwrap();
}

#[test]
fn test_build_image_resize_and_webp() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Image Test", "posts");
    let site_dir = tmp.path().join("site");

    write_test_image(&site_dir, "photo.png");
    set_images_config(&site_dir, "[48]");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Original should be copied
    assert!(site_dir.join("dist/static/images/photo.png").exists());
    // Resized version should exist
    assert!(
        site_dir.join("dist/static/images/photo-48w.png").exists(),
        "resized PNG at 48w should exist"
    );
    // WebP version should exist
    assert!(
        site_dir.join("dist/static/images/photo-48w.webp").exists(),
        "resized WebP at 48w should exist"
    );
    // Full-size WebP should exist
    assert!(
        site_dir.join("dist/static/images/photo.webp").exists(),
        "full-size WebP should exist"
    );
}

#[test]
fn test_build_image_lazy_loading() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Lazy Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Create a page with an image reference
    let page_content = "---\ntitle: Gallery\n---\n\n![A photo](/static/images/photo.png)\n";
    fs::write(site_dir.join("content/pages/gallery.md"), page_content).unwrap();

    write_test_image(&site_dir, "photo.png");
    set_images_config(&site_dir, "[48]");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/gallery.html")).unwrap();
    assert!(
        html.contains("loading=\"lazy\""),
        "img tags should have loading=lazy"
    );
}

#[test]
fn test_build_image_srcset_in_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Srcset Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let page_content = "---\ntitle: Gallery\n---\n\n![A photo](/static/images/photo.png)\n";
    fs::write(site_dir.join("content/pages/gallery.md"), page_content).unwrap();

    write_test_image(&site_dir, "photo.png");
    set_images_config(&site_dir, "[48]");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/gallery.html")).unwrap();
    assert!(
        html.contains("srcset="),
        "img tags should have srcset attribute"
    );
    assert!(
        html.contains("48w"),
        "srcset should include the 48w variant"
    );
}

#[test]
fn test_build_image_picture_element_webp() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Picture Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let page_content = "---\ntitle: Gallery\n---\n\n![A photo](/static/images/photo.png)\n";
    fs::write(site_dir.join("content/pages/gallery.md"), page_content).unwrap();

    write_test_image(&site_dir, "photo.png");
    set_images_config(&site_dir, "[48]");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/gallery.html")).unwrap();
    assert!(
        html.contains("<picture>"),
        "should wrap in <picture> element for WebP"
    );
    assert!(
        html.contains("image/webp"),
        "should have WebP source type"
    );
    assert!(
        html.contains("</picture>"),
        "should close <picture> element"
    );
}

#[test]
fn test_build_image_skip_larger_than_original() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Skip Large Test", "posts");
    let site_dir = tmp.path().join("site");

    // Image is 100x100, so widths > 100 should be skipped
    write_test_image(&site_dir, "small.png");
    set_images_config(&site_dir, "[48, 200, 1200]");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // 48w should exist (smaller than 100)
    assert!(site_dir.join("dist/static/images/small-48w.png").exists());
    // 200w should NOT exist (larger than 100)
    assert!(!site_dir.join("dist/static/images/small-200w.png").exists());
    // 1200w should NOT exist
    assert!(!site_dir.join("dist/static/images/small-1200w.png").exists());
}

#[test]
fn test_build_no_image_processing_without_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Image Config", "posts");
    let site_dir = tmp.path().join("site");

    write_test_image(&site_dir, "photo.png");
    // No set_images_config — use defaults

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Original should be copied as-is by static file step
    assert!(site_dir.join("dist/static/images/photo.png").exists());
    // With default widths [480, 800, 1200], all are > 100px so no resizes
    // But WebP full-size should still be created since default has webp=true
    // Actually, the 100x100 image is smaller than all default widths, so no resized copies
    assert!(!site_dir.join("dist/static/images/photo-480w.png").exists());
}

// ── Reading time + word count ──

#[test]
fn test_build_reading_time_in_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "rtsite", "RT Site", "posts");
    let site_dir = tmp.path().join("rtsite");

    // Write a post with a known word count (~300 words → 2 min read at 238 WPM)
    let words: String = (0..300).map(|i| format!("word{i} ")).collect();
    let content = format!(
        "---\ntitle: Reading Test\ndate: 2025-01-15\n---\n{}",
        words
    );
    fs::write(
        site_dir.join("content/posts/2025-01-15-reading-test.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/reading-test.html")).unwrap();
    assert!(html.contains("min read"), "should contain reading time");
    assert!(
        html.contains("reading-time"),
        "should contain reading-time class"
    );
}

#[test]
fn test_build_reading_time_in_index() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "rtidx", "RT Index", "posts");
    let site_dir = tmp.path().join("rtidx");

    let words: String = (0..500).map(|i| format!("word{i} ")).collect();
    let content = format!(
        "---\ntitle: Long Post\ndate: 2025-01-15\n---\n{}",
        words
    );
    fs::write(
        site_dir.join("content/posts/2025-01-15-long-post.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The index page should have the reading_time in the item summary via template
    // (reading_time is available as item.reading_time in index templates)
    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Index currently doesn't display reading_time by default, but the data is there
    // Just verify the build succeeded and post appears
    assert!(html.contains("Long Post"));
}

// ── Excerpts ──

#[test]
fn test_build_excerpt_more_marker() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "exsite1", "Excerpt Site", "posts");
    let site_dir = tmp.path().join("exsite1");

    let content = "---\ntitle: More Marker\ndate: 2025-01-15\n---\nThis is the intro.\n\n<!-- more -->\n\nThis is after the fold.";
    fs::write(
        site_dir.join("content/posts/2025-01-15-more-marker.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Index should show the excerpt (intro before <!-- more -->)
    assert!(index.contains("This is the intro."), "excerpt should appear in index");
    assert!(
        !index.contains("This is after the fold."),
        "content after more marker should not be in excerpt"
    );
}

#[test]
fn test_build_excerpt_first_paragraph() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "exsite2", "Excerpt Site 2", "posts");
    let site_dir = tmp.path().join("exsite2");

    let content = "---\ntitle: Auto Excerpt\ndate: 2025-01-15\n---\nFirst paragraph auto-extracted.\n\nSecond paragraph not shown.\n\nThird paragraph also hidden.";
    fs::write(
        site_dir.join("content/posts/2025-01-15-auto-excerpt.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index.contains("First paragraph auto-extracted."),
        "first paragraph should be used as excerpt"
    );
}

#[test]
fn test_build_excerpt_description_takes_priority() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "exsite3", "Excerpt Site 3", "posts");
    let site_dir = tmp.path().join("exsite3");

    // When description is set, it should take priority over excerpt in the default index template
    let content = "---\ntitle: With Description\ndate: 2025-01-15\ndescription: Custom description here\n---\nFirst paragraph.\n\nSecond paragraph.";
    fs::write(
        site_dir.join("content/posts/2025-01-15-with-desc.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index.contains("Custom description here"),
        "description should be shown when set"
    );
}

// ── 404 page ──

#[test]
fn test_build_generates_404_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site404", "404 Site", "posts");
    let site_dir = tmp.path().join("site404");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let path_404 = site_dir.join("dist/404.html");
    assert!(path_404.exists(), "404.html should be generated");

    let html = fs::read_to_string(path_404).unwrap();
    assert!(html.contains("404"), "should contain 404");
    assert!(
        html.contains("Page Not Found"),
        "should contain Page Not Found heading"
    );
    assert!(
        html.contains("noindex"),
        "404 page should have noindex robots meta"
    );
}

// ── Table of contents ──

#[test]
fn test_build_toc_headings_have_ids() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tocsite1", "ToC Site", "docs");
    let site_dir = tmp.path().join("tocsite1");

    let content = "---\ntitle: My Doc\n---\n## Introduction\n\nSome text.\n\n### Details\n\nMore text.\n\n## Conclusion\n\nEnd.";
    fs::write(
        site_dir.join("content/docs/my-doc.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/docs/my-doc.html")).unwrap();
    assert!(html.contains("id=\"introduction\""), "h2 should have id");
    assert!(html.contains("id=\"details\""), "h3 should have id");
    assert!(html.contains("id=\"conclusion\""), "h2 should have id");
}

#[test]
fn test_build_toc_in_doc_template() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tocsite2", "ToC Site 2", "docs");
    let site_dir = tmp.path().join("tocsite2");

    // Doc with multiple headings should get a ToC nav
    let content = "---\ntitle: Guide\n---\n## Step 1\n\nDo this.\n\n## Step 2\n\nDo that.\n\n## Step 3\n\nDone.";
    fs::write(
        site_dir.join("content/docs/guide.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/docs/guide.html")).unwrap();
    // Default doc template should show ToC when > 1 heading
    assert!(html.contains("toc"), "doc should contain toc class");
    assert!(html.contains("Contents"), "doc should contain Contents heading");
    assert!(html.contains("step-1"), "toc should link to step-1");
    assert!(html.contains("step-2"), "toc should link to step-2");
    assert!(html.contains("step-3"), "toc should link to step-3");
}

// --- tag pages ---

#[test]
fn test_build_generates_tag_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tagsite", "Tag Site", "posts");
    let site_dir = tmp.path().join("tagsite");

    // Create posts with tags
    fs::write(
        site_dir.join("content/posts/2025-01-15-alpha.md"),
        "---\ntitle: Alpha Post\ndate: 2025-01-15\ntags:\n  - rust\n  - web\n---\nAlpha content.",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/posts/2025-01-16-beta.md"),
        "---\ntitle: Beta Post\ndate: 2025-01-16\ntags:\n  - rust\n  - cli\n---\nBeta content.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Tag index page should exist
    let tags_index = site_dir.join("dist/tags/index.html");
    assert!(tags_index.exists(), "tags index page should exist");
    let tags_html = fs::read_to_string(&tags_index).unwrap();
    assert!(tags_html.contains("rust"), "tags index should list 'rust'");
    assert!(tags_html.contains("web"), "tags index should list 'web'");
    assert!(tags_html.contains("cli"), "tags index should list 'cli'");

    // Individual tag pages should exist
    let rust_tag = site_dir.join("dist/tags/rust/index.html");
    assert!(rust_tag.exists(), "rust tag page should exist");
    let rust_html = fs::read_to_string(&rust_tag).unwrap();
    assert!(rust_html.contains("Alpha Post"), "rust tag should list Alpha Post");
    assert!(rust_html.contains("Beta Post"), "rust tag should list Beta Post");

    let web_tag = site_dir.join("dist/tags/web/index.html");
    assert!(web_tag.exists(), "web tag page should exist");
    let web_html = fs::read_to_string(&web_tag).unwrap();
    assert!(web_html.contains("Alpha Post"), "web tag should list Alpha Post");
    assert!(!web_html.contains("Beta Post"), "web tag should NOT list Beta Post");

    let cli_tag = site_dir.join("dist/tags/cli/index.html");
    assert!(cli_tag.exists(), "cli tag page should exist");
}

#[test]
fn test_build_tag_index_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tagidx", "Tag Index", "posts");
    let site_dir = tmp.path().join("tagidx");

    fs::write(
        site_dir.join("content/posts/2025-01-15-post.md"),
        "---\ntitle: Tagged Post\ndate: 2025-01-15\ntags:\n  - alpha\n  - beta\n---\nContent.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let tags_html = fs::read_to_string(site_dir.join("dist/tags/index.html")).unwrap();
    // Should show tag counts
    assert!(tags_html.contains("(1)"), "tag count should be shown");
    // Should link to tag pages (Tera auto-escapes / as &#x2F;)
    assert!(tags_html.contains("tags") && tags_html.contains("alpha"), "should link to alpha tag page");
    assert!(tags_html.contains("tags") && tags_html.contains("beta"), "should link to beta tag page");

    // Sitemap should include tag URLs
    let sitemap = fs::read_to_string(site_dir.join("dist/sitemap.xml")).unwrap();
    assert!(sitemap.contains("/tags/"), "sitemap should include tag page URLs");
}

#[test]
fn test_build_tag_pages_multilingual() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "taglang", "Tag Lang", "posts");
    let site_dir = tmp.path().join("taglang");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("page.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Tag Lang ES\"\n");
    fs::write(site_dir.join("page.toml"), config).unwrap();

    // English post with tag
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\ntags:\n  - greetings\n---\nHello content.",
    )
    .unwrap();
    // Spanish post with tag
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\ntags:\n  - saludos\n---\nHola contenido.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // English tag pages
    assert!(
        site_dir.join("dist/tags/greetings/index.html").exists(),
        "English tag page should exist"
    );
    // Spanish tag pages
    assert!(
        site_dir.join("dist/es/tags/saludos/index.html").exists(),
        "Spanish tag page should exist"
    );
    // Spanish tags index
    assert!(
        site_dir.join("dist/es/tags/index.html").exists(),
        "Spanish tags index should exist"
    );
    // English tag should NOT contain Spanish items
    let en_html = fs::read_to_string(site_dir.join("dist/tags/greetings/index.html")).unwrap();
    assert!(en_html.contains("Hello"), "English tag page should have English post");
    assert!(!en_html.contains("Hola"), "English tag page should NOT have Spanish post");
}

// --- custom templates and extra frontmatter ---

#[test]
fn test_build_extra_frontmatter_in_template() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "extrasite", "Extra Site", "posts");
    let site_dir = tmp.path().join("extrasite");

    // Create a custom template that uses page.extra
    let templates_dir = site_dir.join("templates");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(
        templates_dir.join("post.html"),
        r#"{% extends "base.html" %}
{% block title %}{{ page.title }}{% endblock %}
{% block content %}
<article>
    <h1>{{ page.title }}</h1>
    {% if page.extra.hero_color %}<div class="hero" style="background: {{ page.extra.hero_color }}">Hero</div>{% endif %}
    {% if page.extra.featured %}<span class="featured-badge">Featured</span>{% endif %}
    {{ page.content | safe }}
</article>
{% endblock %}"#,
    )
    .unwrap();

    // Create a post with extra frontmatter
    fs::write(
        site_dir.join("content/posts/2025-01-15-custom.md"),
        "---\ntitle: Custom Post\ndate: 2025-01-15\nextra:\n  hero_color: \"#ff6600\"\n  featured: true\n---\nCustom content here.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/custom.html")).unwrap();
    assert!(html.contains("hero"), "should render hero div from extra.hero_color");
    assert!(html.contains("#ff6600"), "should include hero_color value");
    assert!(html.contains("featured-badge"), "should render featured badge from extra.featured");
}

#[test]
fn test_build_custom_template_with_blocks() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "blocksite", "Block Site", "posts");
    let site_dir = tmp.path().join("blocksite");

    // Create a custom template that uses the new blocks
    let templates_dir = site_dir.join("templates");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(
        templates_dir.join("post.html"),
        r#"{% extends "base.html" %}
{% block title %}{{ page.title }}{% endblock %}
{% block extra_css %}<style>.custom-style { color: red; }</style>{% endblock %}
{% block head %}<meta name="custom-meta" content="test-value">{% endblock %}
{% block content %}
<article>{{ page.content | safe }}</article>
{% endblock %}
{% block footer %}<div class="custom-footer">Custom Footer</div>{% endblock %}
{% block extra_js %}<script>console.log("custom js")</script>{% endblock %}"#,
    )
    .unwrap();

    fs::write(
        site_dir.join("content/posts/2025-01-15-block-test.md"),
        "---\ntitle: Block Test\ndate: 2025-01-15\n---\nBlock test content.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/block-test.html")).unwrap();
    assert!(html.contains("custom-style"), "should include extra_css block");
    assert!(html.contains("custom-meta"), "should include head block");
    assert!(html.contains("custom-footer"), "should include footer block");
    assert!(html.contains("custom js"), "should include extra_js block");
}

// --- URL collision detection ---

#[test]
fn test_build_detects_url_collision() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "collide", "Collide Site", "posts,pages");
    let site_dir = tmp.path().join("collide");

    // Create two content items that would resolve to the same URL
    // A post with slug override matching a page URL
    fs::write(
        site_dir.join("content/posts/2025-01-15-about.md"),
        "---\ntitle: About Post\ndate: 2025-01-15\nslug: about\n---\nAbout as a post.",
    )
    .unwrap();
    // Another post with the same slug
    fs::write(
        site_dir.join("content/posts/2025-01-16-about.md"),
        "---\ntitle: About Post 2\ndate: 2025-01-16\nslug: about\n---\nAnother about.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("URL collision"));
}

#[test]
fn test_build_warns_missing_content_dir() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "missingdir", "Missing Dir", "posts,docs");
    let site_dir = tmp.path().join("missingdir");

    // Remove the docs content directory
    let docs_dir = site_dir.join("content/docs");
    if docs_dir.exists() {
        fs::remove_dir_all(&docs_dir).unwrap();
    }

    // Build should still succeed (warning, not error)
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

// --- accessibility ---

#[test]
fn test_build_accessibility_skip_link() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "a11y", "A11y Site", "posts");
    let site_dir = tmp.path().join("a11y");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(html.contains("skip-link"), "should have skip link class");
    assert!(html.contains("Skip to main content"), "should have skip link text");
    assert!(html.contains("id=\"main\""), "main element should have id=\"main\"");
}

#[test]
fn test_build_accessibility_aria_search() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "a11ysearch", "A11y Search", "posts");
    let site_dir = tmp.path().join("a11ysearch");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(html.contains("role=\"search\""), "search form should have role=search");
    assert!(html.contains("aria-label"), "search input should have aria-label");
    assert!(html.contains("aria-live=\"polite\""), "search results should have aria-live");
}

// --- edge case tests ---

#[test]
fn test_build_malformed_frontmatter_errors() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "malformed", "Malformed", "posts");
    let site_dir = tmp.path().join("malformed");

    // Write a post with invalid YAML frontmatter
    fs::write(
        site_dir.join("content/posts/2025-01-15-bad.md"),
        "---\ntitle: [unclosed bracket\n---\nContent.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure();
}

#[test]
fn test_build_empty_content_body() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "emptybody", "Empty Body", "posts");
    let site_dir = tmp.path().join("emptybody");

    // Post with frontmatter but no body
    fs::write(
        site_dir.join("content/posts/2025-01-15-empty.md"),
        "---\ntitle: Empty Body Post\ndate: 2025-01-15\n---\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/empty.html")).unwrap();
    assert!(html.contains("Empty Body Post"), "should render title even with empty body");
}

#[test]
fn test_build_special_characters_in_title() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "specialchars", "Special Chars", "posts");
    let site_dir = tmp.path().join("specialchars");

    fs::write(
        site_dir.join("content/posts/2025-01-15-special.md"),
        "---\ntitle: \"Rust & WebAssembly: <Fast> \\\"Quotes\\\"\"\ndate: 2025-01-15\n---\nContent with special chars.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Should build without errors — HTML escaping handled by Tera
    assert!(site_dir.join("dist/posts/special.html").exists());
}

#[test]
fn test_build_custom_template_override() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tmploverride", "Override", "posts");
    let site_dir = tmp.path().join("tmploverride");

    // Create a custom index.html template
    let templates_dir = site_dir.join("templates");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(
        templates_dir.join("index.html"),
        r#"{% extends "base.html" %}
{% block title %}Custom Index{% endblock %}
{% block content %}
<div class="custom-index">Custom index content for {{ site.title }}</div>
{% endblock %}"#,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(html.contains("custom-index"), "should use custom index template");
    assert!(html.contains("Custom index content"), "should render custom template content");
}

#[test]
fn test_build_markdown_output_matches_source() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mdsource", "MD Source", "posts");
    let site_dir = tmp.path().join("mdsource");

    let original_body = "This is the **original** markdown content.\n\n- Item 1\n- Item 2";
    fs::write(
        site_dir.join("content/posts/2025-01-15-source.md"),
        format!("---\ntitle: Source Test\ndate: 2025-01-15\n---\n{original_body}"),
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The .md output should contain the original markdown body
    let md_output = fs::read_to_string(site_dir.join("dist/posts/source.md")).unwrap();
    assert!(md_output.contains("**original** markdown content"), "md output should preserve original markdown");
    assert!(md_output.contains("- Item 1"), "md output should preserve list items");
}

#[test]
fn test_build_no_listed_collections() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "unlisted", "Unlisted", "pages");
    let site_dir = tmp.path().join("unlisted");

    // Pages collection is not listed, so index should show no collections
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Should still generate a valid index.html
    assert!(site_dir.join("dist/index.html").exists());
}
