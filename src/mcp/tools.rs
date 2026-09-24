//! MCP tool implementations — actions AI tools can execute.
//!
//! Tools are invoked via `tools/call` and return structured results.
//! Each tool wraps existing seite CLI functionality.
//!
//! Tool-execution failures (bad or missing arguments, no site, failed build,
//! unknown theme, ...) are returned as ordinary tool results with
//! `isError: true` and an actionable message, per the MCP spec. Only protocol
//! problems (missing tool name, unknown tool) become JSON-RPC errors.

use std::collections::HashMap;

use super::content_index::{self, IndexedItem};
use super::{JsonRpcError, ServerState};
use crate::build::{self, links};
use crate::content::create::{create_content_file, NewContent};
use crate::{config, content, themes};

/// Every tool this server exposes.
const TOOL_NAMES: [&str; 5] = [
    "seite_build",
    "seite_create_content",
    "seite_search",
    "seite_apply_theme",
    "seite_lookup_docs",
];

/// Maximum characters of documentation returned by `seite_lookup_docs`.
const DOCS_OUTPUT_BUDGET: usize = 8_000;
/// Maximum characters per matched documentation section.
const DOCS_SECTION_MAX: usize = 500;
/// Default and maximum `limit` for `seite_search`.
const SEARCH_DEFAULT_LIMIT: u64 = 20;
const SEARCH_MAX_LIMIT: u64 = 100;

/// Handle `tools/list` — enumerate all available tools with JSON schemas.
pub fn list() -> Result<serde_json::Value, JsonRpcError> {
    Ok(serde_json::json!({
        "tools": [
            {
                "name": "seite_build",
                "description": "Build the site to the output directory. Returns build statistics, `warnings` (e.g. custom templates that failed to parse and were replaced by built-in defaults) and `broken_links` ([{target, sources}] of internal links pointing at pages that don't exist). With strict=true, any warning or broken link makes the call fail (isError) with the details.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "drafts": {
                            "type": "boolean",
                            "description": "Include draft content in the build",
                            "default": false
                        },
                        "strict": {
                            "type": "boolean",
                            "description": "Fail (isError) if the build produced warnings or broken internal links",
                            "default": false
                        }
                    },
                    "additionalProperties": false
                }
            },
            {
                "name": "seite_create_content",
                "description": "Create a new content file with frontmatter in a collection (same rules as `seite new`). Refuses to replace an existing file unless overwrite=true. Returns the source `path` (relative to the site root) and the `url` it will be published at.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "collection": {
                            "type": "string",
                            "description": "Collection name (e.g., posts, docs, pages, changelog, roadmap). Singular aliases like 'post' work."
                        },
                        "title": {
                            "type": "string",
                            "description": "Title of the content"
                        },
                        "slug": {
                            "type": "string",
                            "description": "Filename slug (lowercase letters, digits, '-', '_'). Defaults to a slug of the title."
                        },
                        "description": {
                            "type": "string",
                            "description": "Frontmatter description (used for meta description and listings)"
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Tags for the content"
                        },
                        "body": {
                            "type": "string",
                            "description": "Markdown body content. Omit to create a file with frontmatter only."
                        },
                        "draft": {
                            "type": "boolean",
                            "description": "Create as draft (excluded from builds unless seite_build drafts=true)",
                            "default": false
                        },
                        "weight": {
                            "type": "integer",
                            "description": "Ordering weight for non-date collections (lower sorts first)"
                        },
                        "extra": {
                            "type": "object",
                            "description": "Arbitrary frontmatter data exposed to templates as page.extra"
                        },
                        "subdir": {
                            "type": "string",
                            "description": "Sub-directory inside a nested collection, e.g. 'guides' (docs only; no '..' or absolute paths)"
                        },
                        "lang": {
                            "type": "string",
                            "description": "Language code for a translation (must be configured under [languages]); adds a .{lang}.md suffix"
                        },
                        "overwrite": {
                            "type": "boolean",
                            "description": "Replace the file if it already exists",
                            "default": false
                        }
                    },
                    "required": ["collection", "title"],
                    "additionalProperties": false
                }
            },
            {
                "name": "seite_search",
                "description": "Search site content (including drafts) by keyword. Matches titles, descriptions, tags, and body text; results are ranked title > description/tags > body. Returns `total` matches and the `returned` subset, each with source `path` and published `url`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search keywords (case-insensitive substring match)"
                        },
                        "collection": {
                            "type": "string",
                            "description": "Limit search to a specific collection (singular aliases like 'post' work)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum results to return (1-100)",
                            "default": 20,
                            "minimum": 1,
                            "maximum": 100
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            },
            {
                "name": "seite_apply_theme",
                "description": "Apply a bundled or installed theme by writing base.html in the site's template directory. If the current base.html has been customized (matches no known theme), it is first backed up to base.html.bak (or base.html.bak.N) and the backup path is returned.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Theme name: default, minimal, dark, docs, brutalist, bento, landing, terminal, magazine, academic, or an installed theme (lowercase letters, digits, hyphens)"
                        }
                    },
                    "required": ["name"],
                    "additionalProperties": false
                }
            },
            {
                "name": "seite_lookup_docs",
                "description": "Look up seite documentation by topic slug or search it by keyword. With no arguments, returns the list of topics. Output is capped at ~8 KB; narrow broad queries or pass a topic.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search keywords to find in documentation"
                        },
                        "topic": {
                            "type": "string",
                            "description": "Specific doc topic slug (e.g., configuration, templates, deployment)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        ]
    }))
}

// ---------------------------------------------------------------------------
// Tool errors and argument parsing
// ---------------------------------------------------------------------------

/// A tool-execution failure, reported to the client as `isError: true`.
#[derive(Debug)]
pub struct ToolError {
    pub message: String,
    /// Optional structured context appended to the message.
    pub details: Option<serde_json::Value>,
}

impl ToolError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: None,
        }
    }

    fn with_details(message: impl Into<String>, details: serde_json::Value) -> Self {
        Self {
            message: message.into(),
            details: Some(details),
        }
    }
}

impl From<String> for ToolError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

type ToolResult = Result<serde_json::Value, ToolError>;

fn type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Typed, validated view of a tool's `arguments` object.
struct Args<'a> {
    tool: &'static str,
    map: &'a serde_json::Map<String, serde_json::Value>,
}

impl<'a> Args<'a> {
    /// Validate that `value` is an object whose keys are all in `accepted`.
    fn new(
        tool: &'static str,
        value: &'a serde_json::Value,
        accepted: &[&str],
    ) -> Result<Self, ToolError> {
        let map = value.as_object().ok_or_else(|| {
            ToolError::new(format!(
                "{tool}: arguments must be a JSON object (got {})",
                type_name(value)
            ))
        })?;
        let mut unknown: Vec<&str> = map
            .keys()
            .map(String::as_str)
            .filter(|k| !accepted.contains(k))
            .collect();
        if !unknown.is_empty() {
            unknown.sort_unstable();
            return Err(ToolError::new(format!(
                "{tool}: unknown argument(s): {}. Accepted arguments: {}",
                unknown.join(", "),
                accepted.join(", ")
            )));
        }
        Ok(Self { tool, map })
    }

    /// The raw value, treating JSON `null` as absent.
    fn get(&self, key: &str) -> Option<&'a serde_json::Value> {
        self.map.get(key).filter(|v| !v.is_null())
    }

    fn type_error(&self, key: &str, expected: &str, got: &serde_json::Value) -> ToolError {
        ToolError::new(format!(
            "{}: argument '{key}' must be {expected} (got {} {})",
            self.tool,
            type_name(got),
            got
        ))
    }

    fn str(&self, key: &str) -> Result<Option<&'a str>, ToolError> {
        match self.get(key) {
            None => Ok(None),
            Some(serde_json::Value::String(s)) => Ok(Some(s.as_str())),
            Some(other) => Err(self.type_error(key, "a string", other)),
        }
    }

    fn req_str(&self, key: &str) -> Result<&'a str, ToolError> {
        self.str(key)?.ok_or_else(|| {
            ToolError::new(format!("{}: missing required argument '{key}'", self.tool))
        })
    }

    fn bool(&self, key: &str) -> Result<Option<bool>, ToolError> {
        match self.get(key) {
            None => Ok(None),
            Some(serde_json::Value::Bool(b)) => Ok(Some(*b)),
            Some(other) => Err(self.type_error(key, "a boolean (true/false)", other)),
        }
    }

    fn int(&self, key: &str) -> Result<Option<i64>, ToolError> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_i64()
                .map(Some)
                .ok_or_else(|| self.type_error(key, "an integer", v)),
        }
    }

    fn str_array(&self, key: &str) -> Result<Option<Vec<String>>, ToolError> {
        match self.get(key) {
            None => Ok(None),
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| self.type_error(key, "an array of strings", item))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            Some(other) => Err(self.type_error(key, "an array of strings", other)),
        }
    }

    fn object(
        &self,
        key: &str,
    ) -> Result<Option<&'a serde_json::Map<String, serde_json::Value>>, ToolError> {
        match self.get(key) {
            None => Ok(None),
            Some(serde_json::Value::Object(map)) => Ok(Some(map)),
            Some(other) => Err(self.type_error(key, "an object", other)),
        }
    }
}

fn text_block(text: String) -> serde_json::Value {
    serde_json::json!({ "type": "text", "text": text })
}

fn success_result(value: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "content": [text_block(serde_json::to_string_pretty(value).unwrap_or_default())]
    })
}

fn error_result(err: ToolError) -> serde_json::Value {
    let mut text = err.message;
    if let Some(details) = err.details {
        text.push_str("\n\n");
        text.push_str(&serde_json::to_string_pretty(&details).unwrap_or_default());
    }
    serde_json::json!({
        "content": [text_block(text)],
        "isError": true
    })
}

/// Resolve a collection name (with singular aliases) or explain what exists.
fn resolve_collection<'c>(
    config: &'c config::SiteConfig,
    name: &str,
) -> Result<&'c config::CollectionConfig, ToolError> {
    config::find_collection(name, &config.collections).ok_or_else(|| {
        let available: Vec<&str> = config.collections.iter().map(|c| c.name.as_str()).collect();
        ToolError::new(format!(
            "Unknown collection '{name}'. Available collections: {}",
            if available.is_empty() {
                "(none — add [[collections]] to seite.toml)".to_string()
            } else {
                available.join(", ")
            }
        ))
    })
}

/// Handle `tools/call` — dispatch to the appropriate tool.
pub fn call(
    state: &mut ServerState,
    params: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'name' parameter"))?;

    if !TOOL_NAMES.contains(&name) {
        return Err(JsonRpcError::invalid_params(format!(
            "Unknown tool: {name}. Valid tools: {}",
            TOOL_NAMES.join(", ")
        )));
    }

    let empty = serde_json::json!({});
    let arguments = match params.get("arguments") {
        None | Some(serde_json::Value::Null) => &empty,
        Some(args) => args,
    };

    let result = match name {
        "seite_build" => call_build(state, arguments),
        "seite_create_content" => call_create_content(state, arguments),
        "seite_search" => call_search(state, arguments),
        "seite_apply_theme" => call_apply_theme(state, arguments),
        _ => call_lookup_docs(arguments),
    };

    Ok(match result {
        Ok(value) => success_result(&value),
        Err(err) => error_result(err),
    })
}

// ---------------------------------------------------------------------------
// seite_build
// ---------------------------------------------------------------------------

fn call_build(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    let args = Args::new("seite_build", arguments, &["drafts", "strict"])?;
    let include_drafts = args.bool("drafts")?.unwrap_or(false);
    let strict = args.bool("strict")?.unwrap_or(false);
    let (config, paths) = state.site()?;

    let opts = build::BuildOptions {
        include_drafts,
        incremental: false,
    };

    let result = build::build_site(config, paths, &opts)
        .map_err(|e| ToolError::new(format!("Build failed: {e}")))?;

    let items_built: serde_json::Value = result
        .stats
        .items_built
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::json!(v)))
        .collect();

    let broken_links: Vec<serde_json::Value> =
        links::group_broken_links(&result.link_check.broken_links)
            .into_iter()
            .map(|(target, sources)| serde_json::json!({ "target": target, "sources": sources }))
            .collect();

    let mut response = serde_json::json!({
        "success": true,
        "items_built": items_built,
        "static_files_copied": result.stats.static_files_copied,
        "public_files_copied": result.stats.public_files_copied,
        "data_files_loaded": result.stats.data_files_loaded,
        "duration_ms": result.stats.duration_ms,
        "output_dir": content_index::relative_path(&paths.root, &paths.output),
        "links_checked": result.link_check.total_links_checked,
        "warnings": result.warnings,
        "broken_links": broken_links,
    });

    if !result.subdomain_builds.is_empty() {
        let subdomains: Vec<serde_json::Value> = result
            .subdomain_builds
            .iter()
            .map(|sb| {
                serde_json::json!({
                    "collection": sb.collection_name,
                    "subdomain": sb.subdomain,
                    "base_url": sb.base_url,
                    "output_dir": sb.output_dir.display().to_string(),
                })
            })
            .collect();
        response["subdomain_builds"] = serde_json::json!(subdomains);
    }

    if strict && (!result.warnings.is_empty() || !broken_links.is_empty()) {
        response["success"] = serde_json::json!(false);
        return Err(ToolError::with_details(
            format!(
                "Build finished with {} warning(s) and {} broken link target(s); failing because strict=true. \
                 Fix the problems below (or call without strict to accept them).",
                result.warnings.len(),
                broken_links.len()
            ),
            response,
        ));
    }

    Ok(response)
}

// ---------------------------------------------------------------------------
// seite_create_content
// ---------------------------------------------------------------------------

fn call_create_content(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    let args = Args::new(
        "seite_create_content",
        arguments,
        &[
            "collection",
            "title",
            "slug",
            "description",
            "tags",
            "body",
            "draft",
            "weight",
            "extra",
            "subdir",
            "lang",
            "overwrite",
        ],
    )?;
    let collection_name = args.req_str("collection")?;
    let title = args.req_str("title")?;
    let slug = args.str("slug")?;
    let description = args.str("description")?;
    let tags = args.str_array("tags")?.unwrap_or_default();
    let body = args.str("body")?.unwrap_or("");
    let draft = args.bool("draft")?.unwrap_or(false);
    let weight = match args.int("weight")? {
        None => None,
        Some(w) => Some(i32::try_from(w).map_err(|_| {
            ToolError::new(format!(
                "seite_create_content: argument 'weight' is out of range ({w})"
            ))
        })?),
    };
    let extra: HashMap<String, serde_yaml_ng::Value> = match args.object("extra")? {
        None => HashMap::new(),
        Some(map) => map
            .iter()
            .map(|(k, v)| {
                serde_yaml_ng::to_value(v)
                    .map(|y| (k.clone(), y))
                    .map_err(|e| {
                        ToolError::new(format!(
                            "seite_create_content: cannot store extra.{k} in frontmatter: {e}"
                        ))
                    })
            })
            .collect::<Result<_, _>>()?,
    };
    let subdir = args.str("subdir")?;
    let lang = args.str("lang")?;
    let overwrite = args.bool("overwrite")?.unwrap_or(false);

    let (config, paths) = state.site()?;
    let collection = resolve_collection(config, collection_name)?;

    let spec = NewContent {
        title,
        slug,
        description,
        tags,
        draft,
        weight,
        extra,
        lang,
        subdir,
        body,
        overwrite,
    };
    let created = create_content_file(
        config,
        &paths.content,
        collection,
        &spec,
        chrono::Local::now().date_naive(),
    )
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("already exists") {
            ToolError::new(format!(
                "{msg}. Pass \"overwrite\": true to replace it, or use a different slug."
            ))
        } else {
            ToolError::new(format!("seite_create_content: {msg}"))
        }
    })?;

    // Report the URL exactly as the build will generate it.
    let rel_path = content_index::relative_path(&paths.root, &created.path);
    let collection_dir = paths.content.join(&collection.directory);
    let rel_to_collection = created
        .path
        .strip_prefix(&collection_dir)
        .unwrap_or(&created.path)
        .to_path_buf();
    let (fm, _) = content::parse_content_file(&created.path)
        .map_err(|e| ToolError::new(format!("Created {rel_path} but cannot re-read it: {e}")))?;
    let loc =
        build::resolve_item_location(config, collection, &created.path, &rel_to_collection, &fm);

    let mut response = serde_json::json!({
        "path": rel_path,
        "url": loc.url,
        "slug": loc.slug,
        "lang": loc.lang,
        "collection": collection.name,
        "draft": draft,
        "overwritten": created.overwritten,
    });
    let mut notes = Vec::new();
    if draft {
        notes.push(
            "Draft content is excluded from builds unless seite_build is called with drafts=true."
                .to_string(),
        );
    }
    if body.trim().is_empty() {
        notes.push(format!(
            "The body is empty — edit {rel_path} to add content."
        ));
    }
    if !notes.is_empty() {
        response["notes"] = serde_json::json!(notes);
    }
    Ok(response)
}

// ---------------------------------------------------------------------------
// seite_search
// ---------------------------------------------------------------------------

/// Relevance tier: 3 = title, 2 = description/tags, 1 = body only.
fn search_rank(item: &content_index::ParsedItem, query_lower: &str) -> (u8, Vec<&'static str>) {
    let mut matched = Vec::new();
    if item.frontmatter.title.to_lowercase().contains(query_lower) {
        matched.push("title");
    }
    if item
        .frontmatter
        .description
        .as_ref()
        .is_some_and(|d| d.to_lowercase().contains(query_lower))
    {
        matched.push("description");
    }
    if item
        .frontmatter
        .tags
        .iter()
        .any(|t| t.to_lowercase().contains(query_lower))
    {
        matched.push("tags");
    }
    if item.body.to_lowercase().contains(query_lower) {
        matched.push("body");
    }
    let rank = if matched.contains(&"title") {
        3
    } else if matched.contains(&"description") || matched.contains(&"tags") {
        2
    } else if matched.contains(&"body") {
        1
    } else {
        0
    };
    (rank, matched)
}

fn call_search(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    let args = Args::new("seite_search", arguments, &["query", "collection", "limit"])?;
    let query = args.req_str("query")?.trim();
    if query.is_empty() {
        return Err(ToolError::new(
            "seite_search: 'query' must not be empty. To list everything in a collection, read the seite://content/{collection} resource.",
        ));
    }
    let limit = match args.int("limit")? {
        None => SEARCH_DEFAULT_LIMIT,
        Some(n) if (1..=SEARCH_MAX_LIMIT as i64).contains(&n) => n as u64,
        Some(n) => {
            return Err(ToolError::new(format!(
                "seite_search: 'limit' must be between 1 and {SEARCH_MAX_LIMIT} (got {n})"
            )))
        }
    };
    let filter = args.str("collection")?;

    let (config, paths) = state.site()?;
    let collections: Vec<&config::CollectionConfig> = match filter {
        Some(name) => vec![resolve_collection(config, name)?],
        None => config.collections.iter().collect(),
    };

    let query_lower = query.to_lowercase();
    let mut hits: Vec<(u8, usize, IndexedItem, Vec<&'static str>)> = Vec::new();
    let mut parse_errors = Vec::new();

    for collection in collections {
        for item in content_index::scan_collection(config, paths, collection) {
            match &item.parsed {
                Err(e) => parse_errors.push(serde_json::json!({ "path": item.path, "error": e })),
                Ok(parsed) => {
                    let (rank, matched) = search_rank(parsed, &query_lower);
                    if rank > 0 {
                        let occurrences = parsed.body.to_lowercase().matches(&query_lower).count();
                        hits.push((rank, occurrences, item, matched));
                    }
                }
            }
        }
    }

    // Best tier first, then more body occurrences, then title for stability.
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then_with(|| {
            let ta =
                a.2.parsed
                    .as_ref()
                    .map(|p| p.frontmatter.title.as_str())
                    .unwrap_or("");
            let tb =
                b.2.parsed
                    .as_ref()
                    .map(|p| p.frontmatter.title.as_str())
                    .unwrap_or("");
            ta.cmp(tb)
        })
    });

    let total = hits.len();
    let results: Vec<serde_json::Value> = hits
        .iter()
        .take(limit as usize)
        .filter_map(|(_, _, item, matched)| {
            let parsed = item.parsed.as_ref().ok()?;
            let excerpt = if matched.contains(&"body") {
                extract_excerpt(&parsed.body, &query_lower)
            } else {
                first_paragraph(&parsed.body)
            };
            Some(serde_json::json!({
                "title": parsed.frontmatter.title,
                "collection": item.collection,
                "slug": parsed.slug,
                "url": parsed.url,
                "path": item.path,
                "lang": parsed.lang,
                "draft": parsed.frontmatter.draft,
                "tags": parsed.frontmatter.tags,
                "description": parsed.frontmatter.description,
                "date": parsed.date.map(|d| d.to_string()),
                "matched_in": matched,
                "excerpt": excerpt,
            }))
        })
        .collect();

    let mut response = serde_json::json!({
        "query": query,
        "total": total,
        "returned": results.len(),
        "results": results,
    });
    if total > results.len() {
        response["note"] = serde_json::json!(format!(
            "Showing the top {} of {total} matches. Raise 'limit' (max {SEARCH_MAX_LIMIT}) or refine the query.",
            results.len()
        ));
    }
    if !parse_errors.is_empty() {
        response["parse_errors"] = serde_json::json!(parse_errors);
    }
    Ok(response)
}

/// Extract a ~200 character excerpt around the first occurrence of the query.
fn extract_excerpt(body: &str, query_lower: &str) -> String {
    let body_lower = body.to_lowercase();
    let Some(byte_pos) = body_lower.find(query_lower) else {
        return first_paragraph(body);
    };
    // `byte_pos` is an offset into `body_lower`, whose byte layout can differ
    // from `body` (lowercasing can change byte lengths, e.g. `İ`). Work in
    // character units so the slice always lands on a char boundary and never
    // panics on multibyte content (accents, CJK, emoji).
    let chars: Vec<char> = body.chars().collect();
    let char_pos = body_lower[..byte_pos].chars().count().min(chars.len());
    let query_chars = query_lower.chars().count();
    let start = char_pos.saturating_sub(100);
    let end = (char_pos + query_chars + 100).min(chars.len());
    let mut excerpt: String = chars[start..end].iter().collect();
    if start > 0 {
        excerpt = format!("...{excerpt}");
    }
    if end < chars.len() {
        excerpt = format!("{excerpt}...");
    }
    excerpt
}

/// Get the first paragraph of markdown content as an excerpt.
fn first_paragraph(body: &str) -> String {
    body.trim()
        .lines()
        .take_while(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect()
}

// ---------------------------------------------------------------------------
// seite_apply_theme
// ---------------------------------------------------------------------------

fn call_apply_theme(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    let args = Args::new("seite_apply_theme", arguments, &["name"])?;
    let name = args.req_str("name")?;
    let (_, paths) = state.site()?;

    let applied = themes::apply_theme(&paths.root, &paths.templates, name)
        .map_err(|e| ToolError::new(format!("seite_apply_theme: {e}")))?;

    let mut response = serde_json::json!({
        "applied": true,
        "theme": applied.name,
        "description": applied.description,
        "source": applied.source.as_str(),
        "path": content_index::relative_path(&paths.root, &applied.path),
    });
    if let Some(backup) = applied.backup {
        let backup = content_index::relative_path(&paths.root, &backup);
        response["backup"] = serde_json::json!(backup);
        response["note"] = serde_json::json!(format!(
            "The previous base.html was customized, so it was saved to {backup} before applying the theme."
        ));
    }
    Ok(response)
}

// ---------------------------------------------------------------------------
// seite_lookup_docs
// ---------------------------------------------------------------------------

/// Truncate to at most `max` characters on a char boundary.
/// Returns the (possibly shortened) text and whether it was truncated.
fn truncate_chars(text: &str, max: usize) -> (String, bool) {
    match text.char_indices().nth(max) {
        Some((byte_idx, _)) => (text[..byte_idx].to_string(), true),
        None => (text.to_string(), false),
    }
}

fn call_lookup_docs(arguments: &serde_json::Value) -> ToolResult {
    let args = Args::new("seite_lookup_docs", arguments, &["query", "topic"])?;
    let topic = args.str("topic")?;
    let query = args.str("query")?;

    // If topic matches a slug, return that page (capped)
    if let Some(topic_slug) = topic {
        if let Some(doc) = crate::docs::by_slug(topic_slug) {
            let body = crate::docs::strip_frontmatter(doc.raw_content);
            let total_chars = body.chars().count();
            let (content, truncated) = truncate_chars(body, DOCS_OUTPUT_BUDGET);
            let mut response = serde_json::json!({
                "found": true,
                "topic": topic_slug,
                "title": doc.title,
                "description": doc.description,
                "content": content,
                "truncated": truncated,
            });
            if truncated {
                response["note"] = serde_json::json!(format!(
                    "Content truncated to {DOCS_OUTPUT_BUDGET} of {total_chars} characters. \
                     Use 'query' to find a specific section, or read the full page from the \
                     seite://docs/{topic_slug} resource."
                ));
            }
            return Ok(response);
        }
        if query.is_none() {
            let topics: Vec<&str> = crate::docs::all().iter().map(|d| d.slug).collect();
            return Err(ToolError::new(format!(
                "Documentation topic not found: {topic_slug}. Available topics: {}",
                topics.join(", ")
            )));
        }
    }

    // Search across all docs by keyword
    if let Some(query_str) = query {
        let query_str = query_str.trim();
        if query_str.is_empty() {
            return Err(ToolError::new(
                "seite_lookup_docs: 'query' must not be empty. Omit it to list available topics.",
            ));
        }
        let query_lower = query_str.to_lowercase();

        // Docs whose title/description match rank above body-only matches.
        let mut matches: Vec<(u8, crate::docs::DocPage, &'static str)> = Vec::new();
        for doc in crate::docs::all() {
            let body = crate::docs::strip_frontmatter(doc.raw_content);
            let rank = if doc.title.to_lowercase().contains(&query_lower) {
                3
            } else if doc.description.to_lowercase().contains(&query_lower) {
                2
            } else if body.to_lowercase().contains(&query_lower) {
                1
            } else {
                0
            };
            if rank > 0 {
                matches.push((rank, doc, body));
            }
        }
        matches.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.weight.cmp(&b.1.weight)));

        let total = matches.len();
        let mut used = 0usize;
        let mut results = Vec::new();
        let mut omitted = Vec::new();
        for (_, doc, body) in matches {
            if used >= DOCS_OUTPUT_BUDGET {
                omitted.push(doc.slug);
                continue;
            }
            let mut sections = Vec::new();
            for section in extract_matching_sections(body, &query_lower) {
                let len = section.chars().count();
                if used + len > DOCS_OUTPUT_BUDGET {
                    break;
                }
                used += len;
                sections.push(section);
            }
            used += doc.title.len() + doc.description.len();
            results.push(serde_json::json!({
                "slug": doc.slug,
                "title": doc.title,
                "description": doc.description,
                "matched_sections": sections,
            }));
        }

        let mut response = serde_json::json!({
            "query": query_str,
            "count": total,
            "returned": results.len(),
            "results": results,
        });
        if !omitted.is_empty() || used >= DOCS_OUTPUT_BUDGET {
            response["truncated"] = serde_json::json!(true);
            response["omitted_topics"] = serde_json::json!(omitted);
            response["note"] = serde_json::json!(format!(
                "Output capped at ~{} KB. Use a more specific query, or pass 'topic' with one of the slugs above to read a page.",
                DOCS_OUTPUT_BUDGET / 1000
            ));
        }
        return Ok(response);
    }

    // Neither topic nor query provided — return the index
    let docs: Vec<serde_json::Value> = crate::docs::all()
        .iter()
        .map(|d| {
            serde_json::json!({
                "slug": d.slug,
                "title": d.title,
                "description": d.description,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "available_topics": docs,
    }))
}

/// Extract sections (split by `## ` headings) that contain the query string.
/// Each section is capped at [`DOCS_SECTION_MAX`] characters (char-boundary safe).
fn extract_matching_sections(body: &str, query_lower: &str) -> Vec<String> {
    fn push_if_match(sections: &mut Vec<String>, heading: &str, section: &str, query_lower: &str) {
        if section.is_empty() || !section.to_lowercase().contains(query_lower) {
            return;
        }
        let full = if heading.is_empty() {
            section.trim().to_string()
        } else {
            format!("{heading}\n{}", section.trim())
        };
        let (text, truncated) = truncate_chars(&full, DOCS_SECTION_MAX);
        sections.push(if truncated {
            format!("{text}...")
        } else {
            text
        });
    }

    let mut sections = Vec::new();
    let mut current_section = String::new();
    let mut current_heading = String::new();

    for line in body.lines() {
        if line.starts_with("## ") {
            push_if_match(
                &mut sections,
                &current_heading,
                &current_section,
                query_lower,
            );
            current_heading = line.to_string();
            current_section.clear();
        } else {
            current_section.push_str(line);
            current_section.push('\n');
        }
    }
    push_if_match(
        &mut sections,
        &current_heading,
        &current_section,
        query_lower,
    );

    // Limit to 5 most relevant sections
    sections.truncate(5);
    sections
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn empty_state() -> ServerState {
        ServerState {
            config: None,
            paths: None,
            cwd: std::path::PathBuf::new(),
            config_error: None,
        }
    }

    /// A real site on disk (seite.toml + content) loaded like the server does.
    fn site(toml_extra: &str) -> (TempDir, ServerState) {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            format!(
                "[site]\ntitle = \"Test\"\nbase_url = \"http://localhost:3000\"\n\n\
                 [[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\nurl_prefix = \"/posts\"\ndefault_template = \"post.html\"\n\n\
                 [[collections]]\nname = \"docs\"\nlabel = \"Docs\"\ndirectory = \"docs\"\nnested = true\nurl_prefix = \"/docs\"\ndefault_template = \"doc.html\"\n{toml_extra}"
            ),
        )
        .unwrap();
        let state = ServerState::load(tmp.path().to_path_buf());
        assert!(state.config.is_some(), "{:?}", state.config_error);
        (tmp, state)
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    fn call_tool(state: &mut ServerState, name: &str, args: serde_json::Value) -> (bool, String) {
        let result = call(
            state,
            &serde_json::json!({ "name": name, "arguments": args }),
        )
        .unwrap();
        let is_error = result["isError"].as_bool().unwrap_or(false);
        let text = result["content"][0]["text"].as_str().unwrap().to_string();
        (is_error, text)
    }

    fn call_ok(state: &mut ServerState, name: &str, args: serde_json::Value) -> serde_json::Value {
        let (is_error, text) = call_tool(state, name, args);
        assert!(!is_error, "{name} failed: {text}");
        serde_json::from_str(&text).unwrap()
    }

    fn call_err(state: &mut ServerState, name: &str, args: serde_json::Value) -> String {
        let (is_error, text) = call_tool(state, name, args);
        assert!(is_error, "{name} unexpectedly succeeded: {text}");
        text
    }

    #[test]
    fn test_list_returns_all_tools() {
        let result = list().unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 5);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for name in TOOL_NAMES {
            assert!(names.contains(&name));
        }
        for tool in tools {
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        }
    }

    #[test]
    fn test_lookup_docs_by_topic() {
        let args = serde_json::json!({ "topic": "configuration" });
        let result = call_lookup_docs(&args).unwrap();
        assert_eq!(result["found"], true);
        assert_eq!(result["title"], "Configuration");
        assert!(result["content"].as_str().unwrap().contains("seite.toml"));
    }

    #[test]
    fn test_lookup_docs_by_topic_is_capped() {
        let result = call_lookup_docs(&serde_json::json!({ "topic": "configuration" })).unwrap();
        let content = result["content"].as_str().unwrap();
        assert!(content.chars().count() <= DOCS_OUTPUT_BUDGET);
        if result["truncated"] == true {
            assert!(result["note"]
                .as_str()
                .unwrap()
                .contains("seite://docs/configuration"));
        }
    }

    #[test]
    fn test_lookup_docs_broad_query_is_capped() {
        let result = call_lookup_docs(&serde_json::json!({ "query": "the" })).unwrap();
        let size = serde_json::to_string(&result).unwrap().len();
        // Budget is on characters of doc text; allow JSON/escaping overhead.
        assert!(size < DOCS_OUTPUT_BUDGET * 2, "output was {size} bytes");
        assert_eq!(result["truncated"], true);
        assert!(result["note"].as_str().unwrap().contains("specific"));
    }

    #[test]
    fn test_lookup_docs_by_query() {
        let args = serde_json::json!({ "query": "deploy" });
        let result = call_lookup_docs(&args).unwrap();
        assert!(result["count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_lookup_docs_topic_not_found_is_error() {
        let args = serde_json::json!({ "topic": "nonexistent" });
        let err = call_lookup_docs(&args).unwrap_err();
        assert!(err.message.contains("not found"));
        assert!(err.message.contains("configuration"));
    }

    #[test]
    fn test_lookup_docs_empty_query_is_error() {
        let err = call_lookup_docs(&serde_json::json!({ "query": "  " })).unwrap_err();
        assert!(err.message.contains("must not be empty"));
    }

    #[test]
    fn test_lookup_docs_no_args_returns_index() {
        let args = serde_json::json!({});
        let result = call_lookup_docs(&args).unwrap();
        assert!(result["available_topics"].as_array().is_some());
    }

    #[test]
    fn test_lookup_docs_wrong_type_is_error() {
        let err = call_lookup_docs(&serde_json::json!({ "query": 5 })).unwrap_err();
        assert!(err.message.contains("must be a string"));
    }

    #[test]
    fn test_extract_matching_sections() {
        let body = "## Intro\nSome intro text.\n\n## Deploy\nDeploy to GitHub.\n\n## Config\nConfig options.";
        let sections = extract_matching_sections(body, "deploy");
        assert_eq!(sections.len(), 1);
        assert!(sections[0].contains("Deploy"));
    }

    #[test]
    fn test_extract_matching_sections_multibyte_no_panic() {
        // Byte 500 falls inside a multibyte character: byte slicing would panic.
        let long = "é".repeat(600);
        let body = format!(
            "## Unicode\nsearchterm x {long}\n\n## CJK\nsearchterm {}",
            "日本語".repeat(300)
        );
        let sections = extract_matching_sections(&body, "searchterm");
        assert_eq!(sections.len(), 2);
        for s in &sections {
            assert!(s.ends_with("..."));
            assert!(s.chars().count() <= DOCS_SECTION_MAX + 3);
        }
    }

    #[test]
    fn test_truncate_chars_boundaries() {
        assert_eq!(truncate_chars("abc", 5), ("abc".to_string(), false));
        assert_eq!(truncate_chars("abc", 3), ("abc".to_string(), false));
        assert_eq!(truncate_chars("日本語", 2), ("日本".to_string(), true));
        assert_eq!(truncate_chars("🚀🚀", 1), ("🚀".to_string(), true));
    }

    #[test]
    fn test_extract_excerpt() {
        let body = "This is a long text about deploying your site to production servers.";
        let excerpt = extract_excerpt(body, "deploying");
        assert!(excerpt.contains("deploying"));
    }

    #[test]
    fn test_first_paragraph() {
        let body = "First line.\nSecond line.\n\nSecond paragraph.";
        let result = first_paragraph(body);
        assert!(result.contains("First line"));
        assert!(result.contains("Second line"));
        assert!(!result.contains("Second paragraph"));
    }

    #[test]
    fn test_first_paragraph_empty() {
        assert_eq!(first_paragraph(""), "");
    }

    #[test]
    fn test_first_paragraph_truncates_at_200_chars() {
        let long_line = "a".repeat(300);
        let result = first_paragraph(&long_line);
        assert_eq!(result.len(), 200);
    }

    #[test]
    fn test_first_paragraph_max_3_lines() {
        let body = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let result = first_paragraph(body);
        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 2"));
        assert!(result.contains("Line 3"));
        assert!(!result.contains("Line 4"));
    }

    #[test]
    fn test_extract_excerpt_not_found_falls_back() {
        let body = "Hello world";
        let result = extract_excerpt(body, "missing");
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_extract_excerpt_at_start() {
        let body = "deploy is great. More text follows for context and padding.";
        let result = extract_excerpt(body, "deploy");
        assert!(result.contains("deploy"));
        assert!(!result.starts_with("..."));
    }

    #[test]
    fn test_extract_excerpt_at_end() {
        let long_prefix = "x".repeat(200);
        let body = format!("{long_prefix}deploy");
        let result = extract_excerpt(&body, "deploy");
        assert!(result.contains("deploy"));
        assert!(result.starts_with("..."));
    }

    #[test]
    fn test_extract_excerpt_multibyte_no_panic() {
        let prefix = "Café société — naïve façade résumé 日本語 ".repeat(5);
        let body = format!("{prefix}the SEARCHME keyword 🚀 and more 日本語 text after it here.");
        let result = extract_excerpt(&body, "searchme");
        assert!(result.contains("SEARCHME"));
    }

    #[test]
    fn test_extract_excerpt_multibyte_at_boundary() {
        let body = "日本語テキスト searchme 日本語テキスト";
        let result = extract_excerpt(body, "searchme");
        assert!(result.contains("searchme"));
    }

    #[test]
    fn test_extract_excerpt_emoji_window_edges() {
        let pad = "🚀".repeat(150);
        let body = format!("{pad} searchme {pad}");
        let result = extract_excerpt(&body, "searchme");
        assert!(result.contains("searchme"));
        assert!(result.starts_with("..."));
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_extract_matching_sections_no_match() {
        let body = "## Intro\nSome text.\n## Config\nMore text.";
        let sections = extract_matching_sections(body, "zzzzz");
        assert!(sections.is_empty());
    }

    #[test]
    fn test_extract_matching_sections_multiple() {
        let body = "## Section A\nfoo bar\n## Section B\nfoo baz\n## Section C\nunrelated";
        let sections = extract_matching_sections(body, "foo");
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn test_extract_matching_sections_truncates_long() {
        let long_content = "x".repeat(600);
        let body = format!("## Long Section\nsearchterm {long_content}");
        let sections = extract_matching_sections(&body, "searchterm");
        assert_eq!(sections.len(), 1);
        assert!(sections[0].ends_with("..."));
        assert!(sections[0].len() <= 504); // 500 + "..."
    }

    #[test]
    fn test_extract_matching_sections_no_headings() {
        let body = "Just plain text with searchterm in it.";
        let sections = extract_matching_sections(body, "searchterm");
        assert_eq!(sections.len(), 1);
        assert!(sections[0].contains("searchterm"));
    }

    #[test]
    fn test_extract_matching_sections_max_5() {
        let mut body = String::new();
        for i in 0..10 {
            body.push_str(&format!("## Section {i}\nsearchterm content\n"));
        }
        let sections = extract_matching_sections(&body, "searchterm");
        assert_eq!(sections.len(), 5);
    }

    #[test]
    fn test_lookup_docs_all_topics_have_content() {
        for doc in crate::docs::all() {
            let args = serde_json::json!({ "topic": doc.slug });
            let result = call_lookup_docs(&args).unwrap();
            assert_eq!(
                result["found"], true,
                "Doc topic '{}' should be found",
                doc.slug
            );
            assert!(!result["content"].as_str().unwrap().is_empty());
        }
    }

    // --- protocol vs tool errors -------------------------------------------

    #[test]
    fn test_call_missing_name_is_protocol_error() {
        let mut state = empty_state();
        let err = call(&mut state, &serde_json::json!({})).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing 'name'"));
    }

    #[test]
    fn test_call_unknown_tool_lists_valid_tools() {
        let mut state = empty_state();
        let err = call(
            &mut state,
            &serde_json::json!({ "name": "nonexistent_tool" }),
        )
        .unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Unknown tool"));
        assert!(err.message.contains("seite_build"));
        assert!(err.message.contains("seite_lookup_docs"));
    }

    #[test]
    fn test_call_without_config_is_tool_error() {
        let mut state = empty_state();
        for (name, args) in [
            ("seite_build", serde_json::json!({})),
            ("seite_search", serde_json::json!({ "query": "test" })),
            ("seite_apply_theme", serde_json::json!({ "name": "dark" })),
            (
                "seite_create_content",
                serde_json::json!({ "collection": "posts", "title": "Test" }),
            ),
        ] {
            let text = call_err(&mut state, name, args);
            assert!(text.contains("seite project"), "{name}: {text}");
            assert!(text.contains("seite init"), "{name}: {text}");
        }
    }

    #[test]
    fn test_call_with_broken_config_reports_load_error() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("seite.toml"), "[site\n").unwrap();
        let mut state = ServerState::load(tmp.path().to_path_buf());
        let text = call_err(&mut state, "seite_build", serde_json::json!({}));
        assert!(text.contains("Failed to load"), "{text}");
        assert!(!text.contains("Not in a seite project"), "{text}");
    }

    #[test]
    fn test_call_wrong_argument_types_are_errors() {
        let (_tmp, mut state) = site("");
        let text = call_err(
            &mut state,
            "seite_build",
            serde_json::json!({ "drafts": "yes" }),
        );
        assert!(text.contains("'drafts' must be a boolean"), "{text}");
        let text = call_err(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "posts", "title": 7 }),
        );
        assert!(text.contains("'title' must be a string"), "{text}");
        let text = call_err(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "posts", "title": "x", "tags": "a,b" }),
        );
        assert!(text.contains("array of strings"), "{text}");
        let text = call_err(
            &mut state,
            "seite_search",
            serde_json::json!({ "query": "x", "limit": "5" }),
        );
        assert!(text.contains("'limit' must be an integer"), "{text}");
    }

    #[test]
    fn test_call_unknown_arguments_are_errors() {
        let (_tmp, mut state) = site("");
        let text = call_err(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "posts", "title": "x", "summary": "nope" }),
        );
        assert!(text.contains("unknown argument(s): summary"), "{text}");
    }

    #[test]
    fn test_call_missing_required_argument_is_error() {
        let (_tmp, mut state) = site("");
        let text = call_err(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "posts" }),
        );
        assert!(text.contains("missing required argument 'title'"), "{text}");
    }

    // --- seite_create_content ----------------------------------------------

    #[test]
    fn test_create_content_refuses_overwrite() {
        let (tmp, mut state) = site("");
        let first = call_ok(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "doc", "title": "Intro", "tags": ["keep"], "body": "original" }),
        );
        assert_eq!(first["path"], "content/docs/intro.md");
        assert_eq!(first["url"], "/docs/intro");

        let text = call_err(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "docs", "title": "Intro", "body": "replacement" }),
        );
        assert!(text.contains("already exists"), "{text}");
        assert!(text.contains("overwrite"), "{text}");
        let on_disk = fs::read_to_string(tmp.path().join("content/docs/intro.md")).unwrap();
        assert!(on_disk.contains("original") && on_disk.contains("keep"));

        let replaced = call_ok(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "docs", "title": "Intro", "body": "replacement", "overwrite": true }),
        );
        assert_eq!(replaced["overwritten"], true);
    }

    #[test]
    fn test_create_content_empty_slug_is_error() {
        let (tmp, mut state) = site("");
        let text = call_err(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "docs", "title": "!!!" }),
        );
        assert!(text.contains("slug"), "{text}");
        assert!(!tmp.path().join("content/docs/.md").exists());
    }

    #[test]
    fn test_create_content_all_options() {
        let (tmp, mut state) = site("\n[languages.es]\ntitle = \"Sitio\"\n");
        let result = call_ok(
            &mut state,
            "seite_create_content",
            serde_json::json!({
                "collection": "docs",
                "title": "Guía",
                "slug": "intro",
                "description": "Start here",
                "weight": 2,
                "draft": true,
                "extra": { "hero": { "image": "/a.png" } },
                "subdir": "guides",
                "lang": "es",
            }),
        );
        assert_eq!(result["path"], "content/docs/guides/intro.es.md");
        assert_eq!(result["url"], "/es/docs/guides/intro");
        assert_eq!(result["lang"], "es");
        assert!(result["notes"].as_array().unwrap().len() == 2);
        let (fm, body) =
            content::parse_content_file(&tmp.path().join("content/docs/guides/intro.es.md"))
                .unwrap();
        assert_eq!(fm.description.as_deref(), Some("Start here"));
        assert_eq!(fm.weight, Some(2));
        assert!(fm.draft);
        assert!(fm.extra.contains_key("hero"));
        assert!(body.trim().is_empty(), "no placeholder body: {body:?}");
    }

    #[test]
    fn test_create_content_rejects_subdir_escape() {
        let (tmp, mut state) = site("");
        for bad in ["../../etc", "/tmp/x", "a/../../b"] {
            let text = call_err(
                &mut state,
                "seite_create_content",
                serde_json::json!({ "collection": "docs", "title": "X", "subdir": bad }),
            );
            assert!(text.contains("invalid subdir"), "{bad}: {text}");
        }
        assert!(!tmp.path().join("etc").exists());
    }

    #[test]
    fn test_create_content_unknown_collection_lists_available() {
        let (_tmp, mut state) = site("");
        let text = call_err(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "blog", "title": "X" }),
        );
        assert!(text.contains("Unknown collection 'blog'"), "{text}");
        assert!(text.contains("posts, docs"), "{text}");
    }

    // --- seite_search ------------------------------------------------------

    #[test]
    fn test_search_ranks_and_limits() {
        let (tmp, mut state) = site("");
        write(
            tmp.path(),
            "content/docs/body.md",
            "---\ntitle: Alpha\n---\nmentions rust twice: rust\n",
        );
        write(
            tmp.path(),
            "content/docs/tagged.md",
            "---\ntitle: Beta\ntags: [rust]\n---\nnothing\n",
        );
        write(
            tmp.path(),
            "content/posts/2026-01-01-titled.md",
            "---\ntitle: Rust Guide\n---\nnothing\n",
        );
        write(
            tmp.path(),
            "content/docs/bad.md",
            "---\ntitle: [unclosed\n---\n",
        );

        let result = call_ok(
            &mut state,
            "seite_search",
            serde_json::json!({ "query": "Rust" }),
        );
        assert_eq!(result["total"], 3);
        assert_eq!(result["returned"], 3);
        let titles: Vec<&str> = result["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["title"].as_str().unwrap())
            .collect();
        assert_eq!(titles, vec!["Rust Guide", "Beta", "Alpha"]);
        assert_eq!(
            result["results"][0]["path"],
            "content/posts/2026-01-01-titled.md"
        );
        assert_eq!(result["results"][0]["url"], "/posts/titled");
        assert_eq!(result["parse_errors"][0]["path"], "content/docs/bad.md");

        let limited = call_ok(
            &mut state,
            "seite_search",
            serde_json::json!({ "query": "rust", "limit": 1 }),
        );
        assert_eq!(limited["total"], 3);
        assert_eq!(limited["returned"], 1);
        assert!(limited["note"].is_string());

        // Singular alias resolves like create does.
        let posts = call_ok(
            &mut state,
            "seite_search",
            serde_json::json!({ "query": "rust", "collection": "post" }),
        );
        assert_eq!(posts["total"], 1);
    }

    #[test]
    fn test_search_empty_query_and_bad_limit_are_errors() {
        let (_tmp, mut state) = site("");
        assert!(call_err(
            &mut state,
            "seite_search",
            serde_json::json!({ "query": "  " })
        )
        .contains("must not be empty"));
        assert!(call_err(
            &mut state,
            "seite_search",
            serde_json::json!({ "query": "x", "limit": 0 })
        )
        .contains("between 1"));
        assert!(call_err(
            &mut state,
            "seite_search",
            serde_json::json!({ "query": "x", "collection": "nope" })
        )
        .contains("Unknown collection"));
    }

    // --- seite_build -------------------------------------------------------

    #[test]
    fn test_build_reports_template_fallback_and_broken_links() {
        let (tmp, mut state) = site("");
        write(
            tmp.path(),
            "content/posts/2026-01-01-hello.md",
            "---\ntitle: Hello\n---\nSee [missing](/posts/does-not-exist).\n",
        );
        write(tmp.path(), "templates/base.html", "<html>{% if %}</html>");

        let result = call_ok(&mut state, "seite_build", serde_json::json!({}));
        assert_eq!(result["success"], true);
        let warnings = result["warnings"].as_array().unwrap();
        assert!(warnings.iter().any(|w| w
            .as_str()
            .unwrap()
            .contains("failed to parse user templates")));
        let broken = result["broken_links"].as_array().unwrap();
        assert!(broken.iter().any(|b| b["target"] == "/posts/does-not-exist"
            && !b["sources"].as_array().unwrap().is_empty()));

        let text = call_err(
            &mut state,
            "seite_build",
            serde_json::json!({ "strict": true }),
        );
        assert!(text.contains("strict=true"), "{text}");
        assert!(text.contains("/posts/does-not-exist"), "{text}");
    }

    #[test]
    fn test_build_clean_site_has_no_warnings() {
        let (tmp, mut state) = site("");
        write(
            tmp.path(),
            "content/posts/2026-01-01-hello.md",
            "---\ntitle: Hello\n---\nHi.\n",
        );
        let result = call_ok(
            &mut state,
            "seite_build",
            serde_json::json!({ "strict": true }),
        );
        assert_eq!(result["warnings"].as_array().unwrap().len(), 0);
        assert_eq!(result["broken_links"].as_array().unwrap().len(), 0);
    }

    // --- seite_apply_theme -------------------------------------------------

    #[test]
    fn test_apply_theme_rejects_path_escape() {
        let (tmp, mut state) = site("");
        let evil = tmp.path().join("evil");
        fs::write(tmp.path().join("evil.tera"), "<html>evil</html>").unwrap();
        let text = call_err(
            &mut state,
            "seite_apply_theme",
            serde_json::json!({ "name": evil.to_string_lossy() }),
        );
        assert!(text.contains("invalid theme name"), "{text}");
        assert!(!tmp.path().join("templates/base.html").exists());
    }

    #[test]
    fn test_apply_theme_backs_up_custom_and_uses_template_dir() {
        let (tmp, mut state) = site("\n[build]\ntemplate_dir = \"layouts\"\n");
        write(
            tmp.path(),
            "layouts/base.html",
            "<html>my custom work</html>",
        );
        let result = call_ok(
            &mut state,
            "seite_apply_theme",
            serde_json::json!({ "name": "dark" }),
        );
        assert_eq!(result["path"], "layouts/base.html");
        assert_eq!(result["backup"], "layouts/base.html.bak");
        assert_eq!(
            fs::read_to_string(tmp.path().join("layouts/base.html.bak")).unwrap(),
            "<html>my custom work</html>"
        );
        assert!(!tmp.path().join("templates/base.html").exists());
    }

    #[test]
    fn test_apply_theme_unknown_is_error() {
        let (_tmp, mut state) = site("");
        let text = call_err(
            &mut state,
            "seite_apply_theme",
            serde_json::json!({ "name": "nope" }),
        );
        assert!(text.contains("unknown theme"), "{text}");
    }
}
