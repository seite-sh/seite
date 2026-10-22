//! `page.related`: other pages of the same collection and language that share
//! taxonomy terms with this one.
//!
//! Score = sum over shared terms of the taxonomy's weight (a shared tag counts
//! 1, a shared term of any other taxonomy, such as a category, counts 2).
//! Ties break by date proximity (closest first, undated pages last), then
//! title, then URL, so the result is deterministic. Drafts and `index` pages
//! are never related. Limit: the collection's `related`, else `[build]
//! related`, else 3; `0` disables.
//!
//! Computed once per build from a term → pages index, so each page only
//! visits pages that share at least one of its terms.

use std::collections::{BTreeSet, HashMap};

use chrono::NaiveDate;
use serde::Serialize;

use super::taxonomy::{term_slug, Taxonomy};
use crate::config::{CollectionConfig, SiteConfig};
use crate::content::ContentItem;

/// Default number of related pages when neither the collection nor `[build]` sets one.
pub(crate) const DEFAULT_RELATED: usize = 3;

/// One entry of `page.related`.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct RelatedItem {
    pub title: String,
    pub url: String,
    pub date: Option<String>,
    pub description: Option<String>,
    /// Weighted count of shared terms.
    pub score: u32,
}

/// Effective related-pages limit for a collection.
pub(crate) fn related_limit(config: &SiteConfig, collection: &CollectionConfig) -> usize {
    collection
        .related
        .or(config.build.related)
        .unwrap_or(DEFAULT_RELATED)
}

/// Related pages for every eligible item, keyed by item URL.
pub(crate) fn compute_related(
    config: &SiteConfig,
    all_collections: &HashMap<String, Vec<ContentItem>>,
    taxonomies: &[Taxonomy],
) -> HashMap<String, Vec<RelatedItem>> {
    let mut out = HashMap::new();
    for collection in &config.collections {
        let limit = related_limit(config, collection);
        if limit == 0 {
            continue;
        }
        let Some(items) = all_collections.get(&collection.name) else {
            continue;
        };
        let mut by_lang: HashMap<&str, Vec<&ContentItem>> = HashMap::new();
        for item in items
            .iter()
            .filter(|i| !i.frontmatter.draft && i.slug != "index")
        {
            by_lang.entry(item.lang.as_str()).or_default().push(item);
        }
        for group in by_lang.values() {
            out.extend(related_within(group, taxonomies, limit));
        }
    }
    out
}

/// Term keys (taxonomy index, term slug) of an item, deduplicated.
fn item_terms(item: &ContentItem, taxonomies: &[Taxonomy]) -> BTreeSet<(usize, String)> {
    let mut terms = BTreeSet::new();
    for (ti, tax) in taxonomies.iter().enumerate() {
        for term in item.frontmatter.taxonomy_terms(&tax.name) {
            let slug = term_slug(&term);
            if !slug.is_empty() {
                terms.insert((ti, slug));
            }
        }
    }
    terms
}

/// Related pages within one collection + language group.
pub(crate) fn related_within(
    items: &[&ContentItem],
    taxonomies: &[Taxonomy],
    limit: usize,
) -> Vec<(String, Vec<RelatedItem>)> {
    let terms: Vec<BTreeSet<(usize, String)>> =
        items.iter().map(|i| item_terms(i, taxonomies)).collect();
    let mut index: HashMap<&(usize, String), Vec<usize>> = HashMap::new();
    for (idx, set) in terms.iter().enumerate() {
        for key in set {
            index.entry(key).or_default().push(idx);
        }
    }

    let mut out = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        if terms[idx].is_empty() {
            continue;
        }
        let mut scores: HashMap<usize, u32> = HashMap::new();
        for key in &terms[idx] {
            let weight = taxonomies[key.0].related_weight;
            for &other in index.get(key).into_iter().flatten() {
                if other != idx {
                    *scores.entry(other).or_default() += weight;
                }
            }
        }
        if scores.is_empty() {
            continue;
        }
        let date = item.frontmatter.date;
        let mut ranked: Vec<(usize, u32)> = scores.into_iter().collect();
        ranked.sort_by(|&(a, sa), &(b, sb)| {
            sb.cmp(&sa)
                .then_with(|| {
                    distance(date, items[a].frontmatter.date)
                        .cmp(&distance(date, items[b].frontmatter.date))
                })
                .then_with(|| items[a].frontmatter.title.cmp(&items[b].frontmatter.title))
                .then_with(|| items[a].url.cmp(&items[b].url))
        });
        let related: Vec<RelatedItem> = ranked
            .into_iter()
            .take(limit)
            .map(|(i, score)| {
                let r = items[i];
                RelatedItem {
                    title: r.frontmatter.title.clone(),
                    url: r.url.clone(),
                    date: r.frontmatter.date.map(|d| d.to_string()),
                    description: r.frontmatter.description.clone(),
                    score,
                }
            })
            .collect();
        out.push((item.url.clone(), related));
    }
    out
}

/// Days between two dates; `i64::MAX` when either is missing (sorts last).
fn distance(a: Option<NaiveDate>, b: Option<NaiveDate>) -> i64 {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).num_days().abs(),
        _ => i64::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::taxonomy::resolve_taxonomies;
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

    fn config(extra: &str) -> SiteConfig {
        toml::from_str(&format!(
            "[site]\ntitle = \"T\"\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\ndefault_template = \"post.html\"\n{extra}"
        ))
        .unwrap()
    }

    fn titles(r: &[RelatedItem]) -> Vec<&str> {
        r.iter().map(|x| x.title.as_str()).collect()
    }

    #[test]
    fn test_related_scoring_weights_categories_over_tags() {
        let taxes = resolve_taxonomies(&config("[[taxonomies]]\nname = \"categories\"\n"));
        let a = item("A", "tags: [x, y]\ncategories: [c]\n");
        let b = item("B", "tags: [x, y]\n"); // 2 shared tags = 2
        let c = item("C", "categories: [c]\ntags: [x]\n"); // 2 + 1 = 3
        let d = item("D", "tags: [y]\n"); // 1
        let e = item("E", "tags: [unrelated]\n");
        let items = vec![&a, &b, &c, &d, &e];
        let out: HashMap<String, Vec<RelatedItem>> =
            related_within(&items, &taxes, 10).into_iter().collect();
        let ra = &out["/posts/a"];
        assert_eq!(titles(ra), ["C", "B", "D"]);
        assert_eq!(ra[0].score, 3);
        assert!(!out.contains_key("/posts/e"), "no shared terms → no entry");
    }

    #[test]
    fn test_related_tiebreak_by_date_then_title_and_limit() {
        let taxes = resolve_taxonomies(&config(""));
        let a = item("A", "date: 2024-06-10\ntags: [t]\n");
        let far = item("Far", "date: 2020-01-01\ntags: [t]\n");
        let near = item("Near", "date: 2024-06-12\ntags: [t]\n");
        let near2 = item("Abc", "date: 2024-06-08\ntags: [t]\n");
        let undated = item("Aaa", "tags: [t]\n");
        let items = vec![&a, &far, &near, &near2, &undated];
        let out: HashMap<String, Vec<RelatedItem>> =
            related_within(&items, &taxes, 3).into_iter().collect();
        assert_eq!(titles(&out["/posts/a"]), ["Abc", "Near", "Far"]);
        // Deterministic regardless of input order.
        let rev: Vec<&ContentItem> = items.iter().rev().copied().collect();
        let out2: HashMap<String, Vec<RelatedItem>> =
            related_within(&rev, &taxes, 3).into_iter().collect();
        assert_eq!(out, out2);
    }

    #[test]
    fn test_compute_related_excludes_drafts_index_and_honors_limits() {
        let mut cfg = config("[build]\nrelated = 1\n");
        let a = item("A", "tags: [t]\n");
        let b = item("B", "tags: [t]\n");
        let draft = item("Draft", "tags: [t]\ndraft: true\n");
        let mut index = item("Index", "tags: [t]\n");
        index.slug = "index".into();
        let mut all = HashMap::new();
        all.insert("posts".to_string(), vec![a, b, draft, index]);
        let taxes = resolve_taxonomies(&cfg);
        let out = compute_related(&cfg, &all, &taxes);
        assert_eq!(titles(&out["/posts/a"]), ["B"]);
        assert!(!out.contains_key("/posts/draft"));

        cfg.collections[0].related = Some(0);
        assert!(compute_related(&cfg, &all, &taxes).is_empty());
        assert_eq!(related_limit(&config(""), &config("").collections[0]), 3);
    }
}
