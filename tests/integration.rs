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

// --- theme command ---

#[test]
fn test_theme_list() {
    page_cmd()
        .args(["theme", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("default"));
}
