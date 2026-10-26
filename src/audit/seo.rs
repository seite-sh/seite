//! SEO checks: titles, descriptions, canonical, headings, thin and orphan
//! pages, broken anchors, sitemap consistency, image dimensions, og:image,
//! hreflang reciprocity.
//!
//! Thresholds: title ≤ 60 characters; description 50–160 characters; thin
//! content < 150 words (content pages; changelog and roadmap entries are
//! status updates and exempt). Heading checks look inside `<main>` when the
//! page has one (a site-title `<h1>` in the header is not the page's h1).

use std::collections::{BTreeMap, HashSet};

use super::{Finding, Needle, Page, Site};

pub const TITLE_MAX: usize = 60;
pub const DESCRIPTION_MIN: usize = 50;
pub const DESCRIPTION_MAX: usize = 160;
pub const THIN_WORDS: usize = 150;
/// Collections whose entries are short by nature.
const THIN_EXEMPT: &[&str] = &["changelog", "roadmap"];

pub fn check(site: &Site, out: &mut Vec<Finding>) {
    titles(site, out);
    descriptions(site, out);
    for page in site.pages.iter().filter(|p| !p.is_auxiliary()) {
        canonical(site, page, out);
        headings(site, page, out);
        thin(site, page, out);
        image_dimensions(site, page, out);
        og_image(site, page, out);
        hreflang(site, page, out);
        anchors(site, page, out);
    }
    orphans(site, out);
    sitemap(site, out);
}

fn titles(site: &Site, out: &mut Vec<Finding>) {
    let mut by_title: BTreeMap<&str, Vec<&Page>> = BTreeMap::new();
    for page in site.pages.iter().filter(|p| !p.is_auxiliary()) {
        let title = page.facts.titles.first().map(String::as_str).unwrap_or("");
        if title.is_empty() {
            out.push(
                Finding::new(
                    "seo-title-missing",
                    Some(&page.url),
                    "the page has no <title>",
                )
                .at(title_loc(site, page)),
            );
            continue;
        }
        let len = title.chars().count();
        if len > TITLE_MAX {
            out.push(
                Finding::new(
                    "seo-title-long",
                    Some(&page.url),
                    format!("<title> is {len} characters (over {TITLE_MAX}): \"{title}\""),
                )
                .at(title_loc(site, page)),
            );
        }
        if !page.noindex() {
            by_title.entry(title).or_default().push(page);
        }
    }
    for (title, pages) in by_title.into_iter().filter(|(_, p)| p.len() > 1) {
        for page in &pages {
            let others: Vec<&str> = pages
                .iter()
                .filter(|p| p.url != page.url)
                .map(|p| p.url.as_str())
                .take(3)
                .collect();
            out.push(
                Finding::new(
                    "seo-title-duplicate",
                    Some(&page.url),
                    format!("<title> \"{title}\" is also used by {}", others.join(", ")),
                )
                .at(title_loc(site, page)),
            );
        }
    }
}

/// Title findings belong to the page (its `title:` line, else the
/// generated page), never to the shared layout: each page's title is its own.
fn title_loc(site: &Site, page: &Page) -> super::Loc {
    site.locate_page(page, Some("title"))
}

fn descriptions(site: &Site, out: &mut Vec<Finding>) {
    let mut by_desc: BTreeMap<String, Vec<&Page>> = BTreeMap::new();
    for page in site
        .pages
        .iter()
        .filter(|p| p.is_content(site.build_mode) && !p.noindex())
    {
        // Built pages without `description:` get the site description as
        // their meta description: report the missing frontmatter key.
        let desc = match &page.item {
            Some(item) => item.description.clone().unwrap_or_default(),
            // `--dist`: the site description is the themes' fallback.
            None => page
                .facts
                .description
                .clone()
                .filter(|d| site.site_description.is_empty() || d.trim() != site.site_description)
                .unwrap_or_default(),
        };
        let desc = super::html::normalize_space(&desc);
        let loc = site.locate_page(page, Some("description"));
        if desc.is_empty() {
            out.push(
                Finding::new(
                    "seo-description-missing",
                    Some(&page.url),
                    "no description: search engines and link previews will pick their own snippet",
                )
                .at(loc),
            );
            continue;
        }
        let len = desc.chars().count();
        if len > DESCRIPTION_MAX {
            out.push(
                Finding::new(
                    "seo-description-long",
                    Some(&page.url),
                    format!("description is {len} characters (over {DESCRIPTION_MAX})"),
                )
                .at(loc.clone()),
            );
        } else if len < DESCRIPTION_MIN {
            out.push(
                Finding::new(
                    "seo-description-short",
                    Some(&page.url),
                    format!(
                        "description is {len} characters (under {DESCRIPTION_MIN}): \"{desc}\""
                    ),
                )
                .at(loc.clone()),
            );
        }
        by_desc.entry(desc).or_default().push(page);
    }
    for (_, pages) in by_desc.into_iter().filter(|(_, p)| p.len() > 1) {
        for page in &pages {
            let others: Vec<&str> = pages
                .iter()
                .filter(|p| p.url != page.url)
                .map(|p| p.url.as_str())
                .take(3)
                .collect();
            out.push(
                Finding::new(
                    "seo-description-duplicate",
                    Some(&page.url),
                    format!("same description as {}", others.join(", ")),
                )
                .at(site.locate_page(page, Some("description"))),
            );
        }
    }
}

fn canonical(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    if page
        .facts
        .canonical
        .as_deref()
        .is_none_or(|c| c.trim().is_empty())
    {
        out.push(
            Finding::new(
                "seo-canonical-missing",
                Some(&page.url),
                "no <link rel=\"canonical\">",
            )
            .at(site.locate_layout(page, "</head>", Some(page.facts.head_offset))),
        );
    }
}

/// Markdown spelling of a heading, for locating it in the source.
fn md_heading(level: u8, text: &str) -> String {
    format!("{} {text}", "#".repeat(usize::from(level)))
}

fn headings(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    let f = &page.facts;
    let scoped = |h: &&super::html::Heading| !f.has_main || h.in_main;
    let h1s: Vec<_> = f.headings.iter().filter(|h| h.level == 1).collect();
    if h1s.is_empty() {
        out.push(
            Finding::new("seo-h1-missing", Some(&page.url), "the page has no <h1>")
                .at(site.locate_layout(page, "<main", None)),
        );
    }
    let main_h1s: Vec<_> = h1s.iter().copied().filter(scoped).collect();
    if main_h1s.len() > 1 {
        let second = main_h1s[1];
        let md = md_heading(1, &second.text);
        out.push(
            Finding::new(
                "seo-h1-multiple",
                Some(&page.url),
                format!(
                    "{} <h1> elements{}: \"{}\" and \"{}\"",
                    main_h1s.len(),
                    if f.has_main { " in <main>" } else { "" },
                    main_h1s[0].text,
                    second.text
                ),
            )
            .at(site.locate(page, &[Needle::Text(&md)], Some(second.offset))),
        );
    }
    let mut prev: Option<u8> = None;
    for h in &f.headings {
        if let Some(p) = prev {
            if h.level > p + 1 && scoped(&h) {
                let md = md_heading(h.level, &h.text);
                out.push(
                    Finding::new(
                        "seo-heading-skip",
                        Some(&page.url),
                        format!("<h{}> \"{}\" follows an <h{p}>", h.level, h.text),
                    )
                    .at(site.locate(
                        page,
                        &[Needle::Text(&md), Needle::Text(&h.text)],
                        Some(h.offset),
                    )),
                );
            }
        }
        prev = Some(h.level);
    }
}

fn thin(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    if !page.is_content(site.build_mode) || page.noindex() {
        return;
    }
    if site
        .collection_of(page)
        .is_some_and(|c| THIN_EXEMPT.contains(&c))
    {
        return;
    }
    let words = match &page.item {
        Some(item) => item.word_count,
        None => page.facts.main_words,
    };
    if words < THIN_WORDS {
        out.push(
            Finding::new(
                "seo-thin-content",
                Some(&page.url),
                format!("{words} words (under {THIN_WORDS})"),
            )
            .at(site.locate_page(page, None)),
        );
    }
}

/// Image URLs that are inline data (no request, no layout shift concern).
fn is_inline(src: &str) -> bool {
    src.starts_with("data:")
}

fn image_dimensions(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    for img in &page.facts.imgs {
        let Some(src) = img.src.as_deref().filter(|s| !is_inline(s)) else {
            continue;
        };
        if img.width.is_none() || img.height.is_none() {
            out.push(
                Finding::new(
                    "seo-img-dimensions",
                    Some(&page.url),
                    format!("<img src=\"{src}\"> has no width/height (layout shift while loading)"),
                )
                .at(site.locate(page, &[Needle::Url(src)], Some(img.offset))),
            );
        }
    }
}

fn og_image(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    if page.is_content(site.build_mode) && !page.noindex() && !page.facts.og_image {
        out.push(
            Finding::new(
                "seo-og-image-missing",
                Some(&page.url),
                "no og:image: shared links show no preview image",
            )
            .at(site.locate_page(page, Some("image"))),
        );
    }
}

fn hreflang(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    let Some(me) = site.page_at(&page.url) else {
        return;
    };
    for (lang, href) in &page.facts.hreflangs {
        if lang.eq_ignore_ascii_case("x-default") {
            continue;
        }
        let Some((path, _)) = site.internal_target(page, href) else {
            continue;
        };
        let Some(target) = site.page_at(&path) else {
            out.push(
                Finding::new(
                    "seo-hreflang",
                    Some(&page.url),
                    format!("hreflang=\"{lang}\" points at {path}, which was not built"),
                )
                .at(site.output_loc(page, None)),
            );
            continue;
        };
        if target == me {
            continue;
        }
        let other = &site.pages[target];
        let links_back = other.facts.hreflangs.iter().any(|(_, h)| {
            site.internal_target(other, h)
                .and_then(|(p, _)| site.page_at(&p))
                == Some(me)
        });
        if !links_back {
            out.push(
                Finding::new(
                    "seo-hreflang",
                    Some(&page.url),
                    format!(
                        "hreflang=\"{lang}\" points at {}, which has no hreflang link back",
                        other.url
                    ),
                )
                .at(site.output_loc(page, None)),
            );
        }
    }
}

fn anchors(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    for link in &page.facts.links {
        let Some((path, Some(frag))) = site.internal_target(page, &link.href) else {
            continue;
        };
        if frag.is_empty() || frag.eq_ignore_ascii_case("top") || frag.starts_with('!') {
            continue;
        }
        let Some(target) = site.page_at(&path) else {
            continue; // a broken link: `seite check` reports it
        };
        let frag = urlencoding::decode(&frag)
            .map(|d| d.into_owned())
            .unwrap_or(frag);
        if !site.pages[target]
            .facts
            .ids
            .iter()
            .any(|(id, _)| *id == frag)
        {
            out.push(
                Finding::new(
                    "seo-broken-anchor",
                    Some(&page.url),
                    format!(
                        "link `{}`: no element with id \"{frag}\" on {}",
                        link.href, site.pages[target].url
                    ),
                )
                .at(site.locate(
                    page,
                    &[Needle::Url(&link.href)],
                    Some(link.offset),
                )),
            );
        }
    }
}

fn orphans(site: &Site, out: &mut Vec<Finding>) {
    let mut linked: HashSet<usize> = HashSet::new();
    for (i, page) in site.pages.iter().enumerate() {
        if page.is_auxiliary() {
            continue;
        }
        for link in &page.facts.links {
            if let Some(target) = site
                .internal_target(page, &link.href)
                .and_then(|(p, _)| site.page_at(&p))
            {
                if target != i {
                    linked.insert(target);
                }
            }
        }
    }
    for (i, page) in site.pages.iter().enumerate() {
        if page.is_content(site.build_mode)
            && !page.is_listing()
            && !page.noindex()
            && !linked.contains(&i)
        {
            out.push(
                Finding::new(
                    "seo-orphan-page",
                    Some(&page.url),
                    "no other page links here (only the sitemap and llms.txt do)",
                )
                .at(site.locate_page(page, None)),
            );
        }
    }
}

/// `<loc>` values of a sitemap.
pub fn sitemap_locs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<loc>") {
        rest = &rest[i + 5..];
        let Some(end) = rest.find("</loc>") else {
            break;
        };
        out.push(super::html::decode_entities(rest[..end].trim()));
        rest = &rest[end..];
    }
    out
}

fn sitemap(site: &Site, out: &mut Vec<Finding>) {
    let Ok(xml) = std::fs::read_to_string(site.output.join("sitemap.xml")) else {
        out.push(
            Finding::new("seo-sitemap-missing", None, "no sitemap.xml in the output")
                .at(site.site_file("sitemap.xml")),
        );
        return;
    };
    let mut listed: HashSet<usize> = HashSet::new();
    for loc in sitemap_locs(&xml) {
        let target = site
            .resolve_href("/", &loc)
            .and_then(|(p, _)| site.page_at(&p));
        let Some(i) = target else {
            out.push(
                Finding::new(
                    "seo-sitemap-stale-url",
                    None,
                    format!("sitemap.xml lists {loc}, which has no built page"),
                )
                .at(site.site_file("sitemap.xml")),
            );
            continue;
        };
        listed.insert(i);
        let page = &site.pages[i];
        if page.noindex() {
            out.push(
                Finding::new(
                    "seo-noindex-in-sitemap",
                    Some(&page.url),
                    "the page is noindex but listed in sitemap.xml (contradicting signals)",
                )
                .at(site.locate_page(page, Some("robots"))),
            );
        }
    }
    for (i, page) in site.pages.iter().enumerate() {
        let private = page
            .item
            .as_ref()
            .is_some_and(|it| site.private_collections.contains(&it.collection));
        if page.is_content(site.build_mode) && !page.noindex() && !private && !listed.contains(&i) {
            out.push(
                Finding::new(
                    "seo-page-not-in-sitemap",
                    Some(&page.url),
                    "the page is not listed in sitemap.xml",
                )
                .at(site.locate_page(page, None)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{codes, page, site};
    use super::*;

    #[test]
    fn test_titles() {
        let long = "x".repeat(61);
        let s = site(&[
            ("a.html", "<html><head></head><body></body></html>"),
            ("b.html", &format!("<html><head><title>{long}</title></head></html>")),
            ("c.html", "<html><head><title>Same</title></head></html>"),
            ("d.html", "<html><head><title>Same</title></head></html>"),
            ("e.html", "<html><head><title>Same</title><meta name=\"robots\" content=\"noindex\"></head></html>"),
        ]);
        let mut out = Vec::new();
        titles(&s, &mut out);
        let c: Vec<_> = out
            .iter()
            .map(|f| (f.code, f.page_url.clone().unwrap()))
            .collect();
        assert!(c.contains(&("seo-title-missing", "/a".into())));
        assert!(c.contains(&("seo-title-long", "/b".into())));
        assert!(c.contains(&("seo-title-duplicate", "/c".into())));
        assert!(c.contains(&("seo-title-duplicate", "/d".into())));
        assert!(
            !c.contains(&("seo-title-duplicate", "/e".into())),
            "noindex pages don't compete"
        );
    }

    #[test]
    fn test_descriptions_in_dist_mode() {
        let md = "<link rel=\"alternate\" type=\"text/markdown\" href=\"/x.md\">";
        let d = |desc: &str| {
            page(
                &format!("{md}<meta name=\"description\" content=\"{desc}\">"),
                "",
            )
        };
        let ok = "A description that is comfortably longer than fifty characters.";
        let s = site(&[
            ("a.html", &page(md, "")),
            ("b.html", &d("Too short")),
            ("c.html", &d(&"y".repeat(161))),
            ("e.html", &d(ok)),
            ("f.html", &d(ok)),
            // Listing pages are not content pages.
            ("posts/index.html", &page("", "")),
        ]);
        let mut out = Vec::new();
        descriptions(&s, &mut out);
        let c: Vec<_> = out
            .iter()
            .map(|f| (f.code, f.page_url.clone().unwrap()))
            .collect();
        assert_eq!(
            c,
            [
                ("seo-description-missing", "/a".to_string()),
                ("seo-description-short", "/b".to_string()),
                ("seo-description-long", "/c".to_string()),
                ("seo-description-duplicate", "/e".to_string()),
                ("seo-description-duplicate", "/f".to_string()),
            ]
        );
    }

    #[test]
    fn test_headings() {
        let s = site(&[
            ("a.html", &page("", "<h1>A</h1><h3>Skip</h3>")),
            ("b.html", "<html><body><header><h1>Site</h1></header><main><h1>One</h1><h1>Two</h1></main></body></html>"),
            ("c.html", &page("", "<h2>No h1</h2>")),
            // Site-title h1 in the header + page h1 in main: fine.
            ("d.html", "<html><body><header><h1>Site</h1></header><main><h1>Page</h1><h2>x</h2></main></body></html>"),
        ]);
        let mut out = Vec::new();
        for p in &s.pages {
            headings(&s, p, &mut out);
        }
        let c: Vec<_> = out
            .iter()
            .map(|f| (f.code, f.page_url.clone().unwrap()))
            .collect();
        assert_eq!(
            c,
            [
                ("seo-heading-skip", "/a".to_string()),
                ("seo-h1-multiple", "/b".to_string()),
                ("seo-h1-missing", "/c".to_string()),
            ]
        );
    }

    #[test]
    fn test_anchors_orphans_and_canonical() {
        let md = "<link rel=\"alternate\" type=\"text/markdown\" href=\"/x.md\"><link rel=\"canonical\" href=\"/x\">";
        let s = site(&[
            (
                "index.html",
                &page(
                    md,
                    "<a href=\"/a#here\">a</a><a href=\"/a#gone\">g</a><a href=\"#top\">t</a>",
                ),
            ),
            ("a.html", &page(md, "<h2 id=\"here\">Here</h2>")),
            ("lonely.html", &page(md, "")),
            (
                "nocanon.html",
                &page(
                    "<link rel=\"alternate\" type=\"text/markdown\" href=\"/y.md\">",
                    "<a href=\"/lonely\">x</a>",
                ),
            ),
        ]);
        let mut out = Vec::new();
        for p in &s.pages {
            anchors(&s, p, &mut out);
            canonical(&s, p, &mut out);
        }
        orphans(&s, &mut out);
        let c: Vec<_> = out
            .iter()
            .map(|f| (f.code, f.page_url.clone().unwrap()))
            .collect();
        assert!(c.contains(&("seo-broken-anchor", "/".into())), "{c:?}");
        assert_eq!(
            c.iter().filter(|(k, _)| *k == "seo-broken-anchor").count(),
            1
        );
        assert!(c.contains(&("seo-canonical-missing", "/nocanon".into())));
        assert!(c.contains(&("seo-orphan-page", "/nocanon".into())), "{c:?}");
        assert!(!c.contains(&("seo-orphan-page", "/lonely".into())));
        assert!(!c.contains(&("seo-orphan-page", "/a".into())));
    }

    #[test]
    fn test_images_og_and_hreflang() {
        let md = "<link rel=\"alternate\" type=\"text/markdown\" href=\"/x.md\">";
        let s = site(&[
            ("a.html", &page(&format!("{md}<link rel=\"alternate\" hreflang=\"es\" href=\"/es/a\"><link rel=\"alternate\" hreflang=\"de\" href=\"/de/a\">"), "<img src=\"/i.png\" alt=\"x\">")),
            ("es/a.html", &page("<meta property=\"og:image\" content=\"/o.png\">", "<img src=\"data:image/png;base64,AA\" alt=\"\">")),
        ]);
        let mut out = Vec::new();
        for p in &s.pages {
            image_dimensions(&s, p, &mut out);
            og_image(&s, p, &mut out);
            hreflang(&s, p, &mut out);
        }
        let c: Vec<_> = out.iter().map(|f| f.code).collect();
        assert_eq!(
            c,
            [
                "seo-img-dimensions",
                "seo-og-image-missing",
                "seo-hreflang",
                "seo-hreflang"
            ]
        );
        assert!(
            out[2].message.contains("no hreflang link back"),
            "{}",
            out[2].message
        );
        assert!(
            out[3].message.contains("was not built"),
            "{}",
            out[3].message
        );
    }

    #[test]
    fn test_sitemap_locs() {
        let xml = "<urlset><url><loc>https://e.com/a?x=1&amp;y=2</loc></url><url><loc> https://e.com/b </loc></url></urlset>";
        assert_eq!(
            sitemap_locs(xml),
            ["https://e.com/a?x=1&y=2", "https://e.com/b"]
        );
    }

    #[test]
    fn test_sitemap_missing() {
        let s = site(&[("index.html", &page("", ""))]);
        assert_eq!(codes(&s, sitemap), ["seo-sitemap-missing"]);
    }
}
