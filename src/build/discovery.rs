//! Generates AI and SEO discovery files:
//! - robots.txt  — search engine + AI crawler directives
//! - llms.txt    — summary for LLM crawlers (per llmstxt.org spec)
//! - llms-full.txt — full site content in markdown for LLM ingestion

use crate::config::SiteConfig;
use crate::content::ContentItem;

/// Generate robots.txt with sitemap reference and AI crawler hints.
pub fn generate_robots_txt(config: &SiteConfig) -> String {
    let base = config.site.base_url.trim_end_matches('/');
    let mut out = String::new();
    out.push_str("User-agent: *\n");
    out.push_str("Allow: /\n");
    out.push('\n');
    out.push_str(&format!("Sitemap: {base}/sitemap.xml\n"));
    out.push('\n');
    // Point AI crawlers to the LLM-optimized files
    out.push_str("# AI / LLM crawlers\n");
    out.push_str(&format!("# LLMs-txt: {base}/llms.txt\n"));
    out.push_str(&format!("# LLMs-full-txt: {base}/llms-full.txt\n"));
    out
}

/// Generate llms.txt — a short summary page per the llmstxt.org spec.
/// Format:
///   # Site Title
///   > Description
///   ## Collection Name
///   - \[Title\](url): description
pub fn generate_llms_txt(
    config: &SiteConfig,
    collections: &[(String, Vec<&ContentItem>)],
) -> String {
    let base = config.site.base_url.trim_end_matches('/');
    let mut out = String::new();

    // Header
    out.push_str(&format!("# {}\n\n", config.site.title));
    if !config.site.description.is_empty() {
        out.push_str(&format!("> {}\n\n", config.site.description));
    }

    for (label, items) in collections {
        if items.is_empty() {
            continue;
        }
        out.push_str(&format!("## {label}\n\n"));
        for item in items {
            let md_url = format!("{base}{}.md", item.url);
            let desc = item.frontmatter.description.as_deref().unwrap_or("");
            if desc.is_empty() {
                out.push_str(&format!("- [{}]({})\n", item.frontmatter.title, md_url));
            } else {
                out.push_str(&format!(
                    "- [{}]({}): {}\n",
                    item.frontmatter.title, md_url, desc
                ));
            }
        }
        out.push('\n');
    }

    out
}

/// Generate llms-full.txt — the full content of every page as markdown.
/// Each page is separated by a heading and its raw markdown body.
pub fn generate_llms_full_txt(
    config: &SiteConfig,
    collections: &[(String, Vec<&ContentItem>)],
) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", config.site.title));
    if !config.site.description.is_empty() {
        out.push_str(&format!("> {}\n\n", config.site.description));
    }
    out.push_str("---\n\n");

    for (label, items) in collections {
        if items.is_empty() {
            continue;
        }
        out.push_str(&format!("## {label}\n\n"));
        for item in items {
            out.push_str(&format!("### {}\n\n", item.frontmatter.title));
            if let Some(ref desc) = item.frontmatter.description {
                out.push_str(&format!("*{desc}*\n\n"));
            }
            out.push_str(&item.raw_body);
            out.push_str("\n\n---\n\n");
        }
    }

    out
}
