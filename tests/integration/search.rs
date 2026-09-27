use super::common::*;

// --- search index ---

#[test]
fn test_build_generates_search_index() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Search Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // search-index.json must exist
    let index_path = site_dir.join("dist/search-index.json");
    assert!(index_path.exists());

    let index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).unwrap()).unwrap();

    // The scaffolded "Hello World" post should appear
    let arr = index.as_array().unwrap();
    assert!(!arr.is_empty());
    let titles: Vec<&str> = arr
        .iter()
        .map(|e| e["title"].as_str().unwrap_or(""))
        .collect();
    assert!(
        titles.contains(&"Hello World"),
        "expected Hello World in search index"
    );
}

#[test]
fn test_build_search_index_schema() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Schema Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let raw = fs::read_to_string(site_dir.join("dist/search-index.json")).unwrap();
    let index: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let entry = &index[0];

    // All required fields must be present
    assert!(entry.get("title").is_some());
    assert!(entry.get("url").is_some());
    assert!(entry.get("collection").is_some());
    assert!(entry.get("tags").is_some());
    assert!(entry.get("lang").is_some());
}

#[test]
fn test_build_search_index_excludes_drafts() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Draft Search Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create a draft post
    let draft = "---\ntitle: Secret Draft\ndate: 2025-01-01\ndraft: true\n---\n\nNot published.\n";
    fs::write(site_dir.join("content/posts/2025-01-01-secret.md"), draft).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let raw = fs::read_to_string(site_dir.join("dist/search-index.json")).unwrap();
    assert!(
        !raw.contains("Secret Draft"),
        "draft post should not appear in search index"
    );
}

#[test]
fn test_build_search_index_multilingual() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Multilingual Search", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Búsqueda multilingüe");

    let es_post = "---\ntitle: Hola Mundo\ndate: 2025-01-15\n---\n\nContenido.\n";
    fs::write(
        site_dir.join("content/posts/2025-01-15-hola-mundo.es.md"),
        es_post,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default language index at root
    let root_index_path = site_dir.join("dist/search-index.json");
    assert!(root_index_path.exists());
    let root_raw = fs::read_to_string(&root_index_path).unwrap();
    assert!(root_raw.contains("Hello World"));
    assert!(!root_raw.contains("Hola Mundo"));

    // Per-language index for Spanish
    let es_index_path = site_dir.join("dist/es/search-index.json");
    assert!(es_index_path.exists());
    let es_raw = fs::read_to_string(&es_index_path).unwrap();
    assert!(es_raw.contains("Hola Mundo"));
    assert!(!es_raw.contains("Hello World"));
}

// --- additional build tests ---

#[test]
fn test_build_outputs_search_index_json() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "search2", "Search Test", "posts,pages");
    let site_dir = tmp.path().join("search2");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/search-index.json").exists());
}
