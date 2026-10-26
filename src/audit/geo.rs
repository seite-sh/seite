//! GEO (AI visibility) checks: llms.txt present and complete, markdown
//! alternates, JSON-LD that parses and has an `@type`, robots.txt letting AI
//! search crawlers in, and a Content-Signal line.
//!
//! "FAQ-like content without FAQ schema" is deliberately not checked: it
//! can't be detected reliably offline without noisy guesses.

use std::collections::HashMap;

use super::{Finding, Page, Site};

/// AI *search* crawlers (answer engines that cite and link). Training
/// crawlers (GPTBot, ClaudeBot, ...) may be blocked without a finding.
pub const AI_SEARCH_BOTS: &[&str] = &[
    "OAI-SearchBot",
    "ChatGPT-User",
    "PerplexityBot",
    "Claude-SearchBot",
];

pub fn check(site: &Site, out: &mut Vec<Finding>) {
    llms_txt(site, out);
    robots(site, out);
    for page in site.pages.iter().filter(|p| !p.is_auxiliary()) {
        markdown_alternate(site, page, out);
        json_ld(site, page, out);
    }
}

/// llms.txt entries: page key → has a summary (`- [t](url): summary`).
pub fn llms_entries(site: &Site, text: &str) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    let home = site.pages.iter().find(|p| p.url == "/");
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("- [") else {
            continue;
        };
        let Some(close) = rest.find("](") else {
            continue;
        };
        let rest = &rest[close + 2..];
        let Some(end) = rest.find(')') else {
            continue;
        };
        let url = &rest[..end];
        let url = url.strip_suffix(".md").unwrap_or(url);
        let summary = rest[end + 1..]
            .trim_start()
            .strip_prefix(':')
            .is_some_and(|s| !s.trim().is_empty());
        let key = match home.and_then(|h| site.internal_target(h, url)) {
            Some((path, _)) => super::page_key(&path),
            None => continue,
        };
        out.insert(key, summary);
    }
    out
}

fn page_lang<'a>(site: &'a Site, page: &'a Page) -> &'a str {
    page.item
        .as_ref()
        .map(|i| i.lang.as_str())
        .or(page.facts.lang.as_deref())
        .unwrap_or(&site.default_lang)
}

fn llms_txt(site: &Site, out: &mut Vec<Finding>) {
    let Ok(text) = std::fs::read_to_string(site.output.join("llms.txt")) else {
        out.push(
            Finding::new("geo-llms-txt-missing", None, "no llms.txt in the output")
                .at(site.site_file("llms.txt")),
        );
        return;
    };
    let entries = llms_entries(site, &text);
    for page in &site.pages {
        let private = page
            .item
            .as_ref()
            .is_some_and(|it| site.private_collections.contains(&it.collection));
        if !page.is_content(site.build_mode)
            || page.noindex()
            || private
            || page_lang(site, page) != site.default_lang
        {
            continue;
        }
        match entries.get(&super::page_key(&page.url)) {
            None => out.push(
                Finding::new(
                    "geo-llms-txt-unlisted",
                    Some(&page.url),
                    "the page is not listed in llms.txt",
                )
                .at(site.locate_page(page, None)),
            ),
            Some(false) => out.push(
                Finding::new(
                    "geo-llms-no-summary",
                    Some(&page.url),
                    "llms.txt lists the page without a summary (it has no description)",
                )
                .at(site.locate_page(page, Some("description"))),
            ),
            Some(true) => {}
        }
    }
}

fn markdown_alternate(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    if page.is_content(site.build_mode) && page.facts.markdown_alternate.is_none() {
        out.push(
            Finding::new(
                "geo-markdown-alternate-missing",
                Some(&page.url),
                "no <link rel=\"alternate\" type=\"text/markdown\"> to the page's .md copy",
            )
            .at(site.locate_layout(page, "</head>", Some(page.facts.head_offset))),
        );
    }
}

/// True when every object in `value` (an object, an array, or `@graph`)
/// has an `@type`.
fn has_types(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(items) => !items.is_empty() && items.iter().all(has_types),
        serde_json::Value::Object(map) => {
            if let Some(graph) = map.get("@graph") {
                return has_types(graph);
            }
            map.get("@type").is_some_and(|t| !t.is_null())
        }
        _ => false,
    }
}

fn json_ld(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    let blocks: Vec<_> = page.facts.json_ld().collect();
    if blocks.is_empty() {
        out.push(
            Finding::new(
                "geo-jsonld-missing",
                Some(&page.url),
                "no JSON-LD structured data",
            )
            .at(site.locate_layout(page, "</head>", Some(page.facts.head_offset))),
        );
        return;
    }
    for block in blocks {
        let loc = site.locate_layout(page, "application/ld+json", Some(block.offset));
        match serde_json::from_str::<serde_json::Value>(block.body.trim()) {
            Err(e) => out.push(
                Finding::new(
                    "geo-jsonld-invalid",
                    Some(&page.url),
                    format!("JSON-LD does not parse: {e}"),
                )
                .at(loc),
            ),
            Ok(value) if !has_types(&value) => out.push(
                Finding::new(
                    "geo-jsonld-no-type",
                    Some(&page.url),
                    "a JSON-LD object has no \"@type\"",
                )
                .at(loc),
            ),
            Ok(_) => {}
        }
    }
}

/// A `robots.txt` group: its user-agents (lowercased) and rules as
/// `(allow, path)`.
pub type RobotsGroup = (Vec<String>, Vec<(bool, String)>);

/// The groups of a `robots.txt`.
pub fn robots_groups(text: &str) -> Vec<RobotsGroup> {
    let mut groups: Vec<RobotsGroup> = Vec::new();
    let mut last_was_agent = false;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        match key.as_str() {
            "user-agent" => {
                if !last_was_agent || groups.is_empty() {
                    groups.push((Vec::new(), Vec::new()));
                }
                if let Some(g) = groups.last_mut() {
                    g.0.push(value.to_ascii_lowercase());
                }
                last_was_agent = true;
            }
            "allow" | "disallow" => {
                if let Some(g) = groups.last_mut() {
                    g.1.push((key == "allow", value));
                }
                last_was_agent = false;
            }
            _ => last_was_agent = false,
        }
    }
    groups
}

/// Whether `bot` may fetch `/` under `groups` (its own group, else `*`;
/// longest matching rule wins, `Allow` on ties; no rule allows).
pub fn robots_allows_root(groups: &[RobotsGroup], bot: &str) -> bool {
    let bot = bot.to_ascii_lowercase();
    let group = groups
        .iter()
        .find(|(agents, _)| agents.contains(&bot))
        .or_else(|| {
            groups
                .iter()
                .find(|(agents, _)| agents.iter().any(|a| a == "*"))
        });
    let Some((_, rules)) = group else {
        return true;
    };
    let mut best: Option<(usize, bool)> = None;
    for (allow, path) in rules {
        if path.is_empty() {
            continue; // `Disallow:` with no path allows everything
        }
        let matches = path == "/" || path == "/*" || path == "*" || path == "/$";
        if !matches {
            continue;
        }
        let len = path.len();
        if best.is_none_or(|(l, a)| len > l || (len == l && *allow && !a)) {
            best = Some((len, *allow));
        }
    }
    best.is_none_or(|(_, allow)| allow)
}

fn robots(site: &Site, out: &mut Vec<Finding>) {
    let Ok(text) = std::fs::read_to_string(site.output.join("robots.txt")) else {
        out.push(
            Finding::new("geo-robots-missing", None, "no robots.txt in the output")
                .at(site.site_file("robots.txt")),
        );
        return;
    };
    let loc = site.site_file("robots.txt");
    let groups = robots_groups(&text);
    let blocked: Vec<&str> = AI_SEARCH_BOTS
        .iter()
        .copied()
        .filter(|bot| !robots_allows_root(&groups, bot))
        .collect();
    if !blocked.is_empty() {
        out.push(
            Finding::new(
                "geo-robots-blocks-ai-search",
                None,
                format!(
                    "robots.txt blocks AI search crawlers: {} (the site can't be cited in AI answers)",
                    blocked.join(", ")
                ),
            )
            .at(loc.clone()),
        );
    }
    let has_signal = text
        .lines()
        .any(|l| l.trim().to_ascii_lowercase().starts_with("content-signal:"));
    if !has_signal {
        out.push(
            Finding::new(
                "geo-content-signal-missing",
                None,
                "robots.txt has no Content-Signal line (how AI may use the content)",
            )
            .at(loc),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{codes, page, site};
    use super::*;

    #[test]
    fn test_robots_rules() {
        let g = robots_groups(
            "User-agent: *\nAllow: /\n\nUser-agent: GPTBot\nUser-agent: PerplexityBot\nDisallow: /\n\nUser-agent: OAI-SearchBot\nDisallow:\n",
        );
        assert!(robots_allows_root(&g, "ChatGPT-User"));
        assert!(!robots_allows_root(&g, "PerplexityBot"));
        assert!(!robots_allows_root(&g, "gptbot"));
        assert!(robots_allows_root(&g, "OAI-SearchBot"));
        let g = robots_groups("User-agent: *\nDisallow: /\nAllow: /\n");
        assert!(
            robots_allows_root(&g, "Claude-SearchBot"),
            "Allow wins ties"
        );
        assert!(robots_allows_root(&[], "x"));
    }

    #[test]
    fn test_has_types() {
        let v = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
        assert!(has_types(&v(r#"{"@type":"X"}"#)));
        assert!(has_types(&v(r#"[{"@type":"X"},{"@type":"Y"}]"#)));
        assert!(has_types(&v(r#"{"@graph":[{"@type":"X"}]}"#)));
        assert!(!has_types(&v(r#"{"name":"x"}"#)));
        assert!(!has_types(&v(r#"[]"#)));
    }

    #[test]
    fn test_json_ld_and_markdown_alternate() {
        let md = "<link rel=\"alternate\" type=\"text/markdown\" href=\"/x.md\">";
        let s = site(&[
            ("a.html", &page(md, "")),
            (
                "b.html",
                &page(
                    &format!("{md}<script type=\"application/ld+json\">{{bad</script>"),
                    "",
                ),
            ),
            (
                "c.html",
                &page(
                    &format!("{md}<script type=\"application/ld+json\">{{\"name\":1}}</script>"),
                    "",
                ),
            ),
            (
                "d.html",
                &page(
                    "<script type=\"application/ld+json\">{\"@type\":\"WebPage\"}</script>",
                    "",
                ),
            ),
        ]);
        let mut out = Vec::new();
        for p in &s.pages {
            markdown_alternate(&s, p, &mut out);
            json_ld(&s, p, &mut out);
        }
        let c: Vec<_> = out
            .iter()
            .map(|f| (f.code, f.page_url.clone().unwrap()))
            .collect();
        assert_eq!(
            c,
            [
                ("geo-jsonld-missing", "/a".to_string()),
                ("geo-jsonld-invalid", "/b".to_string()),
                ("geo-jsonld-no-type", "/c".to_string()),
            ]
        );
        // /d has no markdown alternate, so it is not a content page in
        // `--dist` mode: no markdown-alternate finding.
    }

    #[test]
    fn test_llms_txt_and_robots_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let md = "<link rel=\"alternate\" type=\"text/markdown\" href=\"/x.md\">";
        let mut s = site(&[
            ("index.html", &page("", "")),
            ("a.html", &page(md, "")),
            ("b.html", &page(md, "")),
            ("c.html", &page(md, "")),
            (
                "es/c.html",
                &page(md, "").replace("lang=\"en\"", "lang=\"es\""),
            ),
        ]);
        s.output = tmp.path().to_path_buf();
        let found = codes(&s, check);
        assert!(found.contains(&"geo-llms-txt-missing"), "{found:?}");
        assert!(found.contains(&"geo-robots-missing"), "{found:?}");
        std::fs::write(
            tmp.path().join("llms.txt"),
            "# S\n\n- [A](https://example.com/a.md): Summary\n- [B](https://example.com/b.md)\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("robots.txt"),
            "User-agent: *\nAllow: /\n\nUser-agent: Claude-SearchBot\nDisallow: /\n",
        )
        .unwrap();
        let mut out = Vec::new();
        llms_txt(&s, &mut out);
        robots(&s, &mut out);
        let c: Vec<_> = out
            .iter()
            .map(|f| (f.code, f.page_url.clone().unwrap_or_default()))
            .collect();
        assert_eq!(
            c,
            [
                ("geo-llms-no-summary", "/b".to_string()),
                ("geo-llms-txt-unlisted", "/c".to_string()),
                ("geo-robots-blocks-ai-search", String::new()),
                ("geo-content-signal-missing", String::new()),
            ]
        );
        assert!(out[2].message.contains("Claude-SearchBot"));
    }
}
