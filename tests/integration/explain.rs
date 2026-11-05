use super::common::*;

#[test]
fn test_explain_known_code() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["explain", "broken-link"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## broken-link"))
        .stdout(predicate::str::contains("How to fix"))
        .stdout(predicate::str::contains(
            "https://seite.sh/docs/errors#broken-link",
        ));
    // As printed in diagnostics.
    let (ok, doc) = run_json(tmp.path(), &["explain", "warning[config-unknown-key]"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["code"], "config-unknown-key");
    assert_eq!(
        doc["data"]["docs_url"],
        "https://seite.sh/docs/errors#config-unknown-key"
    );
    assert!(doc["data"]["explanation"]
        .as_str()
        .unwrap()
        .contains("What it means"));
}

#[test]
fn test_explain_unknown_code_suggests_close_matches() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["explain", "broken-lnk"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unknown diagnostic code 'broken-lnk'",
        ))
        .stderr(predicate::str::contains("did you mean 'broken-link'?"))
        .stderr(predicate::str::contains("Known codes:"));
}

/// Every diagnostic code constructed in `src/` (outside test modules) must
/// have a `## <code>` section in the error reference that `seite explain`
/// and the `docs_url` of each diagnostic point at.
#[test]
fn test_every_emitted_diagnostic_code_is_documented() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let errors_md = fs::read_to_string(root.join("seite-sh/content/docs/errors.md")).unwrap();
    let documented: Vec<&str> = errors_md
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .map(str::trim)
        .collect();

    let mut emitted = std::collections::BTreeSet::new();
    for entry in walk(&root.join("src")) {
        let source = fs::read_to_string(&entry).unwrap();
        // Test modules use placeholder codes ("e", "x", ...).
        let source = source.split("#[cfg(test)]").next().unwrap_or("");
        for marker in ["Diagnostic::error(", "Diagnostic::warning("] {
            for (idx, _) in source.match_indices(marker) {
                let rest = source[idx + marker.len()..].trim_start();
                if let Some(rest) = rest.strip_prefix('"') {
                    if let Some(end) = rest.find('"') {
                        emitted.insert(rest[..end].to_string());
                    }
                }
            }
        }
    }
    // Codes chosen at runtime (`let (code, ..) = match ...` in links.rs).
    emitted.insert("broken-link".to_string());
    emitted.insert("missing-asset".to_string());
    // `seite audit` findings (constructed through `audit::CODES`, which a
    // unit test checks against the checks' sources).
    for info in seite::audit::CODES {
        emitted.insert(info.code.to_string());
    }
    assert!(emitted.len() >= 18, "scan found too few codes: {emitted:?}");

    let missing: Vec<&String> = emitted
        .iter()
        .filter(|c| !documented.contains(&c.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "codes without a `## <code>` section in errors.md: {missing:?}"
    );
    let stale: Vec<&&str> = documented
        .iter()
        .filter(|c| !emitted.contains(**c))
        .collect();
    assert!(
        stale.is_empty(),
        "errors.md documents codes nothing emits: {stale:?}"
    );
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}
