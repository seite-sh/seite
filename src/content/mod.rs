use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::{PageError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<NaiveDate>,
    /// Last-modified date — used in JSON-LD `dateModified` and sitemap `<lastmod>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Absolute URL or path to a social-preview image (og:image / twitter:image).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub draft: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Per-page `<meta name="robots">` value, e.g. `"noindex"` or `"noindex, nofollow"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub robots: Option<String>,
    /// Ordering weight for non-date collections. Lower values sort first.
    /// When unset, items sort after weighted items, alphabetically by title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<i32>,
    /// Arbitrary key-value data passed through to templates as `page.extra`.
    /// Use this for custom per-page data that doesn't fit standard fields.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_yaml_ng::Value>,
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
    pub lang: String,
    /// Auto-extracted excerpt (raw markdown): `<!-- more -->` marker or first paragraph.
    pub excerpt: String,
    /// Table of contents extracted from heading hierarchy.
    pub toc: Vec<crate::build::markdown::TocEntry>,
    /// Pre-computed word count of the raw markdown body.
    pub word_count: usize,
    /// Pre-computed estimated reading time in minutes (238 WPM).
    pub reading_time: usize,
    /// Pre-rendered excerpt HTML.
    pub excerpt_html: String,
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

/// Extract a language suffix from a filename, only if it matches a configured language.
/// Example: "about.es.md" → Some("es") (if "es" is configured)
/// Example: "about.md" → None
/// Example: "about.min.md" → None (if "min" is not a configured language)
pub fn extract_lang_from_filename(path: &Path, configured_langs: &HashSet<&str>) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    if let Some(dot_pos) = stem.rfind('.') {
        let suffix = &stem[dot_pos + 1..];
        if configured_langs.contains(suffix) {
            return Some(suffix.to_string());
        }
    }
    None
}

/// Strip a language suffix from a file stem, only if it matches a configured language.
/// Example: "about.es" → "about" (if "es" is configured)
/// Example: "2025-01-15-hello.fr" → "2025-01-15-hello" (if "fr" is configured)
/// Example: "about" → "about"
pub fn strip_lang_suffix<'a>(stem: &'a str, configured_langs: &HashSet<&str>) -> &'a str {
    if let Some(dot_pos) = stem.rfind('.') {
        let suffix = &stem[dot_pos + 1..];
        if configured_langs.contains(suffix) {
            return &stem[..dot_pos];
        }
    }
    stem
}

/// Extract an excerpt from raw markdown content.
/// Checks for `<!-- more -->` marker first; falls back to the first paragraph
/// (text before the first blank line).
pub fn extract_excerpt(raw_body: &str) -> String {
    // Check for <!-- more --> marker
    if let Some(pos) = raw_body.find("<!-- more -->") {
        return raw_body[..pos].trim().to_string();
    }
    // Fall back to first non-empty paragraph (before first blank line)
    raw_body
        .split("\n\n")
        .find(|p| !p.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
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

    #[test]
    fn test_extract_lang_from_filename() {
        let langs: HashSet<&str> = ["es", "fr", "de"].into_iter().collect();

        assert_eq!(
            extract_lang_from_filename(Path::new("about.es.md"), &langs),
            Some("es".to_string())
        );
        assert_eq!(
            extract_lang_from_filename(Path::new("2025-01-15-hello.fr.md"), &langs),
            Some("fr".to_string())
        );
        assert_eq!(
            extract_lang_from_filename(Path::new("about.md"), &langs),
            None
        );
        // "min" is not a configured language
        assert_eq!(
            extract_lang_from_filename(Path::new("readme.min.md"), &langs),
            None
        );
    }

    #[test]
    fn test_strip_lang_suffix() {
        let langs: HashSet<&str> = ["es", "fr"].into_iter().collect();

        assert_eq!(strip_lang_suffix("about.es", &langs), "about");
        assert_eq!(
            strip_lang_suffix("2025-01-15-hello.fr", &langs),
            "2025-01-15-hello"
        );
        assert_eq!(strip_lang_suffix("about", &langs), "about");
        assert_eq!(strip_lang_suffix("readme.min", &langs), "readme.min");
    }

    #[test]
    fn test_extract_excerpt_more_marker() {
        let body =
            "First paragraph here.\n\nSecond paragraph.\n\n<!-- more -->\n\nThird paragraph.";
        assert_eq!(
            extract_excerpt(body),
            "First paragraph here.\n\nSecond paragraph."
        );
    }

    #[test]
    fn test_extract_excerpt_first_paragraph() {
        let body = "This is the intro paragraph.\n\nThis is the second paragraph.\n\nAnd a third.";
        assert_eq!(extract_excerpt(body), "This is the intro paragraph.");
    }

    #[test]
    fn test_extract_excerpt_empty() {
        assert_eq!(extract_excerpt(""), "");
    }

    #[test]
    fn test_extract_excerpt_single_paragraph() {
        assert_eq!(extract_excerpt("Just one paragraph"), "Just one paragraph");
    }
}
