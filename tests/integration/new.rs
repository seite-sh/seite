use super::common::*;

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

#[test]
fn test_new_refuses_to_overwrite_existing_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Clobber", "roadmap");
    let site_dir = tmp.path().join("site");
    let existing = site_dir.join("content/roadmap/dark-mode.md");
    let before = fs::read_to_string(&existing).unwrap();

    page_cmd()
        .args(["new", "roadmap", "Dark Mode"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    assert_eq!(fs::read_to_string(&existing).unwrap(), before);
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

#[test]
fn test_new_lang_json_url_matches_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Lang Urls", "posts,pages");
    let site = tmp.path().join("site");
    add_language(&site, "es", "Sitio");

    let url_of = |args: &[&str]| {
        let output = page_cmd().args(args).current_dir(&site).output().unwrap();
        assert!(output.status.success());
        json_stdout(&output)["data"]["url"]
            .as_str()
            .unwrap()
            .to_string()
    };
    // Default language: no prefix.
    let en = url_of(&["--json", "new", "page", "English Page", "--lang", "en"]);
    assert_eq!(en, "/english-page");
    // Non-default language: prefixed.
    let es = url_of(&["--json", "new", "page", "Pagina Uno", "--lang", "es"]);
    assert_eq!(es, "/es/pagina-uno");
    let post = url_of(&["--json", "new", "post", "Hola Mundo", "--lang", "es"]);
    assert_eq!(post, "/es/posts/hola-mundo");

    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    for url in [&en, &es, &post] {
        let html = site
            .join("dist")
            .join(format!("{}.html", url.trim_matches('/')));
        assert!(html.exists(), "{} missing", html.display());
    }
}
