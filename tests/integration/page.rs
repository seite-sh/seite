use super::common::*;

fn site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Page Site", "posts,pages");
    let dir = tmp.path().join("site");
    fs::write(
        dir.join("content/posts/2026-01-02-hello.md"),
        "---\ntitle: Hello\ntags: [rust]\n---\n\nHello **world**.\n",
    )
    .unwrap();
    dir
}

#[test]
fn test_page_by_path_human() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    page_cmd()
        .args(["page", "content/posts/2026-01-02-hello.md"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("title:       Hello"))
        .stdout(predicate::str::contains("url:         /posts/hello"))
        .stdout(predicate::str::contains("bytes rendered"))
        // The HTML itself only with --html.
        .stdout(predicate::str::contains("<strong>").not());
}

#[test]
fn test_page_by_url_json_and_html_flag() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    let (ok, doc) = run_json(&dir, &["page", "/posts/hello"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["path"], "content/posts/2026-01-02-hello.md");
    assert_eq!(doc["data"]["frontmatter"]["title"], "Hello");
    assert!(doc["data"]["html"]
        .as_str()
        .unwrap()
        .contains("<strong>world</strong>"));
    page_cmd()
        .args(["page", "/posts/hello", "--html"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("<strong>world</strong>"));
}

#[test]
fn test_page_path_relative_to_subdirectory() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    let (ok, doc) = run_json(&dir.join("content/posts"), &["page", "2026-01-02-hello.md"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["url"], "/posts/hello");
}

#[test]
fn test_page_unknown_target_fails() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    page_cmd()
        .args(["page", "/posts/nope"])
        .current_dir(&dir)
        .assert()
        .failure();
    let (ok, doc) = run_json(&dir, &["page", "content/posts/missing.md"]);
    assert!(!ok);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("no content file"));
}

#[test]
fn test_page_outside_project_fails() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["page", "/x"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not in a seite project"));
}
