use super::common::*;

const POST: &str = "content/posts/2026-01-02-hello.md";

fn site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "FM Site", "posts");
    let dir = tmp.path().join("site");
    fs::write(
        dir.join(POST),
        "---\ntitle: Hello # keep me\nimage: /a.png\n---\n\nBody **stays**.\n",
    )
    .unwrap();
    dir
}

#[test]
fn test_frontmatter_set_parses_values_and_keeps_body() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    page_cmd()
        .args([
            "frontmatter",
            "set",
            POST,
            "draft=true",
            "tags=[rust, web]",
            "description=Note: a colon",
            "weight=3",
        ])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated frontmatter"));
    let raw = fs::read_to_string(dir.join(POST)).unwrap();
    assert!(raw.contains("title: Hello # keep me"), "{raw}");
    assert!(raw.ends_with("---\n\nBody **stays**.\n"), "{raw}");
    let (ok, doc) = run_json(&dir, &["page", POST]);
    assert!(ok);
    let fm = &doc["data"]["frontmatter"];
    assert_eq!(fm["draft"], true);
    assert_eq!(fm["tags"], serde_json::json!(["rust", "web"]));
    assert_eq!(fm["description"], "Note: a colon");
    assert_eq!(fm["weight"], 3);
}

#[test]
fn test_frontmatter_unset_json_and_noop() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    let (ok, doc) = run_json(&dir, &["frontmatter", "unset", POST, "image"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["changed"], true);
    assert!(doc["data"]["frontmatter"].get("image").is_none());
    // Unsetting again changes nothing.
    let (ok, doc) = run_json(&dir, &["frontmatter", "unset", POST, "image"]);
    assert!(ok);
    assert_eq!(doc["data"]["changed"], false);
}

#[test]
fn test_frontmatter_errors() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    page_cmd()
        .args(["frontmatter", "set", POST, "novalue"])
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected KEY=VALUE"));
    page_cmd()
        .args(["frontmatter", "unset", POST, "title"])
        .current_dir(&dir)
        .assert()
        .failure();
    page_cmd()
        .args(["frontmatter", "set", "seite.toml", "a=b"])
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("outside the content directory"));
    page_cmd()
        .args(["frontmatter", "set", POST, "a=b"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not in a seite project"));
}
