//! `seite search` (the CLI command; `search.rs` covers the built search index).

use super::common::*;

fn site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Search Site", "posts,docs");
    let dir = tmp.path().join("site");
    fs::write(
        dir.join("content/docs/rust-guide.md"),
        "---\ntitle: Rust Guide\n---\n\nAll about ownership.\n",
    )
    .unwrap();
    dir
}

#[test]
fn test_search_human_lists_matches() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    page_cmd()
        .args(["search", "ownership"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("1 match for 'ownership'"))
        .stdout(predicate::str::contains("Rust Guide"))
        .stdout(predicate::str::contains("content/docs/rust-guide.md"));
}

#[test]
fn test_search_json_with_collection_and_limit() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    let (ok, doc) = run_json(
        &dir,
        &["search", "rust", "--collection", "doc", "--limit", "5"],
    );
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["total"], 1);
    assert_eq!(doc["data"]["results"][0]["url"], "/docs/rust-guide");
    let (ok, doc) = run_json(&dir, &["search", "zzzz-nothing"]);
    assert!(ok);
    assert_eq!(doc["data"]["total"], 0);
}

#[test]
fn test_search_errors() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    page_cmd()
        .args(["search", "x", "--collection", "nope"])
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown collection 'nope'"));
    let (ok, doc) = run_json(&dir, &["search", "x", "--limit", "500"]);
    assert!(!ok);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("'limit' must be between 1 and 100"));
    page_cmd()
        .args(["search", "x"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not in a seite project"));
}
