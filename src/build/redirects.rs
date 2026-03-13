use std::fs;
use std::path::Path;

use crate::config::SiteConfig;
use crate::content::ContentItem;
use crate::error::Result;

/// A single redirect mapping from an alias path to a canonical URL.
#[derive(Debug)]
struct Redirect {
    from: String,
    to: String,
}

/// Collect all aliases from content items, generate HTML redirect pages
/// at each alias path, and write a `_redirects` file to the output root.
///
/// Returns the number of redirects generated.
pub fn generate_redirects(
    config: &SiteConfig,
    items: &[&ContentItem],
    output_dir: &Path,
) -> Result<usize> {
    let base = config.site.base_url.trim_end_matches('/');

    let redirects: Vec<Redirect> = items
        .iter()
        .flat_map(|item| {
            item.frontmatter.aliases.iter().filter_map(move |alias| {
                let clean = alias.trim_matches('/');
                // Skip empty aliases (root path) and path traversal attempts
                if clean.is_empty() || clean.contains("..") {
                    return None;
                }
                Some(Redirect {
                    from: alias.clone(),
                    to: item.url.clone(),
                })
            })
        })
        .collect();

    if redirects.is_empty() {
        return Ok(0);
    }

    // Generate HTML redirect files
    for redirect in &redirects {
        let canonical = format!("{}{}", base, redirect.to);
        let html = redirect_html(&canonical);
        let clean = redirect.from.trim_matches('/');
        let output_path = output_dir.join(format!("{clean}.html"));

        // Safety: verify the output path stays within output_dir
        if !output_path.starts_with(output_dir) {
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, html)?;
    }

    // Generate _redirects file (Netlify/Cloudflare compatible)
    let mut redirects_content: String = redirects
        .iter()
        .map(|r| format!("{} {} 301", r.from, r.to))
        .collect::<Vec<_>>()
        .join("\n");
    redirects_content.push('\n');
    fs::write(output_dir.join("_redirects"), redirects_content)?;

    Ok(redirects.len())
}

/// Generate a minimal HTML page that redirects to the canonical URL.
/// The URL is HTML-escaped to prevent injection via malformed paths.
fn redirect_html(canonical_url: &str) -> String {
    let escaped = html_escape(canonical_url);
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<link rel="canonical" href="{escaped}">
<meta http-equiv="refresh" content="0; url={escaped}">
<title>Redirecting&hellip;</title>
</head>
<body>
<p>Redirecting to <a href="{escaped}">{escaped}</a>&hellip;</p>
</body>
</html>
"#
    )
}

/// Minimal HTML entity escaping for URLs in attribute values and text content.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SiteConfig;
    use crate::content::{ContentItem, Frontmatter};

    fn test_config() -> SiteConfig {
        SiteConfig {
            site: crate::config::SiteSection {
                title: "Test".into(),
                description: "".into(),
                base_url: "https://example.com".into(),
                language: "en".into(),
                author: "".into(),
            },
            collections: vec![],
            build: Default::default(),
            deploy: Default::default(),
            languages: Default::default(),
            images: Default::default(),
            analytics: None,
            trust: None,
            contact: None,
        }
    }

    fn test_item_with_aliases(url: &str, aliases: Vec<&str>) -> ContentItem {
        ContentItem {
            frontmatter: Frontmatter {
                title: "Test".into(),
                aliases: aliases.into_iter().map(|a| a.to_string()).collect(),
                ..Default::default()
            },
            raw_body: String::new(),
            html_body: String::new(),
            source_path: std::path::PathBuf::from("test.md"),
            slug: "test".into(),
            collection: "posts".into(),
            url: url.into(),
            lang: "en".into(),
            word_count: 0,
            reading_time: 0,
            excerpt: String::new(),
            excerpt_html: String::new(),
            toc: vec![],
        }
    }

    #[test]
    fn test_generate_redirects_basic() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = test_config();
        let item = test_item_with_aliases("/posts/new-slug", vec!["/old-post", "/legacy/url"]);
        let items: Vec<&ContentItem> = vec![&item];

        let count = generate_redirects(&config, &items, tmp.path()).unwrap();
        assert_eq!(count, 2);

        // HTML redirect files
        let html1 = std::fs::read_to_string(tmp.path().join("old-post.html")).unwrap();
        assert!(html1.contains("https://example.com/posts/new-slug"));
        assert!(html1.contains("rel=\"canonical\""));
        assert!(html1.contains("http-equiv=\"refresh\""));

        let html2 = std::fs::read_to_string(tmp.path().join("legacy/url.html")).unwrap();
        assert!(html2.contains("https://example.com/posts/new-slug"));

        // _redirects file
        let redirects = std::fs::read_to_string(tmp.path().join("_redirects")).unwrap();
        assert!(redirects.contains("/old-post /posts/new-slug 301"));
        assert!(redirects.contains("/legacy/url /posts/new-slug 301"));
        assert!(redirects.ends_with('\n'));
    }

    #[test]
    fn test_generate_redirects_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = test_config();
        let item = test_item_with_aliases("/posts/test", vec![]);
        let items: Vec<&ContentItem> = vec![&item];

        let count = generate_redirects(&config, &items, tmp.path()).unwrap();
        assert_eq!(count, 0);
        assert!(!tmp.path().join("_redirects").exists());
    }

    #[test]
    fn test_redirect_html_content() {
        let html = redirect_html("https://example.com/new-page");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("rel=\"canonical\""));
        assert!(html.contains("content=\"0; url=https://example.com/new-page\""));
        assert!(html.contains("href=\"https://example.com/new-page\""));
    }

    #[test]
    fn test_path_traversal_rejected() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = test_config();
        let item = test_item_with_aliases(
            "/posts/safe",
            vec!["/../../etc/passwd", "/../escape", "/valid-alias"],
        );
        let items: Vec<&ContentItem> = vec![&item];

        let count = generate_redirects(&config, &items, tmp.path()).unwrap();
        assert_eq!(count, 1); // only /valid-alias survives
        assert!(tmp.path().join("valid-alias.html").exists());
        assert!(!tmp.path().join("../../etc/passwd.html").exists());
    }

    #[test]
    fn test_root_alias_skipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = test_config();
        let item = test_item_with_aliases("/posts/test", vec!["/"]);
        let items: Vec<&ContentItem> = vec![&item];

        let count = generate_redirects(&config, &items, tmp.path()).unwrap();
        assert_eq!(count, 0);
        assert!(!tmp.path().join("_redirects").exists());
    }

    #[test]
    fn test_html_escape_in_redirect() {
        let html = redirect_html("https://example.com/page?a=1&b=2");
        assert!(html.contains("&amp;"));
        assert!(!html.contains("?a=1&b=2\"")); // raw & should not appear in attributes
    }
}
