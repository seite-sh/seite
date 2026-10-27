//! Generates AI and SEO discovery files:
//! - robots.txt  — search engine + AI crawler directives
//! - llms.txt    — summary for LLM crawlers (per llmstxt.org spec)
//! - llms-full.txt — full site content in markdown for LLM ingestion

use crate::config::SiteConfig;
use crate::content::ContentItem;

/// AI search / user-initiated fetch crawlers: allowed, so content can appear
/// in (and be fetched for) AI-generated answers.
const AI_SEARCH_CRAWLERS: &[&str] = &[
    "ChatGPT-User",
    "OAI-SearchBot",
    "PerplexityBot",
    "Perplexity-User",
    "Claude-SearchBot",
    "Claude-User",
];

/// AI training crawlers: disallowed, so content isn't collected for model training.
const AI_TRAINING_CRAWLERS: &[&str] = &[
    "GPTBot",
    "ClaudeBot",
    "Google-Extended",
    "Applebot-Extended",
    "meta-externalagent",
    "CCBot",
    "Bytespider",
];

/// Cloudflare Content Signals for the `User-agent: *` group — the same policy
/// as the per-crawler groups: search and AI answers yes, training no.
const CONTENT_SIGNAL: &str = "search=yes, ai-input=yes, ai-train=no";

/// Generate robots.txt with sitemap reference and AI crawler directives.
pub fn generate_robots_txt(config: &SiteConfig) -> String {
    let base = config.site.base_url.trim_end_matches('/');
    let mut out = String::new();
    out.push_str("User-agent: *\n");
    out.push_str("# Content Signals: https://contentsignals.org\n");
    out.push_str(&format!("Content-Signal: {CONTENT_SIGNAL}\n"));
    out.push_str("Allow: /\n");
    out.push('\n');
    out.push_str(&format!("Sitemap: {base}/sitemap.xml\n"));
    out.push('\n');

    // AI search crawlers — allow so content appears in AI-generated answers
    out.push_str("# AI search crawlers (allow for AI search visibility)\n");
    for agent in AI_SEARCH_CRAWLERS {
        out.push_str(&format!("User-agent: {agent}\nAllow: /\n\n"));
    }

    // AI training crawlers — disallow by default to prevent training use
    out.push_str("# AI training crawlers (block training, not search)\n");
    for agent in AI_TRAINING_CRAWLERS {
        out.push_str(&format!("User-agent: {agent}\nDisallow: /\n\n"));
    }

    // LLM discovery files
    out.push_str("# LLM discovery\n");
    out.push_str(&format!("# LLMs-txt: {base}/llms.txt\n"));
    out.push_str(&format!("# LLMs-full-txt: {base}/llms-full.txt\n"));
    out
}

/// The one-line blockquote at the top of every published `.md` copy that
/// points agents at the site's discovery files: the page's language's
/// `llms.txt` / `llms-full.txt` (`/<lang>/…` for non-default languages of a
/// multilingual site) and the site-wide `sitemap.md`. Absolute URLs from
/// `base_url`, which already carries any base path.
pub fn markdown_pointer(config: &SiteConfig, lang: &str) -> String {
    let base = config.site.base_url.trim_end_matches('/');
    let lang_base = if config.is_multilingual() && lang != config.site.language {
        format!("{base}/{lang}")
    } else {
        base.to_string()
    };
    format!(
        "> Index of this site for agents: [llms.txt]({lang_base}/llms.txt). \
         Full text: [llms-full.txt]({lang_base}/llms-full.txt). \
         All pages: [sitemap.md]({base}/sitemap.md)."
    )
}

/// Insert `pointer` right after a leading YAML frontmatter block (`---` …
/// `---`), or at the top when there is none, separated by blank lines.
pub fn insert_markdown_pointer(md: &str, pointer: &str) -> String {
    if let Some(after_open) = md.strip_prefix("---\n") {
        let mut offset = 4;
        for line in after_open.split_inclusive('\n') {
            offset += line.len();
            if line.trim_end_matches(['\n', '\r']) == "---" {
                let (head, tail) = md.split_at(offset);
                let head = head.trim_end_matches(['\n', '\r']);
                let tail = tail.trim_start_matches(['\n', '\r']);
                return format!("{head}\n\n{pointer}\n\n{tail}");
            }
        }
    }
    format!("{pointer}\n\n{md}")
}

/// Generate `sitemap.md`: every page that has a markdown copy, as a markdown
/// list an agent can read in one request. `sections` are `(heading, items)`
/// pairs in output order (built by the caller from the same items as
/// `sitemap.xml`); drafts (`--drafts`) and `noindex` pages are left out here,
/// and empty sections are skipped. Each entry links the page's `.md` copy.
pub fn generate_sitemap_md(
    config: &SiteConfig,
    sections: &[(String, Vec<&ContentItem>)],
) -> String {
    let base = config.site.base_url.trim_end_matches('/');
    let mut out = format!("# {}: sitemap\n\n", config.site.title);
    if !config.site.description.is_empty() {
        out.push_str(&format!("> {}\n\n", config.site.description));
    }
    out.push_str(&format!(
        "Every page as markdown. Summary: [llms.txt]({base}/llms.txt). Full text: [llms-full.txt]({base}/llms-full.txt).\n\n"
    ));
    for (heading, items) in sections {
        let entries: Vec<(&ContentItem, String)> = items
            .iter()
            .filter(|i| !i.frontmatter.draft && !is_noindex(i))
            .filter_map(|i| super::item_markdown_url(&i.url).map(|u| (*i, u)))
            .collect();
        if entries.is_empty() {
            continue;
        }
        out.push_str(&format!("## {heading}\n\n"));
        for (item, md_path) in entries {
            out.push_str(&format!("- [{}]({base}{md_path})", item.frontmatter.title));
            match item.frontmatter.description.as_deref().map(str::trim) {
                Some(desc) if !desc.is_empty() => out.push_str(&format!(" — {desc}\n")),
                _ => out.push('\n'),
            }
        }
        out.push('\n');
    }
    out
}

fn is_noindex(item: &ContentItem) -> bool {
    item.frontmatter
        .robots
        .as_deref()
        .is_some_and(|r| r.to_ascii_lowercase().contains("noindex"))
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
    out.push_str(&format!(
        "Every page as markdown: [sitemap.md]({base}/sitemap.md)\n\n"
    ));

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
/// Each page is separated by a heading, source URL, and its raw markdown body.
pub fn generate_llms_full_txt(
    config: &SiteConfig,
    collections: &[(String, Vec<&ContentItem>)],
) -> String {
    let base = config.site.base_url.trim_end_matches('/');
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
            out.push_str(&format!("Source: {}{}\n\n", base, item.url));
            if let Some(ref desc) = item.frontmatter.description {
                out.push_str(&format!("*{desc}*\n\n"));
            }
            out.push_str(&item.raw_body);
            out.push_str("\n\n---\n\n");
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Frontmatter;

    fn test_config(base_url: &str, description: &str) -> SiteConfig {
        SiteConfig {
            site: crate::config::SiteSection {
                title: "Test Site".into(),
                description: description.into(),
                base_url: base_url.into(),
                language: "en".into(),
                author: "".into(),
                timezone: None,
            },
            collections: vec![],
            build: Default::default(),
            deploy: Default::default(),
            languages: Default::default(),
            images: Default::default(),
            analytics: None,
            trust: None,
            contact: None,
            access: None,
            taxonomies: Vec::new(),
            check: None,
            og: None,
        }
    }

    fn test_item(title: &str, url: &str, description: Option<&str>, body: &str) -> ContentItem {
        ContentItem {
            frontmatter: Frontmatter {
                title: title.into(),
                description: description.map(String::from),
                ..Default::default()
            },
            raw_body: body.into(),
            html_body: String::new(),
            source_path: std::path::PathBuf::from("test.md"),
            slug: "test".into(),
            collection: "posts".into(),
            url: url.into(),
            lang: "en".into(),
            excerpt: String::new(),
            toc: vec![],
            word_count: 0,
            reading_time: 0,
            excerpt_html: String::new(),
        }
    }

    #[test]
    fn test_robots_txt_basic() {
        let config = test_config("https://example.com", "");
        let result = generate_robots_txt(&config);
        assert!(result.contains("User-agent: *"));
        assert!(result.contains("Allow: /"));
        assert!(result.contains("Sitemap: https://example.com/sitemap.xml"));
        assert!(result.contains("LLMs-txt: https://example.com/llms.txt"));
        assert!(result.contains("LLMs-full-txt: https://example.com/llms-full.txt"));
        // AI search / user-fetch crawlers allowed
        for agent in [
            "ChatGPT-User",
            "OAI-SearchBot",
            "PerplexityBot",
            "Perplexity-User",
            "Claude-SearchBot",
            "Claude-User",
        ] {
            assert!(
                result.contains(&format!("User-agent: {agent}\nAllow: /\n")),
                "{agent} should be allowed"
            );
        }
        // AI training crawlers blocked
        for agent in [
            "GPTBot",
            "ClaudeBot",
            "Google-Extended",
            "Applebot-Extended",
            "meta-externalagent",
            "CCBot",
            "Bytespider",
        ] {
            assert!(
                result.contains(&format!("User-agent: {agent}\nDisallow: /\n")),
                "{agent} should be disallowed"
            );
        }
    }

    #[test]
    fn test_robots_txt_content_signals_in_wildcard_group() {
        let config = test_config("https://example.com", "");
        let result = generate_robots_txt(&config);
        // The signal belongs to the `User-agent: *` group, before the first blank line.
        let wildcard_group = result.split("\n\n").next().unwrap();
        assert!(wildcard_group.starts_with("User-agent: *\n"));
        assert!(wildcard_group.contains("# Content Signals: https://contentsignals.org"));
        assert!(wildcard_group.contains("Content-Signal: search=yes, ai-input=yes, ai-train=no"));
        assert!(wildcard_group.contains("Allow: /"));
        assert_eq!(result.matches("Content-Signal:").count(), 1);
    }

    #[test]
    fn test_robots_txt_crawler_lists_disjoint() {
        for agent in AI_SEARCH_CRAWLERS {
            assert!(
                !AI_TRAINING_CRAWLERS.contains(agent),
                "{agent} is both allowed and disallowed"
            );
        }
    }

    #[test]
    fn test_robots_txt_strips_trailing_slash() {
        let config = test_config("https://example.com/", "");
        let result = generate_robots_txt(&config);
        assert!(result.contains("https://example.com/sitemap.xml"));
        assert!(!result.contains("https://example.com//"));
    }

    #[test]
    fn test_llms_txt_basic() {
        let config = test_config("https://example.com", "A great site");
        let item = test_item("Hello World", "/posts/hello", Some("A post"), "body");
        let collections = vec![("Posts".to_string(), vec![&item])];
        let result = generate_llms_txt(&config, &collections);
        assert!(result.contains("# Test Site"));
        assert!(result.contains("> A great site"));
        assert!(result.contains("## Posts"));
        assert!(result.contains("[Hello World](https://example.com/posts/hello.md): A post"));
    }

    #[test]
    fn test_llms_txt_no_description() {
        let config = test_config("https://example.com", "");
        let item = test_item("Hello", "/posts/hello", None, "body");
        let collections = vec![("Posts".to_string(), vec![&item])];
        let result = generate_llms_txt(&config, &collections);
        assert!(!result.contains("> "));
        assert!(result.contains("[Hello](https://example.com/posts/hello.md)"));
    }

    #[test]
    fn test_llms_txt_empty_collection_skipped() {
        let config = test_config("https://example.com", "");
        let collections: Vec<(String, Vec<&ContentItem>)> = vec![("Empty".to_string(), vec![])];
        let result = generate_llms_txt(&config, &collections);
        assert!(!result.contains("## Empty"));
    }

    #[test]
    fn test_llms_full_txt_basic() {
        let config = test_config("https://example.com", "Site desc");
        let item = test_item(
            "Hello",
            "/posts/hello",
            Some("A post"),
            "# Content\nParagraph",
        );
        let collections = vec![("Posts".to_string(), vec![&item])];
        let result = generate_llms_full_txt(&config, &collections);
        assert!(result.contains("# Test Site"));
        assert!(result.contains("> Site desc"));
        assert!(result.contains("## Posts"));
        assert!(result.contains("### Hello"));
        assert!(result.contains("Source: https://example.com/posts/hello"));
        assert!(result.contains("*A post*"));
        assert!(result.contains("# Content\nParagraph"));
    }

    #[test]
    fn test_llms_full_txt_no_item_description() {
        let config = test_config("https://example.com", "");
        let item = test_item("Hello", "/posts/hello", None, "Body text");
        let collections = vec![("Posts".to_string(), vec![&item])];
        let result = generate_llms_full_txt(&config, &collections);
        assert!(result.contains("### Hello"));
        assert!(result.contains("Body text"));
    }

    #[test]
    fn test_llms_full_txt_empty_collection_skipped() {
        let config = test_config("https://example.com", "");
        let collections: Vec<(String, Vec<&ContentItem>)> = vec![("Empty".to_string(), vec![])];
        let result = generate_llms_full_txt(&config, &collections);
        assert!(!result.contains("## Empty"));
    }

    #[test]
    fn test_markdown_pointer_uses_base_url_and_language() {
        let mut config = test_config("https://example.com/docs/", "");
        let pointer = markdown_pointer(&config, "en");
        assert_eq!(
            pointer,
            "> Index of this site for agents: [llms.txt](https://example.com/docs/llms.txt). \
             Full text: [llms-full.txt](https://example.com/docs/llms-full.txt). \
             All pages: [sitemap.md](https://example.com/docs/sitemap.md)."
        );
        config.languages.insert("es".into(), Default::default());
        let es = markdown_pointer(&config, "es");
        assert!(es.contains("(https://example.com/docs/es/llms.txt)"));
        assert!(es.contains("(https://example.com/docs/es/llms-full.txt)"));
        assert!(es.contains("(https://example.com/docs/sitemap.md)"));
        assert!(markdown_pointer(&config, "en").contains("(https://example.com/docs/llms.txt)"));
    }

    #[test]
    fn test_insert_markdown_pointer_after_frontmatter() {
        let md = "---\ntitle: Hi\ntags:\n- a\n---\n\n# Body\n\ntext";
        assert_eq!(
            insert_markdown_pointer(md, "> P"),
            "---\ntitle: Hi\ntags:\n- a\n---\n\n> P\n\n# Body\n\ntext"
        );
        // Frontmatter as generated (closing `---` then the body).
        assert_eq!(
            insert_markdown_pointer("---\ntitle: Hi\n---\nBody", "> P"),
            "---\ntitle: Hi\n---\n\n> P\n\nBody"
        );
        // A `---` thematic break in the body is not the frontmatter end.
        assert_eq!(
            insert_markdown_pointer("---\na: 1\n---\n\nx\n\n---\n\ny", "> P"),
            "---\na: 1\n---\n\n> P\n\nx\n\n---\n\ny"
        );
    }

    #[test]
    fn test_insert_markdown_pointer_without_frontmatter() {
        assert_eq!(
            insert_markdown_pointer("# Posts\n\n- [a](/a)\n", "> P"),
            "> P\n\n# Posts\n\n- [a](/a)\n"
        );
        // An unterminated block is not frontmatter.
        assert_eq!(
            insert_markdown_pointer("---\nnot closed", "> P"),
            "> P\n\n---\nnot closed"
        );
        assert_eq!(insert_markdown_pointer("", "> P"), "> P\n\n");
    }

    #[test]
    fn test_sitemap_md_selection_and_order() {
        let config = test_config("https://example.com", "Desc");
        let b = test_item("B post", "/posts/b", Some("About B"), "");
        let a = test_item("A post", "/posts/a", None, "");
        let mut hidden = test_item("Hidden", "/posts/hidden", None, "");
        hidden.frontmatter.robots = Some("NoIndex, follow".into());
        let mut draft = test_item("Draft", "/posts/draft", None, "");
        draft.frontmatter.draft = true;
        let about = test_item("About", "/about", Some("Who"), "");
        let sections = vec![
            ("Posts".to_string(), vec![&b, &hidden, &a, &draft]),
            ("Empty".to_string(), vec![]),
            ("Only noindex".to_string(), vec![&hidden]),
            ("Pages".to_string(), vec![&about]),
        ];
        let md = generate_sitemap_md(&config, &sections);
        assert!(md.starts_with("# Test Site: sitemap\n\n> Desc\n\n"));
        assert!(md.contains(
            "## Posts\n\n- [B post](https://example.com/posts/b.md) — About B\n- [A post](https://example.com/posts/a.md)\n\n## Pages\n\n- [About](https://example.com/about.md) — Who\n"
        ));
        assert!(!md.contains("Hidden"));
        assert!(!md.contains("Draft"));
        assert!(!md.contains("## Empty"));
        assert!(!md.contains("## Only noindex"));
        assert!(!md.contains("Index of this site for agents"));
    }

    #[test]
    fn test_llms_txt_links_sitemap_md() {
        let config = test_config("https://example.com", "");
        let result = generate_llms_txt(&config, &[]);
        assert!(result.contains("[sitemap.md](https://example.com/sitemap.md)"));
    }
}
