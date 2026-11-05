//! Series: pages that share `series: "Name"` frontmatter, read in order.
//!
//! Parts order by `series_order` (lower first; parts without one follow), then
//! date (oldest first; undated last), then source path. Series group by slug
//! within a language, across public collections (private collections never
//! join a series, so a series index never lists a private page). Each page in
//! a series gets `page.series`; each series gets an index page at
//! `/series/<slug>/` (language-prefixed), rendered with `series.html`. Nothing
//! is generated when no page uses `series:`.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::taxonomy::{item_summary, listing_page, term_slug, RenderEnv};
use super::ItemSummary;
use crate::content::ContentItem;
use crate::error::Result;

/// URL prefix of series index pages.
pub(crate) const SERIES_PREFIX: &str = "/series";

#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct SeriesPart {
    pub title: String,
    pub url: String,
    pub current: bool,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct SeriesLink {
    pub title: String,
    pub url: String,
}

/// `page.series` (and `series` on series index pages, where no part is current
/// and `index`/`prev`/`next` are unset).
#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct SeriesContext {
    pub name: String,
    pub slug: String,
    /// URL of the series index page.
    pub url: String,
    pub parts: Vec<SeriesPart>,
    /// 1-based position of the current page.
    pub index: Option<usize>,
    pub total: usize,
    pub prev: Option<SeriesLink>,
    pub next: Option<SeriesLink>,
}

/// One series in one language, parts in reading order.
struct Series {
    name: String,
    slug: String,
    lang: String,
    url: String,
    parts: Vec<ItemSummary>,
    /// Collection of each part (parallel to `parts`).
    collections: Vec<String>,
}

/// All series of a build.
pub(crate) struct SeriesIndex {
    series: Vec<Series>,
    /// Item URL → (series position in `series`, 0-based part index).
    by_url: HashMap<String, (usize, usize)>,
    /// `series-url-collision` warnings: a series index URL that a content
    /// page already uses (the series page would overwrite it).
    pub(crate) warnings: Vec<crate::diagnostics::Diagnostic>,
}

impl SeriesIndex {
    pub(crate) fn build(
        config: &crate::config::SiteConfig,
        all_collections: &indexmap::IndexMap<String, Vec<ContentItem>>,
    ) -> Self {
        let default_lang = config.site.language.as_str();
        // (lang, slug) → parts; BTreeMap for a deterministic series order.
        let mut groups: BTreeMap<(String, String), Vec<&ContentItem>> = BTreeMap::new();
        for c in config.collections.iter().filter(|c| !c.private) {
            for item in all_collections.get(&c.name).into_iter().flatten() {
                let Some(name) = item.frontmatter.series.as_deref() else {
                    continue;
                };
                let slug = term_slug(name);
                if slug.is_empty() || item.slug == "index" {
                    continue;
                }
                groups
                    .entry((item.lang.clone(), slug))
                    .or_default()
                    .push(item);
            }
        }
        let mut series = Vec::new();
        let mut by_url = HashMap::new();
        for ((lang, slug), mut parts) in groups {
            sort_parts(&mut parts);
            let name = parts[0]
                .frontmatter
                .series
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_string();
            let prefix = super::lang_prefix_for(&lang, default_lang);
            for (i, p) in parts.iter().enumerate() {
                by_url.insert(p.url.clone(), (series.len(), i));
            }
            series.push(Series {
                url: format!("{prefix}{SERIES_PREFIX}/{slug}/"),
                name,
                slug,
                lang,
                parts: parts.iter().map(|i| item_summary(i)).collect(),
                collections: parts.iter().map(|i| i.collection.clone()).collect(),
            });
        }
        let page_urls: std::collections::HashSet<&str> = all_collections
            .values()
            .flatten()
            .map(|i| i.url.trim_end_matches('/'))
            .collect();
        let warnings = series
            .iter()
            .filter(|s| page_urls.contains(s.url.trim_end_matches('/')))
            .map(|s| {
                crate::diagnostics::Diagnostic::warning(
                    "series-url-collision",
                    format!(
                        "series \"{}\" has its index page at {}, which a content page already uses; the series page replaces it",
                        s.name, s.url
                    ),
                )
                .with_hint("rename the series or give the page a different `slug:`")
            })
            .collect();
        Self {
            series,
            by_url,
            warnings,
        }
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.series.is_empty()
    }

    /// Collections of every part of the series the item at `url` belongs to
    /// (empty when it is in none). An incremental rebuild re-renders them all
    /// when one part changes, so prev/next and part lists stay current.
    pub(crate) fn collections_of_series(&self, url: &str) -> Vec<String> {
        self.by_url
            .get(url)
            .map(|&(si, _)| self.series[si].collections.clone())
            .unwrap_or_default()
    }

    /// `page.series` for the item at `url`, if it is part of a series.
    pub(crate) fn for_page(&self, url: &str) -> Option<SeriesContext> {
        let &(si, pi) = self.by_url.get(url)?;
        Some(context(&self.series[si], Some(pi)))
    }

    /// Render every series index page; returns their URLs (for the sitemap).
    pub(crate) fn render_pages(&self, env: &RenderEnv<'_>) -> Result<Vec<String>> {
        let mut urls = Vec::new();
        if env.tera.get_template("series.html").is_err() {
            return Ok(urls);
        }
        for s in &self.series {
            let mut ctx = env.base_context(&s.lang);
            ctx.insert("series", &context(s, None));
            ctx.insert("items", &s.parts);
            ctx.insert(
                "page",
                &listing_page(
                    s.name.clone(),
                    format!("{}/{}", SERIES_PREFIX.trim_matches('/'), s.slug),
                    s.url.clone(),
                ),
            );
            env.write_page("series.html", &ctx, &s.url)?;
            urls.push(s.url.clone());
        }
        Ok(urls)
    }
}

fn sort_parts(parts: &mut [&ContentItem]) {
    parts.sort_by(|a, b| {
        let (fa, fb) = (&a.frontmatter, &b.frontmatter);
        // Some(_) before None for both order and date.
        let order = |o: Option<i32>| (o.is_none(), o);
        let date = |d: Option<chrono::NaiveDate>| (d.is_none(), d);
        order(fa.series_order)
            .cmp(&order(fb.series_order))
            .then_with(|| date(fa.date).cmp(&date(fb.date)))
            .then_with(|| a.source_path.cmp(&b.source_path))
    });
}

fn context(s: &Series, current: Option<usize>) -> SeriesContext {
    let link = |i: usize| {
        s.parts.get(i).map(|p| SeriesLink {
            title: p.title.clone(),
            url: p.url.clone(),
        })
    };
    SeriesContext {
        name: s.name.clone(),
        slug: s.slug.clone(),
        url: s.url.clone(),
        parts: s
            .parts
            .iter()
            .enumerate()
            .map(|(i, p)| SeriesPart {
                title: p.title.clone(),
                url: p.url.clone(),
                current: Some(i) == current,
            })
            .collect(),
        index: current.map(|i| i + 1),
        total: s.parts.len(),
        prev: current.and_then(|i| i.checked_sub(1)).and_then(link),
        next: current.and_then(|i| link(i + 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SiteConfig;
    use crate::content::Frontmatter;

    fn item(file: &str, lang: &str, fm_extra: &str) -> ContentItem {
        let yaml = format!("title: {file}\n{fm_extra}");
        let fm: Frontmatter = serde_yaml_ng::from_str(&yaml).unwrap();
        let prefix = if lang == "en" {
            String::new()
        } else {
            format!("/{lang}")
        };
        ContentItem {
            frontmatter: fm,
            raw_body: String::new(),
            html_body: String::new(),
            source_path: format!("content/posts/{file}.md").into(),
            slug: file.to_string(),
            collection: "posts".into(),
            url: format!("{prefix}/posts/{file}"),
            lang: lang.into(),
            excerpt: String::new(),
            toc: Vec::new(),
            word_count: 0,
            reading_time: 0,
            excerpt_html: String::new(),
        }
    }

    fn config() -> SiteConfig {
        toml::from_str("[site]\ntitle = \"T\"\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\ndefault_template = \"post.html\"\n[[collections]]\nname = \"secret\"\nlabel = \"S\"\ndirectory = \"secret\"\ndefault_template = \"post.html\"\nprivate = true\n[languages.es]\n").unwrap()
    }

    #[test]
    fn test_series_order_prev_next() {
        let mut all = indexmap::IndexMap::new();
        all.insert(
            "posts".to_string(),
            vec![
                item("c", "en", "series: Rust\ndate: 2024-01-01\n"),
                item("b", "en", "series: rust\nseries_order: 2\n"),
                item("a", "en", "series: Rust\nseries_order: 1\n"),
                item("d", "en", "series: Rust\n"),
                item("e", "en", "series: Rust\ndate: 2023-01-01\n"),
                item("z", "en", "title2: none\n"),
                item("es1", "es", "series: Rust\n"),
            ],
        );
        all.insert(
            "secret".to_string(),
            vec![item("hidden", "en", "series: Rust\n")],
        );
        let idx = SeriesIndex::build(&config(), &all);
        let s = idx.for_page("/posts/c").unwrap();
        let order: Vec<&str> = s.parts.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(order, ["a", "b", "e", "c", "d"]);
        assert_eq!(s.index, Some(4));
        assert_eq!(s.total, 5);
        assert_eq!(s.prev.as_ref().unwrap().url, "/posts/e");
        assert_eq!(s.next.as_ref().unwrap().url, "/posts/d");
        assert!(s.parts[3].current && !s.parts[0].current);
        assert_eq!(s.url, "/series/rust/");
        assert_eq!(s.name, "Rust");

        let first = idx.for_page("/posts/a").unwrap();
        assert!(first.prev.is_none());
        assert_eq!(idx.for_page("/posts/d").unwrap().next, None);
        assert!(idx.for_page("/posts/z").is_none());
        assert!(idx.for_page("/posts/hidden").is_none());
        assert_eq!(idx.collections_of_series("/posts/a").len(), 5);
        assert!(idx.collections_of_series("/posts/z").is_empty());

        let es = idx.for_page("/es/posts/es1").unwrap();
        assert_eq!(es.url, "/es/series/rust/");
        assert_eq!(es.total, 1);
    }

    #[test]
    fn test_series_empty_when_unused() {
        let mut all = indexmap::IndexMap::new();
        all.insert("posts".to_string(), vec![item("a", "en", "")]);
        assert!(SeriesIndex::build(&config(), &all).is_empty());
    }

    #[test]
    fn test_series_url_collision_warns() {
        let mut all = indexmap::IndexMap::new();
        let mut page = item("rust", "en", "");
        page.url = "/series/rust".into();
        all.insert(
            "posts".to_string(),
            vec![item("a", "en", "series: Rust\n"), page],
        );
        let idx = SeriesIndex::build(&config(), &all);
        assert_eq!(idx.warnings.len(), 1);
        assert_eq!(idx.warnings[0].code, "series-url-collision");
    }
}
