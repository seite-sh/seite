use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::Result;

/// Rewrite root-relative URLs in all HTML files to include the given `base_path`.
///
/// This handles GitHub Pages project sites and other subpath deployments where
/// the site is served from a URL like `https://user.github.io/repo/` instead of
/// the domain root.
///
/// Only runs when `base_path` is non-empty. Rewrites attributes: `href`, `src`,
/// `srcset`, `action`, `poster`, `data-src`. Does not touch absolute URLs
/// (`https://…`, `http://…`, `//…`), fragment-only links (`#…`), or data URIs.
pub fn rewrite_html_base_path(output_dir: &Path, base_path: &str) -> Result<()> {
    if base_path.is_empty() {
        return Ok(());
    }

    for entry in WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "html")
        })
    {
        let html = fs::read_to_string(entry.path())?;
        let rewritten = rewrite_html_urls(&html, base_path);
        if rewritten != html {
            fs::write(entry.path(), rewritten)?;
        }
    }

    Ok(())
}

/// Rewrite root-relative URLs in HTML content to include a base path prefix.
///
/// Iterates through the HTML tag by tag. For each opening/self-closing tag,
/// rewrites URL-bearing attributes. Content outside tags is copied unchanged.
fn rewrite_html_urls(html: &str, base_path: &str) -> String {
    let mut result = String::with_capacity(html.len() + 256);
    let mut pos = 0;
    let bytes = html.as_bytes();

    while pos < bytes.len() {
        if bytes[pos] == b'<' {
            // Find the end of this tag
            if let Some(tag_end) = find_tag_end(html, pos) {
                let tag = &html[pos..=tag_end];

                // Skip comments, DOCTYPE, closing tags, script/style content
                if tag.starts_with("<!--") || tag.starts_with("<!") || tag.starts_with("</") {
                    result.push_str(tag);
                } else {
                    // It's an opening or self-closing tag — rewrite URL attributes
                    result.push_str(&rewrite_tag_attrs(tag, base_path));
                }

                pos = tag_end + 1;
            } else {
                // Malformed HTML — copy the rest as-is
                result.push_str(&html[pos..]);
                break;
            }
        } else {
            // Copy text content between tags
            if let Some(next_tag) = html[pos..].find('<') {
                result.push_str(&html[pos..pos + next_tag]);
                pos += next_tag;
            } else {
                result.push_str(&html[pos..]);
                break;
            }
        }
    }

    result
}

/// Find the position of the `>` that closes the tag starting at `start`.
/// Handles quoted attribute values that may contain `>`.
fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    let mut i = start + 1;
    let mut in_quote: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        match in_quote {
            Some(q) if b == q => in_quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => in_quote = Some(b),
            None if b == b'>' => return Some(i),
            _ => {}
        }
        i += 1;
    }

    None
}

/// URL-bearing HTML attributes to rewrite.
const URL_ATTRS: &[&str] = &["href", "src", "srcset", "action", "poster", "data-src"];

/// Rewrite URL attributes within a single HTML tag string.
fn rewrite_tag_attrs(tag: &str, base_path: &str) -> String {
    let mut result = String::with_capacity(tag.len() + 64);
    let mut remaining = tag;

    while !remaining.is_empty() {
        // Find the next URL attribute
        if let Some((attr, _attr_pos, value_start)) = find_next_attr_in_tag(remaining) {
            // Copy everything before the attribute value
            result.push_str(&remaining[..value_start]);

            let after_value_start = &remaining[value_start..];

            // Find the closing quote
            if let Some(end_quote) = after_value_start[1..].find('"') {
                let value = &after_value_start[1..1 + end_quote];

                result.push('"');
                if attr == "srcset" {
                    result.push_str(&rewrite_srcset(value, base_path));
                } else {
                    result.push_str(&rewrite_url(value, base_path));
                }
                result.push('"');

                remaining = &after_value_start[1 + end_quote + 1..];
            } else {
                // Malformed — copy rest as-is
                result.push_str(after_value_start);
                remaining = "";
            }
        } else {
            result.push_str(remaining);
            remaining = "";
        }
    }

    result
}

/// Find the next URL attribute in a tag fragment.
/// Returns (attr_name, attr_position, position_of_opening_quote).
fn find_next_attr_in_tag(tag: &str) -> Option<(&str, usize, usize)> {
    let mut best: Option<(&str, usize, usize)> = None;

    for &attr in URL_ATTRS {
        // Build pattern: ` attr="` (space-prefixed to avoid partial matches)
        let pattern = format!(" {attr}=\"");
        if let Some(pos) = tag.find(&pattern) {
            let value_quote_pos = pos + pattern.len() - 1; // position of the opening "

            match best {
                Some((_, _, best_pos)) if value_quote_pos < best_pos => {
                    best = Some((attr, pos, value_quote_pos));
                }
                None => {
                    best = Some((attr, pos, value_quote_pos));
                }
                _ => {}
            }
        }
    }

    best
}

/// Rewrite a single URL: prepend base_path if it's a root-relative URL.
fn rewrite_url(url: &str, base_path: &str) -> String {
    // Don't rewrite:
    // - Empty URLs
    // - Absolute URLs (https://, http://, //)
    // - Fragment-only (#section)
    // - Query-only (?param)
    // - Data URIs (data:...)
    // - JavaScript URIs (javascript:...)
    // - URLs that already start with the base_path
    if url.is_empty()
        || url.starts_with("https://")
        || url.starts_with("http://")
        || url.starts_with("//")
        || url.starts_with('#')
        || url.starts_with('?')
        || url.starts_with("data:")
        || url.starts_with("javascript:")
        || url.starts_with(base_path)
    {
        return url.to_string();
    }

    // Only rewrite root-relative URLs (starting with /)
    if url.starts_with('/') {
        return format!("{base_path}{url}");
    }

    // Leave relative URLs as-is
    url.to_string()
}

/// Rewrite srcset attribute value, which contains comma-separated "url size" entries.
fn rewrite_srcset(srcset: &str, base_path: &str) -> String {
    srcset
        .split(',')
        .map(|entry| {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                return String::new();
            }
            // Each entry is "url [size]" — split on first space
            let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
            let url = rewrite_url(parts[0], base_path);
            if parts.len() > 1 {
                format!("{url} {}", parts[1])
            } else {
                url
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_url_root_relative() {
        assert_eq!(rewrite_url("/posts/hello", "/repo"), "/repo/posts/hello");
        assert_eq!(
            rewrite_url("/static/img.jpg", "/repo"),
            "/repo/static/img.jpg"
        );
        assert_eq!(rewrite_url("/feed.xml", "/repo"), "/repo/feed.xml");
    }

    #[test]
    fn test_rewrite_url_skip_absolute() {
        assert_eq!(
            rewrite_url("https://example.com/foo", "/repo"),
            "https://example.com/foo"
        );
        assert_eq!(
            rewrite_url("http://example.com/foo", "/repo"),
            "http://example.com/foo"
        );
        assert_eq!(
            rewrite_url("//cdn.example.com/a.js", "/repo"),
            "//cdn.example.com/a.js"
        );
    }

    #[test]
    fn test_rewrite_url_skip_special() {
        assert_eq!(rewrite_url("#section", "/repo"), "#section");
        assert_eq!(rewrite_url("?q=test", "/repo"), "?q=test");
        assert_eq!(
            rewrite_url("data:image/png;base64,abc", "/repo"),
            "data:image/png;base64,abc"
        );
        assert_eq!(rewrite_url("", "/repo"), "");
    }

    #[test]
    fn test_rewrite_url_skip_already_prefixed() {
        assert_eq!(
            rewrite_url("/repo/posts/hello", "/repo"),
            "/repo/posts/hello"
        );
    }

    #[test]
    fn test_rewrite_url_relative_unchanged() {
        assert_eq!(rewrite_url("image.jpg", "/repo"), "image.jpg");
        assert_eq!(rewrite_url("../style.css", "/repo"), "../style.css");
    }

    #[test]
    fn test_rewrite_srcset() {
        let srcset = "/static/img-480w.jpg 480w, /static/img-800w.jpg 800w";
        let result = rewrite_srcset(srcset, "/repo");
        assert_eq!(
            result,
            "/repo/static/img-480w.jpg 480w, /repo/static/img-800w.jpg 800w"
        );
    }

    #[test]
    fn test_rewrite_html_basic() {
        let html = r#"<a href="/posts/hello">link</a><img src="/static/photo.jpg">"#;
        let result = rewrite_html_urls(html, "/repo");
        assert_eq!(
            result,
            r#"<a href="/repo/posts/hello">link</a><img src="/repo/static/photo.jpg">"#
        );
    }

    #[test]
    fn test_rewrite_html_preserves_absolute() {
        let html = r#"<a href="https://example.com">ext</a><a href="/posts">int</a>"#;
        let result = rewrite_html_urls(html, "/repo");
        assert_eq!(
            result,
            r#"<a href="https://example.com">ext</a><a href="/repo/posts">int</a>"#
        );
    }

    #[test]
    fn test_rewrite_html_srcset() {
        let html = r#"<img src="/static/img.jpg" srcset="/static/img-480w.jpg 480w, /static/img.jpg 1200w">"#;
        let result = rewrite_html_urls(html, "/repo");
        assert_eq!(
            result,
            r#"<img src="/repo/static/img.jpg" srcset="/repo/static/img-480w.jpg 480w, /repo/static/img.jpg 1200w">"#
        );
    }

    #[test]
    fn test_rewrite_html_no_op_when_empty_base() {
        let html = r#"<a href="/posts/hello">link</a>"#;
        let result = rewrite_html_urls(html, "");
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_html_skips_text_content() {
        // href= in text content (not in a tag attribute) should not be rewritten
        let html = r#"<p>Use href="/path" in code</p><a href="/real">link</a>"#;
        let result = rewrite_html_urls(html, "/repo");
        assert!(result.contains(r#"href="/repo/real""#));
        // The text content should be unchanged
        assert!(result.contains(r#"Use href="/path" in code"#));
    }

    #[test]
    fn test_rewrite_html_picture_element() {
        let html = r#"<picture><source type="image/webp" srcset="/static/img-480w.webp 480w"><img src="/static/img.jpg" srcset="/static/img-480w.jpg 480w"></picture>"#;
        let result = rewrite_html_urls(html, "/repo");
        assert!(result.contains(r#"srcset="/repo/static/img-480w.webp 480w""#));
        assert!(result.contains(r#"src="/repo/static/img.jpg""#));
        assert!(result.contains(r#"srcset="/repo/static/img-480w.jpg 480w""#));
    }

    #[test]
    fn test_rewrite_html_meta_tags() {
        let html = r#"<link rel="canonical" href="https://example.com/page"><link rel="alternate" href="/feed.xml">"#;
        let result = rewrite_html_urls(html, "/repo");
        // Absolute URL should not be rewritten
        assert!(result.contains(r#"href="https://example.com/page""#));
        // Root-relative should be rewritten
        assert!(result.contains(r#"href="/repo/feed.xml""#));
    }

    #[test]
    fn test_rewrite_html_closing_tags_unchanged() {
        let html = r#"<div><a href="/link">text</a></div>"#;
        let result = rewrite_html_urls(html, "/repo");
        assert_eq!(result, r#"<div><a href="/repo/link">text</a></div>"#);
    }

    #[test]
    fn test_rewrite_html_self_closing() {
        let html = r#"<img src="/static/photo.jpg" />"#;
        let result = rewrite_html_urls(html, "/repo");
        assert_eq!(result, r#"<img src="/repo/static/photo.jpg" />"#);
    }

    #[test]
    fn test_rewrite_html_multiple_attrs() {
        let html = r#"<a href="/page" data-src="/lazy.jpg">"#;
        let result = rewrite_html_urls(html, "/repo");
        assert_eq!(result, r#"<a href="/repo/page" data-src="/repo/lazy.jpg">"#);
    }
}
