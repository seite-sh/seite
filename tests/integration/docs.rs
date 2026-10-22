use super::common::*;

#[test]
fn test_docs_lists_topics_outside_project() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .arg("docs")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("configuration"))
        .stdout(predicate::str::contains("errors"));
    let (ok, doc) = run_json(tmp.path(), &["docs"]);
    assert!(ok);
    let topics = doc["data"]["available_topics"].as_array().unwrap();
    assert!(topics.iter().any(|t| t["slug"] == "errors"));
}

#[test]
fn test_docs_topic_prints_page() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["docs", "--topic", "configuration"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Configuration"))
        .stdout(predicate::str::contains("base_url"));
    let (ok, doc) = run_json(tmp.path(), &["docs", "--topic", "configuration"]);
    assert!(ok);
    assert_eq!(doc["data"]["found"], true);
    assert_eq!(doc["data"]["topic"], "configuration");
}

#[test]
fn test_docs_query_searches_sections() {
    let tmp = TempDir::new().unwrap();
    let (ok, doc) = run_json(tmp.path(), &["docs", "minify"]);
    assert!(ok);
    assert!(doc["data"]["count"].as_u64().unwrap() >= 1);
    assert!(!doc["data"]["results"].as_array().unwrap().is_empty());
    page_cmd()
        .args(["docs", "minify"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("for 'minify'"));
}

#[test]
fn test_docs_unknown_topic_fails() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["docs", "--topic", "nope"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Documentation topic not found: nope",
        ));
    let (ok, doc) = run_json(tmp.path(), &["docs", "--topic", "nope"]);
    assert!(!ok);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Available topics"));
}
