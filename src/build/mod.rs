pub mod discovery;
pub mod feed;
pub mod markdown;
pub mod sitemap;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
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
    let site_ctx = SiteContext::from_config(config);

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

                let slug = resolve_slug(&fm, rel, collection);

                let mut fm = fm;
                if fm.date.is_none() && collection.has_date {
                    fm.date = parse_date_from_filename(path);
                }

                let html_body = markdown::markdown_to_html(&raw_body);
                let url = build_url(&collection.url_prefix, &slug);

                items.push(ContentItem {
                    frontmatter: fm,
                    raw_body,
                    html_body,
                    source_path: path.to_path_buf(),
                    slug,
                    collection: collection.name.clone(),
                    url,
                });
            }
        }

        // Sort: date-based collections by date desc, others by title
        if collection.has_date {
            items.sort_by(|a, b| b.frontmatter.date.cmp(&a.frontmatter.date));
        } else {
            items.sort_by(|a, b| a.frontmatter.title.cmp(&b.frontmatter.title));
        }

        // Render each item
        for item in &items {
            let ctx = build_page_context(&site_ctx, item);
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

        all_collections.insert(collection.name.clone(), items);
    }

    // Step 4: Render index page
    let mut index_ctx = tera::Context::new();
    index_ctx.insert("site", &site_ctx);

    let collections_ctx: Vec<CollectionContext> = config
        .collections
        .iter()
        .filter(|c| c.listed)
        .map(|c| {
            let items = all_collections.get(&c.name).cloned().unwrap_or_default();
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

    let index_html = tera
        .render("index.html", &index_ctx)
        .map_err(|e| PageError::Build(format!("rendering index: {e}")))?;
    fs::write(paths.output.join("index.html"), index_html)?;

    // Step 5: Generate RSS feed (items from has_rss collections)
    let rss_items: Vec<&ContentItem> = config
        .collections
        .iter()
        .filter(|c| c.has_rss)
        .flat_map(|c| all_collections.get(&c.name).into_iter().flatten())
        .collect();
    let rss = feed::generate_rss(config, &rss_items)?;
    fs::write(paths.output.join("feed.xml"), rss)?;

    // Step 6: Generate sitemap (all items)
    let all_items: Vec<&ContentItem> = all_collections.values().flatten().collect();
    let sitemap_xml = sitemap::generate_sitemap(config, &all_items)?;
    fs::write(paths.output.join("sitemap.xml"), sitemap_xml)?;

    // Step 7: Generate discovery files (robots.txt, llms.txt, llms-full.txt)
    let robots = discovery::generate_robots_txt(config);
    fs::write(paths.output.join("robots.txt"), robots)?;

    let discovery_collections: Vec<(String, Vec<&ContentItem>)> = config
        .collections
        .iter()
        .map(|c| {
            let items: Vec<&ContentItem> = all_collections
                .get(&c.name)
                .into_iter()
                .flatten()
                .collect();
            (c.label.clone(), items)
        })
        .collect();
    let llms_txt = discovery::generate_llms_txt(config, &discovery_collections);
    fs::write(paths.output.join("llms.txt"), llms_txt)?;
    let llms_full = discovery::generate_llms_full_txt(config, &discovery_collections);
    fs::write(paths.output.join("llms-full.txt"), llms_full)?;

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

    // Step 9: Copy static files
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

fn parse_date_from_filename(path: &Path) -> Option<chrono::NaiveDate> {
    let stem = path.file_stem()?.to_str()?;
    if stem.len() >= 10 {
        chrono::NaiveDate::parse_from_str(&stem[..10], "%Y-%m-%d").ok()
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
