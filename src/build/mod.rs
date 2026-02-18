pub mod discovery;
pub mod feed;
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
    pub duration_ms: u64,
}

impl CommandOutput for BuildStats {
    fn human_display(&self) -> String {
        let parts: Vec<String> = self
            .items_built
            .iter()
            .map(|(name, count)| format!("{count} {name}"))
            .collect();
        format!(
            "Built {} in {:.1}s ({} static files copied)",
            parts.join(", "),
            self.duration_ms as f64 / 1000.0,
            self.static_files_copied
        )
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
    description: Option<String>,
    slug: String,
    tags: Vec<String>,
    url: String,
}

#[derive(Serialize)]
struct CollectionContext {
    name: String,
    label: String,
    items: Vec<ItemSummary>,
}

#[derive(Serialize)]
struct ItemSummary {
    title: String,
    date: Option<String>,
    description: Option<String>,
    slug: String,
    tags: Vec<String>,
    url: String,
}

#[derive(Serialize)]
struct NavItem {
    title: String,
    url: String,
    active: bool,
}

#[derive(Serialize)]
struct NavSection {
    name: String,
    label: String,
    items: Vec<NavItem>,
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

    // Step 1: Clean output directory
    if paths.output.exists() {
        fs::remove_dir_all(&paths.output)?;
    }
    fs::create_dir_all(&paths.output)?;

    // Step 2: Load templates (collection-aware)
    let tera = templates::load_templates(&paths.templates, &config.collections)?;

    // Pre-compute configured language codes for filename detection
    let configured_langs = config.configured_lang_codes();
    let is_multilingual = config.is_multilingual();
    let default_lang = &config.site.language;

    // Step 3: Process each collection
    let mut all_collections: HashMap<String, Vec<ContentItem>> = HashMap::new();

    for collection in &config.collections {
        let collection_dir = paths.content.join(&collection.directory);
        let mut items = Vec::new();

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

                let html_body = markdown::markdown_to_html(&raw_body);

                // Build URL: non-default languages get /{lang} prefix
                let base_url = build_url(&collection.url_prefix, &slug);
                let url = if lang != *default_lang {
                    format!("/{lang}{base_url}")
                } else {
                    base_url
                };

                items.push(ContentItem {
                    frontmatter: fm,
                    raw_body,
                    html_body,
                    source_path: path.to_path_buf(),
                    slug,
                    collection: collection.name.clone(),
                    url,
                    lang,
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
    for collection in &config.collections {
        if let Some(items) = all_collections.get(&collection.name) {
            // Nav is built per-language: only sibling items in the same language
            for item in items {
                let lang_items: Vec<&ContentItem> = items
                    .iter()
                    .filter(|i| i.lang == item.lang)
                    .collect();

                let site_ctx_for_item = if item.lang == *default_lang {
                    SiteContext::from_config(config)
                } else {
                    SiteContext::for_lang(config, &item.lang)
                };

                let mut ctx = build_page_context(&site_ctx_for_item, item);

                let nav_items: Vec<ContentItem> = lang_items.into_iter().cloned().collect();
                let nav = build_nav(&nav_items, &item.slug);
                ctx.insert("nav", &nav);
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
    for lang in &config.all_languages() {
        let lang_site_ctx = SiteContext::for_lang(config, lang);
        let mut index_ctx = tera::Context::new();
        index_ctx.insert("site", &lang_site_ctx);
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
                        })
                        .collect(),
                }
            })
            .collect();
        index_ctx.insert("collections", &collections_ctx);

        // Homepage page for this language
        if let Some(homepage) = homepage_pages.iter().find(|p| p.lang == *lang) {
            index_ctx.insert(
                "page",
                &PageContext {
                    title: homepage.frontmatter.title.clone(),
                    content: homepage.html_body.clone(),
                    date: homepage.frontmatter.date.map(|d| d.to_string()),
                    description: homepage.frontmatter.description.clone(),
                    slug: homepage.slug.clone(),
                    tags: homepage.frontmatter.tags.clone(),
                    url: if *lang == *default_lang {
                        "/".to_string()
                    } else {
                        format!("/{lang}/")
                    },
                },
            );
        }

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

    // Step 5: Generate RSS feed(s)
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

    // Step 6: Generate sitemap (all items, all languages)
    let all_items: Vec<&ContentItem> = all_collections.values().flatten().collect();
    let sitemap_xml = sitemap::generate_sitemap(config, &all_items, &translation_map)?;
    fs::write(paths.output.join("sitemap.xml"), sitemap_xml)?;

    // Step 7: Generate discovery files (robots.txt, llms.txt, llms-full.txt)
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

    // Step 8: Output raw markdown alongside HTML for each page
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

    // Step 9: Generate search index
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

    // Step 10: Copy static files
    let mut static_count = 0;
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
            fs::copy(entry.path(), &dest)?;
            static_count += 1;
        }
    }

    let items_built: HashMap<String, usize> = all_collections
        .iter()
        .map(|(name, items)| (name.clone(), items.len()))
        .collect();

    let stats = BuildStats {
        items_built,
        static_files_copied: static_count,
        duration_ms: start.elapsed().as_millis() as u64,
    };

    Ok(BuildResult {
        collections: all_collections,
        stats,
    })
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

fn url_to_output_path(output_dir: &Path, url: &str) -> std::path::PathBuf {
    let clean = url.trim_matches('/');
    output_dir.join(format!("{clean}.html"))
}

fn url_to_md_path(output_dir: &Path, url: &str) -> std::path::PathBuf {
    let clean = url.trim_matches('/');
    output_dir.join(format!("{clean}.md"))
}

fn build_page_context(site: &SiteContext, item: &ContentItem) -> tera::Context {
    let mut ctx = tera::Context::new();
    ctx.insert("site", site);
    ctx.insert(
        "page",
        &PageContext {
            title: item.frontmatter.title.clone(),
            content: item.html_body.clone(),
            date: item.frontmatter.date.map(|d| d.to_string()),
            description: item.frontmatter.description.clone(),
            slug: item.slug.clone(),
            tags: item.frontmatter.tags.clone(),
            url: item.url.clone(),
        },
    );
    ctx
}

/// Build a navigation structure from a collection's items, grouped by directory.
/// Top-level items go in a section with empty name; nested items are grouped by
/// their first path segment (e.g., "guides/setup" → section "guides").
fn build_nav(items: &[ContentItem], current_slug: &str) -> Vec<NavSection> {
    let mut sections: Vec<NavSection> = Vec::new();
    let mut section_map: HashMap<String, usize> = HashMap::new();

    for item in items {
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
            active: item.slug == current_slug,
        };

        if let Some(&idx) = section_map.get(&section_name) {
            sections[idx].items.push(nav_item);
        } else {
            let idx = sections.len();
            section_map.insert(section_name.clone(), idx);
            sections.push(NavSection {
                name: section_name,
                label: section_label,
                items: vec![nav_item],
            });
        }
    }

    sections
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
