pub mod discovery;
pub mod feed;
pub mod images;
pub mod links;
pub mod markdown;
pub mod sitemap;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use walkdir::WalkDir;

use crate::config::{CollectionConfig, ResolvedPaths, SiteConfig};
use crate::content::{self, ContentItem, Frontmatter};
use crate::error::{PageError, Result};
use crate::output::CommandOutput;
use crate::templates;

pub struct BuildOptions {
    pub include_drafts: bool,
}

pub struct BuildResult {
    pub collections: HashMap<String, Vec<ContentItem>>,
    pub stats: BuildStats,
}

#[derive(Debug, Serialize)]
pub struct BuildStats {
    pub items_built: HashMap<String, usize>,
    pub static_files_copied: usize,
    pub data_files_loaded: usize,
    pub duration_ms: u64,
    /// Per-step timing: (step_name, duration_ms)
    pub step_timings: Vec<(String, f64)>,
}

impl CommandOutput for BuildStats {
    fn human_display(&self) -> String {
        let parts: Vec<String> = self
            .items_built
            .iter()
            .map(|(name, count)| format!("{count} {name}"))
            .collect();
        let mut out = format!(
            "Built {} in {:.1}s ({} static files copied",
            parts.join(", "),
            self.duration_ms as f64 / 1000.0,
            self.static_files_copied
        );
        if self.data_files_loaded > 0 {
            out.push_str(&format!(", {} data files loaded", self.data_files_loaded));
        }
        out.push(')');
        if !self.step_timings.is_empty() {
            out.push_str("\n  Timings:");
            for (name, ms) in &self.step_timings {
                if *ms >= 1.0 {
                    out.push_str(&format!("\n    {name}: {ms:.1}ms"));
                } else {
                    out.push_str(&format!("\n    {name}: <1ms"));
                }
            }
        }
        out
    }
}

#[derive(Serialize)]
struct SiteContext {
    title: String,
    description: String,
    base_url: String,
    language: String,
    author: String,
}

#[derive(Serialize)]
struct PageContext {
    title: String,
    content: String,
    date: Option<String>,
    /// Last-modified date string (ISO 8601). Populated from `updated` frontmatter field.
    updated: Option<String>,
    description: Option<String>,
    /// Absolute URL or path to social-preview image (og:image / twitter:image).
    image: Option<String>,
    slug: String,
    tags: Vec<String>,
    url: String,
    /// Collection name ("posts", "docs", "pages", etc.).
    collection: String,
    /// Per-page robots meta value, e.g. "noindex".
    robots: Option<String>,
    /// Word count of the raw markdown body.
    word_count: usize,
    /// Estimated reading time in minutes (based on 238 WPM average).
    reading_time: usize,
    /// Auto-extracted excerpt rendered as HTML.
    excerpt: String,
    /// Table of contents entries extracted from heading hierarchy.
    toc: Vec<markdown::TocEntry>,
    /// Arbitrary key-value data from frontmatter `extra` field.
    extra: std::collections::HashMap<String, serde_yaml_ng::Value>,
}

#[derive(Serialize)]
struct CollectionContext {
    name: String,
    label: String,
    items: Vec<ItemSummary>,
}

#[derive(Serialize, Clone)]
struct ItemSummary {
    title: String,
    date: Option<String>,
    description: Option<String>,
    slug: String,
    tags: Vec<String>,
    url: String,
    /// Word count of the raw markdown body.
    word_count: usize,
    /// Estimated reading time in minutes.
    reading_time: usize,
    /// Auto-extracted excerpt rendered as HTML.
    excerpt: String,
}

#[derive(Serialize, Clone)]
struct NavItem {
    title: String,
    url: String,
    active: bool,
}

#[derive(Serialize, Clone)]
struct NavSection {
    name: String,
    label: String,
    items: Vec<NavItem>,
}

#[derive(Serialize)]
struct PaginationContext {
    current_page: usize,
    total_pages: usize,
    prev_url: Option<String>,
    next_url: Option<String>,
    base_url: String,
}

/// A translation link used in templates and the sitemap's xhtml:link alternates.
#[derive(Serialize, Clone)]
pub(crate) struct TranslationLink {
    pub lang: String,
    pub url: String,
}

#[derive(Serialize)]
struct SearchEntry<'a> {
    title: &'a str,
    description: Option<&'a str>,
    url: &'a str,
    collection: &'a str,
    tags: &'a [String],
    date: Option<String>,
    lang: &'a str,
}

impl SiteContext {
    fn from_config(config: &SiteConfig) -> Self {
        Self {
            title: config.site.title.clone(),
            description: config.site.description.clone(),
            base_url: config.site.base_url.clone(),
            language: config.site.language.clone(),
            author: config.site.author.clone(),
        }
    }

    fn for_lang(config: &SiteConfig, lang: &str) -> Self {
        Self {
            title: config.title_for_lang(lang).to_string(),
            description: config.description_for_lang(lang).to_string(),
            base_url: config.site.base_url.clone(),
            language: lang.to_string(),
            author: config.site.author.clone(),
        }
    }
}

pub fn build_site(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    opts: &BuildOptions,
) -> Result<BuildResult> {
    let start = Instant::now();
    let mut step_timings: Vec<(String, f64)> = Vec::new();

    // Step 1: Clean output directory
    let step_start = Instant::now();
    if paths.output.exists() {
        fs::remove_dir_all(&paths.output)?;
    }
    fs::create_dir_all(&paths.output)?;
    step_timings.push(("Clean output".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 2: Load templates (collection-aware)
    let step_start = Instant::now();
    let tera = templates::load_templates(&paths.templates, &config.collections)?;
    step_timings.push(("Load templates".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 2b: Load shortcode registry (built-in + user-defined)
    let step_start = Instant::now();
    let shortcodes_dir = paths.templates.join("shortcodes");
    let shortcode_registry = crate::shortcodes::ShortcodeRegistry::new(&shortcodes_dir)?;
    step_timings.push(("Load shortcodes".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 2.5: Load data files
    let step_start = Instant::now();
    let data = crate::data::load_data_dir(&paths.data_dir)?;
    step_timings.push(("Load data files".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Pre-compute configured language codes for filename detection
    let configured_langs = config.configured_lang_codes();
    let is_multilingual = config.is_multilingual();
    let default_lang = &config.site.language;

    // Step 3: Process each collection
    let step_start = Instant::now();
    let mut all_collections: HashMap<String, Vec<ContentItem>> = HashMap::new();

    for collection in &config.collections {
        let collection_dir = paths.content.join(&collection.directory);
        let mut items = Vec::new();

        if !collection_dir.exists() {
            tracing::warn!(
                "Content directory '{}' for collection '{}' does not exist",
                collection_dir.display(),
                collection.name
            );
        }
        if collection_dir.exists() {
            for entry in WalkDir::new(&collection_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            {
                let path = entry.path();
                let rel = path.strip_prefix(&collection_dir).unwrap_or(path);

                let (fm, raw_body) = content::parse_content_file(path)?;

                if fm.draft && !opts.include_drafts {
                    continue;
                }

                // Detect language from filename (only in multilingual mode)
                let file_lang = if is_multilingual {
                    content::extract_lang_from_filename(path, &configured_langs)
                } else {
                    None
                };
                let lang = file_lang.as_deref().unwrap_or(default_lang).to_string();

                // Resolve slug — strip lang suffix from stem for translations
                let slug = if file_lang.is_some() {
                    resolve_slug_i18n(&fm, rel, collection, &configured_langs)
                } else {
                    resolve_slug(&fm, rel, collection)
                };

                let mut fm = fm;
                if fm.date.is_none() && collection.has_date {
                    fm.date = parse_date_from_filename(path);
                }

                // Expand shortcodes before markdown rendering
                let sc_page = serde_json::json!({
                    "title": fm.title,
                    "slug": &slug,
                    "collection": &collection.name,
                    "tags": &fm.tags,
                });
                let sc_site = serde_json::json!({
                    "title": &config.site.title,
                    "base_url": &config.site.base_url,
                    "language": &config.site.language,
                });
                let expanded_body =
                    shortcode_registry.expand(&raw_body, path, &sc_page, &sc_site)?;
                let (html_body, toc) = markdown::markdown_to_html(&expanded_body);

                // Build URL: non-default languages get /{lang} prefix
                let base_url = build_url(&collection.url_prefix, &slug);
                let url = if lang != *default_lang {
                    format!("/{lang}{base_url}")
                } else {
                    base_url
                };

                let excerpt = content::extract_excerpt(&expanded_body);
                let (excerpt_html, _) = markdown::markdown_to_html(&excerpt);
                let word_count = raw_body.split_whitespace().count();
                let reading_time = if word_count == 0 { 0 } else { (word_count / 238).max(1) };
                items.push(ContentItem {
                    frontmatter: fm,
                    raw_body,
                    html_body,
                    source_path: path.to_path_buf(),
                    slug,
                    collection: collection.name.clone(),
                    url,
                    lang,
                    excerpt,
                    toc,
                    word_count,
                    reading_time,
                    excerpt_html,
                });
            }
        }

        // Sort: date-based collections by date desc, others by title
        if collection.has_date {
            items.sort_by(|a, b| b.frontmatter.date.cmp(&a.frontmatter.date));
        } else {
            items.sort_by(|a, b| a.frontmatter.title.cmp(&b.frontmatter.title));
        }

        all_collections.insert(collection.name.clone(), items);
    }

    // Detect URL collisions: if two content items resolve to the same URL, that's an error.
    {
        let mut url_map: HashMap<&str, &std::path::Path> = HashMap::new();
        for items in all_collections.values() {
            for item in items {
                if let Some(existing) = url_map.insert(&item.url, &item.source_path) {
                    return Err(PageError::Build(format!(
                        "URL collision: '{}' is claimed by both '{}' and '{}'",
                        item.url,
                        existing.display(),
                        item.source_path.display()
                    )));
                }
            }
        }
    }

    step_timings.push(("Process collections".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Build translation map: (collection, slug) → Vec<TranslationLink>
    let translation_map: HashMap<(String, String), Vec<TranslationLink>> = if is_multilingual {
        let mut map: HashMap<(String, String), Vec<TranslationLink>> = HashMap::new();
        for (coll_name, items) in &all_collections {
            for item in items {
                let key = (coll_name.clone(), item.slug.clone());
                map.entry(key).or_default().push(TranslationLink {
                    lang: item.lang.clone(),
                    url: item.url.clone(),
                });
            }
        }
        map
    } else {
        HashMap::new()
    };

    // Render each item in each collection
    let step_start = Instant::now();

    // Pre-compute SiteContext per language (avoid re-creating per item)
    let mut site_ctx_cache: HashMap<String, SiteContext> = HashMap::new();
    site_ctx_cache.insert(default_lang.clone(), SiteContext::from_config(config));
    for lang in config.all_languages() {
        if lang != *default_lang {
            site_ctx_cache.insert(lang.clone(), SiteContext::for_lang(config, &lang));
        }
    }

    // Pre-serialize an empty nav for non-nested collections
    let empty_nav: Vec<NavSection> = Vec::new();
    let empty_nav_value = serde_json::to_value(&empty_nav).unwrap_or_default();

    for collection in &config.collections {
        if let Some(items) = all_collections.get(&collection.name) {
            // Only build nav for nested collections (e.g., docs with sidebar).
            // Non-nested collections (posts, pages) get an empty nav — their templates
            // don't use it, and building/cloning a 10k-item nav per page is O(n²).
            let nav_by_lang: HashMap<&str, serde_json::Value>;
            let nav_slug_index: HashMap<&str, HashMap<&str, (usize, usize)>>;

            if collection.nested {
                // Group items by language
                let mut items_by_lang: HashMap<&str, Vec<&ContentItem>> = HashMap::new();
                for item in items {
                    items_by_lang.entry(item.lang.as_str()).or_default().push(item);
                }

                let mut by_lang = HashMap::new();
                let mut slug_idx = HashMap::new();

                for (&lang, lang_items) in &items_by_lang {
                    let mut sections: Vec<NavSection> = Vec::new();
                    let mut section_map: HashMap<String, usize> = HashMap::new();
                    let mut si: HashMap<&str, (usize, usize)> = HashMap::new();

                    for item in lang_items {
                        let (section_name, section_label) = if let Some(pos) = item.slug.find('/') {
                            let name = &item.slug[..pos];
                            let label = title_case(name);
                            (name.to_string(), label)
                        } else {
                            (String::new(), String::new())
                        };

                        let nav_item = NavItem {
                            title: item.frontmatter.title.clone(),
                            url: item.url.clone(),
                            active: false,
                        };

                        let section_idx = if let Some(&idx) = section_map.get(&section_name) {
                            sections[idx].items.push(nav_item);
                            idx
                        } else {
                            let idx = sections.len();
                            section_map.insert(section_name.clone(), idx);
                            sections.push(NavSection {
                                name: section_name,
                                label: section_label,
                                items: vec![nav_item],
                            });
                            idx
                        };
                        let item_idx = sections[section_idx].items.len() - 1;
                        si.insert(&item.slug, (section_idx, item_idx));
                    }

                    // Pre-serialize to serde_json::Value so per-item clone is cheap
                    by_lang.insert(lang, serde_json::to_value(&sections).unwrap_or_default());
                    slug_idx.insert(lang, si);
                }

                nav_by_lang = by_lang;
                nav_slug_index = slug_idx;
            } else {
                nav_by_lang = HashMap::new();
                nav_slug_index = HashMap::new();
            }

            for item in items {
                let site_ctx_for_item = site_ctx_cache
                    .get(item.lang.as_str())
                    .unwrap_or_else(|| site_ctx_cache.get(default_lang.as_str()).unwrap());

                let mut ctx = build_page_context(site_ctx_for_item, item, &data);

                // Insert nav: pre-serialized Value for nested collections, empty for others
                if collection.nested {
                    if let Some(base_nav) = nav_by_lang.get(item.lang.as_str()) {
                        let mut nav = base_nav.clone();
                        // Set the active flag on the matching nav item
                        if let Some(si) = nav_slug_index.get(item.lang.as_str()) {
                            if let Some(&(sec_idx, item_idx)) = si.get(item.slug.as_str()) {
                                if let Some(sections) = nav.as_array_mut() {
                                    if let Some(section) = sections.get_mut(sec_idx) {
                                        if let Some(items_arr) = section.get_mut("items").and_then(|i| i.as_array_mut()) {
                                            if let Some(nav_item) = items_arr.get_mut(item_idx) {
                                                nav_item["active"] = serde_json::Value::Bool(true);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        ctx.insert("nav", &nav);
                    }
                } else {
                    ctx.insert("nav", &empty_nav_value);
                }
                ctx.insert("lang", &item.lang);

                // Always provide translations (may be empty vec)
                let empty_translations: Vec<TranslationLink> = Vec::new();
                let translations = translation_map
                    .get(&(collection.name.clone(), item.slug.clone()))
                    .filter(|t| t.len() > 1)
                    .map(|t| t.as_slice())
                    .unwrap_or(&empty_translations);
                ctx.insert("translations", &translations);

                let template_name = item
                    .frontmatter
                    .template
                    .as_deref()
                    .unwrap_or(&collection.default_template);
                let html = tera
                    .render(template_name, &ctx)
                    .map_err(|e| PageError::Build(format!("rendering '{}': {e}", item.slug)))?;

                let output_path = url_to_output_path(&paths.output, &item.url);
                if let Some(parent) = output_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&output_path, html)?;
            }
        }
    }

    step_timings.push(("Render pages".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Extract homepage pages (content/pages/index.md and translations) so they
    // don't also render as standalone pages at /index (which would collide).
    let homepage_pages: Vec<ContentItem> = all_collections
        .get_mut("pages")
        .map(|pages| {
            let mut homepages = Vec::new();
            let mut i = 0;
            while i < pages.len() {
                if pages[i].slug == "index" {
                    homepages.push(pages.remove(i));
                } else {
                    i += 1;
                }
            }
            homepages
        })
        .unwrap_or_default();

    // Step 4: Render index page(s)
    let step_start = Instant::now();
    for lang in &config.all_languages() {
        let lang_site_ctx = SiteContext::for_lang(config, lang);
        let mut index_ctx = tera::Context::new();
        index_ctx.insert("site", &lang_site_ctx);
        index_ctx.insert("data", &data);
        index_ctx.insert("lang", lang);

        // Filter collections for this language
        let collections_ctx: Vec<CollectionContext> = config
            .collections
            .iter()
            .filter(|c| c.listed)
            .map(|c| {
                let items = all_collections
                    .get(&c.name)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|item| item.lang == *lang)
                    .collect::<Vec<_>>();
                CollectionContext {
                    name: c.name.clone(),
                    label: c.label.clone(),
                    items: items
                        .iter()
                        .map(|item| ItemSummary {
                            title: item.frontmatter.title.clone(),
                            date: item.frontmatter.date.map(|d| d.to_string()),
                            description: item.frontmatter.description.clone(),
                            slug: item.slug.clone(),
                            tags: item.frontmatter.tags.clone(),
                            url: item.url.clone(),
                            word_count: item.word_count,
                            reading_time: item.reading_time,
                            excerpt: item.excerpt_html.clone(),
                        })
                        .collect(),
                }
            })
            .collect();
        index_ctx.insert("collections", &collections_ctx);

        // Always insert a `page` context so templates can unconditionally access
        // page.url, page.collection, etc. for SEO/GEO meta tags.
        // When no homepage page exists, `page.content` is empty so
        // `{% if page.content %}` gates the homepage content block correctly.
        let index_page_url = if *lang == *default_lang {
            "/".to_string()
        } else {
            format!("/{lang}/")
        };
        let index_page_ctx =
            if let Some(homepage) = homepage_pages.iter().find(|p| p.lang == *lang) {
                PageContext {
                    title: homepage.frontmatter.title.clone(),
                    content: homepage.html_body.clone(),
                    date: homepage.frontmatter.date.map(|d| d.to_string()),
                    updated: homepage.frontmatter.updated.map(|d| d.to_string()),
                    description: homepage.frontmatter.description.clone(),
                    image: homepage.frontmatter.image.clone(),
                    slug: homepage.slug.clone(),
                    tags: homepage.frontmatter.tags.clone(),
                    url: index_page_url,
                    collection: homepage.collection.clone(),
                    robots: homepage.frontmatter.robots.clone(),
                    word_count: homepage.word_count,
                    reading_time: homepage.reading_time,
                    excerpt: homepage.excerpt_html.clone(),
                    toc: homepage.toc.clone(),
                    extra: homepage.frontmatter.extra.clone(),
                }
            } else {
                PageContext {
                    title: String::new(),
                    content: String::new(),
                    date: None,
                    updated: None,
                    description: None,
                    image: None,
                    slug: "index".to_string(),
                    tags: Vec::new(),
                    url: index_page_url,
                    collection: String::new(),
                    robots: None,
                    word_count: 0,
                    reading_time: 0,
                    excerpt: String::new(),
                    toc: Vec::new(),
                    extra: std::collections::HashMap::new(),
                }
            };
        index_ctx.insert("page", &index_page_ctx);

        // Translation links for the index page (always provide, even if empty)
        let index_translations: Vec<TranslationLink> = if is_multilingual {
            let translations: Vec<TranslationLink> = config
                .all_languages()
                .iter()
                .filter(|l| homepage_pages.iter().any(|p| p.lang == **l) || **l == *default_lang)
                .map(|l| TranslationLink {
                    lang: l.clone(),
                    url: if *l == *default_lang {
                        "/".to_string()
                    } else {
                        format!("/{l}/")
                    },
                })
                .collect();
            if translations.len() > 1 {
                translations
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        index_ctx.insert("translations", &index_translations);

        let index_html = tera
            .render("index.html", &index_ctx)
            .map_err(|e| PageError::Build(format!("rendering index ({lang}): {e}")))?;

        if *lang == *default_lang {
            fs::write(paths.output.join("index.html"), index_html)?;
        } else {
            let lang_dir = paths.output.join(lang);
            fs::create_dir_all(&lang_dir)?;
            fs::write(lang_dir.join("index.html"), index_html)?;
        }
    }

    // Step 4b: Paginated collection index pages
    // For each listed collection with `paginate` set, render per-language paginated indexes.
    // Page 1 → dist/{url_prefix}/index.html  (URL: /{url_prefix}/)
    // Page N → dist/{url_prefix}/page/N/index.html  (URL: /{url_prefix}/page/N/)
    for lang in &config.all_languages() {
        let lang_site_ctx = SiteContext::for_lang(config, lang);
        for c in config.collections.iter().filter(|c| c.listed) {
            let page_size = match c.paginate {
                Some(n) if n > 0 => n,
                _ => continue,
            };
            let url_prefix_trimmed = c.url_prefix.trim_start_matches('/');
            if url_prefix_trimmed.is_empty() {
                continue;
            }

            let all_items: Vec<&ContentItem> = all_collections
                .get(&c.name)
                .map(|v| v.iter().collect())
                .unwrap_or_default();
            let lang_items: Vec<ItemSummary> = all_items
                .iter()
                .filter(|item| item.lang == *lang)
                .map(|item| ItemSummary {
                    title: item.frontmatter.title.clone(),
                    date: item.frontmatter.date.map(|d| d.to_string()),
                    description: item.frontmatter.description.clone(),
                    slug: item.slug.clone(),
                    tags: item.frontmatter.tags.clone(),
                    url: item.url.clone(),
                    word_count: item.word_count,
                    reading_time: item.reading_time,
                    excerpt: item.excerpt_html.clone(),
                })
                .collect();

            let total = lang_items.len();
            if total == 0 {
                continue;
            }
            let total_pages = total.div_ceil(page_size);

            // Base collection URL (language-prefixed for non-default languages)
            let collection_base = if *lang == *default_lang {
                format!("/{url_prefix_trimmed}")
            } else {
                format!("/{lang}/{url_prefix_trimmed}")
            };

            let page_url = |n: usize| -> String {
                if n == 1 {
                    format!("{collection_base}/")
                } else {
                    format!("{collection_base}/page/{n}/")
                }
            };

            for (page_idx, chunk) in lang_items.chunks(page_size).enumerate() {
                let page_num = page_idx + 1;
                let collection_ctx = CollectionContext {
                    name: c.name.clone(),
                    label: c.label.clone(),
                    items: chunk.to_vec(),
                };
                let pagination = PaginationContext {
                    current_page: page_num,
                    total_pages,
                    prev_url: if page_num > 1 { Some(page_url(page_num - 1)) } else { None },
                    next_url: if page_num < total_pages { Some(page_url(page_num + 1)) } else { None },
                    base_url: collection_base.clone(),
                };
                let mut ctx = tera::Context::new();
                ctx.insert("site", &lang_site_ctx);
                ctx.insert("data", &data);
                ctx.insert("lang", lang);
                ctx.insert("collections", &[collection_ctx]);
                ctx.insert("pagination", &pagination);
                ctx.insert("translations", &Vec::<TranslationLink>::new());
                // Always provide a page context so SEO meta tags can access page.url etc.
                ctx.insert("page", &PageContext {
                    title: String::new(),
                    content: String::new(),
                    date: None,
                    updated: None,
                    description: None,
                    image: None,
                    slug: String::new(),
                    tags: Vec::new(),
                    url: page_url(page_num),
                    collection: String::new(),
                    robots: None,
                    word_count: 0,
                    reading_time: 0,
                    excerpt: String::new(),
                    toc: Vec::new(),
                    extra: std::collections::HashMap::new(),
                });
                let html = tera
                    .render("index.html", &ctx)
                    .map_err(|e| PageError::Build(format!("rendering {collection_base} page {page_num}: {e}")))?;

                let out_dir = if *lang == *default_lang {
                    if page_num == 1 {
                        paths.output.join(url_prefix_trimmed)
                    } else {
                        paths.output.join(url_prefix_trimmed).join("page").join(page_num.to_string())
                    }
                } else if page_num == 1 {
                    paths.output.join(lang).join(url_prefix_trimmed)
                } else {
                    paths.output.join(lang).join(url_prefix_trimmed).join("page").join(page_num.to_string())
                };
                fs::create_dir_all(&out_dir)?;
                fs::write(out_dir.join("index.html"), html)?;
            }
        }
    }

    // Write homepage markdown alongside HTML
    for homepage in &homepage_pages {
        let md_content = format!(
            "{}\n\n{}",
            content::generate_frontmatter(&homepage.frontmatter),
            homepage.raw_body
        );
        if homepage.lang == *default_lang {
            fs::write(paths.output.join("index.md"), md_content)?;
        } else {
            let lang_dir = paths.output.join(&homepage.lang);
            fs::create_dir_all(&lang_dir)?;
            fs::write(lang_dir.join("index.md"), md_content)?;
        }
    }

    // Step 4b: Generate 404 page
    if tera.get_template("404.html").is_ok() {
        let mut ctx_404 = tera::Context::new();
        ctx_404.insert("site", &SiteContext::from_config(config));
        ctx_404.insert("data", &data);
        ctx_404.insert("lang", default_lang);
        ctx_404.insert("translations", &Vec::<TranslationLink>::new());
        ctx_404.insert("collections", &Vec::<CollectionContext>::new());
        ctx_404.insert("page", &PageContext {
            title: "Page Not Found".to_string(),
            content: String::new(),
            date: None,
            updated: None,
            description: None,
            image: None,
            slug: "404".to_string(),
            tags: Vec::new(),
            url: "/404".to_string(),
            collection: String::new(),
            robots: Some("noindex".to_string()),
            word_count: 0,
            reading_time: 0,
            excerpt: String::new(),
            toc: Vec::new(),
            extra: std::collections::HashMap::new(),
        });
        let html_404 = tera
            .render("404.html", &ctx_404)
            .map_err(|e| PageError::Build(format!("rendering 404 page: {e}")))?;
        fs::write(paths.output.join("404.html"), html_404)?;
    }

    // Step 4c: Generate tag pages
    // Collect all tags per language from all collections
    let mut tag_page_urls: Vec<String> = Vec::new();
    if tera.get_template("tags.html").is_ok() && tera.get_template("tag.html").is_ok() {
        for lang in &config.all_languages() {
            let lang_prefix = if *lang == *default_lang {
                String::new()
            } else {
                format!("/{lang}")
            };
            let tags_base_url = format!("{lang_prefix}/tags");

            // Gather all tags and their items for this language
            let mut tag_map: HashMap<String, Vec<ItemSummary>> = HashMap::new();
            for c in &config.collections {
                if let Some(items) = all_collections.get(&c.name) {
                    for item in items.iter().filter(|i| i.lang == *lang) {
                        let summary = ItemSummary {
                            title: item.frontmatter.title.clone(),
                            date: item.frontmatter.date.map(|d| d.to_string()),
                            description: item.frontmatter.description.clone(),
                            slug: item.slug.clone(),
                            tags: item.frontmatter.tags.clone(),
                            url: item.url.clone(),
                            word_count: item.word_count,
                            reading_time: item.reading_time,
                            excerpt: item.excerpt_html.clone(),
                        };
                        for tag in &item.frontmatter.tags {
                            let normalized = tag.to_lowercase();
                            tag_map.entry(normalized).or_default().push(summary.clone());
                        }
                    }
                }
            }

            if tag_map.is_empty() {
                continue;
            }

            // Sort tags alphabetically
            let mut sorted_tags: Vec<(String, Vec<ItemSummary>)> =
                tag_map.into_iter().collect();
            sorted_tags.sort_by(|a, b| a.0.cmp(&b.0));

            let lang_site_ctx = SiteContext::for_lang(config, lang);

            // Generate tags index page
            #[derive(Serialize)]
            struct TagIndexEntry {
                name: String,
                url: String,
                count: usize,
            }

            let tag_entries: Vec<TagIndexEntry> = sorted_tags
                .iter()
                .map(|(tag, items)| {
                    let tag_slug = slug::slugify(tag);
                    TagIndexEntry {
                        name: tag.clone(),
                        url: format!("{tags_base_url}/{tag_slug}/"),
                        count: items.len(),
                    }
                })
                .collect();

            let mut tags_ctx = tera::Context::new();
            tags_ctx.insert("site", &lang_site_ctx);
            tags_ctx.insert("data", &data);
            tags_ctx.insert("lang", lang);
            tags_ctx.insert("tags", &tag_entries);
            tags_ctx.insert("translations", &Vec::<TranslationLink>::new());
            tags_ctx.insert("page", &PageContext {
                title: "Tags".to_string(),
                content: String::new(),
                date: None,
                updated: None,
                description: None,
                image: None,
                slug: "tags".to_string(),
                tags: Vec::new(),
                url: format!("{tags_base_url}/"),
                collection: String::new(),
                robots: None,
                word_count: 0,
                reading_time: 0,
                excerpt: String::new(),
                toc: Vec::new(),
                extra: std::collections::HashMap::new(),
            });
            let tags_html = tera
                .render("tags.html", &tags_ctx)
                .map_err(|e| PageError::Build(format!("rendering tags index: {e}")))?;
            let tags_dir = paths.output.join(tags_base_url.trim_start_matches('/'));
            fs::create_dir_all(&tags_dir)?;
            fs::write(tags_dir.join("index.html"), tags_html)?;
            tag_page_urls.push(format!("{tags_base_url}/"));

            // Generate individual tag pages
            for (tag, items) in &sorted_tags {
                let tag_slug = slug::slugify(tag);
                let tag_url = format!("{tags_base_url}/{tag_slug}/");
                let mut tag_ctx = tera::Context::new();
                tag_ctx.insert("site", &lang_site_ctx);
                tag_ctx.insert("data", &data);
                tag_ctx.insert("lang", lang);
                tag_ctx.insert("tag_name", tag);
                tag_ctx.insert("items", items);
                tag_ctx.insert("tags_url", &format!("{tags_base_url}/"));
                tag_ctx.insert("translations", &Vec::<TranslationLink>::new());
                tag_ctx.insert("page", &PageContext {
                    title: format!("Tag: {tag}"),
                    content: String::new(),
                    date: None,
                    updated: None,
                    description: None,
                    image: None,
                    slug: format!("tags/{tag_slug}"),
                    tags: Vec::new(),
                    url: tag_url.clone(),
                    collection: String::new(),
                    robots: None,
                    word_count: 0,
                    reading_time: 0,
                    excerpt: String::new(),
                    toc: Vec::new(),
                    extra: std::collections::HashMap::new(),
                });
                let tag_html = tera
                    .render("tag.html", &tag_ctx)
                    .map_err(|e| PageError::Build(format!("rendering tag '{tag}': {e}")))?;
                let tag_dir = paths.output.join(
                    format!("{}/{tag_slug}", tags_base_url.trim_start_matches('/')),
                );
                fs::create_dir_all(&tag_dir)?;
                fs::write(tag_dir.join("index.html"), tag_html)?;
                tag_page_urls.push(tag_url);
            }
        }
    }

    step_timings.push(("Render indexes".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 5: Generate RSS feed(s)
    let step_start = Instant::now();
    // Default language feed at /feed.xml
    let default_rss_items: Vec<&ContentItem> = config
        .collections
        .iter()
        .filter(|c| c.has_rss)
        .flat_map(|c| all_collections.get(&c.name).into_iter().flatten())
        .filter(|item| item.lang == *default_lang)
        .collect();
    let rss = feed::generate_rss(config, &default_rss_items)?;
    fs::write(paths.output.join("feed.xml"), rss)?;

    // Per-language RSS feeds
    if is_multilingual {
        for lang in config.languages.keys() {
            let lang_rss_items: Vec<&ContentItem> = config
                .collections
                .iter()
                .filter(|c| c.has_rss)
                .flat_map(|c| all_collections.get(&c.name).into_iter().flatten())
                .filter(|item| item.lang == *lang)
                .collect();
            if !lang_rss_items.is_empty() {
                let lang_rss = feed::generate_rss(config, &lang_rss_items)?;
                let lang_dir = paths.output.join(lang);
                fs::create_dir_all(&lang_dir)?;
                fs::write(lang_dir.join("feed.xml"), lang_rss)?;
            }
        }
    }

    step_timings.push(("Generate RSS".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 6: Generate sitemap (all items, all languages)
    let step_start = Instant::now();
    let all_items: Vec<&ContentItem> = all_collections.values().flatten().collect();
    let sitemap_xml = sitemap::generate_sitemap(config, &all_items, &translation_map, &tag_page_urls)?;
    fs::write(paths.output.join("sitemap.xml"), sitemap_xml)?;
    step_timings.push(("Generate sitemap".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 7: Generate discovery files (robots.txt, llms.txt, llms-full.txt)
    let step_start = Instant::now();
    let robots = discovery::generate_robots_txt(config);
    fs::write(paths.output.join("robots.txt"), robots)?;

    // Default language discovery files
    let default_discovery_collections: Vec<(String, Vec<&ContentItem>)> = config
        .collections
        .iter()
        .map(|c| {
            let items: Vec<&ContentItem> = all_collections
                .get(&c.name)
                .into_iter()
                .flatten()
                .filter(|item| item.lang == *default_lang)
                .collect();
            (c.label.clone(), items)
        })
        .collect();
    let llms_txt = discovery::generate_llms_txt(config, &default_discovery_collections);
    fs::write(paths.output.join("llms.txt"), llms_txt)?;
    let llms_full = discovery::generate_llms_full_txt(config, &default_discovery_collections);
    fs::write(paths.output.join("llms-full.txt"), llms_full)?;

    // Per-language discovery files
    if is_multilingual {
        for lang in config.languages.keys() {
            let lang_collections: Vec<(String, Vec<&ContentItem>)> = config
                .collections
                .iter()
                .map(|c| {
                    let items: Vec<&ContentItem> = all_collections
                        .get(&c.name)
                        .into_iter()
                        .flatten()
                        .filter(|item| item.lang == *lang)
                        .collect();
                    (c.label.clone(), items)
                })
                .collect();
            let lang_dir = paths.output.join(lang);
            fs::create_dir_all(&lang_dir)?;
            let llms = discovery::generate_llms_txt(config, &lang_collections);
            fs::write(lang_dir.join("llms.txt"), llms)?;
            let llms_f = discovery::generate_llms_full_txt(config, &lang_collections);
            fs::write(lang_dir.join("llms-full.txt"), llms_f)?;
        }
    }

    step_timings.push(("Generate discovery files".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 8: Output raw markdown alongside HTML for each page
    let step_start = Instant::now();
    for collection in &config.collections {
        if let Some(items) = all_collections.get(&collection.name) {
            for item in items {
                let md_path = url_to_md_path(&paths.output, &item.url);
                if let Some(parent) = md_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let md_content = format!(
                    "{}\n\n{}",
                    content::generate_frontmatter(&item.frontmatter),
                    item.raw_body
                );
                fs::write(&md_path, md_content)?;
            }
        }
    }

    step_timings.push(("Output markdown".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 9: Generate search index
    let step_start = Instant::now();
    let all_search_items: Vec<&ContentItem> = all_collections.values().flatten().collect();

    // Root index: default language only
    let default_search_items: Vec<&ContentItem> = all_search_items
        .iter()
        .filter(|i| i.lang == *default_lang)
        .copied()
        .collect();
    let search_json = generate_search_index(&default_search_items, config);
    fs::write(paths.output.join("search-index.json"), &search_json)?;

    // Per-language indexes for non-default languages
    if is_multilingual {
        for lang_code in config.languages.keys() {
            let lang_items: Vec<&ContentItem> = all_search_items
                .iter()
                .filter(|i| i.lang == *lang_code)
                .copied()
                .collect();
            let lang_json = generate_search_index(&lang_items, config);
            let lang_dir = paths.output.join(lang_code);
            fs::create_dir_all(&lang_dir)?;
            fs::write(lang_dir.join("search-index.json"), lang_json)?;
        }
    }

    step_timings.push(("Generate search index".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 10: Copy static files (with optional minification and fingerprinting)
    let step_start = Instant::now();
    let mut static_count = 0;
    // manifest: maps "/static/foo.css" → "/static/foo.<hash8>.css" (only when fingerprinting)
    let mut asset_manifest: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if paths.static_dir.exists() {
        for entry in WalkDir::new(&paths.static_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let rel = entry
                .path()
                .strip_prefix(&paths.static_dir)
                .unwrap_or(entry.path());
            let dest = paths.output.join("static").join(rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }

            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let is_css = ext == "css";
            let is_js = ext == "js";

            if (config.build.minify && (is_css || is_js)) || config.build.fingerprint {
                let content = fs::read(entry.path())?;
                let processed: Vec<u8> = if config.build.minify && is_css {
                    minify_css(&content).into_bytes()
                } else if config.build.minify && is_js {
                    minify_js(&content).into_bytes()
                } else {
                    content.clone()
                };

                fs::write(&dest, &processed)?;

                if config.build.fingerprint {
                    let hash = fnv_hash8(&processed);
                    // Build fingerprinted name: foo.css → foo.<hash>.css
                    let stem = entry.path().file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let fp_name = format!("{stem}.{hash}.{ext}");
                    let fp_dest = dest
                        .parent()
                        .map(|p| p.join(&fp_name))
                        .unwrap_or_else(|| PathBuf::from(&fp_name));
                    fs::write(&fp_dest, &processed)?;

                    // Record in manifest using Unix-style paths
                    let orig_url = format!("/static/{}", rel.to_string_lossy().replace('\\', "/"));
                    let fp_rel = rel
                        .parent()
                        .map(|p| {
                            let p = p.to_string_lossy();
                            if p.is_empty() {
                                fp_name.clone()
                            } else {
                                format!("{}/{fp_name}", p.replace('\\', "/"))
                            }
                        })
                        .unwrap_or_else(|| fp_name.clone());
                    let fp_url = format!("/static/{fp_rel}");
                    asset_manifest.insert(orig_url, fp_url);
                }
            } else {
                fs::copy(entry.path(), &dest)?;
            }
            static_count += 1;
        }
    }

    // Write asset manifest if fingerprinting is on
    if config.build.fingerprint && !asset_manifest.is_empty() {
        let manifest_json = serde_json::to_string_pretty(&asset_manifest)
            .unwrap_or_else(|_| "{}".to_string());
        fs::write(paths.output.join("asset-manifest.json"), manifest_json)?;
    }

    step_timings.push(("Copy static files".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 11: Process images (resize, WebP, srcset)
    let step_start = Instant::now();
    let image_manifest = if let Some(ref images_config) = config.images {
        if !images_config.widths.is_empty() {
            images::process_images(paths, images_config)?
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    step_timings.push(("Process images".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    // Step 12: Post-process HTML files to rewrite <img> tags
    let step_start = Instant::now();
    let lazy_loading = config.images.as_ref().is_some_and(|img| img.lazy_loading);
    let needs_image_rewrite = !image_manifest.is_empty() || lazy_loading;
    if needs_image_rewrite {
        rewrite_html_files(&paths.output, &image_manifest, lazy_loading)?;
    }

    step_timings.push(("Post-process HTML".to_string(), step_start.elapsed().as_secs_f64() * 1000.0));

    let items_built: HashMap<String, usize> = all_collections
        .iter()
        .map(|(name, items)| (name.clone(), items.len()))
        .collect();

    let data_files_count = crate::data::count_data_files(&paths.data_dir);
    let stats = BuildStats {
        items_built,
        static_files_copied: static_count,
        data_files_loaded: data_files_count,
        duration_ms: start.elapsed().as_millis() as u64,
        step_timings,
    };

    Ok(BuildResult {
        collections: all_collections,
        stats,
    })
}


/// Walk all .html files in the output directory and rewrite <img> tags.
fn rewrite_html_files(
    output_dir: &Path,
    image_manifest: &HashMap<String, images::ProcessedImage>,
    lazy_loading: bool,
) -> Result<()> {
    for entry in WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .is_some_and(|ext| ext == "html")
        })
    {
        let html = fs::read_to_string(entry.path())?;
        if !html.contains("<img ") {
            continue;
        }
        let rewritten = images::rewrite_html_images(&html, image_manifest, lazy_loading);
        if rewritten != html {
            fs::write(entry.path(), rewritten)?;
        }
    }
    Ok(())
}

fn resolve_slug(fm: &Frontmatter, rel_path: &Path, collection: &CollectionConfig) -> String {
    if let Some(ref s) = fm.slug {
        return s.clone();
    }

    let stem = rel_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");

    let filename_slug = if collection.has_date
        && stem.len() > 11
        && stem.as_bytes()[4] == b'-'
        && stem.as_bytes()[7] == b'-'
        && stem.as_bytes()[10] == b'-'
    {
        &stem[11..]
    } else {
        stem
    };

    if collection.nested {
        if let Some(parent) = rel_path.parent() {
            let parent_str = parent.to_str().unwrap_or("");
            if !parent_str.is_empty() {
                return format!("{}/{}", parent_str, filename_slug);
            }
        }
    }

    filename_slug.to_string()
}

/// Resolve slug for a translated file by stripping the language suffix from
/// the filename before delegating to `resolve_slug`.
fn resolve_slug_i18n(
    fm: &Frontmatter,
    rel_path: &Path,
    collection: &CollectionConfig,
    configured_langs: &std::collections::HashSet<&str>,
) -> String {
    // If frontmatter has an explicit slug, use it directly
    if fm.slug.is_some() {
        return resolve_slug(fm, rel_path, collection);
    }
    // Strip lang suffix from the stem: "about.es" → "about"
    let stem = rel_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let stripped = content::strip_lang_suffix(stem, configured_langs);
    let new_filename = format!("{stripped}.md");
    let new_rel = if let Some(parent) = rel_path.parent() {
        if parent.as_os_str().is_empty() {
            PathBuf::from(&new_filename)
        } else {
            parent.join(&new_filename)
        }
    } else {
        PathBuf::from(&new_filename)
    };
    resolve_slug(fm, &new_rel, collection)
}

fn parse_date_from_filename(path: &Path) -> Option<chrono::NaiveDate> {
    let stem = path.file_stem()?.to_str()?;
    if stem.len() >= 10 {
        match chrono::NaiveDate::parse_from_str(&stem[..10], "%Y-%m-%d") {
            Ok(date) => Some(date),
            Err(e) => {
                tracing::warn!(
                    "Malformed date in filename '{}': {e}",
                    path.display()
                );
                None
            }
        }
    } else {
        None
    }
}

fn build_url(url_prefix: &str, slug: &str) -> String {
    let prefix = url_prefix.trim_end_matches('/');
    if prefix.is_empty() {
        format!("/{slug}")
    } else {
        format!("{prefix}/{slug}")
    }
}

/// FNV-1a hash → first 8 hex chars, used for cache-busting fingerprints.
fn fnv_hash8(data: &[u8]) -> String {
    let mut hash: u64 = 14695981039346656037;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{hash:016x}")[..8].to_string()
}

/// Minify CSS: strip comments and collapse whitespace around syntax characters.
fn minify_css(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    // Remove block comments
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next(); // consume '*'
            // Skip until '*/'
            while let Some(c2) = chars.next() {
                if c2 == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    // Collapse runs of whitespace (including newlines) into a single space
    let mut result = String::with_capacity(out.len());
    let mut prev_space = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            prev_space = false;
            result.push(c);
        }
    }
    // Remove spaces around CSS delimiters
    for (pat, rep) in &[
        (" { ", "{"),
        ("{ ", "{"),
        (" {", "{"),
        (" } ", "}"),
        ("} ", "}"),
        (" }", "}"),
        (" : ", ":"),
        (": ", ":"),
        (" :", ":"),
        (" ; ", ";"),
        ("; ", ";"),
        (" ;", ";"),
        (" , ", ","),
        (", ", ","),
        (" ,", ","),
    ] {
        result = result.replace(pat, rep);
    }
    result.trim().to_string()
}

/// Minify JS: strip line comments and collapse blank lines.
/// Deliberately conservative — does not touch string contents or block structure.
fn minify_js(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let mut out = String::with_capacity(s.len());
    let mut in_single = false; // inside single-line string ''
    let mut in_double = false; // inside double-line string ""
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // Toggle string state (simplified — doesn't handle escape sequences perfectly)
        if c == '\'' && !in_double { in_single = !in_single; }
        if c == '"' && !in_single { in_double = !in_double; }
        // Strip // line comments only outside strings
        if !in_single && !in_double && c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' { i += 1; }
            out.push('\n');
            continue;
        }
        out.push(c);
        i += 1;
    }
    // Remove blank lines
    let result: String = out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    result
}

fn url_to_output_path(output_dir: &Path, url: &str) -> std::path::PathBuf {
    let clean = url.trim_matches('/');
    output_dir.join(format!("{clean}.html"))
}

fn url_to_md_path(output_dir: &Path, url: &str) -> std::path::PathBuf {
    let clean = url.trim_matches('/');
    output_dir.join(format!("{clean}.md"))
}

fn build_page_context(site: &SiteContext, item: &ContentItem, data: &serde_json::Value) -> tera::Context {
    let mut ctx = tera::Context::new();
    ctx.insert("site", site);
    ctx.insert("data", data);
    ctx.insert(
        "page",
        &PageContext {
            title: item.frontmatter.title.clone(),
            content: item.html_body.clone(),
            date: item.frontmatter.date.map(|d| d.to_string()),
            updated: item.frontmatter.updated.map(|d| d.to_string()),
            description: item.frontmatter.description.clone(),
            image: item.frontmatter.image.clone(),
            slug: item.slug.clone(),
            tags: item.frontmatter.tags.clone(),
            url: item.url.clone(),
            collection: item.collection.clone(),
            robots: item.frontmatter.robots.clone(),
            word_count: item.word_count,
            reading_time: item.reading_time,
            excerpt: item.excerpt_html.clone(),
            toc: item.toc.clone(),
            extra: item.frontmatter.extra.clone(),
        },
    );
    ctx
}

/// Build a JSON search index from a slice of content items.
/// Only items from `listed: true` collections are included.
fn generate_search_index(items: &[&ContentItem], config: &SiteConfig) -> String {
    let listed: std::collections::HashSet<&str> = config
        .collections
        .iter()
        .filter(|c| c.listed)
        .map(|c| c.name.as_str())
        .collect();

    let entries: Vec<SearchEntry> = items
        .iter()
        .filter(|item| listed.contains(item.collection.as_str()))
        .map(|item| SearchEntry {
            title: &item.frontmatter.title,
            description: item.frontmatter.description.as_deref(),
            url: &item.url,
            collection: &item.collection,
            tags: &item.frontmatter.tags,
            date: item.frontmatter.date.map(|d| d.to_string()),
            lang: &item.lang,
        })
        .collect();

    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string())
}

/// Convert a slug segment to title case (e.g., "getting-started" → "Getting Started").
fn title_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
