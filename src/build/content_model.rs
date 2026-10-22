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
        all_collections: &HashMap<String, Vec<ContentItem>>,
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
        all_collections: &HashMap<String, Vec<ContentItem>>,
    ) -> Result<Vec<String>> {
        let mut urls = render_taxonomy_pages(env, &self.taxonomies, all_collections)?;
        urls.extend(self.series.render_pages(env)?);
        Ok(urls)
    }
}
