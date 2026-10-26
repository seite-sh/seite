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
//! Computed once per build (in parallel) from a term → pages index, so each
//! page only visits pages that share at least one of its terms, and only the
//! top `limit` candidates are ordered. In a group of more than 200 pages, a
//! term carried by more than 20% of them (say a `blog` tag on every post) is
//! "common": it still adds to the score of pages related by another term, but
//! does not by itself make pages related. That keeps a build linear in the
//! number of pages instead of quadratic.

use std::collections::HashMap;

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
    all_collections: &indexmap::IndexMap<String, Vec<ContentItem>>,
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

/// Related pages of one page: (its URL, the related entries).
type PageRelated = (String, Vec<RelatedItem>);

/// Groups with more pages than this apply the common-term rule.
const COMMON_MIN_GROUP: usize = 200;
/// In such a group, a term on more than 1/`COMMON_DIVISOR` (20%) of the pages
/// is common: it does not by itself make two pages related (scanning its
/// pages for every page would be quadratic), but it still adds its weight to
/// pages that share another term.
const COMMON_DIVISOR: usize = 5;

/// Related pages within one collection + language group.
pub(crate) fn related_within(
    items: &[&ContentItem],
    taxonomies: &[Taxonomy],
    limit: usize,
) -> Vec<(String, Vec<RelatedItem>)> {
    related_within_counted(items, taxonomies, limit).0
}

/// [`related_within`], plus the number of term → page postings visited (the
/// dominant cost; tests bound it).
fn related_within_counted(
    items: &[&ContentItem],
    taxonomies: &[Taxonomy],
    limit: usize,
) -> (Vec<(String, Vec<RelatedItem>)>, u64) {
    use rayon::prelude::*;

    let n = items.len();
    if limit == 0 || n < 2 {
        return (Vec::new(), 0);
    }
    // Dense term ids, each item's sorted term ids, and each term's pages.
    let mut ids: HashMap<(usize, String), u32> = HashMap::new();
    let mut weights: Vec<u32> = Vec::new();
    let mut postings: Vec<Vec<u32>> = Vec::new();
    let mut item_terms: Vec<Vec<u32>> = Vec::with_capacity(n);
    for (idx, item) in items.iter().enumerate() {
        let mut own: Vec<u32> = Vec::new();
        for (ti, tax) in taxonomies.iter().enumerate() {
            for term in item.frontmatter.taxonomy_terms(&tax.name) {
                let slug = term_slug(&term);
                if slug.is_empty() {
                    continue;
                }
                let id = *ids.entry((ti, slug)).or_insert_with(|| {
                    weights.push(tax.related_weight);
                    postings.push(Vec::new());
                    (weights.len() - 1) as u32
                });
                own.push(id);
            }
        }
        own.sort_unstable();
        own.dedup();
        for &id in &own {
            postings[id as usize].push(idx as u32);
        }
        item_terms.push(own);
    }
    let common: Vec<bool> = postings
        .iter()
        .map(|p| n > COMMON_MIN_GROUP && p.len() * COMMON_DIVISOR > n)
        .collect();
    let dates: Vec<Option<NaiveDate>> = items.iter().map(|i| i.frontmatter.date).collect();

    let per_item: Vec<(Option<PageRelated>, u64)> = (0..n)
        .into_par_iter()
        .map_init(
            || (vec![0u32; n], Vec::<u32>::new()),
            |(scores, touched), idx| {
                let mut visits = 0u64;
                let own = &item_terms[idx];
                for &t in own.iter().filter(|&&t| !common[t as usize]) {
                    let w = weights[t as usize];
                    let list = &postings[t as usize];
                    visits += list.len() as u64;
                    for &o in list {
                        if o as usize == idx {
                            continue;
                        }
                        if scores[o as usize] == 0 {
                            touched.push(o);
                        }
                        scores[o as usize] += w;
                    }
                }
                if touched.is_empty() {
                    return (None, visits);
                }
                for &t in own.iter().filter(|&&t| common[t as usize]) {
                    for &o in touched.iter() {
                        if item_terms[o as usize].binary_search(&t).is_ok() {
                            scores[o as usize] += weights[t as usize];
                        }
                    }
                }
                let date = dates[idx];
                let cmp = |a: &u32, b: &u32| {
                    let (a, b) = (*a as usize, *b as usize);
                    scores[b]
                        .cmp(&scores[a])
                        .then_with(|| distance(date, dates[a]).cmp(&distance(date, dates[b])))
                        .then_with(|| items[a].frontmatter.title.cmp(&items[b].frontmatter.title))
                        .then_with(|| items[a].url.cmp(&items[b].url))
                };
                let mut ranked = touched.clone();
                if ranked.len() > limit {
                    ranked.select_nth_unstable_by(limit - 1, cmp);
                    ranked.truncate(limit);
                }
                ranked.sort_by(cmp);
                let related: Vec<RelatedItem> = ranked
                    .iter()
                    .map(|&o| {
                        let r = items[o as usize];
                        RelatedItem {
                            title: r.frontmatter.title.clone(),
                            url: r.url.clone(),
                            date: r.frontmatter.date.map(|d| d.to_string()),
                            description: r.frontmatter.description.clone(),
                            score: scores[o as usize],
                        }
                    })
                    .collect();
                for &o in touched.iter() {
                    scores[o as usize] = 0;
                }
                touched.clear();
                (Some((items[idx].url.clone(), related)), visits)
            },
        )
        .collect();
    let visits = per_item.iter().map(|(_, v)| v).sum();
    (
        per_item.into_iter().filter_map(|(r, _)| r).collect(),
        visits,
    )
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
    fn test_related_common_terms_stay_linear() {
        let taxes = resolve_taxonomies(&config(""));
        let owned: Vec<ContentItem> = (0..5000)
            .map(|i| {
                let mut it = item(
                    &format!("P{i:05}"),
                    &format!("date: 2024-01-01\ntags: [blog, t{}]\n", i % 50),
                );
                it.url = format!("/posts/p{i:05}");
                it
            })
            .collect();
        let items: Vec<&ContentItem> = owned.iter().collect();
        let (out, visits) = related_within_counted(&items, &taxes, 3);
        // `blog` is on every page: skipped as a seed, so each page visits only
        // its t-term's 100 pages (quadratic would be 5000 * 5100).
        assert!(visits <= 5000 * 100, "visited {visits} postings");
        assert_eq!(out.len(), 5000);
        let first = &out.iter().find(|(u, _)| u == "/posts/p00000").unwrap().1;
        assert_eq!(first.len(), 3);
        assert!(
            first.iter().all(|r| r.score == 2),
            "t-term + common blog: {first:?}"
        );
        assert_eq!(first[0].title, "P00050", "same date → title order");
        // Deterministic regardless of input order.
        let rev: Vec<&ContentItem> = items.iter().rev().copied().collect();
        let mut a: Vec<_> = out;
        let mut b = related_within(&rev, &taxes, 3);
        a.sort_by(|x, y| x.0.cmp(&y.0));
        b.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(a, b);
    }

    #[test]
    fn test_related_small_groups_keep_common_terms() {
        // Under 200 pages, a tag on every page still relates them.
        let taxes = resolve_taxonomies(&config(""));
        let a = item("A", "tags: [blog]\n");
        let b = item("B", "tags: [blog]\n");
        let out = related_within(&[&a, &b], &taxes, 3);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_compute_related_excludes_drafts_index_and_honors_limits() {
        let mut cfg = config("[build]\nrelated = 1\n");
        let a = item("A", "tags: [t]\n");
        let b = item("B", "tags: [t]\n");
        let draft = item("Draft", "tags: [t]\ndraft: true\n");
        let mut index = item("Index", "tags: [t]\n");
        index.slug = "index".into();
        let mut all = indexmap::IndexMap::new();
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
