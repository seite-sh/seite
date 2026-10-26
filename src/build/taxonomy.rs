//! Taxonomies: `tags` (always on) plus any `[[taxonomies]]` from `seite.toml`.
//!
//! Each taxonomy gets, per language, an index page listing every term with its
//! count, a page per term (paginated when `paginate` is set) and, when `feed`
//! is on, per-term RSS and Atom feeds. Terms group case-insensitively by slug.
//!
//! `tags` keeps its historical output exactly when it is not configured: the
//! `tags.html` / `tag.html` templates, `/tags/` URLs, lowercase term names, and
//! no per-tag feeds. Configuring `[[taxonomies]] name = "tags"` opts it into the
//! same options as every other taxonomy (feeds default to on).

use std::collections::{BTreeMap, HashMap};
use std::fs;

use serde::Serialize;

use super::{
    insert_build_flags, insert_i18n_context, render_failure, ItemSummary, PageContext,
    PaginationContext, SiteContext, TranslationLink,
};
use crate::config::{ResolvedPaths, SiteConfig, TaxonomyConfig};
use crate::content::ContentItem;
use crate::error::Result;

/// A taxonomy with every default resolved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Taxonomy {
    pub name: String,
    pub label: String,
    /// `label` was set in the config (otherwise `t.<name>` may translate it).
    pub label_explicit: bool,
    /// Root-relative URL prefix of the term index, without a trailing slash (`/tags`).
    pub url_prefix: String,
    pub feed: bool,
    pub paginate: Option<usize>,
    pub template: String,
    pub term_template: String,
    /// `tags` without a `[[taxonomies]]` entry: legacy output, no feeds.
    pub implicit: bool,
    /// Weight of one shared term of this taxonomy when scoring related pages.
    pub related_weight: u32,
}

impl Taxonomy {
    fn from_config(tc: &TaxonomyConfig) -> Self {
        let name = tc.name.trim().to_string();
        let is_tags = name == "tags";
        let prefix = tc
            .url_prefix
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.trim_matches('/').is_empty())
            .map(|p| format!("/{}", p.trim_matches('/')))
            .unwrap_or_else(|| format!("/{name}"));
        Self {
            label: tc.label.clone().unwrap_or_else(|| title_case_word(&name)),
            label_explicit: tc.label.is_some(),
            url_prefix: prefix,
            feed: tc.feed.unwrap_or(true),
            paginate: tc.paginate.filter(|n| *n > 0),
            template: tc.template.clone().unwrap_or_else(|| {
                if is_tags {
                    "tags.html"
                } else {
                    "taxonomy.html"
                }
                .to_string()
            }),
            term_template: tc
                .term_template
                .clone()
                .unwrap_or_else(|| if is_tags { "tag.html" } else { "term.html" }.to_string()),
            implicit: false,
            related_weight: if is_tags { 1 } else { 2 },
            name,
        }
    }

    fn implicit_tags() -> Self {
        Self {
            name: "tags".into(),
            label: "Tags".into(),
            label_explicit: false,
            url_prefix: "/tags".into(),
            feed: false,
            paginate: None,
            template: "tags.html".into(),
            term_template: "tag.html".into(),
            implicit: true,
            related_weight: 1,
        }
    }

    /// Display label in `lang`: the configured `label`, else the UI string
    /// named after the taxonomy (`t.tags`, `t.categories`, or a key added in
    /// `data/i18n/<lang>.yaml`), else the title-cased name.
    pub fn label_for_lang(&self, data: &serde_json::Value, lang: &str) -> String {
        if self.label_explicit {
            return self.label.clone();
        }
        super::ui_strings_for_lang(lang, data)
            .get(&self.name)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| self.label.clone())
    }

    pub fn is_tags(&self) -> bool {
        self.name == "tags"
    }

    /// URL of the term index for a language prefix (`""` or `/es`).
    pub fn index_url(&self, lang_prefix: &str) -> String {
        format!("{lang_prefix}{}/", self.url_prefix)
    }

    pub fn term_url(&self, lang_prefix: &str, slug: &str) -> String {
        format!("{lang_prefix}{}/{slug}/", self.url_prefix)
    }
}

/// The site's taxonomies: `tags` first (implicit unless configured), then each
/// configured taxonomy in config order. Entries with an empty or repeated
/// name are skipped.
pub(crate) fn resolve_taxonomies(config: &SiteConfig) -> Vec<Taxonomy> {
    let mut out: Vec<Taxonomy> = Vec::new();
    let tags = config
        .taxonomies
        .iter()
        .find(|t| t.name.trim() == "tags")
        .map(Taxonomy::from_config)
        .unwrap_or_else(Taxonomy::implicit_tags);
    out.push(tags);
    for tc in &config.taxonomies {
        let name = tc.name.trim();
        if name.is_empty() || out.iter().any(|t| t.name == name) {
            continue;
        }
        out.push(Taxonomy::from_config(tc));
    }
    out
}

/// Slug identifying a term in URLs and grouping (case-insensitive).
pub(crate) fn term_slug(term: &str) -> String {
    slug::slugify(term.trim().to_lowercase())
}

/// A term reference on a content page: `page.taxonomies.<name>[]`.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct TermRef {
    pub name: String,
    pub slug: String,
    pub url: String,
}

/// `page.taxonomies`: taxonomy name → the page's terms, in taxonomy order
/// (tags first). Every taxonomy has a key, possibly with an empty list.
pub(crate) fn page_taxonomies(
    taxonomies: &[Taxonomy],
    item: &ContentItem,
    lang_prefix: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for tax in taxonomies {
        let mut seen = std::collections::HashSet::new();
        let refs: Vec<TermRef> = item
            .frontmatter
            .taxonomy_terms(&tax.name)
            .into_iter()
            .filter_map(|name| {
                let slug = term_slug(&name);
                if slug.is_empty() || !seen.insert(slug.clone()) {
                    return None;
                }
                Some(TermRef {
                    url: tax.term_url(lang_prefix, &slug),
                    name,
                    slug,
                })
            })
            .collect();
        map.insert(
            tax.name.clone(),
            serde_json::to_value(refs).unwrap_or_default(),
        );
    }
    map
}

/// One term of a taxonomy in one language, with the items carrying it.
struct TermGroup<'a> {
    slug: String,
    name: String,
    items: Vec<&'a ContentItem>,
}

/// Group the items of one language by term. Items keep their input order;
/// terms sort by display name (then slug). Tags display lowercased (their
/// historical behavior); other taxonomies display the most common spelling.
fn group_terms<'a>(tax: &Taxonomy, items: &[&'a ContentItem]) -> Vec<TermGroup<'a>> {
    // slug → (spelling → count, items)
    let mut groups: BTreeMap<String, (BTreeMap<String, usize>, Vec<&'a ContentItem>)> =
        BTreeMap::new();
    for item in items {
        let mut seen = std::collections::HashSet::new();
        for term in item.frontmatter.taxonomy_terms(&tax.name) {
            let display = if tax.is_tags() {
                term.to_lowercase()
            } else {
                term.trim().to_string()
            };
            let slug = term_slug(&display);
            if slug.is_empty() || !seen.insert(slug.clone()) {
                continue;
            }
            let entry = groups.entry(slug).or_default();
            *entry.0.entry(display).or_default() += 1;
            entry.1.push(item);
        }
    }
    let mut out: Vec<TermGroup<'a>> = groups
        .into_iter()
        .map(|(slug, (spellings, items))| {
            // Most common spelling; ties → lexicographically smallest (BTreeMap order).
            let name = spellings
                .iter()
                .fold(None::<(&String, usize)>, |best, (s, n)| match best {
                    Some((_, bn)) if bn >= *n => best,
                    _ => Some((s, *n)),
                })
                .map(|(s, _)| s.clone())
                .unwrap_or_default();
            TermGroup { slug, name, items }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.slug.cmp(&b.slug)));
    out
}

#[derive(Serialize)]
struct TermEntry {
    name: String,
    slug: String,
    url: String,
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    feed_url: Option<String>,
}

#[derive(Serialize)]
struct TaxonomyCtx<'a> {
    name: &'a str,
    label: &'a str,
    url: String,
}

/// Everything the page-generation steps need from `build_site_inner`.
pub(crate) struct RenderEnv<'a> {
    pub config: &'a SiteConfig,
    pub tera: &'a tera::Tera,
    pub data: &'a serde_json::Value,
    pub paths: &'a ResolvedPaths,
    pub default_lang: &'a str,
}

impl RenderEnv<'_> {
    pub(crate) fn lang_prefix(&self, lang: &str) -> String {
        super::lang_prefix_for(lang, self.default_lang)
    }

    /// A context with the variables every generated listing page gets.
    pub(crate) fn base_context(&self, lang: &str) -> tera::Context {
        let mut ctx = tera::Context::new();
        ctx.insert("site", &SiteContext::for_lang(self.config, lang));
        ctx.insert("data", self.data);
        ctx.insert("lang", lang);
        insert_i18n_context(&mut ctx, lang, self.default_lang, self.data);
        insert_build_flags(&mut ctx, self.config);
        ctx.insert("translations", &Vec::<TranslationLink>::new());
        ctx
    }

    /// Render `template` and write it to `<output>/<url>index.html`.
    pub(crate) fn write_page(&self, template: &str, ctx: &tera::Context, url: &str) -> Result<()> {
        let html = self
            .tera
            .render(template, ctx)
            .map_err(|e| render_failure(&e, template, None, self.paths, ctx))?;
        let dir = self.paths.output.join(url.trim_matches('/'));
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("index.html"), html)?;
        Ok(())
    }
}

/// A listing page's `page` value (no content, no markdown copy).
pub(super) fn listing_page(title: String, slug: String, url: String) -> PageContext {
    PageContext {
        title,
        content: String::new(),
        date: None,
        updated: None,
        description: None,
        image: None,
        slug,
        tags: Vec::new(),
        url,
        markdown_url: None,
        collection: String::new(),
        robots: None,
        word_count: 0,
        reading_time: 0,
        excerpt: String::new(),
        toc: Vec::new(),
        extra: indexmap::IndexMap::new(),
        og_image: None,
        collection_url: None,
    }
}

pub(super) fn item_summary(item: &ContentItem) -> ItemSummary {
    ItemSummary {
        title: item.frontmatter.title.clone(),
        date: item.frontmatter.date.map(|d| d.to_string()),
        description: item.frontmatter.description.clone(),
        slug: item.slug.clone(),
        tags: item.frontmatter.tags.clone(),
        url: item.url.clone(),
        word_count: item.word_count,
        reading_time: item.reading_time,
        excerpt: item.excerpt_html.clone(),
    }
}

/// Items of public collections in `lang`, in config-collection order.
fn public_items_for_lang<'a>(
    config: &SiteConfig,
    all_collections: &'a indexmap::IndexMap<String, Vec<ContentItem>>,
    lang: &str,
) -> Vec<&'a ContentItem> {
    config
        .collections
        .iter()
        .filter(|c| !c.private)
        .filter_map(|c| all_collections.get(&c.name))
        .flat_map(|items| items.iter().filter(|i| i.lang == lang))
        .collect()
}

/// Generate every taxonomy's index, term pages and term feeds for every
/// language. Returns the URLs of the generated HTML pages (for the sitemap).
/// A taxonomy whose templates are missing is skipped, as tags always were.
pub(crate) fn render_taxonomy_pages(
    env: &RenderEnv<'_>,
    taxonomies: &[Taxonomy],
    all_collections: &indexmap::IndexMap<String, Vec<ContentItem>>,
) -> Result<Vec<String>> {
    let mut urls = Vec::new();
    for tax in taxonomies {
        if env.tera.get_template(&tax.template).is_err()
            || env.tera.get_template(&tax.term_template).is_err()
        {
            continue;
        }
        for lang in &env.config.all_languages() {
            let items = public_items_for_lang(env.config, all_collections, lang);
            let groups = group_terms(tax, &items);
            if groups.is_empty() {
                continue;
            }
            urls.extend(render_one(env, tax, lang, &groups)?);
        }
    }
    Ok(urls)
}

fn render_one(
    env: &RenderEnv<'_>,
    tax: &Taxonomy,
    lang: &str,
    groups: &[TermGroup<'_>],
) -> Result<Vec<String>> {
    let mut urls = Vec::new();
    let lang_prefix = env.lang_prefix(lang);
    let index_url = tax.index_url(&lang_prefix);
    let feed_collections: std::collections::HashSet<&str> = env
        .config
        .collections
        .iter()
        .filter(|c| c.has_rss)
        .map(|c| c.name.as_str())
        .collect();

    let label = tax.label_for_lang(env.data, lang);

    // Feeds first, so term entries can say which terms have one.
    let mut feed_urls: HashMap<&str, String> = HashMap::new();
    if tax.feed {
        for group in groups {
            let feed_items: Vec<&ContentItem> = group
                .items
                .iter()
                .copied()
                .filter(|i| feed_collections.contains(i.collection.as_str()))
                .collect();
            if feed_items.is_empty() {
                continue;
            }
            let term_url = tax.term_url(&lang_prefix, &group.slug);
            write_term_feeds(env, &label, lang, &group.name, &term_url, &feed_items)?;
            feed_urls.insert(group.slug.as_str(), format!("{term_url}feed.xml"));
        }
    }

    let entries: Vec<TermEntry> = groups
        .iter()
        .map(|g| TermEntry {
            name: g.name.clone(),
            slug: g.slug.clone(),
            url: tax.term_url(&lang_prefix, &g.slug),
            count: g.items.len(),
            feed_url: feed_urls.get(g.slug.as_str()).cloned(),
        })
        .collect();
    let tax_ctx = TaxonomyCtx {
        name: &tax.name,
        label: &label,
        url: index_url.clone(),
    };

    // Term index.
    let mut ctx = env.base_context(lang);
    ctx.insert("taxonomy", &tax_ctx);
    ctx.insert("terms", &entries);
    ctx.insert("tags", &entries);
    let (title, slug) = if tax.implicit {
        ("Tags".to_string(), "tags".to_string())
    } else {
        (label.clone(), tax.url_prefix.trim_matches('/').to_string())
    };
    ctx.insert("page", &listing_page(title, slug, index_url.clone()));
    env.write_page(&tax.template, &ctx, &index_url)?;
    urls.push(index_url.clone());

    // Term pages.
    for (group, entry) in groups.iter().zip(&entries) {
        let summaries: Vec<ItemSummary> = group.items.iter().map(|i| item_summary(i)).collect();
        let page_size = tax.paginate.unwrap_or(summaries.len().max(1));
        let chunks: Vec<&[ItemSummary]> = summaries.chunks(page_size).collect();
        let total_pages = chunks.len();
        let page_url = |n: usize| {
            if n == 1 {
                entry.url.clone()
            } else {
                format!("{}page/{n}/", entry.url)
            }
        };
        for (i, chunk) in chunks.iter().enumerate() {
            let n = i + 1;
            let url = page_url(n);
            let mut ctx = env.base_context(lang);
            ctx.insert("taxonomy", &tax_ctx);
            ctx.insert("term", entry);
            ctx.insert("terms", &entries);
            ctx.insert("items", chunk);
            ctx.insert("tag_name", &group.name);
            ctx.insert("tags_url", &index_url);
            if tax.paginate.is_some() {
                ctx.insert(
                    "pagination",
                    &PaginationContext {
                        current_page: n,
                        total_pages,
                        prev_url: (n > 1).then(|| page_url(n - 1)),
                        next_url: (n < total_pages).then(|| page_url(n + 1)),
                        base_url: entry.url.trim_end_matches('/').to_string(),
                    },
                );
            }
            let title = if tax.implicit {
                format!("Tag: {}", group.name)
            } else {
                format!("{label}: {}", group.name)
            };
            let slug = format!("{}/{}", tax.url_prefix.trim_matches('/'), group.slug);
            ctx.insert("page", &listing_page(title, slug, url.clone()));
            env.write_page(&tax.term_template, &ctx, &url)?;
            urls.push(url);
        }
    }
    Ok(urls)
}

fn write_term_feeds(
    env: &RenderEnv<'_>,
    label: &str,
    lang: &str,
    term_name: &str,
    term_url: &str,
    items: &[&ContentItem],
) -> Result<()> {
    let mut feed_config = env.config.clone();
    feed_config.site.title = format!(
        "{} — {}: {}",
        env.config.title_for_lang(lang),
        label,
        term_name
    );
    feed_config.site.language = lang.to_string();
    let base = env.config.site.base_url.trim_end_matches('/');
    // The term page is the feeds' channel link, Atom id and alternate link,
    // so every term feed has its own identity.
    let page_url = format!("{base}{term_url}");
    let rss = super::feed::generate_rss_channel(&feed_config, items, &page_url)?;
    let atom = super::feed::generate_atom_with(
        &feed_config,
        items,
        &format!("{base}{term_url}atom.xml"),
        &page_url,
        &page_url,
    )?;
    let dir = env.paths.output.join(term_url.trim_matches('/'));
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("feed.xml"), rss)?;
    fs::write(dir.join("atom.xml"), atom)?;
    Ok(())
}

fn title_case_word(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

/// Frontmatter fields seite reads itself: a taxonomy with one of these names
/// can never receive terms (`tags` is the built-in taxonomy, so it is fine).
const RESERVED_NAMES: &[&str] = &[
    "title",
    "date",
    "updated",
    "description",
    "image",
    "slug",
    "draft",
    "template",
    "robots",
    "weight",
    "aliases",
    "extra",
    "series",
    "series_order",
];

/// Problems with `[[taxonomies]]`: `taxonomy-url-collision` errors (a URL
/// prefix that equals a collection's, another taxonomy's, `/series`, or a
/// language code, so its pages would overwrite others) and
/// `taxonomy-reserved-name` warnings. `source` (the text of `seite.toml`), when
/// given, locates each problem at its `[[taxonomies]]` line.
pub(crate) fn config_diagnostics(
    config: &SiteConfig,
    source: Option<&str>,
) -> Vec<crate::diagnostics::Diagnostic> {
    use crate::diagnostics::Diagnostic;
    let table_lines: Vec<usize> = source
        .map(|src| {
            src.lines()
                .enumerate()
                .filter(|(_, l)| l.trim() == "[[taxonomies]]")
                .map(|(i, _)| i + 1)
                .collect()
        })
        .unwrap_or_default();
    let locate = |d: Diagnostic, idx: usize| {
        let d = d.with_file("seite.toml");
        match table_lines.get(idx) {
            Some(&line) => d.with_line(line),
            None => d,
        }
    };
    let norm = |p: &str| p.trim().trim_matches('/').to_ascii_lowercase();

    let resolved = resolve_taxonomies(config);
    let mut out = Vec::new();
    for (idx, tc) in config.taxonomies.iter().enumerate() {
        let name = tc.name.trim();
        if RESERVED_NAMES.contains(&name) {
            out.push(locate(
                Diagnostic::warning(
                    "taxonomy-reserved-name",
                    format!(
                        "taxonomy `{name}` is named after a built-in frontmatter field, so no page can assign terms to it"
                    ),
                )
                .with_hint("rename the taxonomy (e.g. `categories`, `topics`)"),
                idx,
            ));
        }
        let Some(tax) = resolved.iter().find(|t| t.name == name) else {
            continue;
        };
        let prefix = norm(&tax.url_prefix);
        let mut clash: Option<String> = None;
        if let Some(c) = config
            .collections
            .iter()
            .filter(|c| c.subdomain.is_none())
            .find(|c| norm(&c.url_prefix) == prefix)
        {
            clash = Some(format!("collection `{}`", c.name));
        } else if let Some(other) = resolved
            .iter()
            .find(|t| t.name != tax.name && norm(&t.url_prefix) == prefix)
        {
            clash = Some(format!("taxonomy `{}`", other.name));
        } else if prefix == norm(super::series::SERIES_PREFIX) {
            clash = Some("the series index pages".to_string());
        } else if config.languages.contains_key(prefix.as_str()) {
            clash = Some(format!("the `{prefix}` language's pages"));
        }
        if let Some(what) = clash {
            out.push(locate(
                Diagnostic::error(
                    "taxonomy-url-collision",
                    format!(
                        "taxonomy `{}` uses URL prefix `/{prefix}`, already used by {what}; their pages would overwrite each other",
                        tax.name
                    ),
                )
                .with_hint("set a different `url_prefix` on the taxonomy"),
                idx,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Frontmatter;

    fn item(title: &str, fm_extra: &str) -> ContentItem {
        let yaml = format!("title: {title}\n{fm_extra}");
        let fm: Frontmatter = serde_yaml_ng::from_str(&yaml).unwrap();
        ContentItem {
            frontmatter: fm,
            raw_body: String::new(),
            html_body: String::new(),
            source_path: format!("content/posts/{title}.md").into(),
            slug: title.to_lowercase(),
            collection: "posts".into(),
            url: format!("/posts/{}", title.to_lowercase()),
            lang: "en".into(),
            excerpt: String::new(),
            toc: Vec::new(),
            word_count: 0,
            reading_time: 0,
            excerpt_html: String::new(),
        }
    }

    fn config(toml_extra: &str) -> SiteConfig {
        toml::from_str(&format!(
            "[site]\ntitle = \"T\"\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\ndefault_template = \"post.html\"\n{toml_extra}"
        ))
        .unwrap()
    }

    #[test]
    fn test_resolve_taxonomies_implicit_tags_only() {
        let taxes = resolve_taxonomies(&config(""));
        assert_eq!(taxes.len(), 1);
        assert!(taxes[0].implicit);
        assert!(!taxes[0].feed);
        assert_eq!(taxes[0].url_prefix, "/tags");
        assert_eq!(taxes[0].template, "tags.html");
    }

    #[test]
    fn test_resolve_taxonomies_configured() {
        let taxes = resolve_taxonomies(&config(
            "[[taxonomies]]\nname = \"categories\"\n[[taxonomies]]\nname = \"tags\"\npaginate = 5\n[[taxonomies]]\nname = \"topics\"\nurl_prefix = \"t/\"\nlabel = \"Subjects\"\nfeed = false\n[[taxonomies]]\nname = \"categories\"\n",
        ));
        let names: Vec<&str> = taxes.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["tags", "categories", "topics"]);
        assert!(!taxes[0].implicit && taxes[0].feed);
        assert_eq!(taxes[0].paginate, Some(5));
        assert_eq!(taxes[0].template, "tags.html");
        assert_eq!(taxes[1].label, "Categories");
        assert_eq!(taxes[1].template, "taxonomy.html");
        assert_eq!(taxes[1].term_template, "term.html");
        assert_eq!(taxes[1].related_weight, 2);
        assert_eq!(taxes[2].url_prefix, "/t");
        assert_eq!(taxes[2].label, "Subjects");
        assert!(!taxes[2].feed);
    }

    #[test]
    fn test_page_taxonomies_terms_and_urls() {
        let taxes = resolve_taxonomies(&config("[[taxonomies]]\nname = \"categories\"\n"));
        let it = item(
            "A",
            "tags: [Rust, rust, Web]\ncategories: Programming Languages\n",
        );
        let map = page_taxonomies(&taxes, &it, "/es");
        let keys: Vec<&String> = map.keys().collect();
        assert_eq!(keys, ["tags", "categories"]);
        let tags = map["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2, "duplicate slug dropped");
        assert_eq!(tags[0]["name"], "Rust");
        assert_eq!(tags[0]["url"], "/es/tags/rust/");
        assert_eq!(
            map["categories"][0]["url"],
            "/es/categories/programming-languages/"
        );
    }

    #[test]
    fn test_group_terms_spelling_and_order() {
        let taxes = resolve_taxonomies(&config("[[taxonomies]]\nname = \"categories\"\n"));
        let a = item("A", "categories: [Web Dev, zeta]\ntags: [Zed]\n");
        let b = item("B", "categories: [web dev]\n");
        let c = item("C", "categories: [Web Dev, '']\n");
        let items = vec![&a, &b, &c];
        let cats = group_terms(&taxes[1], &items);
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[0].name, "Web Dev");
        assert_eq!(cats[0].items.len(), 3);
        assert_eq!(cats[1].name, "zeta");
        let tags = group_terms(&taxes[0], &items);
        assert_eq!(tags[0].name, "zed", "tags display lowercased");
    }

    #[test]
    fn test_config_diagnostics_collisions_and_reserved_names() {
        let mut cfg = config(
            "[languages.es]\n[[taxonomies]]\nname = \"categories\"\nurl_prefix = \"/posts/\"\n[[taxonomies]]\nname = \"topics\"\nurl_prefix = \"tags\"\n[[taxonomies]]\nname = \"extra\"\nurl_prefix = \"/series\"\n[[taxonomies]]\nname = \"es\"\n[[taxonomies]]\nname = \"fine\"\n",
        );
        cfg.collections[0].url_prefix = "/posts".into();
        let source = "[[taxonomies]]\n[[taxonomies]]\n";
        let diags = config_diagnostics(&cfg, Some(source));
        let codes: Vec<(&str, String)> =
            diags.iter().map(|d| (d.code, d.message.clone())).collect();
        assert_eq!(diags.len(), 5, "{codes:?}");
        assert!(codes[0].1.contains("collection `posts`"), "{codes:?}");
        assert_eq!(diags[0].line, Some(1));
        assert!(codes[1].1.contains("taxonomy `tags`"), "{codes:?}");
        assert_eq!(codes[2].0, "taxonomy-reserved-name");
        assert!(codes[3].1.contains("series"), "{codes:?}");
        assert!(codes[4].1.contains("language"), "{codes:?}");
        assert!(config_diagnostics(&config(""), None).is_empty());
    }
}
