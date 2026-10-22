use super::common::*;

fn site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Stats Site", "posts,docs");
    let dir = tmp.path().join("site");
    fs::write(
        dir.join("content/posts/2026-01-05-draft.md"),
        "---\ntitle: Draft\ndraft: true\n---\n\nWIP.\n",
    )
    .unwrap();
    dir
}

#[test]
fn test_stats_human_and_json() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    page_cmd()
        .arg("stats")
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("2 items, 1 drafts"))
        .stdout(predicate::str::contains("missing description"))
        .stdout(predicate::str::contains("Total:"));
    let (ok, doc) = run_json(&dir, &["stats"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["totals"]["items"], 2);
    assert_eq!(doc["data"]["totals"]["drafts"], 1);
}

#[test]
fn test_stats_single_collection_and_errors() {
    let tmp = TempDir::new().unwrap();
    let dir = site(&tmp);
    let (ok, doc) = run_json(&dir, &["stats", "--collection", "docs"]);
    assert!(ok);
    let collections = doc["data"]["collections"].as_array().unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0]["name"], "docs");
    page_cmd()
        .args(["stats", "--collection", "nope"])
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown collection"));
    page_cmd()
        .arg("stats")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not in a seite project"));
}
