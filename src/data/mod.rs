use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;

use crate::error::{PageError, Result};

/// Load all data files from the given directory, returning a nested
/// `serde_json::Value::Object` representing the `{{ data }}` template context.
///
/// Files are keyed by filename without extension. Nested directories
/// create nested objects: `data/nav/main.yaml` → `data.nav.main`.
///
/// Returns `Ok(Value::Object(empty))` if the directory doesn't exist.
pub fn load_data_dir(data_dir: &Path) -> Result<serde_json::Value> {
    let mut root = serde_json::Map::new();

    if !data_dir.exists() {
        return Ok(serde_json::Value::Object(root));
    }

    // Collect all data files to check for conflicts before parsing
    let mut files: Vec<(Vec<String>, PathBuf)> = Vec::new();

    for entry in WalkDir::new(data_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        // Only process known extensions
        match ext.as_deref() {
            Some("yaml" | "yml" | "json" | "toml") => {}
            _ => {
                eprintln!(
                    "Warning: skipping unknown data file format: {}",
                    path.display()
                );
                continue;
            }
        }

        let rel = path.strip_prefix(data_dir).map_err(|_| PageError::Data {
            path: path.to_path_buf(),
            message: "failed to compute relative path".into(),
        })?;

        // Build key segments: directory components + filename without extension
        let mut segments: Vec<String> = Vec::new();
        if let Some(parent) = rel.parent() {
            for component in parent.components() {
                if let Component::Normal(s) = component {
                    segments.push(s.to_string_lossy().to_string());
                }
            }
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PageError::Data {
                path: path.to_path_buf(),
                message: "invalid filename".into(),
            })?;
        segments.push(stem.to_string());

        files.push((segments, path.to_path_buf()));
    }

    // Check for key conflicts (e.g., authors.yaml and authors.json)
    check_conflicts(&files)?;

    // Parse each file and insert into the nested map
    for (segments, path) in &files {
        let value = parse_data_file(path)?;
        insert_nested(&mut root, segments, value, path)?;
    }

    Ok(serde_json::Value::Object(root))
}

/// Returns the number of data files that would be loaded from the directory.
pub fn count_data_files(data_dir: &Path) -> usize {
    if !data_dir.exists() {
        return 0;
    }
    WalkDir::new(data_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            matches!(
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase())
                    .as_deref(),
                Some("yaml" | "yml" | "json" | "toml")
            )
        })
        .count()
}

/// Parse a single data file into a `serde_json::Value`.
fn parse_data_file(path: &Path) -> Result<serde_json::Value> {
    let content = std::fs::read_to_string(path)?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "yaml" | "yml" => serde_yaml_ng::from_str::<serde_json::Value>(&content).map_err(|e| {
            PageError::Data {
                path: path.to_path_buf(),
                message: format!("invalid YAML: {e}"),
            }
        }),
        "json" => serde_json::from_str::<serde_json::Value>(&content).map_err(|e| {
            PageError::Data {
                path: path.to_path_buf(),
                message: format!("invalid JSON: {e}"),
            }
        }),
        "toml" => {
            let toml_value: toml::Value =
                content
                    .parse()
                    .map_err(|e: toml::de::Error| PageError::Data {
                        path: path.to_path_buf(),
                        message: format!("invalid TOML: {e}"),
                    })?;
            serde_json::to_value(toml_value).map_err(|e| PageError::Data {
                path: path.to_path_buf(),
                message: format!("TOML conversion error: {e}"),
            })
        }
        _ => Err(PageError::Data {
            path: path.to_path_buf(),
            message: format!("unsupported file extension: .{ext}"),
        }),
    }
}

/// Check for key conflicts where two files resolve to the same key path.
fn check_conflicts(files: &[(Vec<String>, PathBuf)]) -> Result<()> {
    let mut seen: HashMap<Vec<String>, &Path> = HashMap::new();
    for (segments, path) in files {
        if let Some(existing) = seen.get(segments) {
            return Err(PageError::Data {
                path: path.to_path_buf(),
                message: format!(
                    "data key conflict: '{}' and '{}' both resolve to data.{}",
                    existing.display(),
                    path.display(),
                    segments.join(".")
                ),
            });
        }
        seen.insert(segments.clone(), path);
    }
    Ok(())
}

/// Insert a value into a nested JSON object at the given key path.
fn insert_nested(
    root: &mut serde_json::Map<String, serde_json::Value>,
    segments: &[String],
    value: serde_json::Value,
    source_path: &Path,
) -> Result<()> {
    if segments.is_empty() {
        return Ok(());
    }
    if segments.len() == 1 {
        root.insert(segments[0].clone(), value);
        return Ok(());
    }

    let key = &segments[0];
    let child = root
        .entry(key.clone())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

    match child.as_object_mut() {
        Some(map) => insert_nested(map, &segments[1..], value, source_path),
        None => Err(PageError::Data {
            path: source_path.to_path_buf(),
            message: format!(
                "key '{}' is both a file and a directory in the data path",
                key
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_yaml() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path();
        std::fs::write(
            data_dir.join("authors.yaml"),
            "- name: Alice\n  role: Editor\n- name: Bob\n  role: Writer\n",
        )
        .unwrap();

        let data = load_data_dir(data_dir).unwrap();
        let authors = data.get("authors").unwrap();
        assert!(authors.is_array());
        assert_eq!(authors.as_array().unwrap().len(), 2);
        assert_eq!(authors[0]["name"], "Alice");
        assert_eq!(authors[1]["name"], "Bob");
    }

    #[test]
    fn test_load_yml_extension() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path();
        std::fs::write(data_dir.join("config.yml"), "key: value\n").unwrap();

        let data = load_data_dir(data_dir).unwrap();
        assert_eq!(data["config"]["key"], "value");
    }

    #[test]
    fn test_load_json() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path();
        std::fs::write(
            data_dir.join("social.json"),
            r#"[{"name": "GitHub", "url": "https://github.com"}]"#,
        )
        .unwrap();

        let data = load_data_dir(data_dir).unwrap();
        let social = data.get("social").unwrap();
        assert!(social.is_array());
        assert_eq!(social[0]["name"], "GitHub");
    }

    #[test]
    fn test_load_toml() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path();
        std::fs::write(
            data_dir.join("settings.toml"),
            "show_banner = true\nmax_posts = 10\n",
        )
        .unwrap();

        let data = load_data_dir(data_dir).unwrap();
        assert_eq!(data["settings"]["show_banner"], true);
        assert_eq!(data["settings"]["max_posts"], 10);
    }

    #[test]
    fn test_nested_directories() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path();
        std::fs::create_dir_all(data_dir.join("nav")).unwrap();
        std::fs::write(
            data_dir.join("nav/main.yaml"),
            "- title: Home\n  url: /\n- title: About\n  url: /about\n",
        )
        .unwrap();

        let data = load_data_dir(data_dir).unwrap();
        let nav_main = &data["nav"]["main"];
        assert!(nav_main.is_array());
        assert_eq!(nav_main[0]["title"], "Home");
    }

    #[test]
    fn test_conflict_detection() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path();
        std::fs::write(data_dir.join("authors.yaml"), "- name: Alice\n").unwrap();
        std::fs::write(
            data_dir.join("authors.json"),
            r#"[{"name": "Bob"}]"#,
        )
        .unwrap();

        let result = load_data_dir(data_dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("data key conflict"), "error was: {err}");
    }

    #[test]
    fn test_missing_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("nonexistent");

        let data = load_data_dir(&data_dir).unwrap();
        assert!(data.is_object());
        assert!(data.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let data = load_data_dir(tmp.path()).unwrap();
        assert!(data.is_object());
        assert!(data.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_unknown_extension_skipped() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path();
        std::fs::write(data_dir.join("readme.txt"), "This is not data").unwrap();
        std::fs::write(data_dir.join("config.yaml"), "key: value\n").unwrap();

        let data = load_data_dir(data_dir).unwrap();
        // txt file should be skipped, yaml should be loaded
        assert!(data.get("readme").is_none());
        assert_eq!(data["config"]["key"], "value");
    }

    #[test]
    fn test_count_data_files() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path();
        std::fs::write(data_dir.join("a.yaml"), "key: val\n").unwrap();
        std::fs::write(data_dir.join("b.json"), "{}").unwrap();
        std::fs::write(data_dir.join("c.toml"), "k = 1\n").unwrap();
        std::fs::write(data_dir.join("d.txt"), "ignored").unwrap();

        assert_eq!(count_data_files(data_dir), 3);
    }

    #[test]
    fn test_count_data_files_missing_dir() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(count_data_files(&tmp.path().join("nonexistent")), 0);
    }
}
