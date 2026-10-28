//! Ghost JSON export (Settings → Labs/Migrate → Export content).
//!
//! Shapes: `{"db":[{"meta":…,"data":{…}}]}` (current) or `{"data":{…}}`
//! (older). Uses `posts`, `tags`, `posts_tags`, `users`, `posts_authors`,
//! `posts_meta` and `settings`. Bodies come from `html` (Ghost cards are
//! HTML there), else `plaintext`. Modern exports write the site URL as
//! `__GHOST_URL__`: it is replaced with the `url` setting or `--site-url`,
//! else with nothing (root-relative paths).

use std::collections::HashMap;
use std::path::Path;

use anyhow::bail;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;

use super::convert::{self, Tokens};
use super::{json_str, Item, Kind, ParseCtx, Parsed, Skipped, Warning};

const GHOST_URL: &str = "__GHOST_URL__";

pub(crate) fn parse(path: &Path, ctx: &ParseCtx) -> anyhow::Result<Parsed> {
    let text = super::read_export(path, &ctx.display)?;
    let root: Value = serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!(
            "{}:{}:{}: invalid JSON: {e}",
            ctx.display,
            e.line(),
            e.column()
        )
    })?;
    let data = root
        .get("db")
        .and_then(|db| db.get(0))
        .and_then(|db| db.get("data"))
        .or_else(|| root.get("data"))
        .filter(|d| d.get("posts").is_some_and(Value::is_array));
    let Some(data) = data else {
        bail!(
            "{} is not a Ghost export: expected {{\"db\":[{{\"data\":{{\"posts\":[…]}}}}]}} \
             (export it from Ghost Admin → Settings → Migrate → Export content)",
            ctx.display
        );
    };
    let list = |key: &str| {
        data.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };

    let settings: HashMap<String, Value> = list("settings")
        .into_iter()
        .filter_map(|s| Some((s.get("key")?.as_str()?.to_string(), s.get("value")?.clone())))
        .collect();
    let site_url = ctx.site_url.clone().or_else(|| {
        settings
            .get("url")
            .and_then(Value::as_str)
            .and_then(|u| url::Url::parse(u).ok())
            .filter(|u| matches!(u.scheme(), "http" | "https"))
    });
    let base = site_url
        .as_ref()
        .map(|u| u.as_str().trim_end_matches('/').to_string())
        .unwrap_or_default();

    let tags: HashMap<String, Value> = list("tags")
        .into_iter()
        .filter_map(|t| Some((id_of(&t)?, t)))
        .collect();
    let users: HashMap<String, String> = list("users")
        .into_iter()
        .filter_map(|u| Some((id_of(&u)?, json_str(u.get("name"))?)))
        .collect();
    let mut post_tags: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    for pt in list("posts_tags") {
        if let (Some(post), Some(tag)) = (str_of(pt.get("post_id")), str_of(pt.get("tag_id"))) {
            let order = pt.get("sort_order").and_then(Value::as_i64).unwrap_or(0);
            post_tags.entry(post).or_default().push((order, tag));
        }
    }
    let mut post_authors: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    for pa in list("posts_authors") {
        if let (Some(post), Some(author)) = (str_of(pa.get("post_id")), str_of(pa.get("author_id")))
        {
            let order = pa.get("sort_order").and_then(Value::as_i64).unwrap_or(0);
            post_authors.entry(post).or_default().push((order, author));
        }
    }
    let meta: HashMap<String, Value> = list("posts_meta")
        .into_iter()
        .filter_map(|m| Some((str_of(m.get("post_id"))?, m)))
        .collect();

    let mut parsed = Parsed {
        site_url: site_url.clone(),
        ..Default::default()
    };
    let mut needs_url = false;
    for (index, post) in list("posts").iter().enumerate() {
        let id = id_of(post).unwrap_or_else(|| index.to_string());
        let source_id = format!("ghost:{id}");
        let source = format!("{}#posts[{index}]", ctx.display);
        let title = json_str(post.get("title")).unwrap_or_else(|| "Untitled".to_string());
        let is_page = post.get("type").and_then(Value::as_str) == Some("page")
            || matches!(post.get("page"), Some(Value::Bool(true)))
            || post.get("page").and_then(Value::as_i64) == Some(1);
        let kind = if is_page { Kind::Page } else { Kind::Post };
        let status = post
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("draft");
        let visibility = post
            .get("visibility")
            .and_then(Value::as_str)
            .unwrap_or("public");
        let mut draft = match status {
            "published" => None,
            "scheduled" => Some("scheduled".to_string()),
            "draft" => Some("draft".to_string()),
            "sent" => Some("newsletter-only (sent)".to_string()),
            other => {
                parsed.skipped.push(Skipped {
                    source_id,
                    source,
                    kind: Some(kind),
                    title,
                    reason: format!("status '{other}' is not imported"),
                });
                continue;
            }
        };
        if draft.is_none() && visibility != "public" {
            draft = Some(format!("{visibility}-only in Ghost"));
        }

        let replace_url = |s: &str| s.replace(GHOST_URL, &base);
        let mut item = Item {
            source_id: source_id.clone(),
            source_ref: source.clone(),
            kind: Some(kind),
            title,
            slug: json_str(post.get("slug")).unwrap_or_default(),
            draft,
            ..Default::default()
        };
        let published =
            instant(post.get("published_at")).or_else(|| instant(post.get("created_at")));
        item.date = published.map(|t| crate::content::schedule::today_at(ctx.config, t));
        item.updated = instant(post.get("updated_at"))
            .map(|t| crate::content::schedule::today_at(ctx.config, t))
            .filter(|u| Some(*u) > item.date);
        let post_meta = meta.get(&id);
        item.description = json_str(post.get("custom_excerpt"))
            .or_else(|| json_str(post.get("meta_description")))
            .or_else(|| post_meta.and_then(|m| json_str(m.get("meta_description"))));
        item.image = json_str(post.get("feature_image")).map(|u| replace_url(&u));
        if item.image.as_deref().is_some_and(|u| u.contains(GHOST_URL))
            || post
                .get("html")
                .and_then(Value::as_str)
                .is_some_and(|h| h.contains(GHOST_URL))
        {
            needs_url |= base.is_empty();
        }

        let mut tag_ids = post_tags.remove(&id).unwrap_or_default();
        tag_ids.sort();
        for (_, tag_id) in tag_ids {
            let Some(tag) = tags.get(&tag_id) else {
                continue;
            };
            let name = json_str(tag.get("name")).unwrap_or_default();
            let internal = tag.get("visibility").and_then(Value::as_str) == Some("internal")
                || name.starts_with('#');
            if !internal
                && !name.is_empty()
                && !item.tags.iter().any(|t| t.eq_ignore_ascii_case(&name))
            {
                item.tags.push(name);
            }
        }
        let mut author_ids = post_authors.remove(&id).unwrap_or_default();
        author_ids.sort();
        item.author = author_ids
            .first()
            .map(|(_, a)| a.clone())
            .or_else(|| str_of(post.get("author_id")))
            .and_then(|a| users.get(&a).cloned());

        // Old URLs: Ghost's default `/{slug}/`, and a same-site canonical.
        if !item.slug.is_empty() {
            item.aliases.push(format!("/{}/", item.slug));
        }
        if let Some(canonical) = json_str(post.get("canonical_url"))
            .or_else(|| post_meta.and_then(|m| json_str(m.get("canonical_url"))))
        {
            let canonical = replace_url(&canonical);
            match url::Url::parse(&canonical) {
                Ok(u) if site_url.as_ref().is_some_and(|s| s.host_str() == u.host_str() && s.port() == u.port()) => {
                    item.aliases.push(u.path().to_string());
                }
                Ok(u) => parsed.warnings.push(Warning::for_item(
                    &item,
                    None,
                    "canonical-elsewhere",
                    format!(
                        "canonical URL points to {} (a cross-post?); add `<link rel=\"canonical\">` via a template override if it should stay",
                        u
                    ),
                )),
                Err(_) if canonical.starts_with('/') => item.aliases.push(canonical),
                Err(_) => {}
            }
        }
        if draft_is_members(&item) {
            parsed.warnings.push(Warning::for_item(
                &item,
                None,
                "members-only",
                "members-only in Ghost: seite has no paywall; it is public once you remove `draft: true`",
            ));
        }

        let html = post.get("html").and_then(Value::as_str).map(replace_url);
        item.body = match html.filter(|h| !h.trim().is_empty()) {
            Some(html) => {
                let mut tokens = Tokens::default();
                let prepared = convert::prepare_html(&html, &mut tokens);
                let md =
                    convert::neutralize_shortcode_delimiters(&convert::html_to_markdown(&prepared));
                tokens.restore(&md)
            }
            None => match json_str(post.get("plaintext")) {
                Some(text) => convert::neutralize_shortcode_delimiters(&text),
                None => {
                    parsed.warnings.push(Warning::for_item(
                        &item,
                        None,
                        "empty-body",
                        "no `html` or `plaintext` in the export (mobiledoc/lexical only?): the body is empty",
                    ));
                    String::new()
                }
            },
        };
        parsed.items.push(item);
    }
    if needs_url {
        parsed.warnings.push(Warning::new(
            ctx.display.clone(),
            "ghost-url",
            "the export writes the site URL as __GHOST_URL__ and names no `url` setting: links became \
             root-relative and /content/images/… were not downloaded (re-run with --site-url https://your-ghost-site)",
        ));
    }
    Ok(parsed)
}

fn draft_is_members(item: &Item) -> bool {
    item.draft
        .as_deref()
        .is_some_and(|d| d.ends_with("-only in Ghost"))
}

fn id_of(v: &Value) -> Option<String> {
    str_of(v.get("id"))
}

fn str_of(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// A Ghost timestamp: RFC 3339 (`2020-01-02T10:00:00.000Z`), `YYYY-MM-DD
/// HH:MM:SS` (UTC, older exports) or epoch milliseconds.
fn instant(v: Option<&Value>) -> Option<DateTime<Utc>> {
    match v? {
        Value::String(s) => DateTime::parse_from_rfc3339(s.trim())
            .map(|d| d.with_timezone(&Utc))
            .ok()
            .or_else(|| {
                NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S")
                    .ok()
                    .map(|n| n.and_utc())
            }),
        Value::Number(n) => n.as_i64().and_then(DateTime::from_timestamp_millis),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instant_formats() {
        let d = |v: Value| instant(Some(&v)).map(|t| t.date_naive().to_string());
        assert_eq!(
            d(Value::from("2020-01-02T23:30:00.000Z")).as_deref(),
            Some("2020-01-02")
        );
        assert_eq!(
            d(Value::from("2020-01-02 10:00:00")).as_deref(),
            Some("2020-01-02")
        );
        assert_eq!(
            d(Value::from(1_577_960_000_000_i64)).as_deref(),
            Some("2020-01-02")
        );
        assert_eq!(d(Value::from("nope")), None);
    }
}
