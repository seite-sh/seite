//! `seite_content_stats` — content health overview per collection.

use std::collections::{BTreeMap, HashSet};

use super::{
    input_object, object_schema, prop, resolve_collection, string_array, Args, ServerState,
    ToolResult,
};
use crate::config;
use crate::mcp::content_index;

/// Maximum paths listed per category and collection (counts are exact).
const LIST_CAP: usize = 50;

pub fn input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
            "collection": prop("string", "Only report this collection (singular aliases like 'post' work)")
        }),
        &[],
    )
}

pub fn output_schema() -> serde_json::Value {
    let path_error = object_schema(
        serde_json::json!({ "path": { "type": "string" }, "error": { "type": "string" } }),
        &["path", "error"],
    );
    let path_date = object_schema(
        serde_json::json!({ "path": { "type": "string" }, "date": { "type": "string" } }),
        &["path", "date"],
    );
    let collection = object_schema(
        serde_json::json!({
            "name": { "type": "string" },
            "total": prop("integer", "Parsed items, drafts included"),
            "drafts": { "type": "integer" },
            "missing_description": string_array(),
            "missing_tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Dated collections only (tags drive tag pages and feeds)"
            },
            "future_dated": { "type": "array", "items": path_date },
            "expired": {
                "type": "array",
                "items": path_date,
                "description": "Past their `expires:` date (left out of builds when [build] schedule = true); `date` is the expiry date"
            },
            "parse_errors": { "type": "array", "items": path_error },
            "missing_translations": {
                "type": "object",
                "description": "Language code -> default-language source paths without that translation (multilingual sites)",
                "additionalProperties": string_array()
            },
            "truncated": prop("boolean", "A list was capped; the counts in `totals` are exact")
        }),
        &[
            "name",
            "total",
            "drafts",
            "missing_description",
            "missing_tags",
            "future_dated",
            "expired",
            "parse_errors",
            "missing_translations",
            "truncated",
        ],
    );
    let count = || serde_json::json!({ "type": "integer" });
    let (taxonomies, series) = content_model_schema();
    object_schema(
        serde_json::json!({
            "taxonomies": taxonomies,
            "series": series,
            "today": { "type": "string" },
            "today": prop("string", "Today's date in the site's time zone ([site] timezone, else the local date)"),
            "next_scheduled": object_schema(
                serde_json::json!({
                    "path": { "type": "string" },
                    "collection": { "type": "string" },
                    "title": { "type": "string" },
                    "date": { "type": "string" }
                }),
                &["path", "collection", "title", "date"],
            ),
            "languages": string_array(),
            "collections": { "type": "array", "items": collection },
            "totals": object_schema(
                serde_json::json!({
                    "items": count(),
                    "drafts": count(),
                    "missing_description": count(),
                    "missing_tags": count(),
                    "future_dated": count(),
                    "expired": count(),
                    "parse_errors": count(),
                    "missing_translations": count()
                }),
                &[
                    "items",
                    "drafts",
                    "missing_description",
                    "missing_tags",
                    "future_dated",
                    "expired",
                    "parse_errors",
                    "missing_translations",
                ],
            )
        }),
        &["today", "languages", "collections", "totals"],
    )
}

/// Schema of the add-only `taxonomies` / `series` summaries.
fn content_model_schema() -> (serde_json::Value, serde_json::Value) {
    let term = object_schema(
        serde_json::json!({
            "name": { "type": "string" },
            "slug": { "type": "string" },
            "count": { "type": "integer" }
        }),
        &["name", "slug", "count"],
    );
    let taxonomies = serde_json::json!({
        "type": "array",
        "description": "Terms per taxonomy (tags plus [[taxonomies]]), counted over non-draft items, most used first",
        "items": object_schema(
            serde_json::json!({
                "name": { "type": "string" },
                "label": { "type": "string" },
                "url_prefix": { "type": "string" },
                "terms": { "type": "array", "items": term }
            }),
            &["name", "label", "url_prefix", "terms"],
        )
    });
    let series = serde_json::json!({
        "type": "array",
        "description": "Series used by non-draft items (`series:` frontmatter) and their part counts",
        "items": object_schema(
            serde_json::json!({
                "name": { "type": "string" },
                "slug": { "type": "string" },
                "parts": { "type": "integer" }
            }),
            &["name", "slug", "parts"],
        )
    });
    (taxonomies, series)
}

/// Accumulates term and series usage across the scanned items.
#[derive(Default)]
struct ModelCounts {
    /// (taxonomy index, slug) → (first spelling, count)
    terms: BTreeMap<(usize, String), (String, usize)>,
    /// slug → (first spelling, parts)
    series: BTreeMap<String, (String, usize)>,
}

impl ModelCounts {
    fn add(
        &mut self,
        taxes: &[crate::build::taxonomy::Taxonomy],
        fm: &crate::content::Frontmatter,
    ) {
        use crate::build::taxonomy::term_slug;
        for (ti, tax) in taxes.iter().enumerate() {
            let mut seen = HashSet::new();
            for term in fm.taxonomy_terms(&tax.name) {
                let slug = term_slug(&term);
                if slug.is_empty() || !seen.insert(slug.clone()) {
                    continue;
                }
                self.terms.entry((ti, slug)).or_insert((term, 0)).1 += 1;
            }
        }
        if let Some(name) = fm.series.as_deref() {
            let slug = term_slug(name);
            if !slug.is_empty() {
                self.series
                    .entry(slug)
                    .or_insert((name.trim().to_string(), 0))
                    .1 += 1;
            }
        }
    }

    fn to_json(
        &self,
        taxes: &[crate::build::taxonomy::Taxonomy],
    ) -> (serde_json::Value, serde_json::Value) {
        let taxonomies: Vec<serde_json::Value> = taxes
            .iter()
            .enumerate()
            .map(|(ti, tax)| {
                let mut terms: Vec<(&String, &String, usize)> = self
                    .terms
                    .iter()
                    .filter(|((i, _), _)| *i == ti)
                    .map(|((_, slug), (name, n))| (slug, name, *n))
                    .collect();
                terms.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(b.0)));
                serde_json::json!({
                    "name": tax.name,
                    "label": tax.label,
                    "url_prefix": tax.url_prefix,
                    "terms": terms
                        .iter()
                        .map(|(slug, name, count)| serde_json::json!({ "name": name, "slug": slug, "count": count }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        let series: Vec<serde_json::Value> = self
            .series
            .iter()
            .map(|(slug, (name, parts))| serde_json::json!({ "name": name, "slug": slug, "parts": parts }))
            .collect();
        (serde_json::json!(taxonomies), serde_json::json!(series))
    }
}

#[derive(Default)]
struct Totals {
    items: usize,
    drafts: usize,
    missing_description: usize,
    missing_tags: usize,
    future_dated: usize,
    expired: usize,
    parse_errors: usize,
    missing_translations: usize,
}

fn capped<T: Clone>(items: &[T], truncated: &mut bool) -> Vec<T> {
    if items.len() > LIST_CAP {
        *truncated = true;
    }
    items.iter().take(LIST_CAP).cloned().collect()
}

pub fn call_content_stats(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    let args = Args::new("seite_content_stats", arguments, &["collection"])?;
    let filter = args.str("collection")?;
    let (config, paths) = state.site()?;
    let collections: Vec<&config::CollectionConfig> = match filter {
        Some(name) => vec![resolve_collection(config, name)?],
        None => config.collections.iter().collect(),
    };

    let today = crate::content::schedule::today(config);
    // Earliest future-dated, non-draft item: (date, path, collection, title).
    let mut next: Option<(chrono::NaiveDate, String, String, String)> = None;
    let default_lang = config.site.language.as_str();
    let other_langs: Vec<String> = config
        .all_languages()
        .into_iter()
        .filter(|l| l != default_lang)
        .collect();

    let mut totals = Totals::default();
    let taxes = crate::build::taxonomy::resolve_taxonomies(config);
    let mut model = ModelCounts::default();
    let mut out = Vec::new();
    for collection in collections {
        let items = content_index::scan_collection(config, paths, collection);
        let mut total = 0;
        let mut drafts = 0;
        let mut missing_description = Vec::new();
        let mut missing_tags = Vec::new();
        let mut future_dated = Vec::new();
        let mut expired = Vec::new();
        let mut parse_errors = Vec::new();
        let mut present: HashSet<(String, String)> = HashSet::new();
        let mut defaults: Vec<(String, String)> = Vec::new(); // (slug, path)

        for item in &items {
            let parsed = match &item.parsed {
                Ok(p) => p,
                Err(e) => {
                    parse_errors.push(serde_json::json!({ "path": item.path, "error": e }));
                    continue;
                }
            };
            total += 1;
            let fm = &parsed.frontmatter;
            if fm.draft {
                drafts += 1;
            } else {
                model.add(&taxes, fm);
            }
            if fm
                .description
                .as_deref()
                .is_none_or(|d| d.trim().is_empty())
            {
                missing_description.push(item.path.clone());
            }
            if collection.has_date && fm.tags.is_empty() {
                missing_tags.push(item.path.clone());
            }
            // `expired` and `next_scheduled` follow `[build] schedule`
            // (dated collections only); `future_dated` is reported always.
            let scheduling = config.build.schedule && collection.has_date;
            if let Some(date) = parsed.date.filter(|d| *d > today) {
                future_dated
                    .push(serde_json::json!({ "path": item.path, "date": date.to_string() }));
                if scheduling && !fm.draft && next.as_ref().is_none_or(|(d, ..)| date < *d) {
                    next = Some((
                        date,
                        item.path.clone(),
                        collection.name.clone(),
                        fm.title.clone(),
                    ));
                }
            }
            if let Some(date) = fm.expires.filter(|e| scheduling && *e < today) {
                expired.push(serde_json::json!({ "path": item.path, "date": date.to_string() }));
            }
            present.insert((parsed.lang.clone(), parsed.slug.clone()));
            if parsed.lang == default_lang {
                defaults.push((parsed.slug.clone(), item.path.clone()));
            }
        }

        let mut missing_translations: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for lang in &other_langs {
            let missing: Vec<String> = defaults
                .iter()
                .filter(|(slug, _)| !present.contains(&(lang.clone(), slug.clone())))
                .map(|(_, path)| path.clone())
                .collect();
            totals.missing_translations += missing.len();
            missing_translations.insert(lang.clone(), missing);
        }

        totals.items += total;
        totals.drafts += drafts;
        totals.missing_description += missing_description.len();
        totals.missing_tags += missing_tags.len();
        totals.future_dated += future_dated.len();
        totals.expired += expired.len();
        totals.parse_errors += parse_errors.len();

        let mut truncated = false;
        let missing_translations: serde_json::Map<String, serde_json::Value> = missing_translations
            .iter()
            .map(|(lang, paths)| {
                (
                    lang.clone(),
                    serde_json::json!(capped(paths, &mut truncated)),
                )
            })
            .collect();
        out.push(serde_json::json!({
            "name": collection.name,
            "total": total,
            "drafts": drafts,
            "missing_description": capped(&missing_description, &mut truncated),
            "missing_tags": capped(&missing_tags, &mut truncated),
            "future_dated": capped(&future_dated, &mut truncated),
            "expired": capped(&expired, &mut truncated),
            "parse_errors": capped(&parse_errors, &mut truncated),
            "missing_translations": missing_translations,
            "truncated": truncated,
        }));
    }

    let (taxonomies, series) = model.to_json(&taxes);
    let mut result = serde_json::json!({
        "today": today.to_string(),
        "languages": config.all_languages(),
        "collections": out,
        "taxonomies": taxonomies,
        "series": series,
        "totals": {
            "items": totals.items,
            "drafts": totals.drafts,
            "missing_description": totals.missing_description,
            "missing_tags": totals.missing_tags,
            "future_dated": totals.future_dated,
            "expired": totals.expired,
            "parse_errors": totals.parse_errors,
            "missing_translations": totals.missing_translations,
        },
    });
    if let Some((date, path, collection, title)) = next {
        result["next_scheduled"] = serde_json::json!({
            "path": path,
            "collection": collection,
            "title": title,
            "date": date.to_string(),
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;

    #[test]
    fn test_content_stats_reports_gaps() {
        let (tmp, mut state) = site("\n[languages.es]\ntitle = \"Prueba\"\n");
        let root = tmp.path();
        write(
            root,
            "content/posts/2026-01-01-complete.md",
            "---\ntitle: Complete\ndescription: yes\ntags: [a]\n---\nx\n",
        );
        write(
            root,
            "content/posts/2026-01-01-complete.es.md",
            "---\ntitle: Completo\ndescription: si\ntags: [a]\n---\nx\n",
        );
        write(
            root,
            "content/posts/2999-01-01-future.md",
            "---\ntitle: Future\ndraft: true\n---\nx\n",
        );
        write(
            root,
            "content/docs/guides/intro.md",
            "---\ntitle: Intro\n---\nx\n",
        );
        write(root, "content/docs/bad.md", "---\ntitle: [unclosed\n---\n");

        let out = call_ok(&mut state, "seite_content_stats", serde_json::json!({}));
        assert_eq!(out["languages"], serde_json::json!(["en", "es"]));
        let posts = &out["collections"][0];
        assert_eq!(posts["name"], "posts");
        assert_eq!(posts["total"], 3);
        assert_eq!(posts["drafts"], 1);
        assert_eq!(
            posts["missing_description"],
            serde_json::json!(["content/posts/2999-01-01-future.md"])
        );
        assert_eq!(
            posts["missing_tags"],
            serde_json::json!(["content/posts/2999-01-01-future.md"])
        );
        assert_eq!(posts["future_dated"][0]["date"], "2999-01-01");
        assert_eq!(
            posts["missing_translations"]["es"],
            serde_json::json!(["content/posts/2999-01-01-future.md"])
        );
        let docs = &out["collections"][1];
        assert_eq!(docs["total"], 1);
        assert_eq!(docs["parse_errors"][0]["path"], "content/docs/bad.md");
        // Undated collections are not flagged for tags.
        assert_eq!(docs["missing_tags"], serde_json::json!([]));
        assert_eq!(out["totals"]["items"], 4);
        assert_eq!(out["totals"]["missing_translations"], 2);
        assert_eq!(out["totals"]["parse_errors"], 1);

        let only = call_ok(
            &mut state,
            "seite_content_stats",
            serde_json::json!({ "collection": "doc" }),
        );
        assert_eq!(only["collections"].as_array().unwrap().len(), 1);
        assert_eq!(only["collections"][0]["name"], "docs");
    }

    #[test]
    fn test_content_stats_invalid_args() {
        let (_tmp, mut state) = site("");
        let err = call_err(
            &mut state,
            "seite_content_stats",
            serde_json::json!({ "collection": "nope" }),
        );
        assert!(err.contains("Unknown collection 'nope'"), "{err}");
        let err = call_err(
            &mut state,
            "seite_content_stats",
            serde_json::json!({ "collection": 3 }),
        );
        assert!(err.contains("must be a string"), "{err}");
        let out = call_ok(&mut state, "seite_content_stats", serde_json::json!({}));
        // Monolingual site: no translation keys.
        assert_eq!(
            out["collections"][0]["missing_translations"],
            serde_json::json!({})
        );
    }
}
