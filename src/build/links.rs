use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::Result;

/// A broken internal link found during validation.
#[derive(Debug)]
pub struct BrokenLink {
    /// Relative path of the HTML file containing the broken link (e.g., "posts/hello-world.html").
    pub source_file: String,
    /// The broken href value (e.g., "/posts/nonexistent").
    pub href: String,
}

/// Result of an internal link check across all HTML files in the output directory.
pub struct LinkCheckResult {
    /// Total number of internal links checked.
    pub total_links_checked: usize,
    /// Broken links grouped by target href. Each entry lists all source files linking to it.
    pub broken_links: Vec<BrokenLink>,
}

/// Check all internal links in HTML files under `output_dir`.
///
/// Walks every `.html` file, extracts `href="/..."` values, and validates each
/// against the set of files present in `output_dir`. Returns a summary of all
/// broken links found.
pub fn check_internal_links(output_dir: &Path) -> Result<LinkCheckResult> {
    let valid_urls = build_valid_urls(output_dir);
    let mut total_checked: usize = 0;
    let mut broken: Vec<BrokenLink> = Vec::new();

    for entry in WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "html")
        })
    {
        let html = fs::read_to_string(entry.path())?;
        let links = extract_internal_links(&html);
        total_checked += links.len();

        let rel_path = entry
            .path()
            .strip_prefix(output_dir)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");

        for href in links {
            if !valid_urls.contains(&href) {
                broken.push(BrokenLink {
                    source_file: rel_path.clone(),
                    href,
                });
            }
        }
    }

    Ok(LinkCheckResult {
        total_links_checked: total_checked,
        broken_links: broken,
    })
}

/// Build the set of all valid internal URL paths from files in the output directory.
///
/// For each file, computes the URL paths that would resolve to it:
/// - Exact file path: `/feed.xml`, `/static/style.css`
/// - Clean URL for `.html` files: `/posts/hello-world` (from `posts/hello-world.html`)
/// - Directory index variants: `/posts/` and `/posts` (from `posts/index.html`)
fn build_valid_urls(output_dir: &Path) -> HashSet<String> {
    let entries: Vec<_> = WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();
    build_valid_urls_from_entries(output_dir, &entries)
}

/// Build valid URL set from pre-collected WalkDir entries (avoids a redundant walk).
pub fn build_valid_urls_from_entries(
    output_dir: &Path,
    entries: &[walkdir::DirEntry],
) -> HashSet<String> {
    let mut urls = HashSet::new();

    for entry in entries {
        let rel = entry
            .path()
            .strip_prefix(output_dir)
            .unwrap_or(entry.path());
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        // Exact file path is always valid
        urls.insert(format!("/{rel_str}"));

        // For .html files, add clean URL variants
        if let Some(stripped) = rel_str.strip_suffix(".html") {
            if stripped == "index" {
                // Root index
                urls.insert("/".to_string());
            } else if let Some(dir) = stripped.strip_suffix("/index") {
                // Directory index: dist/posts/index.html → /posts/ and /posts
                urls.insert(format!("/{dir}/"));
                urls.insert(format!("/{dir}"));
            } else {
                // Regular page: dist/posts/hello-world.html → /posts/hello-world
                urls.insert(format!("/{stripped}"));
            }
        }
    }

    urls
}

/// Extract all internal link hrefs from an HTML string.
///
/// Finds `href="/..."` values (both single and double quotes), strips fragments
/// and query strings, and returns deduplicated paths. Only paths starting with
/// `/` are considered internal links. External URLs, anchors, and relative paths
/// are ignored.
pub fn extract_internal_links(html: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Find next "href="
        match html[pos..].find("href=") {
            Some(idx) => {
                let attr_start = pos + idx + 5; // position after "href="
                if attr_start >= len {
                    break;
                }
                let quote = bytes[attr_start];
                if quote == b'"' || quote == b'\'' {
                    let val_start = attr_start + 1;
                    if let Some(end_offset) = html[val_start..].find(quote as char) {
                        let href = &html[val_start..val_start + end_offset];
                        if href.starts_with('/') && !href.starts_with("//") {
                            // Strip fragment
                            let href = href.split('#').next().unwrap_or(href);
                            // Strip query string
                            let href = href.split('?').next().unwrap_or(href);
                            // Skip /favicon.ico — injected by all bundled themes but the file
                            // is optional (user places it in public/ when they have one)
                            if href == "/favicon.ico" {
                                pos = val_start + end_offset + 1;
                                continue;
                            }
                            if !href.is_empty() && seen.insert(href.to_string()) {
                                links.push(href.to_string());
                            }
                        }
                        pos = val_start + end_offset + 1;
                        continue;
                    }
                }
                pos = attr_start + 1;
            }
            None => break,
        }
    }

    links
}

/// Rewrite internal links that target subdomain collections to absolute URLs.
///
/// Given a map of URL path prefixes to absolute base URLs, scans `href="..."` values
/// and rewrites matching paths. For example, with `{"/docs" => "https://docs.example.com"}`:
/// - `href="/docs/setup"` → `href="https://docs.example.com/setup"`
/// - `href="/docs/"` → `href="https://docs.example.com/"`
/// - `href="/docs"` → `href="https://docs.example.com"`
///
/// Fragments and query strings are preserved. Only `href` attributes are rewritten.
pub fn rewrite_subdomain_links(html: &str, rewrites: &HashMap<String, String>) -> String {
    if rewrites.is_empty() {
        return html.to_string();
    }

    let mut result = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        match html[pos..].find("href=") {
            Some(idx) => {
                let attr_start = pos + idx + 5;
                // Copy everything up to and including "href="
                result.push_str(&html[pos..attr_start]);

                if attr_start >= len {
                    break;
                }

                let quote = bytes[attr_start];
                if quote == b'"' || quote == b'\'' {
                    result.push(quote as char);
                    let val_start = attr_start + 1;
                    if let Some(end_offset) = html[val_start..].find(quote as char) {
                        let href = &html[val_start..val_start + end_offset];

                        if let Some(rewritten) = rewrite_href(href, rewrites) {
                            result.push_str(&rewritten);
                        } else {
                            result.push_str(href);
                        }

                        result.push(quote as char);
                        pos = val_start + end_offset + 1;
                    } else {
                        // No closing quote, copy as-is
                        pos = val_start;
                    }
                } else {
                    pos = attr_start;
                }
            }
            None => {
                result.push_str(&html[pos..]);
                break;
            }
        }
    }

    result
}

/// Try to rewrite a single href value using the subdomain rewrite map.
/// Returns `Some(rewritten)` if the href matched a prefix, `None` otherwise.
fn rewrite_href(href: &str, rewrites: &HashMap<String, String>) -> Option<String> {
    // Only rewrite root-relative paths, skip absolute URLs
    if !href.starts_with('/') || href.starts_with("//") {
        return None;
    }

    // Split off fragment and query for preservation
    let (path, suffix) = split_href_suffix(href);

    for (prefix, base_url) in rewrites {
        if path == prefix {
            // Exact match: /docs → https://docs.example.com
            return Some(format!("{base_url}{suffix}"));
        }
        if let Some(rest) = path.strip_prefix(prefix) {
            if rest.starts_with('/') {
                // Path match: /docs/setup → https://docs.example.com/setup
                return Some(format!("{base_url}{rest}{suffix}"));
            }
        }
    }

    None
}

/// Split an href into the path portion and the suffix (fragment + query).
/// Returns (path, suffix) where suffix includes the leading `#` or `?`.
fn split_href_suffix(href: &str) -> (&str, &str) {
    // Find the earliest fragment or query marker
    let frag_pos = href.find('#');
    let query_pos = href.find('?');

    let split_pos = match (frag_pos, query_pos) {
        (Some(f), Some(q)) => Some(f.min(q)),
        (Some(f), None) => Some(f),
        (None, Some(q)) => Some(q),
        (None, None) => None,
    };

    match split_pos {
        Some(pos) => (&href[..pos], &href[pos..]),
        None => (href, ""),
    }
}

/// Group broken links by href, collecting all source files that link to each broken target.
pub fn group_broken_links(broken: &[BrokenLink]) -> Vec<(String, Vec<String>)> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for link in broken {
        map.entry(link.href.clone())
            .or_default()
            .push(link.source_file.clone());
    }
    let mut grouped: Vec<(String, Vec<String>)> = map.into_iter().collect();
    grouped.sort_by(|a, b| a.0.cmp(&b.0));
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_internal_links_basic() {
        let html = r#"<a href="/posts/hello">Hello</a> <a href="/about">About</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/posts/hello", "/about"]);
    }

    #[test]
    fn test_extract_internal_links_single_quotes() {
        let html = "<a href='/posts/hello'>Hello</a>";
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/posts/hello"]);
    }

    #[test]
    fn test_extract_internal_links_strips_fragment() {
        let html = r#"<a href="/posts/hello#section-1">Hello</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/posts/hello"]);
    }

    #[test]
    fn test_extract_internal_links_strips_query() {
        let html = r#"<a href="/search?q=test">Search</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/search"]);
    }

    #[test]
    fn test_extract_internal_links_ignores_external() {
        let html = r#"<a href="https://example.com">External</a> <a href="/internal">Internal</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/internal"]);
    }

    #[test]
    fn test_extract_internal_links_ignores_protocol_relative() {
        let html = r#"<a href="//cdn.example.com/lib.js">CDN</a>"#;
        let links = extract_internal_links(html);
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_internal_links_ignores_relative() {
        let html = r#"<a href="relative-page">Relative</a>"#;
        let links = extract_internal_links(html);
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_internal_links_deduplicates() {
        let html = r#"<a href="/about">About</a> <a href="/about">About again</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/about"]);
    }

    #[test]
    fn test_extract_internal_links_includes_non_anchor_hrefs() {
        // link tags for stylesheets, canonical, etc. also have href
        let html = r#"<link rel="stylesheet" href="/static/style.css"><a href="/about">About</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/static/style.css", "/about"]);
    }

    #[test]
    fn test_build_valid_urls() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = tmp.path();

        // Create file structure
        fs::create_dir_all(out.join("posts")).unwrap();
        fs::create_dir_all(out.join("tags/rust")).unwrap();
        fs::create_dir_all(out.join("static")).unwrap();

        fs::write(out.join("index.html"), "<html></html>").unwrap();
        fs::write(out.join("posts/hello-world.html"), "<html></html>").unwrap();
        fs::write(out.join("posts/index.html"), "<html></html>").unwrap();
        fs::write(out.join("tags/rust/index.html"), "<html></html>").unwrap();
        fs::write(out.join("feed.xml"), "<rss></rss>").unwrap();
        fs::write(out.join("static/style.css"), "body{}").unwrap();

        let urls = build_valid_urls(out);

        // Root index
        assert!(urls.contains("/"));
        assert!(urls.contains("/index.html"));

        // Regular page (clean URL)
        assert!(urls.contains("/posts/hello-world"));
        assert!(urls.contains("/posts/hello-world.html"));

        // Directory index
        assert!(urls.contains("/posts/"));
        assert!(urls.contains("/posts"));
        assert!(urls.contains("/posts/index.html"));

        // Nested directory index
        assert!(urls.contains("/tags/rust/"));
        assert!(urls.contains("/tags/rust"));

        // Non-HTML files
        assert!(urls.contains("/feed.xml"));
        assert!(urls.contains("/static/style.css"));
    }

    #[test]
    fn test_check_internal_links_all_valid() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = tmp.path();

        fs::create_dir_all(out.join("posts")).unwrap();
        fs::write(out.join("index.html"), r#"<a href="/posts/hello">link</a>"#).unwrap();
        fs::write(out.join("posts/hello.html"), r#"<a href="/">home</a>"#).unwrap();

        let result = check_internal_links(out).unwrap();
        assert_eq!(result.total_links_checked, 2);
        assert!(result.broken_links.is_empty());
    }

    #[test]
    fn test_check_internal_links_detects_broken() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = tmp.path();

        fs::write(
            out.join("index.html"),
            r#"<a href="/nonexistent">broken</a> <a href="/also-missing">also broken</a>"#,
        )
        .unwrap();

        let result = check_internal_links(out).unwrap();
        assert_eq!(result.total_links_checked, 2);
        assert_eq!(result.broken_links.len(), 2);
    }

    #[test]
    fn test_group_broken_links() {
        let broken = vec![
            BrokenLink {
                source_file: "index.html".to_string(),
                href: "/missing".to_string(),
            },
            BrokenLink {
                source_file: "about.html".to_string(),
                href: "/missing".to_string(),
            },
            BrokenLink {
                source_file: "index.html".to_string(),
                href: "/other".to_string(),
            },
        ];
        let grouped = group_broken_links(&broken);
        assert_eq!(grouped.len(), 2);
        // Sorted alphabetically by href
        assert_eq!(grouped[0].0, "/missing");
        assert_eq!(grouped[0].1.len(), 2);
        assert_eq!(grouped[1].0, "/other");
        assert_eq!(grouped[1].1.len(), 1);
    }

    fn make_rewrites(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_rewrite_subdomain_links_basic() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/docs/setup">Setup</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(
            result,
            r#"<a href="https://docs.example.com/setup">Setup</a>"#
        );
    }

    #[test]
    fn test_rewrite_subdomain_links_index() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);

        let html = r#"<a href="/docs/">Docs</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, r#"<a href="https://docs.example.com/">Docs</a>"#);

        let html = r#"<a href="/docs">Docs</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, r#"<a href="https://docs.example.com">Docs</a>"#);
    }

    #[test]
    fn test_rewrite_subdomain_links_preserves_fragment() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/docs/setup#section-1">Setup</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(
            result,
            r#"<a href="https://docs.example.com/setup#section-1">Setup</a>"#
        );
    }

    #[test]
    fn test_rewrite_subdomain_links_preserves_query() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/docs/search?q=test">Search</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(
            result,
            r#"<a href="https://docs.example.com/search?q=test">Search</a>"#
        );
    }

    #[test]
    fn test_rewrite_subdomain_links_skips_non_matching() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/posts/hello">Hello</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_subdomain_links_empty_map() {
        let rewrites = HashMap::new();
        let html = r#"<a href="/docs/setup">Setup</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_subdomain_links_multiple_prefixes() {
        let rewrites = make_rewrites(&[
            ("/docs", "https://docs.example.com"),
            ("/blog", "https://blog.example.com"),
        ]);
        let html = r#"<a href="/docs/setup">Docs</a> <a href="/blog/hello">Blog</a> <a href="/about">About</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(
            result,
            r#"<a href="https://docs.example.com/setup">Docs</a> <a href="https://blog.example.com/hello">Blog</a> <a href="/about">About</a>"#
        );
    }

    #[test]
    fn test_rewrite_subdomain_links_skips_external() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="https://example.com/docs/setup">External</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_subdomain_links_single_quotes() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = "<a href='/docs/setup'>Setup</a>";
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, "<a href='https://docs.example.com/setup'>Setup</a>");
    }

    #[test]
    fn test_rewrite_subdomain_links_no_false_prefix_match() {
        // /docs-extra should NOT match /docs prefix
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/docs-extra/page">Page</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, html);
    }
}
