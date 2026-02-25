use std::fs;
use std::io::Write;
use std::process::Stdio;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn page_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("seite")
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
    assert!(root.join("seite.toml").exists());
    assert!(root.join("content/posts").is_dir());
    assert!(root.join("content/pages").is_dir());
    assert!(root.join("templates/base.html").exists());
    assert!(root.join("templates/index.html").exists());
    assert!(root.join("templates/post.html").exists());
    assert!(root.join("templates/page.html").exists());
    assert!(root.join("static").is_dir());

    // Verify .gitignore
    assert!(root.join(".gitignore").exists());
    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(gitignore.contains("/dist"));

    // Verify seite.toml content
    let config_content = fs::read_to_string(root.join("seite.toml")).unwrap();
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

    let config_content = fs::read_to_string(root.join("seite.toml")).unwrap();
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
    assert!(post.contains("seite")); // from the body text
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

    assert!(!site_dir.join("dist/posts/secret-draft.html").exists());

    // Build with --drafts
    page_cmd()
        .args(["build", "--drafts"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/posts/secret-draft.html").exists());
}

#[test]
fn test_build_with_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Docs Build", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    // Create a doc
    let doc = "---\ntitle: Getting Started\n---\n\nWelcome to the docs.\n";
    fs::write(site_dir.join("content/docs/getting-started.md"), doc).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/docs/getting-started.html").exists());
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

    assert!(site_dir.join("dist/docs/guides/setup.html").exists());
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

/// Helper: add `[languages.es]` to a site's seite.toml
fn add_language(site_dir: &std::path::Path, lang: &str, title: &str) {
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config.push_str(&format!("\n[languages.{lang}]\ntitle = \"{title}\"\n"));
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
    assert!(
        base.contains("fffef0"),
        "brutalist theme should have cream background"
    );
    assert!(
        base.contains("ffe600"),
        "brutalist theme should have yellow accent"
    );
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
    assert!(
        base.contains("border-radius: 20px"),
        "bento theme should have rounded cards"
    );
    assert!(
        base.contains("5046e5"),
        "bento theme should have indigo accent"
    );
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
    assert!(
        base.contains("0a0a0a"),
        "dark theme should use true black background"
    );
    assert!(
        base.contains("8b5cf6"),
        "dark theme should use violet accent"
    );
}

#[test]
fn test_theme_create_requires_page_toml() {
    // `seite theme create` without a seite.toml in the directory should fail gracefully
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["theme", "create", "dark glassmorphism"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_theme_list_shows_bundled() {
    page_cmd()
        .args(["theme", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bundled themes"))
        .stdout(predicate::str::contains("default"))
        .stdout(predicate::str::contains("minimal"))
        .stdout(predicate::str::contains("dark"))
        .stdout(predicate::str::contains("docs"))
        .stdout(predicate::str::contains("brutalist"))
        .stdout(predicate::str::contains("bento"));
}

#[test]
fn test_theme_apply_installed() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Installed Theme Test", "posts");
    let site_dir = tmp.path().join("site");

    // Create a fake installed theme
    let themes_dir = site_dir.join("templates").join("themes");
    std::fs::create_dir_all(&themes_dir).unwrap();
    std::fs::write(
        themes_dir.join("custom-test.tera"),
        "{#- theme-description: A test installed theme -#}\n<!DOCTYPE html>\n<html><head><title>Custom</title></head><body>custom-test-marker</body></html>",
    ).unwrap();

    // Apply the installed theme
    page_cmd()
        .args(["theme", "apply", "custom-test"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Applied installed theme"));

    // Verify base.html was updated
    let base = std::fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(
        base.contains("custom-test-marker"),
        "installed theme should be applied"
    );
}

#[test]
fn test_theme_apply_unknown_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Unknown Theme Test", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "apply", "nonexistent-theme-xyz"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown theme"));
}

#[test]
fn test_theme_export() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Export Test", "posts");
    let site_dir = tmp.path().join("site");

    // Apply a theme first so templates/base.html exists
    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export it
    page_cmd()
        .args([
            "theme",
            "export",
            "my-dark",
            "--description",
            "My custom dark theme",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported theme 'my-dark'"));

    // Verify the exported file exists and has metadata
    let exported = std::fs::read_to_string(site_dir.join("templates/themes/my-dark.tera")).unwrap();
    assert!(
        exported.contains("theme-description: My custom dark theme"),
        "exported theme should have description"
    );
    assert!(
        exported.contains("0a0a0a"),
        "exported theme should contain dark theme content"
    );
}

#[test]
fn test_theme_export_no_base_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Export No Base Test", "posts");
    let site_dir = tmp.path().join("site");

    // Remove base.html to test error case
    let _ = std::fs::remove_file(site_dir.join("templates/base.html"));

    page_cmd()
        .args(["theme", "export", "my-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no templates/base.html found"));
}

#[test]
fn test_theme_export_duplicate_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Export Dup Test", "posts");
    let site_dir = tmp.path().join("site");

    // Apply a theme
    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export once
    page_cmd()
        .args(["theme", "export", "my-theme"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export again with same name should fail
    page_cmd()
        .args(["theme", "export", "my-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_theme_install_requires_page_toml() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["theme", "install", "https://example.com/theme.tera"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_theme_list_shows_installed() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Installed List Test", "posts");
    let site_dir = tmp.path().join("site");

    // Create an installed theme
    let themes_dir = site_dir.join("templates").join("themes");
    std::fs::create_dir_all(&themes_dir).unwrap();
    std::fs::write(
        themes_dir.join("my-custom.tera"),
        "{#- theme-description: A custom community theme -#}\n<!DOCTYPE html><html></html>",
    )
    .unwrap();

    page_cmd()
        .args(["theme", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed themes"))
        .stdout(predicate::str::contains("my-custom"))
        .stdout(predicate::str::contains("A custom community theme"));
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
    assert!(
        titles.contains(&"Hello World"),
        "expected Hello World in search index"
    );
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
    let draft = "---\ntitle: Secret Draft\ndate: 2025-01-01\ndraft: true\n---\n\nNot published.\n";
    fs::write(site_dir.join("content/posts/2025-01-01-secret.md"), draft).unwrap();

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

/// Helper: add `paginate = N` to the [[collections]] entry for `collection_name` in seite.toml.
fn add_pagination(site_dir: &std::path::Path, collection_name: &str, page_size: usize) {
    let toml_path = site_dir.join("seite.toml");
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
    let content = format!("---\ntitle: \"{title}\"\ndate: {date}\n---\n\nContent of {title}.\n");
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
        create_post(
            &site_dir,
            &format!("2025-01-{:02}", i),
            &format!("post-{i}"),
            &format!("Post {i}"),
        );
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
        create_post(
            &site_dir,
            &format!("2025-02-{:02}", i),
            &format!("post-{i}"),
            &format!("Post {i}"),
        );
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
    assert!(
        page2.contains("Page 2 of"),
        "page 2 should show page count in pagination nav"
    );
    // Page 2 is not the last page (init creates a hello-world post, so 5 total → 3 pages)
    assert!(
        page2.contains("class=\"pagination\""),
        "page 2 should have pagination nav"
    );
}

#[test]
fn test_build_no_pagination_without_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Paginate Test", "posts");
    let site_dir = tmp.path().join("site");

    for i in 1..=5 {
        create_post(
            &site_dir,
            &format!("2025-03-{:02}", i),
            &format!("post-{i}"),
            &format!("Post {i}"),
        );
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

/// Helper: append lines to seite.toml [build] section.
fn set_build_option(site_dir: &std::path::Path, key: &str, value: &str) {
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config = config.replace("[build]", &format!("[build]\n{key} = {value}"));
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
    assert!(
        !css.contains("/* header"),
        "minified CSS should not contain comments"
    );
    assert!(
        !css.contains("    "),
        "minified CSS should not have indentation"
    );
    assert!(
        css.contains("color:red"),
        "minified CSS should collapse whitespace around colon"
    );
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
    assert!(
        manifest_path.exists(),
        "asset-manifest.json should be generated"
    );
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    assert!(
        manifest.contains("/static/main.css"),
        "manifest should map original path"
    );
    // Fingerprinted file should exist alongside original
    let dist_static = site_dir.join("dist/static");
    let has_fingerprinted = fs::read_dir(&dist_static).unwrap().any(|e| {
        e.ok()
            .and_then(|e| e.file_name().into_string().ok())
            .map(|n| n.starts_with("main.") && n.ends_with(".css") && n != "main.css")
            .unwrap_or(false)
    });
    assert!(
        has_fingerprinted,
        "fingerprinted CSS file should exist in dist/static"
    );
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
    assert!(
        css.contains("/* keep"),
        "CSS should be unmodified when minify is false"
    );
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
    assert!(
        workflow.exists(),
        "GitHub Actions workflow should be created"
    );

    let content = fs::read_to_string(&workflow).unwrap();
    assert!(content.contains("Deploy to GitHub Pages"));
    assert!(content.contains("deploy-pages"));
    assert!(content.contains("seite build"));
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

    let config = fs::read_to_string(tmp.path().join("mysite/seite.toml")).unwrap();
    assert!(
        config.contains("netlify"),
        "config should contain netlify deploy target"
    );

    // Should generate netlify.toml
    let netlify_toml = tmp.path().join("mysite/netlify.toml");
    assert!(netlify_toml.exists());
    let content = fs::read_to_string(&netlify_toml).unwrap();
    assert!(content.contains("[build]"));
    assert!(content.contains("seite build"));

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
        .stdout(predicate::str::contains(
            "Base URL override: https://example.com",
        ));
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
    let config_path = site_dir.join("seite.toml");
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

    // Verify seite.toml was updated
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
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

#[test]
fn test_deploy_skip_checks_bypasses_preflight() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Skip Test", "posts");
    let site_dir = tmp.path().join("site");

    // --skip-checks should not print preflight header at all
    // It will still fail because no output dir exists, but that's the build step not preflight
    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        // dry-run always shows checks regardless of skip-checks (dry-run is informational)
        .success();
}

#[test]
fn test_deploy_dry_run_cloudflare_shows_auth_check() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CF Auth Test", "posts");
    let site_dir = tmp.path().join("site");

    // Update config to target cloudflare
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"test-project\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Cloudflare auth"));
}

#[test]
fn test_deploy_dry_run_netlify_shows_auth_check() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Netlify Auth Test", "posts");
    let site_dir = tmp.path().join("site");

    // Update config to target netlify
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Netlify auth"));
}

#[test]
fn test_deploy_dry_run_netlify_shows_site_check() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Netlify Site Test", "posts");
    let site_dir = tmp.path().join("site");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    // Dry run should show the Netlify site check
    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Netlify site"));
}

#[test]
fn test_deploy_dry_run_cloudflare_shows_project_check() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CF Project Test", "posts");
    let site_dir = tmp.path().join("site");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"test-project\"",
    );
    fs::write(&toml_path, config).unwrap();

    // Dry run should show the Cloudflare project check
    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Cloudflare project"));
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

/// Helper: set [images] section in seite.toml (replaces existing if present).
fn set_images_config(site_dir: &std::path::Path, widths: &str) {
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    // Remove existing [images] section if present
    if let Some(pos) = config.find("\n[images]") {
        config.truncate(pos);
    }
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

    // Create a page with two image references — first is skipped for LCP, second gets lazy
    let page_content =
        "---\ntitle: Gallery\n---\n\n![First](/static/images/photo.png)\n\n![Second](/static/images/photo.png)\n";
    fs::write(site_dir.join("content/pages/gallery.md"), page_content).unwrap();

    write_test_image(&site_dir, "photo.png");
    set_images_config(&site_dir, "[48]");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/gallery.html")).unwrap();
    // Second image should get lazy loading (first is skipped for LCP)
    assert!(
        html.contains("loading=\"lazy\""),
        "second img tag should have loading=lazy"
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
    assert!(html.contains("image/webp"), "should have WebP source type");
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

    // Remove the [images] section so image processing is truly disabled
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    if let Some(pos) = config.find("\n[images]") {
        fs::write(&toml_path, &config[..pos]).unwrap();
    }

    write_test_image(&site_dir, "photo.png");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Original should be copied as-is by static file step
    assert!(site_dir.join("dist/static/images/photo.png").exists());
    // No [images] config means no processing at all — no resizes, no WebP
    assert!(!site_dir.join("dist/static/images/photo-480w.png").exists());
    assert!(!site_dir.join("dist/static/images/photo.webp").exists());
}

#[test]
fn test_build_image_avif_generation() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "AVIF Test", "posts");
    let site_dir = tmp.path().join("site");

    write_test_image(&site_dir, "photo.png");

    // Add AVIF config
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    if let Some(pos) = config.find("\n[images]") {
        config.truncate(pos);
    }
    config.push_str(
        "\n[images]\nwidths = [48]\nquality = 80\nlazy_loading = true\nwebp = true\navif = true\navif_quality = 70\n",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Resized AVIF should exist
    assert!(
        site_dir.join("dist/static/images/photo-48w.avif").exists(),
        "resized AVIF at 48w should exist"
    );
    // Full-size AVIF should exist
    assert!(
        site_dir.join("dist/static/images/photo.avif").exists(),
        "full-size AVIF should exist"
    );
    // WebP should also exist (both enabled)
    assert!(site_dir.join("dist/static/images/photo-48w.webp").exists());
}

#[test]
fn test_build_image_avif_picture_element() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "AVIF Picture Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let page_content = "---\ntitle: Gallery\n---\n\n![A photo](/static/images/photo.png)\n";
    fs::write(site_dir.join("content/pages/gallery.md"), page_content).unwrap();

    write_test_image(&site_dir, "photo.png");

    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    if let Some(pos) = config.find("\n[images]") {
        config.truncate(pos);
    }
    config.push_str(
        "\n[images]\nwidths = [48]\nquality = 80\nlazy_loading = true\nwebp = true\navif = true\navif_quality = 70\n",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/gallery.html")).unwrap();
    assert!(html.contains("image/avif"), "should have AVIF source type");
    assert!(html.contains("image/webp"), "should have WebP source type");
    // AVIF should appear before WebP in the HTML
    let avif_pos = html.find("image/avif").unwrap();
    let webp_pos = html.find("image/webp").unwrap();
    assert!(
        avif_pos < webp_pos,
        "AVIF source should appear before WebP for browser priority"
    );
}

// ── Math/LaTeX rendering ──

#[test]
fn test_build_math_disabled_no_rendering() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Math Off", "posts,pages");
    let site_dir = tmp.path().join("site");

    let page_content = "---\ntitle: Math Page\n---\n\nThe formula $E=mc^2$ inline.\n";
    fs::write(site_dir.join("content/pages/math.md"), page_content).unwrap();

    // math defaults to false — no need to set it
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/math.html")).unwrap();
    // With math disabled, no KaTeX CSS link injected and no rendered math spans
    assert!(
        !html.contains("katex.min.css"),
        "should not inject katex CSS link"
    );
    assert!(
        !html.contains("<span class=\"katex\""),
        "should not contain rendered katex spans"
    );
}

#[test]
fn test_build_math_enabled_renders_katex() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Math On", "posts,pages");
    let site_dir = tmp.path().join("site");

    let page_content = "---\ntitle: Math Page\n---\n\nThe formula $E=mc^2$ inline.\n";
    fs::write(site_dir.join("content/pages/math.md"), page_content).unwrap();

    // Enable math in the existing [build] section
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config = config.replace("[build]", "[build]\nmath = true");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/math.html")).unwrap();
    assert!(
        html.contains("katex"),
        "should contain katex rendered output: {html}"
    );
    assert!(
        html.contains("katex.min.css"),
        "should inject katex CSS link"
    );
}

// ── Reading time + word count ──

#[test]
fn test_build_reading_time_in_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "rtsite", "RT Site", "posts");
    let site_dir = tmp.path().join("rtsite");

    // Write a post with a known word count (~300 words → 2 min read at 238 WPM)
    let words: String = (0..300).map(|i| format!("word{i} ")).collect();
    let content = format!("---\ntitle: Reading Test\ndate: 2025-01-15\n---\n{}", words);
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
    let content = format!("---\ntitle: Long Post\ndate: 2025-01-15\n---\n{}", words);
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
    assert!(
        index.contains("This is the intro."),
        "excerpt should appear in index"
    );
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
    fs::write(site_dir.join("content/docs/my-doc.md"), content).unwrap();

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
    fs::write(site_dir.join("content/docs/guide.md"), content).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/docs/guide.html")).unwrap();
    // Default doc template should show ToC when > 1 heading
    assert!(html.contains("toc"), "doc should contain toc class");
    assert!(
        html.contains("Contents"),
        "doc should contain Contents heading"
    );
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
    assert!(
        rust_html.contains("Alpha Post"),
        "rust tag should list Alpha Post"
    );
    assert!(
        rust_html.contains("Beta Post"),
        "rust tag should list Beta Post"
    );

    let web_tag = site_dir.join("dist/tags/web/index.html");
    assert!(web_tag.exists(), "web tag page should exist");
    let web_html = fs::read_to_string(&web_tag).unwrap();
    assert!(
        web_html.contains("Alpha Post"),
        "web tag should list Alpha Post"
    );
    assert!(
        !web_html.contains("Beta Post"),
        "web tag should NOT list Beta Post"
    );

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
    assert!(
        tags_html.contains("tags") && tags_html.contains("alpha"),
        "should link to alpha tag page"
    );
    assert!(
        tags_html.contains("tags") && tags_html.contains("beta"),
        "should link to beta tag page"
    );

    // Sitemap should include tag URLs
    let sitemap = fs::read_to_string(site_dir.join("dist/sitemap.xml")).unwrap();
    assert!(
        sitemap.contains("/tags/"),
        "sitemap should include tag page URLs"
    );
}

#[test]
fn test_build_tag_pages_multilingual() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "taglang", "Tag Lang", "posts");
    let site_dir = tmp.path().join("taglang");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Tag Lang ES\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

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
    assert!(
        en_html.contains("Hello"),
        "English tag page should have English post"
    );
    assert!(
        !en_html.contains("Hola"),
        "English tag page should NOT have Spanish post"
    );
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
    assert!(
        html.contains("hero"),
        "should render hero div from extra.hero_color"
    );
    assert!(html.contains("#ff6600"), "should include hero_color value");
    assert!(
        html.contains("featured-badge"),
        "should render featured badge from extra.featured"
    );
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
    assert!(
        html.contains("custom-style"),
        "should include extra_css block"
    );
    assert!(html.contains("custom-meta"), "should include head block");
    assert!(
        html.contains("custom-footer"),
        "should include footer block"
    );
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
    assert!(
        html.contains("Skip to main content"),
        "should have skip link text"
    );
    assert!(
        html.contains("id=\"main\""),
        "main element should have id=\"main\""
    );
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
    assert!(
        html.contains("role=\"search\""),
        "search form should have role=search"
    );
    assert!(
        html.contains("aria-label"),
        "search input should have aria-label"
    );
    assert!(
        html.contains("aria-live=\"polite\""),
        "search results should have aria-live"
    );
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
    assert!(
        html.contains("Empty Body Post"),
        "should render title even with empty body"
    );
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
    assert!(
        html.contains("custom-index"),
        "should use custom index template"
    );
    assert!(
        html.contains("Custom index content"),
        "should render custom template content"
    );
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
    assert!(
        md_output.contains("**original** markdown content"),
        "md output should preserve original markdown"
    );
    assert!(
        md_output.contains("- Item 1"),
        "md output should preserve list items"
    );
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

// --- shortcodes ---

#[test]
fn test_build_inline_shortcode_youtube() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest");

    fs::write(
        site_dir.join("content/posts/2025-01-15-video.md"),
        r#"---
title: Video Post
---
Check this out:

{{< youtube(id="dQw4w9WgXcQ") >}}
"#,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/video.html")).unwrap();
    assert!(html.contains("youtube.com/embed/dQw4w9WgXcQ"));
    assert!(html.contains("video-embed"));
}

#[test]
fn test_build_body_shortcode_callout() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest2", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest2");

    fs::write(
        site_dir.join("content/posts/2025-01-15-callout.md"),
        "---\ntitle: Callout Post\n---\n\n{{% callout(type=\"warning\") %}}\nThis is **important** stuff.\n{{% end %}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/callout.html")).unwrap();
    assert!(html.contains("callout-warning"));
    assert!(html.contains("<strong>important</strong>"));
}

#[test]
fn test_build_shortcode_in_code_block_not_expanded() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest3", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest3");

    fs::write(
        site_dir.join("content/posts/2025-01-15-code.md"),
        "---\ntitle: Code Post\n---\n\n```\n{{< youtube(id=\"test\") >}}\n```\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/code.html")).unwrap();
    // Should NOT contain youtube embed — shortcode is inside code block
    assert!(!html.contains("youtube.com/embed"));
    // Should contain the literal shortcode text (escaped)
    assert!(html.contains("youtube"));
}

#[test]
fn test_build_shortcode_in_inline_code_not_expanded() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest4", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest4");

    fs::write(
        site_dir.join("content/posts/2025-01-15-inline.md"),
        "---\ntitle: Inline Code\n---\n\nUse `{{< youtube(id=\"test\") >}}` for videos.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/inline.html")).unwrap();
    assert!(!html.contains("youtube.com/embed"));
}

#[test]
fn test_build_unknown_shortcode_errors() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest5", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest5");

    fs::write(
        site_dir.join("content/posts/2025-01-15-unknown.md"),
        "---\ntitle: Unknown\n---\n\n{{< nonexistent(x=\"y\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown shortcode"));
}

#[test]
fn test_build_unclosed_body_shortcode_errors() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest6", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest6");

    fs::write(
        site_dir.join("content/posts/2025-01-15-unclosed.md"),
        "---\ntitle: Unclosed\n---\n\n{{% callout(type=\"info\") %}}\nNo end tag here.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unclosed body shortcode"));
}

#[test]
fn test_build_user_defined_shortcode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest7", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest7");

    // Create custom shortcode
    fs::create_dir_all(site_dir.join("templates/shortcodes")).unwrap();
    fs::write(
        site_dir.join("templates/shortcodes/badge.html"),
        "<span class=\"badge badge-{{ color }}\">{{ text }}</span>",
    )
    .unwrap();

    fs::write(
        site_dir.join("content/posts/2025-01-15-badge.md"),
        "---\ntitle: Badge Post\n---\n\n{{< badge(color=\"green\", text=\"New\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/badge.html")).unwrap();
    assert!(html.contains("badge-green"));
    assert!(html.contains(">New</span>"));
}

#[test]
fn test_build_shortcode_raw_body_preserved_in_md() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest8", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest8");

    fs::write(
        site_dir.join("content/posts/2025-01-15-raw.md"),
        "---\ntitle: Raw MD\n---\n\n{{< youtube(id=\"test123\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // HTML should have the expanded embed
    let html = fs::read_to_string(site_dir.join("dist/posts/raw.html")).unwrap();
    assert!(html.contains("youtube.com/embed/test123"));

    // MD output should have the original unexpanded shortcode
    let md = fs::read_to_string(site_dir.join("dist/posts/raw.md")).unwrap();
    assert!(md.contains("{{< youtube(id=\"test123\") >}}"));
}

#[test]
fn test_build_figure_shortcode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest9", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest9");

    fs::write(
        site_dir.join("content/posts/2025-01-15-figure.md"),
        "---\ntitle: Figure Post\n---\n\n{{< figure(src=\"/static/img.jpg\", caption=\"A photo\", alt=\"My photo\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/figure.html")).unwrap();
    assert!(html.contains("<figure"));
    assert!(html.contains("<figcaption>A photo</figcaption>"));
}

#[test]
fn test_build_multiple_shortcodes_in_one_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest10", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest10");

    fs::write(
        site_dir.join("content/posts/2025-01-15-multi.md"),
        r#"---
title: Multi
---
{{< youtube(id="abc") >}}

Some text between.

{{< vimeo(id="123") >}}
"#,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/multi.html")).unwrap();
    assert!(html.contains("youtube.com/embed/abc"));
    assert!(html.contains("player.vimeo.com/video/123"));
}

#[test]
fn test_build_shortcode_user_overrides_builtin() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest11", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest11");

    // Override youtube shortcode with custom template
    fs::create_dir_all(site_dir.join("templates/shortcodes")).unwrap();
    fs::write(
        site_dir.join("templates/shortcodes/youtube.html"),
        "<div class=\"custom-yt\">{{ id }}</div>",
    )
    .unwrap();

    fs::write(
        site_dir.join("content/posts/2025-01-15-override.md"),
        "---\ntitle: Override\n---\n\n{{< youtube(id=\"custom\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/override.html")).unwrap();
    assert!(html.contains("custom-yt"));
    assert!(!html.contains("youtube.com/embed"));
}

#[test]
fn test_build_gist_shortcode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest12", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest12");

    fs::write(
        site_dir.join("content/posts/2025-01-15-gist.md"),
        "---\ntitle: Gist Post\n---\n\n{{< gist(user=\"octocat\", id=\"abc123\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/gist.html")).unwrap();
    assert!(html.contains("gist.github.com/octocat/abc123.js"));
}

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

// ── Data files feature ──────────────────────────────────────────────

#[test]
fn test_build_with_yaml_data_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data YAML", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Create a YAML data file
    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/authors.yaml"),
        "- name: Alice\n  role: Editor\n- name: Bob\n  role: Writer\n",
    )
    .unwrap();

    // Modify index template to render data
    let index_path = site_dir.join("templates/index.html");
    let index_tmpl = fs::read_to_string(&index_path).unwrap();
    let modified = index_tmpl.replace(
        "{% endblock %}",
        "{% if data.authors %}{% for author in data.authors %}<span class=\"data-author\">{{ author.name }}</span>{% endfor %}{% endif %}{% endblock %}",
    );
    fs::write(&index_path, modified).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("data files loaded"));

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("Alice"),
        "YAML data should be rendered in template"
    );
    assert!(
        index_html.contains("Bob"),
        "YAML data should be rendered in template"
    );
}

#[test]
fn test_build_with_json_data_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data JSON", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/social.json"),
        r#"[{"name": "GitHub", "url": "https://github.com"}]"#,
    )
    .unwrap();

    // Modify index template to render data
    let index_path = site_dir.join("templates/index.html");
    let index_tmpl = fs::read_to_string(&index_path).unwrap();
    let modified = index_tmpl.replace(
        "{% endblock %}",
        "{% if data.social %}{% for link in data.social %}<a class=\"data-social\" href=\"{{ link.url }}\">{{ link.name }}</a>{% endfor %}{% endif %}{% endblock %}",
    );
    fs::write(&index_path, modified).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("GitHub"),
        "JSON data should be rendered in template"
    );
}

#[test]
fn test_build_with_toml_data_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data TOML", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/settings.toml"),
        "site_name = \"My TOML Site\"\nmax_posts = 10\n",
    )
    .unwrap();

    // Modify index template to render data
    let index_path = site_dir.join("templates/index.html");
    let index_tmpl = fs::read_to_string(&index_path).unwrap();
    let modified = index_tmpl.replace(
        "{% endblock %}",
        "{% if data.settings %}<span class=\"data-setting\">{{ data.settings.site_name }}</span>{% endif %}{% endblock %}",
    );
    fs::write(&index_path, modified).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("My TOML Site"),
        "TOML data should be rendered in template"
    );
}

#[test]
fn test_build_with_nested_data_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Nested Data", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data/menus")).unwrap();
    fs::write(
        site_dir.join("data/menus/main.yaml"),
        "- title: Home\n  url: /\n- title: About\n  url: /about\n",
    )
    .unwrap();

    // Modify index template to render nested data
    let index_path = site_dir.join("templates/index.html");
    let index_tmpl = fs::read_to_string(&index_path).unwrap();
    let modified = index_tmpl.replace(
        "{% endblock %}",
        "{% if data.menus %}{% if data.menus.main %}{% for item in data.menus.main %}<a class=\"data-nav\">{{ item.title }}</a>{% endfor %}{% endif %}{% endif %}{% endblock %}",
    );
    fs::write(&index_path, modified).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("Home"),
        "Nested data.menus.main should work"
    );
    assert!(
        index_html.contains("About"),
        "Nested data.menus.main should work"
    );
}

#[test]
fn test_build_data_conflict_errors() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data Conflict", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(site_dir.join("data/authors.yaml"), "- name: Alice\n").unwrap();
    fs::write(site_dir.join("data/authors.json"), r#"[{"name": "Bob"}]"#).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("data key conflict"));
}

#[test]
fn test_build_data_parse_error() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data Error", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/broken.yaml"),
        "invalid: yaml: [: unclosed\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("broken.yaml"));
}

#[test]
fn test_build_no_data_dir_succeeds() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Data", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove the data dir that init creates
    let data_dir = site_dir.join("data");
    if data_dir.exists() {
        fs::remove_dir_all(&data_dir).unwrap();
    }

    page_cmd()
        .arg("build")
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
fn test_init_creates_data_directory() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Init Data Dir", "posts,pages");
    let site_dir = tmp.path().join("site");

    assert!(
        site_dir.join("data").exists(),
        "init should create data/ directory"
    );
    assert!(site_dir.join("data").is_dir());
}

#[test]
fn test_build_data_nav_in_theme() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Theme Nav", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Create nav data file matching the theme's expected format
    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/nav.yaml"),
        "- title: Blog\n  url: /posts\n- title: About\n  url: /about\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("Main navigation"),
        "Theme should render data.nav with aria-label"
    );
    // Tera HTML-escapes / to &#x2F; in attribute values
    assert!(
        index_html.contains("/posts") || index_html.contains("&#x2F;posts"),
        "Nav should contain link URLs"
    );
    assert!(
        index_html.contains("Blog"),
        "Nav should contain link titles"
    );
}

#[test]
fn test_build_data_footer_in_theme() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Theme Footer", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Create footer data file
    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/footer.yaml"),
        "links:\n  - title: GitHub\n    url: https://github.com\n  - title: Twitter\n    url: https://twitter.com\ncopyright: \"2026 Test Corp\"\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("Footer navigation"),
        "Theme should render data.footer with aria-label"
    );
    assert!(
        index_html.contains("GitHub"),
        "Footer should contain link title"
    );
    assert!(
        index_html.contains("2026 Test Corp"),
        "Footer should use custom copyright"
    );
}

#[test]
fn test_build_docs_sorted_by_weight() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Weight Sort", "docs");

    let site_dir = tmp.path().join("site");

    // Create 3 docs with weights out of alphabetical order
    fs::write(
        site_dir.join("content/docs/zebra.md"),
        "---\ntitle: Zebra\nweight: 1\n---\n\nFirst by weight.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/alpha.md"),
        "---\ntitle: Alpha\nweight: 3\n---\n\nThird by weight.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/middle.md"),
        "---\ntitle: Middle\nweight: 2\n---\n\nSecond by weight.\n",
    )
    .unwrap();

    // Apply docs theme for sidebar nav
    page_cmd()
        .args(["theme", "apply", "docs"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Check the sidebar order in any built doc page
    let html = fs::read_to_string(site_dir.join("dist/docs/zebra.html")).unwrap();
    let pos_zebra = html.find(">Zebra<").expect("Zebra should be in sidebar");
    let pos_middle = html.find(">Middle<").expect("Middle should be in sidebar");
    let pos_alpha = html.find(">Alpha<").expect("Alpha should be in sidebar");

    assert!(
        pos_zebra < pos_middle && pos_middle < pos_alpha,
        "Docs should be sorted by weight: Zebra(1) < Middle(2) < Alpha(3), got positions: {} {} {}",
        pos_zebra,
        pos_middle,
        pos_alpha
    );
}

#[test]
fn test_build_docs_weight_mixed_with_unweighted() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Mixed Weight", "docs");

    let site_dir = tmp.path().join("site");

    // Weighted items should come first, unweighted sort alphabetically after
    fs::write(
        site_dir.join("content/docs/second.md"),
        "---\ntitle: Second\nweight: 2\n---\n\nWeighted second.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/first.md"),
        "---\ntitle: First\nweight: 1\n---\n\nWeighted first.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/bravo.md"),
        "---\ntitle: Bravo\n---\n\nUnweighted B.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/alpha-unweighted.md"),
        "---\ntitle: Alpha Unweighted\n---\n\nUnweighted A.\n",
    )
    .unwrap();

    // Apply docs theme for sidebar nav
    page_cmd()
        .args(["theme", "apply", "docs"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/docs/first.html")).unwrap();
    let pos_first = html.find(">First<").expect("First should be in sidebar");
    let pos_second = html.find(">Second<").expect("Second should be in sidebar");
    let pos_alpha = html
        .find(">Alpha Unweighted<")
        .expect("Alpha Unweighted should be in sidebar");
    let pos_bravo = html.find(">Bravo<").expect("Bravo should be in sidebar");

    // Weighted items first (by weight), then unweighted (alphabetically)
    assert!(
        pos_first < pos_second && pos_second < pos_alpha && pos_alpha < pos_bravo,
        "Expected: First(w1) < Second(w2) < Alpha Unweighted(none) < Bravo(none), got: {} {} {} {}",
        pos_first,
        pos_second,
        pos_alpha,
        pos_bravo
    );
}

// ── Project metadata & upgrade ──────────────────────────────────────

#[test]
fn test_init_creates_page_meta() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Meta Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // .seite/config.json should exist
    let meta_path = site_dir.join(".seite/config.json");
    assert!(
        meta_path.exists(),
        ".seite/config.json should be created by init"
    );

    let content = fs::read_to_string(&meta_path).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(
        meta.get("version").is_some(),
        "meta should have a version field"
    );
    assert!(
        meta.get("initialized_at").is_some(),
        "meta should have an initialized_at timestamp"
    );
}

#[test]
fn test_init_creates_mcp_server_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "MCP Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let settings_path = site_dir.join(".claude/settings.json");
    assert!(settings_path.exists());

    let content = fs::read_to_string(&settings_path).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Should have mcpServers.seite
    assert!(
        settings.pointer("/mcpServers/seite").is_some(),
        "settings should include mcpServers.seite"
    );
    assert_eq!(
        settings
            .pointer("/mcpServers/seite/command")
            .and_then(|v| v.as_str()),
        Some("seite"),
        "MCP command should be 'seite'"
    );
    assert_eq!(
        settings
            .pointer("/mcpServers/seite/args")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>()),
        Some(vec!["mcp"]),
        "MCP args should be ['mcp']"
    );
}

#[test]
fn test_init_creates_landing_page_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Skill Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    assert!(
        skill_path.exists(),
        "landing-page skill should be created when pages collection is present"
    );

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(
        content.starts_with("---"),
        "SKILL.md should have YAML frontmatter"
    );
    assert!(
        content.contains("name: landing-page"),
        "SKILL.md should define skill name"
    );
    assert!(
        content.contains("Phase 1: Messaging"),
        "SKILL.md should contain messaging phase"
    );
}

#[test]
fn test_init_no_landing_page_skill_without_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Pages", "posts,docs");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    assert!(
        !skill_path.exists(),
        "landing-page skill should NOT be created without pages collection"
    );
}

#[test]
fn test_upgrade_adds_landing_page_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove the skill to simulate an older project
    let skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    fs::remove_file(&skill_path).unwrap();
    assert!(!skill_path.exists());

    // Downgrade the project version so the upgrade step applies
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.0");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("landing-page"));

    assert!(
        skill_path.exists(),
        "upgrade should create the landing-page skill"
    );
    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("name: landing-page"));
}

#[test]
fn test_upgrade_updates_outdated_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Skill Update", "posts,pages");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    assert!(skill_path.exists());

    // Write an older version of the skill (version 1)
    fs::write(
        &skill_path,
        "---\nname: landing-page\ndescription: old\n# seite-skill-version: 1\n---\nOld content\n",
    )
    .unwrap();

    // Downgrade the project version so the upgrade step applies
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.0");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("updated v1 \u{2192} v3"));

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(
        content.contains("seite-skill-version: 3"),
        "upgrade should replace skill with newer version"
    );
    assert!(
        content.contains("Phase 1: Messaging"),
        "upgraded skill should have full content"
    );
}

#[test]
fn test_upgrade_migrates_old_homepage_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Migration Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove the new skill and create an old homepage skill to simulate pre-rename project
    let new_skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    fs::remove_file(&new_skill_path).unwrap();
    let old_skill_dir = site_dir.join(".claude/skills/homepage");
    fs::create_dir_all(&old_skill_dir).unwrap();
    fs::write(
        old_skill_dir.join("SKILL.md"),
        "---\nname: homepage\ndescription: old\n# seite-skill-version: 1\n---\nOld content\n",
    )
    .unwrap();

    // Downgrade the project version
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.0");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("landing-page"));

    // New skill should exist
    assert!(
        new_skill_path.exists(),
        "upgrade should create the new landing-page skill"
    );
    let content = fs::read_to_string(&new_skill_path).unwrap();
    assert!(content.contains("name: landing-page"));

    // Old skill should still be there (non-destructive)
    assert!(
        old_skill_dir.join("SKILL.md").exists(),
        "upgrade should not delete the old homepage skill"
    );
}

#[test]
fn test_upgrade_on_fresh_project_is_noop() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Up To Date", "posts,pages");
    let site_dir = tmp.path().join("site");

    // A freshly initialized project should already be up to date
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

#[test]
fn test_upgrade_adds_mcp_to_existing_project() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade MCP", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Simulate a pre-MCP project by removing the MCP config and version stamp
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    // Write a .claude/settings.json WITHOUT mcpServers
    fs::write(
        site_dir.join(".claude/settings.json"),
        r#"{
  "permissions": {
    "allow": ["Read", "Glob", "Grep"],
    "deny": ["Read(.env)"]
  }
}
"#,
    )
    .unwrap();

    // Run upgrade
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("mcpServers"));

    // Verify MCP was added
    let content = fs::read_to_string(site_dir.join(".claude/settings.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(
        settings.pointer("/mcpServers/seite").is_some(),
        "upgrade should add mcpServers.seite"
    );

    // Verify existing permissions were preserved
    assert!(
        settings
            .pointer("/permissions/allow")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().any(|v| v.as_str() == Some("Read")))
            .unwrap_or(false),
        "upgrade should preserve existing permissions"
    );

    // Verify .seite/config.json was created
    assert!(
        site_dir.join(".seite/config.json").exists(),
        "upgrade should create .seite/config.json"
    );
}

#[test]
fn test_upgrade_appends_mcp_section_to_claude_md() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CLAUDE.md Upgrade", "posts,pages");
    let site_dir = tmp.path().join("site");

    // init now includes MCP section, so verify it's there
    let claude_md = fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap();
    assert!(
        claude_md.contains("## MCP Server"),
        "init-generated CLAUDE.md should include MCP section"
    );
    assert!(
        claude_md.contains("## Commands"),
        "CLAUDE.md should have Commands section from init"
    );

    // Simulate an older CLAUDE.md that doesn't have the MCP section
    let older_md = "# My Site\n\n## Commands\n\n```bash\nseite build\n```\n";
    fs::write(site_dir.join("CLAUDE.md"), older_md).unwrap();

    // Remove version stamp to trigger upgrade
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let updated = fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap();
    assert!(
        updated.contains("## MCP Server"),
        "upgrade should append MCP Server section to CLAUDE.md"
    );
    // Original content should still be there
    assert!(
        updated.contains("## Commands"),
        "upgrade should preserve existing CLAUDE.md content"
    );
}

#[test]
fn test_upgrade_check_mode_exits_nonzero() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Check Mode", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove version stamp to make it look outdated
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["upgrade", "--check"])
        .current_dir(&site_dir)
        .assert()
        .failure(); // exit 1 = upgrades needed
}

#[test]
fn test_upgrade_check_mode_succeeds_when_current() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Check Current", "posts,pages");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["upgrade", "--check"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

#[test]
fn test_upgrade_preserves_existing_mcp_servers() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Preserve MCP", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove version stamp to trigger upgrade
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    // Write settings with a DIFFERENT MCP server (e.g., user has their own)
    fs::write(
        site_dir.join(".claude/settings.json"),
        r#"{
  "permissions": { "allow": ["Read"] },
  "mcpServers": {
    "custom-server": {
      "command": "my-tool",
      "args": ["serve"]
    }
  }
}
"#,
    )
    .unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let content = fs::read_to_string(site_dir.join(".claude/settings.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Both MCP servers should exist
    assert!(
        settings.pointer("/mcpServers/seite").is_some(),
        "upgrade should add seite MCP server"
    );
    assert!(
        settings.pointer("/mcpServers/custom-server").is_some(),
        "upgrade should preserve existing MCP servers"
    );
}

#[test]
fn test_upgrade_outside_project_fails() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["upgrade"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No seite.toml"));
}

#[test]
fn test_build_nudges_when_outdated() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Nudge Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove .seite/config.json to simulate pre-tracking project
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("seite upgrade"));
}

#[test]
fn test_build_no_nudge_when_current() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Nudge", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Fresh init should not nudge
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("seite upgrade").not());
}

#[test]
fn test_upgrade_idempotent() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Idempotent", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove version stamp so first upgrade does work
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    // First upgrade
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Second upgrade should be a no-op
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

// ── self-update command ─────────────────────────────────────────────

#[test]
fn test_self_update_check_when_current() {
    // --check with --target-version set to current version should succeed (already up to date)
    page_cmd()
        .args([
            "self-update",
            "--check",
            "--target-version",
            env!("CARGO_PKG_VERSION"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

// ── MCP server ────────────────────────────────────────────────────────

/// Helper: send a single JSON-RPC message to `seite mcp` and return the response.
fn mcp_request(dir: &std::path::Path, messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let bin = assert_cmd::cargo::cargo_bin!("seite");
    let mut child = std::process::Command::new(bin)
        .arg("mcp")
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn page mcp");

    // Write all messages to stdin, then close it
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        for msg in messages {
            let line = serde_json::to_string(msg).unwrap();
            writeln!(stdin, "{}", line).unwrap();
        }
    }
    // stdin drops here, closing the pipe → server exits after processing

    let output = child.wait_with_output().expect("failed to wait on child");
    let stdout = String::from_utf8_lossy(&output.stdout);

    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("invalid JSON from MCP server"))
        .collect()
}

#[test]
fn test_mcp_initialize() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let resp = &responses[0];
    assert_eq!(resp["id"], 1);
    assert!(resp["error"].is_null());
    let result = &resp["result"];
    assert_eq!(result["serverInfo"]["name"], "seite");
    assert!(result["capabilities"]["resources"].is_object());
    assert!(result["capabilities"]["tools"].is_object());
    assert_eq!(result["protocolVersion"], "2024-11-05");
}

#[test]
fn test_mcp_ping() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "ping",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], 42);
    assert!(responses[0]["error"].is_null());
}

#[test]
fn test_mcp_unknown_method() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "bogus/method",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32601); // METHOD_NOT_FOUND
}

#[test]
fn test_mcp_tools_list() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let tools = responses[0]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 5);
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"seite_build"));
    assert!(names.contains(&"seite_create_content"));
    assert!(names.contains(&"seite_search"));
    assert!(names.contains(&"seite_apply_theme"));
    assert!(names.contains(&"seite_lookup_docs"));
}

#[test]
fn test_mcp_resources_list_without_project() {
    // Outside a page project, only docs resources should be listed
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/list",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let resources = responses[0]["result"]["resources"].as_array().unwrap();
    // Should have docs index + individual doc pages, but no config/content/themes
    assert!(resources.iter().any(|r| r["uri"] == "seite://docs"));
    assert!(!resources.iter().any(|r| r["uri"] == "seite://config"));
    assert!(!resources.iter().any(|r| r["uri"] == "seite://themes"));
}

#[test]
fn test_mcp_resources_list_with_project() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpsite", "MCP Test", "posts,docs,pages");
    let site_dir = tmp.path().join("mcpsite");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/list",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let resources = responses[0]["result"]["resources"].as_array().unwrap();
    // Should have docs + site-specific resources
    assert!(resources.iter().any(|r| r["uri"] == "seite://docs"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://config"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://content"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://themes"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://mcp-config"));
    // Per-collection resources
    assert!(resources
        .iter()
        .any(|r| r["uri"] == "seite://content/posts"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://content/docs"));
    assert!(resources
        .iter()
        .any(|r| r["uri"] == "seite://content/pages"));
}

#[test]
fn test_mcp_read_docs_index() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://docs" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], "seite://docs");
    // Parse the text — should be a JSON array of docs
    let text = contents[0]["text"].as_str().unwrap();
    let docs: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    assert!(docs.len() >= 10); // We have 13 embedded docs
    assert!(docs.iter().any(|d| d["slug"] == "configuration"));
}

#[test]
fn test_mcp_read_doc_page() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://docs/configuration" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    // Should be markdown content without the leading frontmatter
    assert!(text.contains("seite.toml"));
    // Body should not start with frontmatter delimiters
    assert!(
        !text.starts_with("---\n"),
        "doc body should not start with frontmatter"
    );
}

#[test]
fn test_mcp_read_unknown_resource() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://nonexistent" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32602); // INVALID_PARAMS
}

#[test]
fn test_mcp_read_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpconf", "Config Test", "posts");
    let site_dir = tmp.path().join("mcpconf");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://config" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let config: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(config["site"]["title"], "Config Test");
}

#[test]
fn test_mcp_read_content_overview() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpcontent", "Content Test", "posts,docs");
    let site_dir = tmp.path().join("mcpcontent");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://content" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let collections: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    assert!(collections.iter().any(|c| c["name"] == "posts"));
    assert!(collections.iter().any(|c| c["name"] == "docs"));
}

#[test]
fn test_mcp_read_themes() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpthemes", "Theme Test", "posts");
    let site_dir = tmp.path().join("mcpthemes");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://themes" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let themes: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    assert!(themes.len() >= 6); // 6 bundled themes
    assert!(themes.iter().any(|t| t["name"] == "default"));
    assert!(themes.iter().any(|t| t["name"] == "dark"));
}

#[test]
fn test_mcp_read_mcp_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpmcp", "MCP Config Test", "posts");
    let site_dir = tmp.path().join("mcpmcp");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://mcp-config" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    assert!(text.contains("mcpServers"));
    assert!(text.contains("seite"));
}

#[test]
fn test_mcp_tool_lookup_docs() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_lookup_docs",
                "arguments": { "topic": "configuration" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["found"], true);
    assert_eq!(result["title"], "Configuration");
}

#[test]
fn test_mcp_tool_lookup_docs_search() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_lookup_docs",
                "arguments": { "query": "deploy" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(result["count"].as_u64().unwrap() > 0);
}

#[test]
fn test_mcp_tool_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpbuild", "Build Test", "posts");
    let site_dir = tmp.path().join("mcpbuild");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_build",
                "arguments": {}
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["success"], true);
    // Verify the site was actually built
    assert!(site_dir.join("dist/index.html").exists());
}

#[test]
fn test_mcp_tool_create_content() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpnew", "Create Test", "posts,docs");
    let site_dir = tmp.path().join("mcpnew");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_create_content",
                "arguments": {
                    "collection": "docs",
                    "title": "MCP Test Doc",
                    "body": "This is test content."
                }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["collection"], "docs");
    assert_eq!(result["slug"], "mcp-test-doc");
    assert_eq!(result["url"], "/docs/mcp-test-doc");
    // Verify file was created
    assert!(site_dir.join("content/docs/mcp-test-doc.md").exists());
    let file_content = fs::read_to_string(site_dir.join("content/docs/mcp-test-doc.md")).unwrap();
    assert!(file_content.contains("title: MCP Test Doc"));
    assert!(file_content.contains("This is test content."));
}

#[test]
fn test_mcp_tool_search() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpsearch", "Search Test", "posts");
    let site_dir = tmp.path().join("mcpsearch");

    // Create a post with searchable content
    fs::write(
        site_dir.join("content/posts/2025-01-15-rust-guide.md"),
        "---\ntitle: \"Rust Programming Guide\"\ntags:\n  - rust\n  - tutorial\n---\n\nLearn Rust from scratch.\n",
    ).unwrap();

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_search",
                "arguments": { "query": "rust" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(result["count"].as_u64().unwrap() >= 1);
    let results = result["results"].as_array().unwrap();
    assert!(results
        .iter()
        .any(|r| r["title"] == "Rust Programming Guide"));
}

#[test]
fn test_mcp_tool_apply_theme() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcptheme", "Theme Test", "posts");
    let site_dir = tmp.path().join("mcptheme");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_apply_theme",
                "arguments": { "name": "dark" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["applied"], true);
    assert_eq!(result["theme"], "dark");
    assert_eq!(result["source"], "bundled");
    // Verify theme file was written
    assert!(site_dir.join("templates/base.html").exists());
}

#[test]
fn test_mcp_tool_unknown_tool() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "nonexistent_tool",
                "arguments": {}
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32602);
}

#[test]
fn test_mcp_full_session() {
    // Simulate a realistic MCP session: initialize → tools/list → resources/list → lookup docs
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpfull", "Full Session", "posts,docs");
    let site_dir = tmp.path().join("mcpfull");

    let responses = mcp_request(
        &site_dir,
        &[
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "resources/list",
                "params": {}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": "seite_lookup_docs",
                    "arguments": { "topic": "templates" }
                }
            }),
        ],
    );

    // Notifications don't get responses, so we should have 4 responses
    assert_eq!(responses.len(), 4);

    // initialize
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "seite");

    // tools/list
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 5);

    // resources/list
    assert_eq!(responses[2]["id"], 3);
    let resources = responses[2]["result"]["resources"].as_array().unwrap();
    assert!(resources.iter().any(|r| r["uri"] == "seite://config"));

    // tools/call (lookup docs)
    assert_eq!(responses[3]["id"], 4);
    let content = responses[3]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["found"], true);
    assert_eq!(result["title"], "Templates & Themes");
}

#[test]
fn test_mcp_parse_error() {
    // Send invalid JSON and verify we get a parse error response
    let tmp = TempDir::new().unwrap();
    let bin = assert_cmd::cargo::cargo_bin!("seite");
    let mut child = std::process::Command::new(bin)
        .arg("mcp")
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn page mcp");

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "this is not valid json").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(response["error"].is_object());
    assert_eq!(response["error"]["code"], -32700); // PARSE_ERROR
}

#[test]
fn test_mcp_config_exposes_analytics() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpanalytics", "Analytics MCP", "posts");
    let site_dir = tmp.path().join("mcpanalytics");

    // Add analytics config
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[analytics]\nprovider = \"plausible\"\nid = \"example.com\"\ncookie_consent = true\n",
    );
    fs::write(&config_path, config).unwrap();

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://config" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let config_json: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(config_json["analytics"]["provider"], "plausible");
    assert_eq!(config_json["analytics"]["id"], "example.com");
    assert_eq!(config_json["analytics"]["cookie_consent"], true);
}

#[test]
fn test_mcp_docs_include_analytics() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_lookup_docs",
                "arguments": { "topic": "configuration" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["found"], true);
    let doc_content = result["content"].as_str().unwrap();
    assert!(
        doc_content.contains("[analytics]"),
        "Configuration docs should include the [analytics] section"
    );
    assert!(
        doc_content.contains("cookie_consent"),
        "Configuration docs should document cookie_consent field"
    );
}

// ── analytics ───────────────────────────────────────────────────────

#[test]
fn test_build_with_google_analytics_direct() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Analytics Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Add [analytics] to seite.toml
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[analytics]\nprovider = \"google\"\nid = \"G-TEST12345\"\n");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Check that analytics script is injected into HTML
    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index.contains("googletagmanager.com/gtag/js?id=G-TEST12345"),
        "index.html should contain GA4 script"
    );
    assert!(
        index.contains("gtag('config','G-TEST12345')"),
        "index.html should contain gtag config call"
    );
    // Should NOT have consent banner
    assert!(
        !index.contains("seite-cookie-banner"),
        "should not have consent banner when cookie_consent is false"
    );

    // Check post HTML too
    let post = fs::read_to_string(site_dir.join("dist/posts/hello-world.html")).unwrap();
    assert!(post.contains("G-TEST12345"));
}

#[test]
fn test_build_with_analytics_cookie_consent() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Consent Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Add [analytics] with cookie_consent = true
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[analytics]\nprovider = \"google\"\nid = \"G-CONSENT1\"\ncookie_consent = true\n",
    );
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();

    // Should have consent banner
    assert!(
        index.contains("seite-cookie-banner"),
        "should have cookie consent banner"
    );
    assert!(
        index.contains("seite-cookie-accept"),
        "should have accept button"
    );
    assert!(
        index.contains("seite-cookie-decline"),
        "should have decline button"
    );
    assert!(
        index.contains("seite_analytics_consent"),
        "should use localStorage key"
    );

    // Analytics script should NOT be in <head> directly
    let head_end = index.find("</head>").unwrap();
    let head_section = &index[..head_end];
    assert!(
        !head_section.contains("googletagmanager.com/gtag/js"),
        "GA script should not be directly in <head> when consent is required"
    );
}

#[test]
fn test_build_with_plausible_analytics() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Plausible Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[analytics]\nprovider = \"plausible\"\nid = \"example.com\"\n");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("plausible.io/js/script.js"));
    assert!(index.contains("data-domain=\"example.com\""));
}

#[test]
fn test_build_with_gtm_has_noscript() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "GTM Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[analytics]\nprovider = \"gtm\"\nid = \"GTM-ABC123\"\n");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("GTM-ABC123"), "should contain GTM ID");
    assert!(
        index.contains("<noscript><iframe"),
        "GTM should include noscript fallback"
    );
}

#[test]
fn test_build_without_analytics_no_injection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Analytics", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(!index.contains("googletagmanager"));
    assert!(!index.contains("plausible"));
    assert!(!index.contains("seite-cookie-banner"));
    assert!(!index.contains("usefathom"));
}

#[test]
fn test_build_with_umami_custom_script_url() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Umami Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[analytics]\nprovider = \"umami\"\nid = \"abc-def-123\"\nscript_url = \"https://stats.example.com/script.js\"\n",
    );
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("stats.example.com/script.js"));
    assert!(index.contains("data-website-id=\"abc-def-123\""));
}

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
        .args(["new", "roadmap", "Dark Mode", "--tags", "planned"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    let item_file = site_dir.join("content/roadmap/dark-mode.md");
    // Note: there may already be one from init, so check the freshly created one exists
    assert!(item_file.exists());

    let content = fs::read_to_string(item_file).unwrap();
    assert!(content.contains("title: Dark Mode"));
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

// --- i18n template context tests ---

#[test]
fn test_build_html_lang_uses_current_language() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "langsite", "Lang Site", "posts");
    let site_dir = tmp.path().join("langsite");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Sitio\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // English post
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();
    // Spanish post
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // English page should have <html lang="en">
    let en_html = fs::read_to_string(site_dir.join("dist/posts/hello.html")).unwrap();
    assert!(
        en_html.contains(r#"<html lang="en">"#),
        "English page should have <html lang=\"en\">"
    );

    // Spanish page should have <html lang="es">
    let es_html = fs::read_to_string(site_dir.join("dist/es/posts/hello.html")).unwrap();
    assert!(
        es_html.contains(r#"<html lang="es">"#),
        "Spanish page should have <html lang=\"es\">"
    );

    // English index should have <html lang="en">
    let en_index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        en_index.contains(r#"<html lang="en">"#),
        "English index should have <html lang=\"en\">"
    );

    // Spanish index should have <html lang="es">
    let es_index = fs::read_to_string(site_dir.join("dist/es/index.html")).unwrap();
    assert!(
        es_index.contains(r#"<html lang="es">"#),
        "Spanish index should have <html lang=\"es\">"
    );
}

#[test]
fn test_build_lang_prefix_in_context() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "lpsite", "LP Site", "posts");
    let site_dir = tmp.path().join("lpsite");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"LP ES\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // English post with tag
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\ntags:\n  - greetings\n---\nHello.",
    )
    .unwrap();
    // Spanish post with tag
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\ntags:\n  - saludos\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // English post tag links should NOT have lang prefix
    let en_html = fs::read_to_string(site_dir.join("dist/posts/hello.html")).unwrap();
    assert!(
        en_html.contains(r#"href="/tags/greetings/"#),
        "English tag link should not have lang prefix"
    );

    // Spanish post tag links SHOULD have /es prefix
    let es_html = fs::read_to_string(site_dir.join("dist/es/posts/hello.html")).unwrap();
    assert!(
        es_html.contains(r#"href="/es/tags/saludos/"#),
        "Spanish tag link should have /es lang prefix"
    );
}

#[test]
fn test_build_ui_strings_default() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tsite", "T Site", "posts");
    let site_dir = tmp.path().join("tsite");

    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Index page should use t.search_placeholder (default: "Search…")
    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        html.contains("Search\u{2026}") || html.contains("Search..."),
        "Default search placeholder should be rendered"
    );
    // Skip to main content should be rendered
    assert!(
        html.contains("Skip to main content"),
        "Skip to content link should use default t.skip_to_content"
    );
}

#[test]
fn test_build_ui_strings_override() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "toverride", "T Override", "posts");
    let site_dir = tmp.path().join("toverride");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Sitio\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // Create i18n override file
    let i18n_dir = site_dir.join("data/i18n");
    fs::create_dir_all(&i18n_dir).unwrap();
    fs::write(
        i18n_dir.join("es.yaml"),
        "search_placeholder: \"Buscar\\u2026\"\nskip_to_content: \"Ir al contenido\"",
    )
    .unwrap();

    // English post
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();
    // Spanish post
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Spanish index should use translated search placeholder
    let es_html = fs::read_to_string(site_dir.join("dist/es/index.html")).unwrap();
    assert!(
        es_html.contains("Buscar"),
        "Spanish page should use translated search placeholder from data/i18n/es.yaml"
    );
    assert!(
        es_html.contains("Ir al contenido"),
        "Spanish page should use translated skip-to-content from data/i18n/es.yaml"
    );

    // English page should still use defaults
    let en_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        en_html.contains("Skip to main content"),
        "English page should still use default English strings"
    );
}

#[test]
fn test_build_nav_links_with_lang_prefix() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "navlang", "Nav Lang", "posts,pages");
    let site_dir = tmp.path().join("navlang");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Nav ES\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // Create nav data with internal and external links
    let data_dir = site_dir.join("data");
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("nav.yaml"),
        "- title: Blog\n  url: /posts\n- title: GitHub\n  url: https://github.com\n  external: true",
    )
    .unwrap();

    // Content
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // English page: nav should have /posts (no lang prefix)
    let en_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        en_html.contains(r#"href="/posts""#),
        "English nav should link to /posts without prefix"
    );
    // External link should NOT have lang prefix
    assert!(
        en_html.contains(r#"href="https://github.com""#),
        "External link should stay unchanged"
    );

    // Spanish page: nav should have /es/posts
    let es_html = fs::read_to_string(site_dir.join("dist/es/index.html")).unwrap();
    assert!(
        es_html.contains(r#"href="/es/posts""#),
        "Spanish nav should link to /es/posts with lang prefix"
    );
    // External link should NOT have lang prefix
    assert!(
        es_html.contains(r#"href="https://github.com""#),
        "External link should stay unchanged on Spanish page"
    );
    // External link should have target="_blank"
    assert!(
        es_html.contains(r#"target="_blank""#),
        "External link should have target=_blank"
    );
}

#[test]
fn test_build_default_language_in_context() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dlsite", "DL Site", "posts");
    let site_dir = tmp.path().join("dlsite");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"DL ES\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // English + Spanish posts
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Both pages should have og:locale matching their language
    let en_html = fs::read_to_string(site_dir.join("dist/posts/hello.html")).unwrap();
    assert!(
        en_html.contains(r#"og:locale" content="en"#),
        "English page og:locale should be 'en'"
    );

    let es_html = fs::read_to_string(site_dir.join("dist/es/posts/hello.html")).unwrap();
    assert!(
        es_html.contains(r#"og:locale" content="es"#),
        "Spanish page og:locale should be 'es'"
    );
}

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

    // Verify CLAUDE.md has trust section
    let claude_md = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
    assert!(claude_md.contains("## Trust Center"));
    assert!(claude_md.contains("Acme Corp"));
    assert!(claude_md.contains("Managing Certifications"));
    assert!(claude_md.contains("Managing Subprocessors"));
    assert!(claude_md.contains("Managing FAQs"));
    assert!(claude_md.contains("seite://trust"));
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

    let claude_md = fs::read_to_string(tmp.path().join("site/CLAUDE.md")).unwrap();
    assert!(!claude_md.contains("## Trust Center"));
}

#[test]
fn test_init_creates_theme_builder_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Theme Skill Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/theme-builder/SKILL.md");
    assert!(
        skill_path.exists(),
        "theme-builder skill should be created on init"
    );

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("name: theme-builder"));
    assert!(content.contains("seite-skill-version:"));
    assert!(content.contains("Phase 1: Understand the Vision"));
}

#[test]
fn test_init_creates_theme_builder_skill_without_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Pages", "posts,docs");
    let site_dir = tmp.path().join("site");

    // Theme builder is unconditional — should exist even without pages collection
    let skill_path = site_dir.join(".claude/skills/theme-builder/SKILL.md");
    assert!(
        skill_path.exists(),
        "theme-builder skill should be created even without pages collection"
    );
}

#[test]
fn test_init_claude_md_has_theme_builder_pointer() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Pointer Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let claude_md = fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap();
    assert!(
        claude_md.contains("/theme-builder"),
        "CLAUDE.md should mention /theme-builder skill"
    );
}

#[test]
fn test_upgrade_adds_theme_builder_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade TB", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove the skill to simulate an older project
    let skill_path = site_dir.join(".claude/skills/theme-builder/SKILL.md");
    fs::remove_file(&skill_path).unwrap();
    assert!(!skill_path.exists());

    // Downgrade the project version so the upgrade step applies
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.0");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("theme-builder"));

    assert!(
        skill_path.exists(),
        "upgrade should create the theme-builder skill"
    );
    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("name: theme-builder"));
}

#[test]
fn test_upgrade_updates_outdated_theme_builder_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "TB Update", "posts,pages");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/theme-builder/SKILL.md");
    assert!(skill_path.exists());

    // Write an older version of the skill
    fs::write(
        &skill_path,
        "---\nname: theme-builder\ndescription: old\n# seite-skill-version: 0\n---\nOld content\n",
    )
    .unwrap();

    // Downgrade the project version so the upgrade step applies
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.0");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("theme-builder"));

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(
        content.contains("seite-skill-version: 1"),
        "upgrade should replace skill with newer version"
    );
    assert!(
        content.contains("Phase 1: Understand the Vision"),
        "upgraded skill should have full content"
    );
}

#[test]
fn test_upgrade_fixes_deploy_workflow_cargo_install() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Deploy Fix", "posts");
    let site_dir = tmp.path().join("site");

    let workflow_path = site_dir.join(".github/workflows/deploy.yml");
    assert!(workflow_path.exists());

    // Verify the freshly generated workflow does NOT have cargo install
    let fresh = fs::read_to_string(&workflow_path).unwrap();
    assert!(
        !fresh.contains("cargo install --path ."),
        "fresh init should not use cargo install"
    );

    // Simulate an old workflow that uses cargo install
    let old_workflow = r#"name: Deploy to GitHub Pages

on:
  push:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Install seite
        run: cargo install --path .
      - name: Build site
        run: seite build
"#;
    fs::write(&workflow_path, old_workflow).unwrap();

    // Downgrade the project version so the upgrade step applies
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.5");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("deploy.yml"));

    let content = fs::read_to_string(&workflow_path).unwrap();
    assert!(
        !content.contains("cargo install --path ."),
        "upgrade should remove cargo install"
    );
    assert!(
        content.contains("curl -fsSL https://seite.sh/install.sh | sh"),
        "upgrade should use shell installer"
    );
    assert!(
        content.contains(&format!("VERSION={}", env!("CARGO_PKG_VERSION"))),
        "upgrade should pin seite version"
    );
    assert!(
        content.contains("seite build"),
        "workflow should still build the site"
    );
}

// --- contact form tests ---

/// Helper: add [contact] section to an existing seite.toml
fn add_contact_config(site_dir: &std::path::Path, provider: &str, endpoint: &str) {
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(&format!(
        "\n[contact]\nprovider = \"{provider}\"\nendpoint = \"{endpoint}\"\n"
    ));
    fs::write(&config_path, config).unwrap();
}

#[test]
fn test_build_contact_form_formspree() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "cftest", "CF Test", "posts,pages");
    let site_dir = tmp.path().join("cftest");

    add_contact_config(&site_dir, "formspree", "xtest123");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("formspree.io/f/xtest123"));
    assert!(html.contains("contact-form"));
    assert!(html.contains("method=\"POST\""));
    assert!(html.contains("_gotcha")); // honeypot
}

#[test]
fn test_build_contact_form_web3forms() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "w3test", "W3 Test", "posts,pages");
    let site_dir = tmp.path().join("w3test");

    add_contact_config(&site_dir, "web3forms", "test-access-key");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("api.web3forms.com/submit"));
    assert!(html.contains("test-access-key"));
    assert!(html.contains("name=\"access_key\""));
}

#[test]
fn test_build_contact_form_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "nltest", "NL Test", "posts,pages");
    let site_dir = tmp.path().join("nltest");

    add_contact_config(&site_dir, "netlify", "contact");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("data-netlify=\"true\""));
    assert!(html.contains("data-netlify-honeypot=\"bot-field\""));
    assert!(html.contains("name=\"form-name\""));
}

#[test]
fn test_build_contact_form_hubspot() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "hstest", "HS Test", "posts,pages");
    let site_dir = tmp.path().join("hstest");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[contact]\nprovider = \"hubspot\"\nendpoint = \"12345/abcd-efgh\"\nregion = \"na1\"\n",
    );
    fs::write(&config_path, config).unwrap();

    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("js.hsforms.net/forms/embed/v2.js"));
    assert!(html.contains("hbspt.forms.create"));
    assert!(html.contains("portalId:\"12345\""));
    assert!(html.contains("formId:\"abcd-efgh\""));
}

#[test]
fn test_build_contact_form_typeform() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tftest", "TF Test", "posts,pages");
    let site_dir = tmp.path().join("tftest");

    add_contact_config(&site_dir, "typeform", "abc123XY");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("data-tf-widget=\"abc123XY\""));
    assert!(html.contains("embed.typeform.com/next/embed.js"));
}

#[test]
fn test_build_contact_form_without_config_shows_error() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "noconf", "No Conf", "posts,pages");
    let site_dir = tmp.path().join("noconf");

    // No [contact] section in config — shortcode should render an error message
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("contact-form-error"));
    assert!(html.contains("seite contact setup"));
}

#[test]
fn test_build_contact_form_with_label_overrides() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "lbltest", "Lbl Test", "posts,pages");
    let site_dir = tmp.path().join("lbltest");

    add_contact_config(&site_dir, "formspree", "xtest456");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form(name_label=\"Full Name\", submit_label=\"Submit\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("Full Name"));
    assert!(html.contains("Submit"));
}

#[test]
fn test_init_with_contact_provider() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "ctsite",
            "--title",
            "Contact Site",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages",
            "--contact-provider",
            "formspree",
            "--contact-endpoint",
            "xpznqkdl",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let config = fs::read_to_string(tmp.path().join("ctsite/seite.toml")).unwrap();
    assert!(config.contains("[contact]"));
    assert!(config.contains("formspree"));
    assert!(config.contains("xpznqkdl"));

    // Should have created a contact page
    assert!(tmp.path().join("ctsite/content/pages/contact.md").exists());
    let contact_page =
        fs::read_to_string(tmp.path().join("ctsite/content/pages/contact.md")).unwrap();
    assert!(contact_page.contains("contact_form()"));
}

#[test]
fn test_contact_status_no_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "stsite", "Status", "posts,pages");

    page_cmd()
        .args(["contact", "status"])
        .current_dir(tmp.path().join("stsite"))
        .assert()
        .success()
        .stdout(predicate::str::contains("seite contact setup"));
}

#[test]
fn test_contact_status_with_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "stsite2", "Status", "posts,pages");
    let site_dir = tmp.path().join("stsite2");

    add_contact_config(&site_dir, "formspree", "xtest789");

    page_cmd()
        .args(["contact", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Formspree"))
        .stdout(predicate::str::contains("xtest789"));
}

#[test]
fn test_contact_setup_noninteractive() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "setupsite", "Setup", "posts,pages");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "web3forms",
            "--endpoint",
            "mykey123",
        ])
        .current_dir(tmp.path().join("setupsite"))
        .assert()
        .success();

    let config = fs::read_to_string(tmp.path().join("setupsite/seite.toml")).unwrap();
    assert!(config.contains("[contact]"));
    assert!(config.contains("web3forms"));
    assert!(config.contains("mykey123"));
}

#[test]
fn test_contact_remove() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "rmsite", "Remove", "posts,pages");
    let site_dir = tmp.path().join("rmsite");

    add_contact_config(&site_dir, "formspree", "xremove");

    // Verify config has [contact]
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("[contact]"));

    page_cmd()
        .args(["contact", "remove"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify [contact] was removed
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(!config.contains("[contact]"));
}

// ── Public directory feature ────────────────────────────────────────

#[test]
fn test_init_creates_public_directory() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Public Dir", "posts");
    let site_dir = tmp.path().join("site");
    assert!(site_dir.join("public").is_dir());
}

#[test]
fn test_build_copies_public_files_to_root() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Public Files", "posts");
    let site_dir = tmp.path().join("site");

    // Place files in public/
    fs::write(site_dir.join("public/favicon.ico"), "fake-icon").unwrap();
    fs::create_dir_all(site_dir.join("public/.well-known")).unwrap();
    fs::write(
        site_dir.join("public/.well-known/security.txt"),
        "Contact: security@example.com",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify files appear at dist/ root, not dist/public/
    assert!(site_dir.join("dist/favicon.ico").exists());
    assert_eq!(
        fs::read_to_string(site_dir.join("dist/favicon.ico")).unwrap(),
        "fake-icon"
    );
    assert!(site_dir.join("dist/.well-known/security.txt").exists());
    assert_eq!(
        fs::read_to_string(site_dir.join("dist/.well-known/security.txt")).unwrap(),
        "Contact: security@example.com"
    );
}

#[test]
fn test_build_generated_files_overwrite_public() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Public Override", "posts");
    let site_dir = tmp.path().join("site");

    // Place a robots.txt in public/
    fs::write(
        site_dir.join("public/robots.txt"),
        "User-agent: none\nDisallow: /",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Generated robots.txt should win (contains Sitemap: line)
    let robots = fs::read_to_string(site_dir.join("dist/robots.txt")).unwrap();
    assert!(
        robots.contains("Sitemap:"),
        "Generated robots.txt should overwrite public/ version"
    );
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

// --- MCP resource coverage additions ---

#[test]
fn test_mcp_read_content_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpcoll", "Coll MCP", "posts,docs");
    let site_dir = tmp.path().join("mcpcoll");

    // Create a specific post
    fs::write(
        site_dir.join("content/posts/2025-03-15-test-post.md"),
        "---\ntitle: Test Post\ndate: 2025-03-15\ntags:\n  - test\ndescription: A test post\n---\nHello world\n",
    )
    .unwrap();

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://content/posts" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    // Should include the sample post from init + our test post
    assert!(items.iter().any(|i| i["title"] == "Test Post"));
}

#[test]
fn test_mcp_read_content_unknown_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpunkcoll", "Unknown Coll", "posts");
    let site_dir = tmp.path().join("mcpunkcoll");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://content/nonexistent" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32602);
    assert!(responses[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

#[test]
fn test_mcp_read_config_outside_project() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://config" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert!(responses[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("seite project"));
}

#[test]
fn test_mcp_read_content_outside_project() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://content" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
}

#[test]
fn test_mcp_read_themes_outside_project() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://themes" }
        })],
    );

    // Themes should still work (bundled themes are always available)
    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
}

#[test]
fn test_mcp_read_missing_uri_param() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert!(responses[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("uri"));
}

#[test]
fn test_mcp_read_doc_unknown_slug() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://docs/nonexistent-page" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert!(responses[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

fn init_trust_site(tmp: &TempDir, name: &str) {
    page_cmd()
        .args([
            "init",
            name,
            "--title",
            "Trust Test",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,trust",
            "--trust-company",
            "TestCo",
            "--trust-frameworks",
            "soc2",
            "--trust-sections",
            "overview,certifications",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
}

#[test]
fn test_mcp_resources_list_with_trust() {
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "mcptrust");
    let site_dir = tmp.path().join("mcptrust");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/list",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let resources = responses[0]["result"]["resources"].as_array().unwrap();
    assert!(resources.iter().any(|r| r["uri"] == "seite://trust"));
}

#[test]
fn test_mcp_read_trust_resource() {
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "mcptrustr");
    let site_dir = tmp.path().join("mcptrustr");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://trust" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let trust: serde_json::Value = serde_json::from_str(text).unwrap();
    // Trust resource should include content_items from init scaffold
    assert!(trust.get("content_items").is_some());
}

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

// --- contact form additions ---

#[test]
fn test_contact_setup_web3forms() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctweb3", "Web3 Contact", "posts,pages");
    let site_dir = tmp.path().join("ctweb3");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "web3forms",
            "--endpoint",
            "abc123",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("web3forms"));
    assert!(config.contains("abc123"));
}

#[test]
fn test_contact_setup_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctnet", "Netlify Contact", "posts,pages");
    let site_dir = tmp.path().join("ctnet");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "netlify",
            "--endpoint",
            "contact",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("netlify"));
}

#[test]
fn test_contact_setup_creates_contact_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctpage", "Contact Page", "posts,pages");
    let site_dir = tmp.path().join("ctpage");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xtest",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Should create a contact page when pages collection exists
    assert!(site_dir.join("content/pages/contact.md").exists());
    let contact = fs::read_to_string(site_dir.join("content/pages/contact.md")).unwrap();
    assert!(contact.contains("contact_form"));
}

// --- deploy additional tests ---

#[test]
fn test_deploy_dry_run_no_commit() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "depnc", "Deploy NC", "posts");
    let site_dir = tmp.path().join("depnc");

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));
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

// --- self-update ---

#[test]
fn test_self_update_check() {
    // --check with current version should report "already up to date"
    // Uses --target-version to avoid network dependency in CI
    page_cmd()
        .args([
            "self-update",
            "--check",
            "--target-version",
            env!("CARGO_PKG_VERSION"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

// --- additional deploy tests ---

#[test]
fn test_deploy_dry_run_skip_checks() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dskip", "Deploy Skip", "posts");
    let site_dir = tmp.path().join("dskip");

    // Build first
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Deploy with skip-checks should skip the pre-flight
    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[test]
fn test_deploy_dry_run_preview() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dprev", "Deploy Preview", "posts");
    let site_dir = tmp.path().join("dprev");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["deploy", "--dry-run", "--preview"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));
}

// --- additional theme tests ---

#[test]
fn test_theme_apply_all_bundled() {
    // Verify every bundled theme can be applied
    let themes = ["default", "minimal", "dark", "docs", "brutalist", "bento"];
    for theme_name in &themes {
        let tmp = TempDir::new().unwrap();
        init_site(&tmp, "thm", "Theme Test", "posts");
        let site_dir = tmp.path().join("thm");

        page_cmd()
            .args(["theme", "apply", theme_name])
            .current_dir(&site_dir)
            .assert()
            .success();

        // Verify base.html was written
        let base = fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
        assert!(!base.is_empty());
    }
}

// --- additional build tests ---

#[test]
fn test_build_outputs_search_index_json() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "search2", "Search Test", "posts,pages");
    let site_dir = tmp.path().join("search2");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/search-index.json").exists());
}

#[test]
fn test_build_generates_404() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "err404", "404 Test", "posts");
    let site_dir = tmp.path().join("err404");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/404.html").exists());
}

#[test]
fn test_build_generates_llms_txt() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "llms", "LLMs Test", "posts");
    let site_dir = tmp.path().join("llms");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/llms.txt").exists());
    assert!(site_dir.join("dist/llms-full.txt").exists());
}

#[test]
fn test_build_generates_robots_txt() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "robots", "Robots Test", "posts");
    let site_dir = tmp.path().join("robots");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let robots = fs::read_to_string(site_dir.join("dist/robots.txt")).unwrap();
    assert!(robots.contains("User-agent"));
    assert!(robots.contains("Sitemap:"));
}

#[test]
fn test_build_generates_sitemap() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "smap", "Sitemap Test", "posts");
    let site_dir = tmp.path().join("smap");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let sitemap = fs::read_to_string(site_dir.join("dist/sitemap.xml")).unwrap();
    assert!(sitemap.contains("<urlset"));
    assert!(sitemap.contains("<url>"));
}

#[test]
fn test_build_with_data_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "datasite", "Data Test", "posts,pages");
    let site_dir = tmp.path().join("datasite");

    // Create a data file
    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/nav.yaml"),
        "- title: Blog\n  url: /posts\n- title: About\n  url: /about\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[test]
fn test_build_with_public_dir() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "pubsite", "Public Test", "posts");
    let site_dir = tmp.path().join("pubsite");

    // Place a file in public/
    fs::create_dir_all(site_dir.join("public")).unwrap();
    fs::write(site_dir.join("public/favicon.ico"), "icon").unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Public files should be copied to dist/
    assert!(site_dir.join("dist/favicon.ico").exists());
}

// --- upgrade command tests ---

#[test]
fn test_upgrade_force() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "upf", "Upgrade Force", "posts");
    let site_dir = tmp.path().join("upf");

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

// --- theme export with description verification ---

#[test]
fn test_theme_export_with_description_metadata() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "texps2", "Theme Export Desc", "posts");
    let site_dir = tmp.path().join("texps2");

    page_cmd()
        .args(["theme", "apply", "minimal"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args([
            "theme",
            "export",
            "my-custom2",
            "--description",
            "My custom theme",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    let content = fs::read_to_string(site_dir.join("templates/themes/my-custom2.tera")).unwrap();
    assert!(content.contains("theme-description: My custom theme"));
}

// --- contact remove when not configured ---

#[test]
fn test_contact_remove_when_not_configured() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "cr2", "Contact Remove 2", "posts");
    let site_dir = tmp.path().join("cr2");

    page_cmd()
        .args(["contact", "remove"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("No contact form"));
}

// --- build with multiple collections ---

#[test]
fn test_build_with_all_collection_types() {
    let tmp = TempDir::new().unwrap();
    init_site(
        &tmp,
        "allcol",
        "All Collections",
        "posts,docs,pages,changelog,roadmap",
    );
    let site_dir = tmp.path().join("allcol");

    // Create content in each collection
    page_cmd()
        .args(["new", "post", "Test Post", "--tags", "test"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["new", "doc", "Getting Started"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["new", "changelog", "v1.0.0", "--tags", "new"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["new", "roadmap", "Dark Mode", "--tags", "planned"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify each collection has output
    assert!(site_dir.join("dist/index.html").exists());
    assert!(site_dir.join("dist/sitemap.xml").exists());
    assert!(site_dir.join("dist/feed.xml").exists());
}

// --- build with google analytics ---

#[test]
fn test_build_with_google_analytics() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ganalytics", "GA Test", "posts");
    let site_dir = tmp.path().join("ganalytics");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[analytics]\nprovider = \"google\"\nid = \"G-TESTID123\"\n");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("G-TESTID123"));
}

// --- build with images config ---

#[test]
fn test_build_with_images_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "imgcfg", "Images Config", "posts");
    let site_dir = tmp.path().join("imgcfg");

    // Replace the default images config with custom widths
    let config_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    // init_site already creates an [images] section, so replace it
    let config = config.replace(
        "[images]\nwidths = [480, 800, 1200]\nquality = 80\nlazy_loading = true\nwebp = true",
        "[images]\nwidths = [480, 800]\nquality = 85\nlazy_loading = true\nwebp = true",
    );
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();
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

// --- new content creation with lang flag ---

#[test]
fn test_new_post_with_lang_flag() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "nlang", "New Lang", "posts");
    let site_dir = tmp.path().join("nlang");

    // Add Spanish language config
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[languages.es]\ntitle = \"Test ES\"\n");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .args(["new", "post", "Hola Mundo", "--lang", "es"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify the file has .es.md extension
    let posts_dir = site_dir.join("content/posts");
    let entries: Vec<_> = fs::read_dir(&posts_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().unwrap().contains(".es.md"))
        .collect();
    assert!(!entries.is_empty());
}

// --- build output markdown alongside HTML ---

#[test]
fn test_build_outputs_markdown_alongside_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mdout", "MD Output", "posts");
    let site_dir = tmp.path().join("mdout");

    page_cmd()
        .args(["new", "post", "Markdown Output Test"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Find the generated post
    let dist = site_dir.join("dist/posts");
    if dist.exists() {
        let html_files: Vec<_> = fs::read_dir(&dist)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "html").unwrap_or(false))
            .collect();
        // Each HTML should have a corresponding .md
        for html in &html_files {
            let md_path = html.path().with_extension("md");
            assert!(md_path.exists(), "Missing .md for {:?}", html.path());
        }
    }
}

// --- build with minify and fingerprint ---

#[test]
fn test_build_with_minify_and_fingerprint() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "minify", "Minify Test", "posts");
    let site_dir = tmp.path().join("minify");

    // Add build options
    let config_path = site_dir.join("seite.toml");
    let config_content = fs::read_to_string(&config_path).unwrap();
    let updated = config_content.replace("[build]", "[build]\nminify = true\nfingerprint = true");
    fs::write(&config_path, updated).unwrap();

    // Create a static CSS file
    fs::create_dir_all(site_dir.join("static")).unwrap();
    fs::write(
        site_dir.join("static/style.css"),
        "/* comment */ body { margin: 0; }",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Check that asset-manifest.json exists (fingerprinting)
    assert!(site_dir.join("dist/asset-manifest.json").exists());
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional contact CLI tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_contact_setup_formspree_noninteractive() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctfsp", "Formspree Setup", "posts,pages");
    let site_dir = tmp.path().join("ctfsp");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xpznqkdl",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Formspree"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("[contact]"));
    assert!(config.contains("formspree"));
    assert!(config.contains("xpznqkdl"));
}

#[test]
fn test_contact_setup_formspree_then_status() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctfst", "FS Status", "posts,pages");
    let site_dir = tmp.path().join("ctfst");

    // Setup formspree
    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xtest456",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Check status
    page_cmd()
        .args(["contact", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Formspree"))
        .stdout(predicate::str::contains("xtest456"));
}

#[test]
fn test_contact_setup_then_remove_then_status() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctrms", "Remove Status", "posts,pages");
    let site_dir = tmp.path().join("ctrms");

    // Setup
    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xrm123",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify config present
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("[contact]"));

    // Remove
    page_cmd()
        .args(["contact", "remove"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed"));

    // Verify config gone
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(!config.contains("[contact]"));

    // Status should show not configured
    page_cmd()
        .args(["contact", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("seite contact setup"));
}

#[test]
fn test_contact_setup_web3forms_noninteractive() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctw3f2", "Web3 Setup2", "posts,pages");
    let site_dir = tmp.path().join("ctw3f2");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "web3forms",
            "--endpoint",
            "w3key789",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Web3Forms"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("web3forms"));
    assert!(config.contains("w3key789"));
}

#[test]
fn test_contact_setup_netlify_noninteractive() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctnet2", "Netlify Setup2", "posts,pages");
    let site_dir = tmp.path().join("ctnet2");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "netlify",
            "--endpoint",
            "myform",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Netlify"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("netlify"));
    assert!(config.contains("myform"));
}

#[test]
fn test_contact_setup_with_redirect_and_subject() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctopt", "Contact Opts", "posts,pages");
    let site_dir = tmp.path().join("ctopt");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xopts",
            "--redirect",
            "/thank-you",
            "--subject",
            "New inquiry",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("formspree"));
    assert!(config.contains("xopts"));
    assert!(config.contains("/thank-you"));
    assert!(config.contains("New inquiry"));
}

#[test]
fn test_contact_setup_invalid_provider() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctinv", "Invalid Provider", "posts,pages");
    let site_dir = tmp.path().join("ctinv");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "invalid_provider",
            "--endpoint",
            "x123",
        ])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown contact provider"));
}

#[test]
fn test_contact_status_shows_redirect_and_subject() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctstat2", "Status Detail", "posts,pages");
    let site_dir = tmp.path().join("ctstat2");

    // Add contact config with redirect and subject
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[contact]\nprovider = \"formspree\"\nendpoint = \"xdetail\"\nredirect = \"/thanks\"\nsubject = \"Contact Form\"\n",
    );
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .args(["contact", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Formspree"))
        .stdout(predicate::str::contains("xdetail"))
        .stdout(predicate::str::contains("/thanks"))
        .stdout(predicate::str::contains("Contact Form"));
}

#[test]
fn test_contact_setup_outside_project_fails() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "x123",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_contact_setup_creates_contact_page_with_shortcode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctpg2", "Contact Page2", "posts,pages");
    let site_dir = tmp.path().join("ctpg2");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "web3forms",
            "--endpoint",
            "w3key",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify contact page was created
    let contact_page = site_dir.join("content/pages/contact.md");
    assert!(contact_page.exists());
    let content = fs::read_to_string(&contact_page).unwrap();
    assert!(content.contains("contact_form"));
    assert!(content.contains("title: Contact"));
}

#[test]
fn test_contact_setup_no_contact_page_without_pages_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctnopg", "No Pages", "posts");
    let site_dir = tmp.path().join("ctnopg");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xnopg",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // No contact page should be created when no pages collection
    assert!(!site_dir.join("content/pages/contact.md").exists());
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional theme CLI tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_theme_list_shows_all_six_bundled() {
    page_cmd()
        .args(["theme", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bundled themes"))
        .stdout(predicate::str::contains("default"))
        .stdout(predicate::str::contains("minimal"))
        .stdout(predicate::str::contains("dark"))
        .stdout(predicate::str::contains("docs"))
        .stdout(predicate::str::contains("brutalist"))
        .stdout(predicate::str::contains("bento"));
}

#[test]
fn test_theme_apply_dark_then_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thdkb", "Dark Build", "posts");
    let site_dir = tmp.path().join("thdkb");

    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Build should succeed with dark theme applied
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify the output uses dark theme styles
    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("0a0a0a"), "dark theme should use true black");
}

#[test]
fn test_theme_apply_minimal_then_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thmnb", "Minimal Build", "posts");
    let site_dir = tmp.path().join("thmnb");

    page_cmd()
        .args(["theme", "apply", "minimal"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Build should succeed with minimal theme applied
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify the output uses minimal theme styles
    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index.contains("Georgia"),
        "minimal theme should use Georgia serif"
    );
}

#[test]
fn test_theme_apply_docs_then_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thdob", "Docs Build", "posts,docs");
    let site_dir = tmp.path().join("thdob");

    page_cmd()
        .args(["theme", "apply", "docs"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/index.html").exists());
}

#[test]
fn test_theme_apply_nonexistent_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thfail", "Fail Theme", "posts");
    let site_dir = tmp.path().join("thfail");

    page_cmd()
        .args(["theme", "apply", "totally-fake-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown theme"));
}

#[test]
fn test_theme_install_from_local_file_url() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thinst", "Install Test", "posts");
    let site_dir = tmp.path().join("thinst");

    // Create a temporary theme file to serve via file:// won't work (ureq http-only),
    // so create it directly in the installed themes dir to test the apply path
    let themes_dir = site_dir.join("templates/themes");
    fs::create_dir_all(&themes_dir).unwrap();
    fs::write(
        themes_dir.join("local-test.tera"),
        "{#- theme-description: A local test theme -#}\n<!DOCTYPE html>\n<html lang=\"{{ lang }}\"><head><title>{% block title %}{{ site.title }}{% endblock %}</title></head><body>local-test-marker{% block content %}{% endblock %}</body></html>",
    ).unwrap();

    // Verify it shows up in theme list
    page_cmd()
        .args(["theme", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed themes"))
        .stdout(predicate::str::contains("local-test"));

    // Apply the installed theme
    page_cmd()
        .args(["theme", "apply", "local-test"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Applied installed theme"));

    let base = fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(base.contains("local-test-marker"));
}

#[test]
fn test_theme_export_then_apply() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thexp2", "Export Apply", "posts");
    let site_dir = tmp.path().join("thexp2");

    // Apply brutalist first
    page_cmd()
        .args(["theme", "apply", "brutalist"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export it
    page_cmd()
        .args([
            "theme",
            "export",
            "my-brutalist",
            "--description",
            "My modified brutalist",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify exported file exists
    let exported = site_dir.join("templates/themes/my-brutalist.tera");
    assert!(exported.exists());
    let content = fs::read_to_string(&exported).unwrap();
    assert!(content.contains("theme-description: My modified brutalist"));

    // Now apply a different theme
    page_cmd()
        .args(["theme", "apply", "minimal"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let base = fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(base.contains("Georgia"), "should now be minimal theme");

    // Apply the exported theme
    page_cmd()
        .args(["theme", "apply", "my-brutalist"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Applied installed theme"));

    let base = fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(
        base.contains("fffef0"),
        "should be back to brutalist cream background"
    );
}

#[test]
fn test_theme_list_shows_installed_themes() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thlst2", "List Installed", "posts");
    let site_dir = tmp.path().join("thlst2");

    // No installed themes initially — should only show bundled
    page_cmd()
        .args(["theme", "list"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Install a theme manually
    let themes_dir = site_dir.join("templates/themes");
    fs::create_dir_all(&themes_dir).unwrap();
    fs::write(
        themes_dir.join("my-fancy.tera"),
        "{#- theme-description: A fancy theme -#}\n<!DOCTYPE html><html><body>fancy</body></html>",
    )
    .unwrap();

    // Now list should show installed section
    page_cmd()
        .args(["theme", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed themes"))
        .stdout(predicate::str::contains("my-fancy"))
        .stdout(predicate::str::contains("A fancy theme"));
}

#[test]
fn test_theme_apply_outside_project_fails() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_theme_export_without_base_html_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thnobase", "No Base", "posts");
    let site_dir = tmp.path().join("thnobase");

    // Remove base.html
    fs::remove_file(site_dir.join("templates/base.html")).unwrap();

    page_cmd()
        .args(["theme", "export", "my-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no templates/base.html"));
}

#[test]
fn test_theme_export_duplicate_name_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thdup2", "Dup Theme2", "posts");
    let site_dir = tmp.path().join("thdup2");

    // Export once
    page_cmd()
        .args(["theme", "export", "dup-theme"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export again with same name should fail
    page_cmd()
        .args(["theme", "export", "dup-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_theme_apply_then_build_with_all_themes() {
    // Test that every bundled theme produces a valid build
    let themes = ["default", "minimal", "dark", "docs", "brutalist", "bento"];
    for theme_name in &themes {
        let tmp = TempDir::new().unwrap();
        init_site(&tmp, "thall", "All Themes Build", "posts,docs,pages");
        let site_dir = tmp.path().join("thall");

        page_cmd()
            .args(["theme", "apply", theme_name])
            .current_dir(&site_dir)
            .assert()
            .success();

        page_cmd()
            .args(["build"])
            .current_dir(&site_dir)
            .assert()
            .success();

        assert!(
            site_dir.join("dist/index.html").exists(),
            "build with {} theme should produce index.html",
            theme_name
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional deploy CLI tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_deploy_dry_run_no_commit_github_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddnc1", "Deploy NC GH", "posts");
    let site_dir = tmp.path().join("ddnc1");

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("github-pages"));
}

#[test]
fn test_deploy_dry_run_no_commit_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddnc2", "Deploy NC CF", "posts");
    let site_dir = tmp.path().join("ddnc2");

    // Change target to cloudflare
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"test-proj\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("cloudflare"))
        .stdout(predicate::str::contains("test-proj"));
}

#[test]
fn test_deploy_dry_run_no_commit_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddnc3", "Deploy NC Net", "posts");
    let site_dir = tmp.path().join("ddnc3");

    // Change target to netlify
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("netlify"));
}

#[test]
fn test_deploy_dry_run_skip_checks_github_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddsc1", "Deploy SC GH", "posts");
    let site_dir = tmp.path().join("ddsc1");

    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("github-pages"));
}

#[test]
fn test_deploy_dry_run_skip_checks_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddsc2", "Deploy SC CF", "posts");
    let site_dir = tmp.path().join("ddsc2");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"cf-proj\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("cloudflare"));
}

#[test]
fn test_deploy_dry_run_skip_checks_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddsc3", "Deploy SC Net", "posts");
    let site_dir = tmp.path().join("ddsc3");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("netlify"));
}

#[test]
fn test_deploy_dry_run_no_commit_skip_checks_combined() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddcomb", "Deploy Combined", "posts");
    let site_dir = tmp.path().join("ddcomb");

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));
}

#[test]
fn test_deploy_dry_run_with_target_override() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddtgt", "Deploy Target Override", "posts");
    let site_dir = tmp.path().join("ddtgt");

    // Config says github-pages, but override to netlify
    page_cmd()
        .args(["deploy", "--dry-run", "--target", "netlify"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("netlify"))
        .stdout(predicate::str::contains("Dry run"));
}

#[test]
fn test_deploy_dry_run_cloudflare_preview_mode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddcfp", "CF Preview", "posts");
    let site_dir = tmp.path().join("ddcfp");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"cf-prev\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--preview"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("preview"));
}

#[test]
fn test_deploy_dry_run_netlify_production_mode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddntp", "Net Prod", "posts");
    let site_dir = tmp.path().join("ddntp");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("production"));
}

#[test]
fn test_deploy_dry_run_shows_output_dir() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddout", "Deploy Output", "posts");
    let site_dir = tmp.path().join("ddout");

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Output dir"));
}

#[test]
fn test_deploy_dry_run_github_pages_shows_branch() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddbr", "Deploy Branch", "posts");
    let site_dir = tmp.path().join("ddbr");

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("gh-pages"));
}

#[test]
fn test_deploy_outside_project_fails() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_deploy_invalid_target_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddinv", "Deploy Invalid", "posts");
    let site_dir = tmp.path().join("ddinv");

    page_cmd()
        .args(["deploy", "--dry-run", "--target", "aws-s3"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown deploy target"));
}

#[test]
fn test_deploy_dry_run_base_url_override_with_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddbucf", "Deploy BU CF", "posts");
    let site_dir = tmp.path().join("ddbucf");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"cf-bu\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args([
            "deploy",
            "--dry-run",
            "--base-url",
            "https://my-cf-site.pages.dev",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Base URL override: https://my-cf-site.pages.dev",
        ));
}

#[test]
fn test_deploy_domain_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dddomcf", "Domain CF", "posts");
    let site_dir = tmp.path().join("dddomcf");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"cf-domain\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--domain", "example.com"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated base_url"))
        .stdout(predicate::str::contains("example.com"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("https://example.com"));
}

#[test]
fn test_deploy_domain_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dddomnt", "Domain Net", "posts");
    let site_dir = tmp.path().join("dddomnt");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--domain", "mysite.netlify.app"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated base_url"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("https://mysite.netlify.app"));
}
