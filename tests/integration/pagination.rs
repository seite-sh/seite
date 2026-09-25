use super::common::*;

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
