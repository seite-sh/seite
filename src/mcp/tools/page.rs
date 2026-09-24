//! `seite_get_page` and `seite_update_frontmatter` — one content file, as the
//! build sees it.

use std::path::{Path, PathBuf};

use super::{
    input_object, nullable, object_schema, prop, string_array, Args, ServerState, ToolError,
    ToolResult,
};
use crate::build::{self, markdown, math};
use crate::config::{CollectionConfig, ResolvedPaths, SiteConfig};
use crate::content::{self, Frontmatter};
use crate::mcp::content_index;

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

pub fn get_page_input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
            "path": prop(
                "string",
                "Source file, relative to the site root (content/docs/guides/intro.md) or to the content directory (docs/guides/intro.md). Pass this or url."
            ),
            "url": prop(
                "string",
                "Published URL path (/docs/guides/intro); a full URL on the site's base_url also works. Pass this or path."
            )
        }),
        &[],
    )
}

pub fn get_page_output_schema() -> serde_json::Value {
    object_schema(
        serde_json::json!({
            "path": prop("string", "Source path relative to the site root"),
            "collection": { "type": "string" },
            "url": prop("string", "Published URL path"),
            "slug": { "type": "string" },
            "lang": { "type": "string" },
            "draft": { "type": "boolean" },
            "date": nullable("string"),
            "frontmatter": {
                "type": "object",
                "description": "Frontmatter with defaults resolved (e.g. date from a YYYY-MM-DD- filename prefix)",
                "additionalProperties": true
            },
            "word_count": { "type": "integer" },
            "reading_time": prop("integer", "Estimated minutes (238 wpm)"),
            "body": prop("string", "Markdown body (after the frontmatter)"),
            "html": {
                "type": ["string", "null"],
                "description": "Rendered body HTML (shortcodes + markdown, no page template); null if rendering failed"
            },
            "render_error": prop("string", "Why rendering failed (e.g. unknown shortcode)"),
            "output_path": {
                "type": ["string", "null"],
                "description": "Built HTML file relative to the site root, if it exists"
            }
        }),
        &[
            "path",
            "collection",
            "url",
            "slug",
            "lang",
            "draft",
            "date",
            "frontmatter",
            "word_count",
            "reading_time",
            "body",
            "html",
            "output_path",
        ],
    )
}

pub fn update_frontmatter_input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
            "path": prop(
                "string",
                "Content file (.md) relative to the site root or the content directory, e.g. content/posts/2026-01-02-hello.md"
            ),
            "set": {
                "type": "object",
                "description": "Top-level frontmatter keys to add or replace, e.g. {\"description\": \"...\", \"tags\": [\"a\"], \"weight\": 2, \"extra\": {...}}",
                "additionalProperties": true
            },
            "unset": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Top-level frontmatter keys to remove ('title' cannot be removed)"
            }
        }),
        &["path"],
    )
}

pub fn update_frontmatter_output_schema() -> serde_json::Value {
    object_schema(
        serde_json::json!({
            "path": { "type": "string" },
            "collection": { "type": "string" },
            "url": { "type": "string" },
            "changed": prop("boolean", "false when the file already had these values (nothing written)"),
            "frontmatter": {
                "type": "object",
                "description": "The file's frontmatter after the update",
                "additionalProperties": true
            },
            "notes": string_array()
        }),
        &["path", "collection", "url", "changed", "frontmatter"],
    )
}

// ---------------------------------------------------------------------------
// Resolving a content file
// ---------------------------------------------------------------------------

/// A content file inside a collection, with canonical paths.
struct ContentFile<'c> {
    /// Canonical absolute path.
    abs: PathBuf,
    /// Path relative to the site root, `/`-separated.
    rel: String,
    /// Path relative to the collection directory.
    rel_to_collection: PathBuf,
    collection: &'c CollectionConfig,
}

fn canonical(path: &Path) -> Result<PathBuf, ToolError> {
    path.canonicalize()
        .map_err(|e| ToolError::new(format!("cannot resolve {}: {e}", path.display())))
}

/// Resolve a user-supplied path to a `.md` file inside a collection of the
/// content directory. Rejects anything that resolves outside it (`..`,
/// absolute paths, symlinks).
fn resolve_content_file<'c>(
    tool: &str,
    config: &'c SiteConfig,
    paths: &ResolvedPaths,
    raw: &str,
) -> Result<ContentFile<'c>, ToolError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(ToolError::new(format!("{tool}: 'path' must not be empty")));
    }
    let root = canonical(&paths.root)?;
    let content_dir = canonical(&paths.content).map_err(|_| {
        ToolError::new(format!(
            "{tool}: content directory {} does not exist",
            paths.content.display()
        ))
    })?;
    let given = Path::new(raw);
    let candidates: Vec<PathBuf> = if given.is_absolute() {
        vec![given.to_path_buf()]
    } else {
        vec![paths.root.join(given), paths.content.join(given)]
    };
    let found = candidates.iter().find(|p| p.is_file()).ok_or_else(|| {
        ToolError::new(format!(
            "{tool}: no content file at '{raw}'. Pass a path relative to the site root \
             (e.g. content/posts/2026-01-02-hello.md); seite_search finds files by title."
        ))
    })?;
    let abs = canonical(found)?;
    if !abs.starts_with(&content_dir) {
        return Err(ToolError::new(format!(
            "{tool}: '{raw}' is outside the content directory ({}); only content files can be used",
            content_index::relative_path(&root, &content_dir)
        )));
    }
    if abs.extension().is_none_or(|e| e != "md") {
        return Err(ToolError::new(format!(
            "{tool}: '{raw}' is not a markdown (.md) content file"
        )));
    }
    let collection = config
        .collections
        .iter()
        .filter(|c| abs.starts_with(content_dir.join(&c.directory)))
        .max_by_key(|c| c.directory.len())
        .ok_or_else(|| {
            ToolError::new(format!(
                "{tool}: '{raw}' is not inside any collection directory ({}). \
                 Only files in a configured collection are built.",
                config
                    .collections
                    .iter()
                    .map(|c| format!("content/{}", c.directory))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
    let rel_to_collection = abs
        .strip_prefix(content_dir.join(&collection.directory))
        .unwrap_or(&abs)
        .to_path_buf();
    Ok(ContentFile {
        rel: content_index::relative_path(&root, &abs),
        abs,
        rel_to_collection,
        collection,
    })
}

/// Normalize a URL or path for comparison: no origin, query, fragment,
/// trailing slash, or `.html`/`.md` suffix; always starts with `/`.
fn normalize_url(config: &SiteConfig, raw: &str) -> String {
    let mut url = raw.trim();
    let base = config.site.base_url.trim_end_matches('/');
    if let Some(rest) = url.strip_prefix(base) {
        url = rest;
    } else if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    {
        url = rest.find('/').map_or("", |i| &rest[i..]);
    }
    let url = url.split(['?', '#']).next().unwrap_or("");
    let url = url
        .strip_suffix(".html")
        .or_else(|| url.strip_suffix(".md"))
        .unwrap_or(url);
    let url = url.trim_end_matches('/');
    let url = url.strip_suffix("/index").unwrap_or(url);
    if url.starts_with('/') {
        url.to_string()
    } else {
        format!("/{url}")
    }
}

/// Collection used for URL resolution: subdomain collections are built as
/// root-mounted sites, so their URLs have no prefix (as in content_index).
fn url_collection(collection: &CollectionConfig) -> CollectionConfig {
    if collection.subdomain.is_some() {
        CollectionConfig {
            url_prefix: String::new(),
            ..collection.clone()
        }
    } else {
        collection.clone()
    }
}

fn find_by_url(config: &SiteConfig, paths: &ResolvedPaths, raw: &str) -> Result<String, ToolError> {
    let wanted = normalize_url(config, raw);
    for collection in &config.collections {
        for item in content_index::scan_collection(config, paths, collection) {
            if let Ok(parsed) = &item.parsed {
                if normalize_url(config, &parsed.url) == wanted {
                    return Ok(item.path);
                }
            }
        }
    }
    Err(ToolError::new(format!(
        "seite_get_page: no content file is published at '{wanted}'. \
         Read seite://content/{{collection}} for the URLs that exist, or pass 'path'."
    )))
}

// ---------------------------------------------------------------------------
// seite_get_page
// ---------------------------------------------------------------------------

/// Render a body exactly like the build's content step (shortcodes, then
/// math, then markdown) — without the page template or HTML post-processing.
/// Keep in sync with the "Process each collection" step of `build_site_inner`.
fn render_body(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    file: &ContentFile,
    fm: &Frontmatter,
    body: &str,
    slug: &str,
    lang: &str,
) -> Result<String, String> {
    let registry = crate::shortcodes::ShortcodeRegistry::new(&paths.templates.join("shortcodes"))
        .map_err(|e| e.to_string())?;
    let data = crate::data::load_data_dir(&paths.data_dir).map_err(|e| e.to_string())?;
    let t = build::ui_strings_for_lang(lang, &data);
    let sc_site = serde_json::json!({
        "title": &config.site.title,
        "base_url": &config.site.base_url,
        "language": &config.site.language,
        "contact": config.contact.as_ref().map(|c| serde_json::json!({
            "provider": serde_json::to_value(&c.provider).unwrap_or_default(),
            "endpoint": &c.endpoint,
            "region": &c.region,
            "redirect": &c.redirect,
            "subject": &c.subject,
        })),
    });
    let sc_page = serde_json::json!({
        "title": fm.title,
        "slug": slug,
        "collection": &file.collection.name,
        "tags": &fm.tags,
    });
    let expanded = registry
        .expand(body, &file.abs, &sc_page, &sc_site, &t)
        .map_err(|e| e.to_string())?;
    let input = if config.build.math {
        math::render_math(&expanded)
    } else {
        expanded
    };
    let (html, _toc) = markdown::markdown_to_html_with(&input, config.build.mermaid);
    Ok(html)
}

pub fn call_get_page(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    let args = Args::new("seite_get_page", arguments, &["path", "url"])?;
    let path_arg = args.str("path")?;
    let url_arg = args.str("url")?;
    let (config, paths) = state.site()?;

    let raw_path =
        match (path_arg, url_arg) {
            (Some(p), None) => p.to_string(),
            (None, Some(u)) => find_by_url(config, paths, u)?,
            _ => return Err(ToolError::new(
                "seite_get_page: pass exactly one of 'path' (source file) or 'url' (published URL)",
            )),
        };
    let file = resolve_content_file("seite_get_page", config, paths, &raw_path)?;
    let (fm, body) = content::parse_content_file(&file.abs)
        .map_err(|e| ToolError::new(format!("seite_get_page: {e}")))?;

    let loc = build::resolve_item_location(
        config,
        &url_collection(file.collection),
        &file.abs,
        &file.rel_to_collection,
        &fm,
    );
    let date = build::resolve_item_date(&fm, &file.abs, file.collection);
    let mut resolved = fm.clone();
    resolved.date = date;
    let frontmatter = serde_json::to_value(&resolved)
        .map_err(|e| ToolError::new(format!("seite_get_page: {e}")))?;

    let (html, render_error) =
        match render_body(config, paths, &file, &fm, &body, &loc.slug, &loc.lang) {
            Ok(html) => (Some(html), None),
            Err(e) => (None, Some(e)),
        };

    let output_dir = if file.collection.subdomain.is_some() {
        paths.subdomain_output(&file.collection.name)
    } else {
        paths.output.clone()
    };
    let output_file = output_dir.join(format!("{}.html", loc.url.trim_matches('/')));
    let output_path = output_file
        .is_file()
        .then(|| content_index::relative_path(&paths.root, &output_file));

    let word_count = body.split_whitespace().count();
    let reading_time = if word_count == 0 {
        0
    } else {
        (word_count / 238).max(1)
    };

    let mut response = serde_json::json!({
        "path": file.rel,
        "collection": file.collection.name,
        "url": loc.url,
        "slug": loc.slug,
        "lang": loc.lang,
        "draft": fm.draft,
        "date": date.map(|d| d.to_string()),
        "frontmatter": frontmatter,
        "word_count": word_count,
        "reading_time": reading_time,
        "body": body,
        "html": html,
        "output_path": output_path,
    });
    if let Some(err) = render_error {
        response["render_error"] = err.into();
    }
    Ok(response)
}

// ---------------------------------------------------------------------------
// seite_update_frontmatter
// ---------------------------------------------------------------------------

/// Byte ranges of a file's frontmatter: `(prefix_end, yaml, rest_start)` where
/// `raw[..prefix_end]` is leading whitespace, `yaml` is the text between the
/// delimiters, and `raw[rest_start..]` is everything after the closing `---`
/// (its line break and the body), which is preserved byte-for-byte.
/// Mirrors the delimiter rules of `content::parse_content_file`.
fn frontmatter_span(raw: &str) -> Option<(usize, &str, usize)> {
    let trimmed = raw.trim_start();
    let prefix_end = raw.len() - trimmed.len();
    let after_first = trimmed.strip_prefix("---")?;
    if !after_first.lines().next().unwrap_or("").trim().is_empty() {
        return None;
    }
    let base = prefix_end + 3;
    let mut offset = 0;
    while let Some(rel) = after_first[offset..].find("\n---") {
        let dash_start = offset + rel + 1;
        if after_first[dash_start + 3..]
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            return Some((
                prefix_end,
                &after_first[..dash_start],
                base + dash_start + 3,
            ));
        }
        offset = dash_start + 3;
    }
    None
}

fn yaml_key(key: &str) -> serde_yaml_ng::Value {
    serde_yaml_ng::Value::String(key.to_string())
}

pub fn call_update_frontmatter(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    const TOOL: &str = "seite_update_frontmatter";
    let args = Args::new(TOOL, arguments, &["path", "set", "unset"])?;
    let raw_path = args.req_str("path")?;
    let set = args.object("set")?;
    let unset = args.str_array("unset")?.unwrap_or_default();
    if set.is_none_or(|s| s.is_empty()) && unset.is_empty() {
        return Err(ToolError::new(format!(
            "{TOOL}: nothing to do — pass 'set' (keys to add/replace) and/or 'unset' (keys to remove)"
        )));
    }
    let set = set.cloned().unwrap_or_default();
    if unset.iter().any(|k| k == "title") {
        return Err(ToolError::new(format!(
            "{TOOL}: 'title' is required and cannot be unset (set a new title instead)"
        )));
    }
    if let Some(k) = unset.iter().find(|k| set.contains_key(k.as_str())) {
        return Err(ToolError::new(format!(
            "{TOOL}: key '{k}' is in both 'set' and 'unset'"
        )));
    }
    if let Some((k, _)) = set.iter().find(|(_, v)| v.is_null()) {
        return Err(ToolError::new(format!(
            "{TOOL}: set.{k} is null — use \"unset\": [\"{k}\"] to remove a key"
        )));
    }
    if let Some(title) = set.get("title") {
        if title.as_str().is_none_or(|t| t.trim().is_empty()) {
            return Err(ToolError::new(format!(
                "{TOOL}: set.title must be a non-empty string"
            )));
        }
    }

    let (config, paths) = state.site()?;
    let file = resolve_content_file(TOOL, config, paths, raw_path)?;
    let raw = std::fs::read_to_string(&file.abs)
        .map_err(|e| ToolError::new(format!("{TOOL}: cannot read {}: {e}", file.rel)))?;
    let (prefix_end, yaml, rest_start) = frontmatter_span(&raw).ok_or_else(|| {
        ToolError::new(format!(
            "{TOOL}: {} has no frontmatter (it must start with a `---` line and close with another `---` line)",
            file.rel
        ))
    })?;
    let original: serde_yaml_ng::Mapping = if yaml.trim().is_empty() {
        serde_yaml_ng::Mapping::new()
    } else {
        serde_yaml_ng::from_str(yaml).map_err(|e| {
            ToolError::new(format!(
                "{TOOL}: the existing frontmatter of {} is not a YAML mapping: {e}",
                file.rel
            ))
        })?
    };

    let mut map = original.clone();
    let mut notes = Vec::new();
    for key in &unset {
        if map.shift_remove(yaml_key(key)).is_none() {
            notes.push(format!("'{key}' was not set"));
        }
    }
    for (key, value) in &set {
        let yaml_value = serde_yaml_ng::to_value(value)
            .map_err(|e| ToolError::new(format!("{TOOL}: cannot store set.{key}: {e}")))?;
        map.insert(yaml_key(key), yaml_value);
    }

    // The result must still be valid frontmatter with its required fields.
    let fm: Frontmatter = serde_yaml_ng::from_value(serde_yaml_ng::Value::Mapping(map.clone()))
        .map_err(|e| {
            ToolError::new(format!(
                "{TOOL}: the updated frontmatter would be invalid: {e}. \
                 Dates use YYYY-MM-DD, tags/aliases are lists of strings, weight is an integer, draft is a boolean."
            ))
        })?;
    if fm.title.trim().is_empty() {
        return Err(ToolError::new(format!("{TOOL}: 'title' must not be empty")));
    }
    if file.collection.has_date
        && build::resolve_item_date(&fm, &file.abs, file.collection).is_none()
    {
        return Err(ToolError::new(format!(
            "{TOOL}: '{}' is a dated collection: keep a `date` (YYYY-MM-DD) or use a YYYY-MM-DD- filename prefix",
            file.collection.name
        )));
    }

    let changed = map != original;
    if changed {
        let new_yaml = serde_yaml_ng::to_string(&map)
            .map_err(|e| ToolError::new(format!("{TOOL}: cannot serialize frontmatter: {e}")))?;
        let updated = format!(
            "{}---\n{}---{}",
            &raw[..prefix_end],
            new_yaml,
            &raw[rest_start..]
        );
        std::fs::write(&file.abs, &updated)
            .map_err(|e| ToolError::new(format!("{TOOL}: cannot write {}: {e}", file.rel)))?;
        if let Err(e) = content::parse_content_file(&file.abs) {
            // Should be impossible; restore the original rather than leave a broken file.
            let _ = std::fs::write(&file.abs, &raw);
            return Err(ToolError::new(format!(
                "{TOOL}: the rewritten file did not parse ({e}); the original was restored"
            )));
        }
    }

    let loc = build::resolve_item_location(
        config,
        &url_collection(file.collection),
        &file.abs,
        &file.rel_to_collection,
        &fm,
    );
    let frontmatter = serde_json::to_value(serde_yaml_ng::Value::Mapping(map))
        .map_err(|e| ToolError::new(format!("{TOOL}: frontmatter cannot be shown as JSON: {e}")))?;
    let mut response = serde_json::json!({
        "path": file.rel,
        "collection": file.collection.name,
        "url": loc.url,
        "changed": changed,
        "frontmatter": frontmatter,
    });
    if !notes.is_empty() {
        response["notes"] = serde_json::json!(notes);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use std::fs;

    #[test]
    fn test_frontmatter_span_preserves_rest() {
        let raw = "\n---\ntitle: A\n---\r\n\nBody --- with dashes\n---\nmore\n";
        let (prefix, yaml, rest) = frontmatter_span(raw).unwrap();
        assert_eq!(&raw[..prefix], "\n");
        assert_eq!(yaml, "\ntitle: A\n");
        assert_eq!(&raw[rest..], "\r\n\nBody --- with dashes\n---\nmore\n");
        assert!(frontmatter_span("no frontmatter").is_none());
        assert!(frontmatter_span("---\ntitle: x\n").is_none());
        // A `---` inside a value does not close the frontmatter.
        let raw = "---\ndescription: a --- b\n---\nbody";
        let (_, yaml, rest) = frontmatter_span(raw).unwrap();
        assert!(yaml.contains("a --- b"));
        assert_eq!(&raw[rest..], "\nbody");
    }

    #[test]
    fn test_normalize_url() {
        let (_tmp, state) = site("");
        let config = state.config.as_ref().unwrap();
        for (input, want) in [
            ("/docs/intro", "/docs/intro"),
            ("/docs/intro/", "/docs/intro"),
            ("docs/intro", "/docs/intro"),
            ("/docs/intro.html", "/docs/intro"),
            ("/docs/intro.md", "/docs/intro"),
            ("http://localhost:3000/docs/intro?x=1#top", "/docs/intro"),
            ("https://example.com/docs/intro/", "/docs/intro"),
            ("/", "/"),
            ("/index.html", "/"),
        ] {
            assert_eq!(normalize_url(config, input), want, "{input}");
        }
    }

    #[test]
    fn test_get_page_nested_doc_by_path_and_url() {
        let (tmp, mut state) = site("");
        write(
            tmp.path(),
            "content/docs/guides/intro.md",
            "---\ntitle: Intro Guide\ndescription: Start here\n---\n# Hello\n\n{{% callout(type=\"tip\") %}}\nUse **tools**.\n{{% end %}}\n",
        );
        let by_path = call_ok(
            &mut state,
            "seite_get_page",
            serde_json::json!({ "path": "content/docs/guides/intro.md" }),
        );
        assert_eq!(by_path["url"], "/docs/guides/intro");
        assert_eq!(by_path["slug"], "guides/intro");
        assert_eq!(by_path["collection"], "docs");
        assert_eq!(by_path["lang"], "en");
        assert_eq!(by_path["draft"], false);
        assert_eq!(by_path["frontmatter"]["title"], "Intro Guide");
        assert_eq!(by_path["frontmatter"]["description"], "Start here");
        assert!(by_path["body"].as_str().unwrap().starts_with("# Hello"));
        let html = by_path["html"].as_str().unwrap();
        assert!(html.contains("<h1"), "{html}");
        assert!(html.contains("callout callout-tip"), "{html}");
        assert!(html.contains("<strong>tools</strong>"), "{html}");
        assert!(by_path["output_path"].is_null());
        assert!(by_path["word_count"].as_u64().unwrap() > 3);

        // Content-dir-relative path and URL lookups resolve to the same file.
        for args in [
            serde_json::json!({ "path": "docs/guides/intro.md" }),
            serde_json::json!({ "url": "/docs/guides/intro/" }),
            serde_json::json!({ "url": "http://localhost:3000/docs/guides/intro" }),
        ] {
            let other = call_ok(&mut state, "seite_get_page", args);
            assert_eq!(other["path"], "content/docs/guides/intro.md");
        }

        // After a build, the output file is reported and contains the body HTML.
        call_ok(&mut state, "seite_build", serde_json::json!({}));
        let built = call_ok(
            &mut state,
            "seite_get_page",
            serde_json::json!({ "path": "content/docs/guides/intro.md" }),
        );
        let out = built["output_path"].as_str().unwrap();
        assert_eq!(out, "dist/docs/guides/intro.html");
        let page = fs::read_to_string(tmp.path().join(out)).unwrap();
        assert!(page.contains("callout callout-tip"));
    }

    #[test]
    fn test_get_page_dated_post_resolves_date_and_render_error() {
        let (tmp, mut state) = site("");
        write(
            tmp.path(),
            "content/posts/2026-03-04-hello.md",
            "---\ntitle: Hello\ndraft: true\n---\n{{< nosuchshortcode() >}}\n",
        );
        let out = call_ok(
            &mut state,
            "seite_get_page",
            serde_json::json!({ "path": "content/posts/2026-03-04-hello.md" }),
        );
        assert_eq!(out["date"], "2026-03-04");
        assert_eq!(out["frontmatter"]["date"], "2026-03-04");
        assert_eq!(out["url"], "/posts/hello");
        assert_eq!(out["draft"], true);
        assert!(out["html"].is_null());
        assert!(out["render_error"]
            .as_str()
            .unwrap()
            .contains("nosuchshortcode"));
    }

    #[test]
    fn test_get_page_invalid_args() {
        let (tmp, mut state) = site("");
        write(tmp.path(), "content/docs/a.md", "---\ntitle: A\n---\nx\n");
        write(tmp.path(), "secret.md", "---\ntitle: S\n---\nx\n");
        write(tmp.path(), "content/docs/notes.txt", "x");
        for (args, needle) in [
            (serde_json::json!({}), "exactly one"),
            (
                serde_json::json!({ "path": "content/docs/a.md", "url": "/docs/a" }),
                "exactly one",
            ),
            (
                serde_json::json!({ "path": "content/docs/zzz.md" }),
                "no content file",
            ),
            (
                serde_json::json!({ "path": "content/../secret.md" }),
                "outside the content directory",
            ),
            (
                serde_json::json!({ "path": "secret.md" }),
                "outside the content directory",
            ),
            (
                serde_json::json!({ "path": "content/docs/notes.txt" }),
                "not a markdown",
            ),
            (
                serde_json::json!({ "url": "/nope" }),
                "no content file is published",
            ),
            (serde_json::json!({ "path": 5 }), "must be a string"),
        ] {
            let err = call_err(&mut state, "seite_get_page", args.clone());
            assert!(err.contains(needle), "{args}: {err}");
        }
        let abs = tmp.path().join("secret.md");
        let err = call_err(
            &mut state,
            "seite_get_page",
            serde_json::json!({ "path": abs.to_string_lossy() }),
        );
        assert!(err.contains("outside the content directory"), "{err}");
    }

    #[test]
    fn test_update_frontmatter_round_trip_preserves_body() {
        let (tmp, mut state) = site("");
        let body = "\n# Title\r\n\nBody with --- dashes\n---\nand a fake delimiter, trailing spaces   \n\n\n";
        let original = format!("---\ntitle: Hello\ncustom_key: keep me\ntags: [old]\n---{body}");
        let rel = "content/posts/2026-01-02-hello.md";
        write(tmp.path(), rel, &original);

        let out = call_ok(
            &mut state,
            "seite_update_frontmatter",
            serde_json::json!({
                "path": rel,
                "set": { "description": "New desc", "tags": ["rust", "web"], "extra": { "hero": true } },
                "unset": ["custom_key"]
            }),
        );
        assert_eq!(out["changed"], true);
        assert_eq!(out["url"], "/posts/hello");
        assert_eq!(out["frontmatter"]["description"], "New desc");
        assert_eq!(
            out["frontmatter"]["tags"],
            serde_json::json!(["rust", "web"])
        );
        assert_eq!(out["frontmatter"]["extra"]["hero"], true);
        assert!(out["frontmatter"].get("custom_key").is_none());

        let updated = fs::read_to_string(tmp.path().join(rel)).unwrap();
        assert!(updated.ends_with(body), "body changed:\n{updated:?}");
        let (fm, _) = content::parse_content_file(&tmp.path().join(rel)).unwrap();
        assert_eq!(fm.title, "Hello");
        assert_eq!(fm.tags, vec!["rust", "web"]);

        // Same update again: nothing to write (idempotent).
        let again = call_ok(
            &mut state,
            "seite_update_frontmatter",
            serde_json::json!({
                "path": rel,
                "set": { "description": "New desc", "tags": ["rust", "web"], "extra": { "hero": true } }
            }),
        );
        assert_eq!(again["changed"], false);
        assert_eq!(fs::read_to_string(tmp.path().join(rel)).unwrap(), updated);

        // Unknown keys are kept; order is preserved.
        let out = call_ok(
            &mut state,
            "seite_update_frontmatter",
            serde_json::json!({ "path": rel, "set": { "title": "Renamed" }, "unset": ["missing"] }),
        );
        assert_eq!(out["frontmatter"]["title"], "Renamed");
        assert_eq!(out["notes"][0], "'missing' was not set");
        let text = fs::read_to_string(tmp.path().join(rel)).unwrap();
        assert!(text.ends_with(body));
        let pos = |k: &str| text.find(k).unwrap();
        assert!(pos("title: Renamed") < pos("tags:"), "{text}");
        assert!(pos("tags:") < pos("description:"), "{text}");
    }

    #[test]
    fn test_update_frontmatter_keeps_explicit_dates_valid() {
        let (tmp, mut state) = site("");
        let rel = "content/posts/dated.md";
        write(
            tmp.path(),
            rel,
            "---\ntitle: Dated\ndate: 2026-05-06\nupdated: 2026-05-07\n---\nBody\n",
        );
        let out = call_ok(
            &mut state,
            "seite_update_frontmatter",
            serde_json::json!({ "path": rel, "set": { "description": "x", "weight": 3 } }),
        );
        assert_eq!(out["frontmatter"]["date"], "2026-05-06");
        let (fm, body) = content::parse_content_file(&tmp.path().join(rel)).unwrap();
        assert_eq!(fm.date.unwrap().to_string(), "2026-05-06");
        assert_eq!(fm.updated.unwrap().to_string(), "2026-05-07");
        assert_eq!(fm.weight, Some(3));
        assert_eq!(body, "Body\n");
        // Setting a new date as a string works too.
        call_ok(
            &mut state,
            "seite_update_frontmatter",
            serde_json::json!({ "path": rel, "set": { "date": "2027-01-01" } }),
        );
        let (fm, _) = content::parse_content_file(&tmp.path().join(rel)).unwrap();
        assert_eq!(fm.date.unwrap().to_string(), "2027-01-01");
    }

    #[test]
    fn test_update_frontmatter_rejects_invalid_updates() {
        let (tmp, mut state) = site("");
        let rel = "content/posts/hello.md";
        let original = "---\ntitle: Hello\ndate: 2026-01-02\n---\nbody\n";
        write(tmp.path(), rel, original);
        write(tmp.path(), "outside.md", "---\ntitle: O\n---\n");
        write(tmp.path(), "content/docs/nofm.md", "just text\n");
        for (args, needle) in [
            (serde_json::json!({ "path": rel }), "nothing to do"),
            (
                serde_json::json!({ "path": rel, "set": {} }),
                "nothing to do",
            ),
            (
                serde_json::json!({ "path": rel, "unset": ["title"] }),
                "cannot be unset",
            ),
            (
                serde_json::json!({ "path": rel, "set": { "title": "" } }),
                "non-empty string",
            ),
            (
                serde_json::json!({ "path": rel, "set": { "title": 3 } }),
                "non-empty string",
            ),
            (
                serde_json::json!({ "path": rel, "set": { "draft": null } }),
                "use \"unset\"",
            ),
            (
                serde_json::json!({ "path": rel, "set": { "tags": ["a"] }, "unset": ["tags"] }),
                "both 'set' and 'unset'",
            ),
            (
                serde_json::json!({ "path": rel, "set": { "date": "next tuesday" } }),
                "would be invalid",
            ),
            (
                serde_json::json!({ "path": rel, "set": { "draft": "yes" } }),
                "would be invalid",
            ),
            // Dated collection without a filename date must keep `date`.
            (
                serde_json::json!({ "path": rel, "unset": ["date"] }),
                "dated collection",
            ),
            (
                // Resolves (relative to the content dir) to the site root.
                serde_json::json!({ "path": "../outside.md", "set": { "draft": true } }),
                "outside the content directory",
            ),
            (
                serde_json::json!({ "path": "content/../outside.md", "set": { "draft": true } }),
                "outside the content directory",
            ),
            (
                serde_json::json!({ "path": "content/docs/nofm.md", "set": { "draft": true } }),
                "has no frontmatter",
            ),
            (
                serde_json::json!({ "path": rel, "set": { "draft": true }, "body": "x" }),
                "unknown argument",
            ),
            (
                serde_json::json!({ "path": rel, "set": [1] }),
                "must be an object",
            ),
        ] {
            let err = call_err(&mut state, "seite_update_frontmatter", args.clone());
            assert!(err.contains(needle), "{args}: {err}");
        }
        // Nothing was modified by the rejected calls.
        assert_eq!(fs::read_to_string(tmp.path().join(rel)).unwrap(), original);
        assert_eq!(
            fs::read_to_string(tmp.path().join("outside.md")).unwrap(),
            "---\ntitle: O\n---\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_update_frontmatter_rejects_symlink_escape() {
        let (tmp, mut state) = site("");
        write(tmp.path(), "outside.md", "---\ntitle: O\n---\n");
        fs::create_dir_all(tmp.path().join("content/docs")).unwrap();
        std::os::unix::fs::symlink(
            tmp.path().join("outside.md"),
            tmp.path().join("content/docs/link.md"),
        )
        .unwrap();
        let err = call_err(
            &mut state,
            "seite_update_frontmatter",
            serde_json::json!({ "path": "content/docs/link.md", "set": { "draft": true } }),
        );
        assert!(err.contains("outside the content directory"), "{err}");
    }
}
