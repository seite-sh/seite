use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::{PageError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub draft: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Debug, Clone)]
pub struct ContentItem {
    pub frontmatter: Frontmatter,
    pub raw_body: String,
    pub html_body: String,
    pub source_path: PathBuf,
    pub slug: String,
    pub collection: String,
    pub url: String,
}

/// Parse a markdown file with YAML frontmatter delimited by `---`.
pub fn parse_content_file(path: &Path) -> Result<(Frontmatter, String)> {
    let raw = std::fs::read_to_string(path)?;
    let (fm_str, body) = split_frontmatter(&raw).ok_or_else(|| PageError::Content {
        path: path.to_path_buf(),
        message: "missing frontmatter delimiters".into(),
    })?;
    let frontmatter: Frontmatter =
        serde_yaml_ng::from_str(fm_str).map_err(|e| PageError::Frontmatter {
            path: path.to_path_buf(),
            source: e,
        })?;
    Ok((frontmatter, body.to_string()))
}

fn split_frontmatter(raw: &str) -> Option<(&str, &str)> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_first = &trimmed[3..];
    let end = after_first.find("---")?;
    let fm = &after_first[..end];
    let body = &after_first[end + 3..];
    Some((
        fm.trim(),
        body.trim_start_matches('\n').trim_start_matches('\r'),
    ))
}

/// Serialize frontmatter back to a YAML string wrapped in `---` delimiters.
pub fn generate_frontmatter(fm: &Frontmatter) -> String {
    let yaml = serde_yaml_ng::to_string(fm).unwrap_or_default();
    format!("---\n{}---", yaml)
}

/// Generate a URL-safe slug from a title.
pub fn slug_from_title(title: &str) -> String {
    slug::slugify(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_frontmatter_valid() {
        let raw = "---\ntitle: Hello\n---\nBody content here.";
        let (fm, body) = split_frontmatter(raw).unwrap();
        assert_eq!(fm, "title: Hello");
        assert_eq!(body, "Body content here.");
    }

    #[test]
    fn test_split_frontmatter_missing() {
        assert!(split_frontmatter("No frontmatter here").is_none());
    }

    #[test]
    fn test_slug_generation() {
        assert_eq!(slug_from_title("Hello World!"), "hello-world");
        assert_eq!(slug_from_title("Rust & WebAssembly"), "rust-webassembly");
        assert_eq!(slug_from_title("My First Post"), "my-first-post");
    }

    #[test]
    fn test_generate_and_parse_frontmatter() {
        let fm = Frontmatter {
            title: "Test Post".into(),
            date: Some(NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()),
            tags: vec!["rust".into(), "web".into()],
            draft: false,
            ..Default::default()
        };
        let generated = generate_frontmatter(&fm);
        assert!(generated.starts_with("---\n"));
        assert!(generated.ends_with("---"));
        assert!(generated.contains("title: Test Post"));
    }

    #[test]
    fn test_frontmatter_skips_empty_fields() {
        let fm = Frontmatter {
            title: "Minimal".into(),
            ..Default::default()
        };
        let generated = generate_frontmatter(&fm);
        assert!(generated.contains("title: Minimal"));
        assert!(!generated.contains("date:"));
        assert!(!generated.contains("description:"));
        assert!(!generated.contains("slug:"));
        assert!(!generated.contains("tags:"));
        assert!(!generated.contains("draft:"));
        assert!(!generated.contains("template:"));
    }

    #[test]
    fn test_frontmatter_includes_draft_when_true() {
        let fm = Frontmatter {
            title: "Draft Post".into(),
            draft: true,
            ..Default::default()
        };
        let generated = generate_frontmatter(&fm);
        assert!(generated.contains("draft: true"));
    }
}
