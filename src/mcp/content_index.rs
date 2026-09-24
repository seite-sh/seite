//! Content inventory shared by MCP resources and tools.
//!
//! Uses the build's own slug/URL/language resolution
//! ([`crate::build::resolve_item_location`]) so listed URLs always match the
//! generated site — including nested docs, `slug:` overrides, date prefixes,
//! and `.{lang}.md` translations. Files that fail to parse are reported with
//! their error instead of being silently skipped. Drafts are included and
//! flagged.

use std::path::Path;

use chrono::NaiveDate;
use walkdir::WalkDir;

use crate::build::{resolve_item_date, resolve_item_location};
use crate::config::{CollectionConfig, ResolvedPaths, SiteConfig};
use crate::content::{self, Frontmatter};

/// A successfully parsed content file.
pub struct ParsedItem {
    pub frontmatter: Frontmatter,
    pub body: String,
    pub slug: String,
    pub url: String,
    pub lang: String,
    pub date: Option<NaiveDate>,
}

/// One `.md` file in a collection.
pub struct IndexedItem {
    pub collection: String,
    /// Source path relative to the site root, `/`-separated.
    pub path: String,
    pub parsed: Result<ParsedItem, String>,
}

/// Path of `path` relative to `root`, with forward slashes.
pub fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Scan every `.md` file in `collection`, in a stable (sorted) order.
pub fn scan_collection(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    collection: &CollectionConfig,
) -> Vec<IndexedItem> {
    let dir = paths.content.join(&collection.directory);
    if !dir.exists() {
        return Vec::new();
    }

    // Subdomain collections are built as root-mounted sites (see
    // build::build_subdomain_sites), so their URLs have no prefix.
    let effective;
    let collection_for_urls = if collection.subdomain.is_some() {
        effective = CollectionConfig {
            url_prefix: String::new(),
            ..collection.clone()
        };
        &effective
    } else {
        collection
    };

    WalkDir::new(&dir)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .map(|entry| {
            let path = entry.path();
            let rel_to_collection = path.strip_prefix(&dir).unwrap_or(path);
            let parsed = content::parse_content_file(path)
                .map(|(fm, body)| {
                    let loc = resolve_item_location(
                        config,
                        collection_for_urls,
                        path,
                        rel_to_collection,
                        &fm,
                    );
                    let date = resolve_item_date(&fm, path, collection);
                    ParsedItem {
                        frontmatter: fm,
                        body,
                        slug: loc.slug,
                        url: loc.url,
                        lang: loc.lang,
                        date,
                    }
                })
                .map_err(|e| e.to_string());
            IndexedItem {
                collection: collection.name.clone(),
                path: relative_path(&paths.root, path),
                parsed,
            }
        })
        .collect()
}

/// JSON summary of an item for listings. Unparseable files become
/// `{ "path", "parse_error" }` entries.
pub fn item_summary(item: &IndexedItem) -> serde_json::Value {
    match &item.parsed {
        Ok(p) => serde_json::json!({
            "title": p.frontmatter.title,
            "slug": p.slug,
            "url": p.url,
            "path": item.path,
            "lang": p.lang,
            "draft": p.frontmatter.draft,
            "date": p.date.map(|d| d.to_string()),
            "tags": p.frontmatter.tags,
            "description": p.frontmatter.description,
            "weight": p.frontmatter.weight,
        }),
        Err(e) => serde_json::json!({
            "path": item.path,
            "parse_error": e,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BuildSection, DeploySection, LanguageConfig, SiteSection};
    use std::collections::BTreeMap;

    fn config(multilingual: bool) -> SiteConfig {
        let mut languages = BTreeMap::new();
        if multilingual {
            languages.insert("es".to_string(), LanguageConfig::default());
        }
        SiteConfig {
            site: SiteSection {
                title: "T".into(),
                description: String::new(),
                base_url: "http://localhost:3000".into(),
                language: "en".into(),
                author: String::new(),
            },
            collections: vec![
                CollectionConfig::preset_posts(),
                CollectionConfig::preset_docs(),
            ],
            build: BuildSection::default(),
            deploy: DeploySection::default(),
            languages,
            images: None,
            analytics: None,
            trust: None,
            contact: None,
            access: None,
        }
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    fn find<'a>(items: &'a [IndexedItem], path: &str) -> &'a ParsedItem {
        items
            .iter()
            .find(|i| i.path == path)
            .unwrap_or_else(|| panic!("no item {path}"))
            .parsed
            .as_ref()
            .unwrap()
    }

    #[test]
    fn test_scan_uses_real_output_urls() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = config(true);
        let paths = cfg.resolve_paths(tmp.path());
        // Nested doc whose title differs from its filename.
        write(
            tmp.path(),
            "content/docs/guides/intro.md",
            "---\ntitle: Intro Guide\n---\nx\n",
        );
        // Translation whose title is unrelated to its filename.
        write(
            tmp.path(),
            "content/docs/getting-started.es.md",
            "---\ntitle: Spanish\n---\nx\n",
        );
        // Renamed title: URL follows the filename, not the title.
        write(
            tmp.path(),
            "content/posts/2026-01-02-old-name.md",
            "---\ntitle: Brand New Title\ndraft: true\n---\nx\n",
        );
        write(tmp.path(), "content/docs/broken.md", "no frontmatter here");

        let docs = scan_collection(&cfg, &paths, &CollectionConfig::preset_docs());
        let intro = find(&docs, "content/docs/guides/intro.md");
        assert_eq!(intro.url, "/docs/guides/intro");
        assert_eq!(intro.slug, "guides/intro");
        let es = find(&docs, "content/docs/getting-started.es.md");
        assert_eq!(es.url, "/es/docs/getting-started");
        assert_eq!(es.lang, "es");
        let broken = docs
            .iter()
            .find(|i| i.path == "content/docs/broken.md")
            .unwrap();
        assert!(broken.parsed.is_err());
        let summary = item_summary(broken);
        assert!(summary["parse_error"].is_string());

        let posts = scan_collection(&cfg, &paths, &CollectionConfig::preset_posts());
        let post = find(&posts, "content/posts/2026-01-02-old-name.md");
        assert_eq!(post.url, "/posts/old-name");
        assert!(post.frontmatter.draft);
        assert_eq!(
            post.date.map(|d| d.to_string()).as_deref(),
            Some("2026-01-02")
        );
        let summary = item_summary(&posts[0]);
        assert_eq!(summary["draft"], true);
        assert_eq!(summary["lang"], "en");
    }

    #[test]
    fn test_scan_missing_dir_is_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = config(false);
        let paths = cfg.resolve_paths(tmp.path());
        assert!(scan_collection(&cfg, &paths, &CollectionConfig::preset_docs()).is_empty());
    }
}
