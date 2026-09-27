use super::common::*;

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

// --- Atom feed ---

#[test]
fn test_atom_feed_valid() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Atom Test", "posts,pages");

    page_cmd()
        .arg("build")
        .current_dir(tmp.path().join("site"))
        .assert()
        .success();

    let atom = fs::read_to_string(tmp.path().join("site/dist/atom.xml")).unwrap();
    assert!(atom.contains("xmlns=\"http://www.w3.org/2005/Atom\""));
    assert!(atom.contains("<title>Atom Test</title>"));
    assert!(atom.contains("Hello World"));
    assert!(atom.contains("rel=\"self\""));
    assert!(atom.contains("atom.xml"));
    assert!(atom.contains("<updated>"));
}

#[test]
fn test_atom_feed_excludes_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Atom Docs Test", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    let doc = "---\ntitle: My Doc\n---\n\nDoc content.\n";
    fs::write(site_dir.join("content/docs/my-doc.md"), doc).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let atom = fs::read_to_string(site_dir.join("dist/atom.xml")).unwrap();
    assert!(atom.contains("Hello World")); // posts are in Atom
    assert!(!atom.contains("My Doc")); // docs are NOT in Atom
}

#[test]
fn test_atom_autodiscovery_in_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Atom Disc", "posts,pages");

    page_cmd()
        .arg("build")
        .current_dir(tmp.path().join("site"))
        .assert()
        .success();

    let index = fs::read_to_string(tmp.path().join("site/dist/index.html")).unwrap();
    assert!(index.contains("application/atom+xml"));
    assert!(index.contains("/atom.xml"));
}

// --- Redirects ---

#[test]
fn test_redirects_from_aliases() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Redirect Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create a post with aliases
    let post =
        "---\ntitle: New Post\naliases:\n  - /old-post\n  - /legacy/path\n---\n\nNew content.\n";
    fs::write(site_dir.join("content/posts/2026-01-01-new-post.md"), post).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // HTML redirect files should be generated
    let redirect1 = fs::read_to_string(site_dir.join("dist/old-post.html")).unwrap();
    assert!(redirect1.contains("http-equiv=\"refresh\""));
    assert!(redirect1.contains("/posts/new-post"));
    assert!(redirect1.contains("rel=\"canonical\""));

    let redirect2 = fs::read_to_string(site_dir.join("dist/legacy/path.html")).unwrap();
    assert!(redirect2.contains("/posts/new-post"));

    // _redirects file should exist
    let redirects_file = fs::read_to_string(site_dir.join("dist/_redirects")).unwrap();
    assert!(redirects_file.contains("/old-post /posts/new-post 301"));
    assert!(redirects_file.contains("/legacy/path /posts/new-post 301"));
}

#[test]
fn test_no_redirects_without_aliases() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Redirect", "posts,pages");

    page_cmd()
        .arg("build")
        .current_dir(tmp.path().join("site"))
        .assert()
        .success();

    // _redirects should not exist when no aliases are defined
    assert!(!tmp.path().join("site/dist/_redirects").exists());
}

// --- Sitemap ---

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
