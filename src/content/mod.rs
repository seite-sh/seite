use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::{PageError, Result};

pub mod create;

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
    /// Old URL paths that should redirect to this page.
    /// Each alias generates an HTML redirect file and an entry in `_redirects`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
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

/// A content file parsed for the build, with enough layout information to map
/// positions in the body (e.g. shortcode lines) back to lines in the file.
#[derive(Debug, Clone)]
pub struct ParsedContent {
    pub frontmatter: Frontmatter,
    pub body: String,
    /// 1-based file line on which `body` starts.
    pub body_line: usize,
}

impl ParsedContent {
    /// Convert a 1-based line within `body` to a 1-based line in the file.
    pub fn file_line(&self, body_line: usize) -> usize {
        self.body_line + body_line.saturating_sub(1)
    }
}

/// Like [`parse_content_file`], but reports problems as a located
/// [`Diagnostic`](crate::diagnostics::Diagnostic) (`frontmatter-missing`, `frontmatter-parse`,
/// `content-invalid`) so the build can collect every broken file in one pass.
/// Frontmatter error lines are converted from YAML-relative to file lines.
pub fn parse_content_diagnostic(
    path: &Path,
) -> std::result::Result<ParsedContent, Box<crate::diagnostics::Diagnostic>> {
    use crate::diagnostics::{line_col_at, Diagnostic};

    let raw = std::fs::read_to_string(path).map_err(|e| {
        Box::new(
            Diagnostic::error("content-invalid", format!("cannot read content file: {e}"))
                .with_file(path),
        )
    })?;
    let Some((fm_str, body)) = split_frontmatter(&raw) else {
        return Err(Box::new(Diagnostic::error(
            "frontmatter-missing",
            "missing frontmatter (the file must start with a `---` line and close it with another `---` line)",
        )
        .with_file(path)
        .with_line(1)
        .with_hint("start the file with:\n---\ntitle: \"My Page\"\n---")));
    };
    let offset_of = |slice: &str| slice.as_ptr() as usize - raw.as_ptr() as usize;
    let (fm_line, fm_col) = line_col_at(&raw, offset_of(fm_str));
    let (body_line, _) = line_col_at(&raw, offset_of(body));

    let frontmatter: Frontmatter = serde_yaml_ng::from_str(fm_str).map_err(|e| {
        let mut d = Diagnostic::error(
            "frontmatter-parse",
            format!(
                "invalid frontmatter: {}",
                strip_yaml_location(&e.to_string())
            ),
        )
        .with_file(path);
        match e.location() {
            Some(loc) => {
                let line = fm_line + loc.line().saturating_sub(1);
                let column = if loc.line() <= 1 {
                    fm_col + loc.column().saturating_sub(1)
                } else {
                    loc.column()
                };
                d = d.with_line(line).with_column(column);
            }
            None => d = d.with_line(fm_line),
        }
        if e.to_string().contains("missing field `title`") {
            d = d.with_hint("every page needs a `title:` in its frontmatter");
        }
        Box::new(d)
    })?;
    Ok(ParsedContent {
        frontmatter,
        body: body.to_string(),
        body_line,
    })
}

/// serde_yaml/serde_json embed ` at line L column C` in their messages
/// (relative to the parsed snippet, and sometimes mid-message); drop every
/// occurrence because diagnostics carry the file-relative position separately.
pub(crate) fn strip_yaml_location(message: &str) -> String {
    const MARKER: &str = " at line ";
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(idx) = rest.find(MARKER) {
        let after = &rest[idx + MARKER.len()..];
        let line_len = after.bytes().take_while(u8::is_ascii_digit).count();
        let tail = &after[line_len..];
        let col_len = tail
            .strip_prefix(" column ")
            .map(|t| t.bytes().take_while(u8::is_ascii_digit).count())
            .unwrap_or(0);
        if line_len > 0 && col_len > 0 {
            out.push_str(&rest[..idx]);
            rest = &tail[" column ".len() + col_len..];
        } else {
            out.push_str(&rest[..idx + MARKER.len()]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn split_frontmatter(raw: &str) -> Option<(&str, &str)> {
    let trimmed = raw.trim_start();
    let after_first = trimmed.strip_prefix("---")?;
    // The opening `---` must be the entire first line (allow a trailing CR).
    if !after_first.lines().next().unwrap_or("").trim().is_empty() {
        return None;
    }
    // Find the closing `---` delimiter, which must appear on its own line.
    // Anchoring on `\n---` (rather than a bare substring search) avoids
    // matching a `---` that appears inside a YAML value such as
    // `description: "before --- after"`, which would truncate the frontmatter.
    let mut offset = 0;
    while let Some(rel) = after_first[offset..].find("\n---") {
        let dash_start = offset + rel + 1; // index of the first `-`
                                           // The remainder of the delimiter line must be empty (only `---`).
        if after_first[dash_start + 3..]
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            let fm = &after_first[..dash_start];
            let body = &after_first[dash_start + 3..];
            return Some((fm.trim(), body.trim_start_matches(['\r', '\n'])));
        }
        offset = dash_start + 3;
    }
    None
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

    #[test]
    fn test_split_frontmatter_with_leading_whitespace() {
        let raw = "  \n---\ntitle: Test\n---\nBody";
        let (fm, body) = split_frontmatter(raw).unwrap();
        assert_eq!(fm, "title: Test");
        assert_eq!(body, "Body");
    }

    #[test]
    fn test_split_frontmatter_unclosed() {
        assert!(split_frontmatter("---\ntitle: Test\nNo closing").is_none());
    }

    #[test]
    fn test_split_frontmatter_body_with_newlines() {
        let raw = "---\ntitle: Test\n---\n\n\nBody with leading newlines";
        let (_, body) = split_frontmatter(raw).unwrap();
        assert_eq!(body, "Body with leading newlines");
    }

    #[test]
    fn test_split_frontmatter_triple_dash_in_value() {
        // A `---` inside a quoted YAML value must not terminate the frontmatter.
        let raw = "---\ntitle: Hello\ndescription: \"Before --- After\"\n---\nBody.";
        let (fm, body) = split_frontmatter(raw).unwrap();
        assert!(fm.contains("description: \"Before --- After\""));
        assert_eq!(body, "Body.");
        // And the recovered frontmatter must parse as valid YAML.
        let parsed: Frontmatter = serde_yaml_ng::from_str(fm).unwrap();
        assert_eq!(parsed.description.as_deref(), Some("Before --- After"));
    }

    #[test]
    fn test_split_frontmatter_horizontal_rule_in_body() {
        // A markdown horizontal rule in the body is fine; the first on-its-own
        // line `---` after the opening closes the frontmatter.
        let raw = "---\ntitle: Test\n---\nIntro\n\n---\n\nMore body.";
        let (fm, body) = split_frontmatter(raw).unwrap();
        assert_eq!(fm, "title: Test");
        assert_eq!(body, "Intro\n\n---\n\nMore body.");
    }

    #[test]
    fn test_slug_from_title_unicode() {
        let slug = slug_from_title("Héllo Wörld");
        assert!(!slug.is_empty());
        assert!(!slug.contains(' '));
    }

    #[test]
    fn test_slug_from_title_special_chars() {
        let slug = slug_from_title("What's New? (2026)");
        assert!(!slug.contains('\''));
        assert!(!slug.contains('?'));
        assert!(!slug.contains('('));
    }

    #[test]
    fn test_generate_frontmatter_with_all_fields() {
        let fm = Frontmatter {
            title: "Full Post".into(),
            date: Some(NaiveDate::from_ymd_opt(2025, 6, 15).unwrap()),
            updated: Some(NaiveDate::from_ymd_opt(2025, 7, 1).unwrap()),
            description: Some("A description".into()),
            image: Some("/static/hero.jpg".into()),
            slug: Some("custom-slug".into()),
            tags: vec!["rust".into(), "web".into()],
            draft: true,
            template: Some("custom.html".into()),
            robots: Some("noindex".into()),
            weight: Some(5),
            aliases: vec!["/old-url".into()],
            extra: HashMap::new(),
        };
        let generated = generate_frontmatter(&fm);
        assert!(generated.contains("title: Full Post"));
        assert!(generated.contains("2025-06-15"));
        assert!(generated.contains("2025-07-01"));
        assert!(generated.contains("A description"));
        assert!(generated.contains("custom-slug"));
        assert!(generated.contains("draft: true"));
        assert!(generated.contains("noindex"));
    }

    #[test]
    fn test_extract_lang_from_filename_no_extension() {
        let langs: HashSet<&str> = ["es"].into_iter().collect();
        assert_eq!(extract_lang_from_filename(Path::new("noext"), &langs), None);
    }

    #[test]
    fn test_extract_excerpt_more_marker_at_start() {
        assert_eq!(extract_excerpt("<!-- more -->\nrest"), "");
    }

    #[test]
    fn test_extract_excerpt_blank_first_paragraph() {
        let body = "\n\nActual first paragraph.\n\nSecond.";
        assert_eq!(extract_excerpt(body), "Actual first paragraph.");
    }

    #[test]
    fn test_is_false_helper() {
        assert!(is_false(&false));
        assert!(!is_false(&true));
    }

    #[test]
    fn test_parse_content_file_real() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.md");
        std::fs::write(
            &path,
            "---\ntitle: Hello World\ntags:\n  - rust\n---\nThis is the body.",
        )
        .unwrap();
        let (fm, body) = parse_content_file(&path).unwrap();
        assert_eq!(fm.title, "Hello World");
        assert_eq!(fm.tags, vec!["rust"]);
        assert_eq!(body, "This is the body.");
    }

    #[test]
    fn test_parse_content_file_missing() {
        let result = parse_content_file(Path::new("/nonexistent/file.md"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_content_file_no_frontmatter() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("nofm.md");
        std::fs::write(&path, "Just text, no frontmatter").unwrap();
        let result = parse_content_file(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("frontmatter"));
    }

    #[test]
    fn test_strip_lang_suffix_multiple_dots() {
        let langs: HashSet<&str> = ["es"].into_iter().collect();
        // "readme.min.es" should strip "es"
        assert_eq!(strip_lang_suffix("readme.min.es", &langs), "readme.min");
        // "readme.min" should not strip
        assert_eq!(strip_lang_suffix("readme.min", &langs), "readme.min");
    }

    #[test]
    fn test_parse_content_diagnostic_maps_yaml_line_to_file_line() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("bad.md");
        // Line 3 of the file (line 2 of the YAML) has an unterminated string.
        std::fs::write(&path, "---\ntitle: ok\ndate: [2024\n---\nBody").unwrap();
        let d = parse_content_diagnostic(&path).unwrap_err();
        assert_eq!(d.code, "frontmatter-parse");
        assert!(d.line.unwrap() >= 3, "{d}");
        assert!(!d.message.contains(" at line "), "{d}");
    }

    #[test]
    fn test_parse_content_diagnostic_type_error_line() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("bad.md");
        std::fs::write(&path, "\n---\ntitle: ok\nweight: heavy\n---\nBody").unwrap();
        let d = parse_content_diagnostic(&path).unwrap_err();
        assert_eq!(d.code, "frontmatter-parse");
        assert_eq!(d.line, Some(4), "{d}");
        assert_eq!(d.column, Some(9), "{d}");
    }

    #[test]
    fn test_parse_content_diagnostic_missing_and_ok() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("none.md");
        std::fs::write(&path, "no frontmatter").unwrap();
        let d = parse_content_diagnostic(&path).unwrap_err();
        assert_eq!(d.code, "frontmatter-missing");
        assert_eq!(d.line, Some(1));

        let missing_title = tmp.path().join("notitle.md");
        std::fs::write(&missing_title, "---\ndraft: true\n---\n").unwrap();
        let d = parse_content_diagnostic(&missing_title).unwrap_err();
        assert!(d.hint.unwrap().contains("title"));

        let good = tmp.path().join("good.md");
        std::fs::write(&good, "---\ntitle: Hi\n---\n\nBody here").unwrap();
        let parsed = parse_content_diagnostic(&good).unwrap();
        assert_eq!(parsed.frontmatter.title, "Hi");
        assert_eq!(parsed.body, "Body here");
        assert_eq!(parsed.body_line, 5);
        assert_eq!(parsed.file_line(2), 6);

        let d = parse_content_diagnostic(&tmp.path().join("gone.md")).unwrap_err();
        assert_eq!(d.code, "content-invalid");
    }

    #[test]
    fn test_strip_yaml_location() {
        assert_eq!(
            strip_yaml_location("invalid type: string at line 2 column 9"),
            "invalid type: string"
        );
        assert_eq!(strip_yaml_location("no location here"), "no location here");
        assert_eq!(
            strip_yaml_location("did not find ']' at line 3 column 1, while parsing"),
            "did not find ']', while parsing"
        );
        assert_eq!(
            strip_yaml_location("x at line two column 3"),
            "x at line two column 3"
        );
    }
}
