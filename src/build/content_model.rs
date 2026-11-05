//! Cross-page content model computed once per build: taxonomies, related
//! pages and series. Feeds `page.taxonomies`, `page.related` and `page.series`
//! on content pages, and generates the taxonomy and series listing pages.

use std::collections::HashMap;

use super::related::{compute_related, RelatedItem};
use super::series::SeriesIndex;
use super::taxonomy::{
    page_taxonomies, render_taxonomy_pages, resolve_taxonomies, RenderEnv, Taxonomy,
};
use crate::config::SiteConfig;
use crate::content::ContentItem;
use crate::error::Result;

pub(crate) struct ContentModel {
    taxonomies: Vec<Taxonomy>,
    related: HashMap<String, Vec<RelatedItem>>,
    series: SeriesIndex,
}

impl ContentModel {
    pub(crate) fn build(
        config: &SiteConfig,
        all_collections: &indexmap::IndexMap<String, Vec<ContentItem>>,
    ) -> Self {
        let taxonomies = resolve_taxonomies(config);
        let related = compute_related(config, all_collections, &taxonomies);
        let series = SeriesIndex::build(config, all_collections);
        Self {
            taxonomies,
            related,
            series,
        }
    }

    /// Collections to re-render in an incremental build when the item at
    /// `url` changed: those of every part of its series.
    pub(crate) fn series_collections(&self, url: &str) -> Vec<String> {
        self.series.collections_of_series(url)
    }

    /// Warnings found while building the model (`series-url-collision`).
    pub(crate) fn diagnostics(&self) -> &[crate::diagnostics::Diagnostic] {
        &self.series.warnings
    }

    /// Frontmatter keys of the configured taxonomies other than `tags`
    /// (published in `.md` copies).
    pub(crate) fn taxonomy_keys(&self) -> Vec<String> {
        self.taxonomies
            .iter()
            .filter(|t| !t.is_tags())
            .map(|t| t.name.clone())
            .collect()
    }

    /// Add `taxonomies`, `related` and `series` to the `page` value of a
    /// content page's context. `related` is always a list (possibly empty) and
    /// `series` is null when the page is not part of one.
    pub(crate) fn augment_page(
        &self,
        ctx: &mut tera::Context,
        item: &ContentItem,
        lang_prefix: &str,
    ) {
        let Some(mut page) = ctx.remove("page") else {
            return;
        };
        if let Some(obj) = page.as_object_mut() {
            obj.insert(
                "taxonomies".into(),
                serde_json::Value::Object(page_taxonomies(&self.taxonomies, item, lang_prefix)),
            );
            let related = self
                .related
                .get(&item.url)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            obj.insert(
                "related".into(),
                serde_json::to_value(related).unwrap_or_default(),
            );
            obj.insert(
                "series".into(),
                serde_json::to_value(self.series.for_page(&item.url)).unwrap_or_default(),
            );
        }
        ctx.insert("page", &page);
    }

    /// Generate taxonomy index/term pages (with term feeds) and series index
    /// pages. Returns the generated page URLs for the sitemap.
    pub(crate) fn render_pages(
        &self,
        env: &RenderEnv<'_>,
        all_collections: &indexmap::IndexMap<String, Vec<ContentItem>>,
    ) -> Result<Vec<String>> {
        let mut urls = render_taxonomy_pages(env, &self.taxonomies, all_collections)?;
        urls.extend(self.series.render_pages(env)?);
        Ok(urls)
    }
}

#[cfg(test)]
mod tests {
    use crate::build::{build_site, BuildOptions};
    use crate::config::{CollectionConfig, SiteConfig};
    use std::fs;

    #[test]
    fn test_incremental_rebuild_refreshes_series_siblings_in_other_collections() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config: SiteConfig = toml::from_str(
            "[site]\ntitle = \"T\"\nbase_url = \"https://example.com\"\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\ndefault_template = \"post.html\"\n",
        )
        .unwrap();
        config.collections = vec![
            CollectionConfig::preset_posts(),
            CollectionConfig::preset_docs(),
        ];
        let paths = config.resolve_paths(tmp.path());
        fs::write(tmp.path().join("seite.toml"), "# config\n").unwrap();
        fs::create_dir_all(paths.content.join("posts")).unwrap();
        fs::create_dir_all(paths.content.join("docs")).unwrap();
        fs::write(
            paths.content.join("posts/2024-01-01-a.md"),
            "---\ntitle: A\nseries: S\nseries_order: 1\n---\nA\n",
        )
        .unwrap();
        let doc = paths.content.join("docs/b.md");
        fs::write(
            &doc,
            "---\ntitle: Old B\nseries: S\nseries_order: 2\n---\nB\n",
        )
        .unwrap();
        let opts = BuildOptions {
            include_drafts: false,
            incremental: true,
        };
        build_site(&config, &paths, &opts).unwrap();
        let post = paths.output.join("posts/a.html");
        assert!(fs::read_to_string(&post).unwrap().contains("Old B"));

        // Only the docs page changes; the post in another collection must be
        // re-rendered because its series box names the doc.
        fs::write(
            &doc,
            "---\ntitle: New B\nseries: S\nseries_order: 2\n---\nB\n",
        )
        .unwrap();
        // The build cache compares whole-second mtimes: move it forward.
        fs::File::options()
            .write(true)
            .open(&doc)
            .unwrap()
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
            .unwrap();
        let result = build_site(&config, &paths, &opts).unwrap();
        assert!(result.stats.incremental);
        let html = fs::read_to_string(&post).unwrap();
        assert!(html.contains("New B"), "{html}");
    }
}
