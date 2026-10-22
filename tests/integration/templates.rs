use super::common::*;

#[test]
fn test_templates_human_and_json() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Templates Site", "posts,pages");
    let dir = tmp.path().join("site");
    page_cmd()
        .arg("templates")
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Active theme: default"))
        .stdout(predicate::str::contains("templates/base.html"))
        .stdout(predicate::str::contains("Context variables"))
        .stdout(predicate::str::contains("youtube"));
    let (ok, doc) = run_json(&dir, &["templates"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["active_theme"], "default");
    assert!(doc["data"]["base_template"]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b == "content"));
    assert!(!doc["data"]["shortcodes"].as_array().unwrap().is_empty());
}

#[test]
fn test_templates_outside_project_fails() {
    let tmp = TempDir::new().unwrap();
    let (ok, doc) = run_json(tmp.path(), &["templates"]);
    assert!(!ok);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not in a seite project"));
}
