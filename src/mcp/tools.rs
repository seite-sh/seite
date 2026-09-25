//! MCP tool implementations — actions AI tools can execute.
//!
//! Tools are invoked via `tools/call` and return structured results. Each
//! tool wraps existing seite library functionality (never a subprocess).
//!
//! Tools are registered in the [`TOOLS`] table: name, display title,
//! description, input/output JSON Schemas, behavior annotations, and handler.
//! `tools/list` and `tools/call` are both driven by that table, so adding a
//! tool is one entry plus its handler.
//!
//! Tool-execution failures (bad or missing arguments, no site, failed build,
//! unknown theme, ...) are returned as ordinary tool results with
//! `isError: true` and an actionable message, per the MCP spec. Only protocol
//! problems (missing tool name, unknown tool) become JSON-RPC errors.
//!
//! Successful results carry the JSON object as compact text and — on
//! revisions that define it — as `structuredContent`. A tool declares an
//! `outputSchema` only when every success path conforms to it (strict clients
//! reject non-conforming results); a test validates each declared schema
//! against real outputs.

mod collection;
mod page;
mod stats;
mod templates;

use std::collections::HashMap;

use super::content_index::{self, IndexedItem};
use super::protocol::Protocol;
use super::{JsonRpcError, ServerState};
use crate::build::{self, links};
use crate::content::create::{create_content_file, NewContent};
use crate::{config, content, themes};

/// Maximum characters of documentation returned by `seite_lookup_docs`.
const DOCS_OUTPUT_BUDGET: usize = 8_000;
/// Maximum characters per matched documentation section.
const DOCS_SECTION_MAX: usize = 500;
/// Default and maximum `limit` for `seite_search`.
const SEARCH_DEFAULT_LIMIT: u64 = 20;
const SEARCH_MAX_LIMIT: u64 = 100;

// ---------------------------------------------------------------------------
// Tool registry
// ---------------------------------------------------------------------------

/// MCP tool annotations (2025-03-26+). Hints only — clients must not trust
/// them for security — but they drive auto-approval and UI in many clients,
/// so they must be accurate.
#[derive(Debug, Clone, Copy)]
pub struct Annotations {
    /// Never modifies its environment.
    pub read_only: bool,
    /// May delete or overwrite existing data (meaningful when not read-only).
    pub destructive: bool,
    /// Repeating the call with the same arguments has no additional effect.
    pub idempotent: bool,
    /// Interacts with an open world of external entities (network, ...).
    pub open_world: bool,
}

const READ_ONLY: Annotations = Annotations {
    read_only: true,
    destructive: false,
    idempotent: true,
    open_world: false,
};

/// One tool exposed by the server.
pub struct ToolDef {
    pub name: &'static str,
    /// Human-readable display name (`title`, 2025-06-18+).
    pub title: &'static str,
    pub description: &'static str,
    /// Plain object schema: no top-level combinators, typed properties,
    /// `additionalProperties: false` (see `test_input_schema_invariants`).
    pub input_schema: fn() -> serde_json::Value,
    /// Declared only when every success path conforms.
    pub output_schema: Option<fn() -> serde_json::Value>,
    pub annotations: Annotations,
    pub handler: fn(&ServerState, &serde_json::Value) -> ToolResult,
}

/// Every tool this server exposes, in `tools/list` order.
pub static TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "seite_build",
        title: "Build site",
        description: "Build the site to the output directory. Returns build statistics, `warnings` (e.g. custom templates that failed to parse and were replaced by built-in defaults) and `broken_links` ([{target, sources}] of internal links pointing at pages that don't exist). With strict=true, any warning or broken link makes the call fail (isError) with the details.",
        input_schema: build_input_schema,
        output_schema: Some(build_output_schema),
        // Rewrites generated output only; the same sources give the same output.
        annotations: Annotations {
            read_only: false,
            destructive: false,
            idempotent: true,
            open_world: false,
        },
        handler: call_build,
    },
    ToolDef {
        name: "seite_check",
        title: "Check site",
        description: "Report every problem in the site without touching the output directory: seite.toml syntax and unknown keys, frontmatter, shortcodes, data files, templates, render errors, broken internal links and missing assets. Each diagnostic has severity, a stable `code`, message, and the source `file`/`line` when known. `ok` is false when there are errors (or any warning with strict=true). Run it after making changes.",
        input_schema: check_input_schema,
        output_schema: Some(check_output_schema),
        // Renders into a scratch directory that is deleted afterwards.
        annotations: READ_ONLY,
        handler: call_check,
    },
    ToolDef {
        name: "seite_create_content",
        title: "Create content",
        description: "Create a new content file with frontmatter in a collection (same rules as `seite new`). Refuses to replace an existing file unless overwrite=true. Returns the source `path` (relative to the site root) and the `url` it will be published at.",
        input_schema: create_content_input_schema,
        output_schema: Some(create_content_output_schema),
        // overwrite=true replaces an existing file.
        annotations: Annotations {
            read_only: false,
            destructive: true,
            idempotent: false,
            open_world: false,
        },
        handler: call_create_content,
    },
    ToolDef {
        name: "seite_get_page",
        title: "Get page",
        description: "Inspect one content file as the build sees it. Pass exactly one of `path` (source file) or `url` (published URL). Returns the resolved frontmatter (JSON), url, lang, draft, collection, word count, the markdown body, the rendered body HTML (markdown + shortcodes, without the page template), and `output_path` when a built file exists.",
        input_schema: page::get_page_input_schema,
        output_schema: Some(page::get_page_output_schema),
        annotations: READ_ONLY,
        handler: page::call_get_page,
    },
    ToolDef {
        name: "seite_update_frontmatter",
        title: "Update frontmatter",
        description: "Edit only the frontmatter of a content file under the content directory; the markdown body is preserved byte-for-byte. `set` replaces top-level keys (e.g. {\"tags\": [\"rust\"], \"draft\": false, \"extra\": {...}}), `unset` removes keys. The result must still parse and keep its required fields (title; a date for dated collections); nothing is written otherwise. Only the lines of the keys you set/unset change (new keys go at the end), so comments and the formatting of other keys are preserved. Returns the new frontmatter.",
        input_schema: page::update_frontmatter_input_schema,
        output_schema: Some(page::update_frontmatter_output_schema),
        annotations: Annotations {
            read_only: false,
            destructive: false,
            idempotent: true,
            open_world: false,
        },
        handler: page::call_update_frontmatter,
    },
    ToolDef {
        name: "seite_search",
        title: "Search content",
        description: "Search site content (including drafts) by keyword. Matches titles, descriptions, tags, and body text; results are ranked title > description/tags > body. Returns `total` matches and the `returned` subset, each with source `path` and published `url`.",
        input_schema: search_input_schema,
        output_schema: Some(search_output_schema),
        annotations: READ_ONLY,
        handler: call_search,
    },
    ToolDef {
        name: "seite_content_stats",
        title: "Content stats",
        description: "Content health overview: per-collection counts, drafts, items missing a description or tags, future-dated items, unparseable files, and (multilingual sites) items missing a translation per configured language. Optionally limited to one collection.",
        input_schema: stats::input_schema,
        output_schema: Some(stats::output_schema),
        annotations: READ_ONLY,
        handler: stats::call_content_stats,
    },
    ToolDef {
        name: "seite_list_templates",
        title: "List templates",
        description: "Everything needed before editing templates: user templates and which embedded defaults they override, the blocks defined by the active base template, the Tera context variables available to each template kind, built-in and custom shortcodes with their parameters, and the keys of loaded data files.",
        input_schema: empty_input_schema,
        output_schema: Some(templates::output_schema),
        annotations: READ_ONLY,
        handler: templates::call_list_templates,
    },
    ToolDef {
        name: "seite_apply_theme",
        title: "Apply theme",
        description: "Apply a bundled or installed theme by writing base.html in the site's template directory. If the current base.html has been customized (matches no known theme), it is first backed up to base.html.bak (or base.html.bak.N) and the backup path is returned.",
        input_schema: apply_theme_input_schema,
        output_schema: Some(apply_theme_output_schema),
        // Customized templates are backed up before being replaced.
        annotations: Annotations {
            read_only: false,
            destructive: false,
            idempotent: true,
            open_world: false,
        },
        handler: call_apply_theme,
    },
    ToolDef {
        name: "seite_create_collection",
        title: "Add collection",
        description: "Add a collection to seite.toml from a preset (posts, docs, pages, changelog, roadmap, trust) and create its content directory — the same as `seite collection add`. Fails if the collection already exists.",
        input_schema: collection::input_schema,
        output_schema: Some(collection::output_schema),
        annotations: Annotations {
            read_only: false,
            destructive: false,
            idempotent: false,
            open_world: false,
        },
        handler: collection::call_create_collection,
    },
    ToolDef {
        name: "seite_lookup_docs",
        title: "Look up seite docs",
        description: "Look up seite documentation by topic slug or search it by keyword. With no arguments, returns the list of topics. Output is capped at ~8 KB; narrow broad queries or pass a topic.",
        input_schema: lookup_docs_input_schema,
        // Three result shapes (topic page, search results, topic index) would
        // need a oneOf union, which strict clients handle poorly: no schema.
        output_schema: None,
        annotations: READ_ONLY,
        handler: lookup_docs_tool,
    },
];

/// Whether a tool with this name is registered.
pub fn has_tool(name: &str) -> bool {
    TOOLS.iter().any(|t| t.name == name)
}

fn tool_names() -> Vec<&'static str> {
    TOOLS.iter().map(|t| t.name).collect()
}

/// Handle `tools/list` — enumerate all tools, with fields gated on the
/// negotiated protocol revision.
pub fn list(proto: Protocol) -> serde_json::Value {
    let tools: Vec<serde_json::Value> = TOOLS.iter().map(|t| tool_json(t, proto)).collect();
    serde_json::json!({ "tools": tools })
}

fn tool_json(tool: &ToolDef, proto: Protocol) -> serde_json::Value {
    let mut value = serde_json::json!({
        "name": tool.name,
        "description": tool.description,
        "inputSchema": (tool.input_schema)(),
    });
    if proto.titles() {
        value["title"] = tool.title.into();
    }
    if proto.tool_annotations() {
        let a = tool.annotations;
        value["annotations"] = serde_json::json!({
            // `annotations.title` is the display name on 2025-03-26.
            "title": tool.title,
            "readOnlyHint": a.read_only,
            "destructiveHint": a.destructive,
            "idempotentHint": a.idempotent,
            "openWorldHint": a.open_world,
        });
    }
    if proto.structured_output() {
        if let Some(schema) = tool.output_schema {
            value["outputSchema"] = schema();
        }
    }
    value
}

// ---------------------------------------------------------------------------
// Schema helpers
// ---------------------------------------------------------------------------

/// `{"type": ty, "description": desc}`.
fn prop(ty: &str, description: &str) -> serde_json::Value {
    serde_json::json!({ "type": ty, "description": description })
}

/// Object schema with the given properties and required keys (no
/// `additionalProperties` restriction — used for output schemas).
fn object_schema(properties: serde_json::Value, required: &[&str]) -> serde_json::Value {
    let mut schema = serde_json::json!({ "type": "object", "properties": properties });
    if !required.is_empty() {
        schema["required"] = serde_json::json!(required);
    }
    schema
}

/// Closed object schema for tool inputs.
fn input_object(properties: serde_json::Value, required: &[&str]) -> serde_json::Value {
    let mut schema = object_schema(properties, required);
    schema["additionalProperties"] = false.into();
    schema
}

fn empty_input_schema() -> serde_json::Value {
    input_object(serde_json::json!({}), &[])
}

fn string_array() -> serde_json::Value {
    serde_json::json!({ "type": "array", "items": { "type": "string" } })
}

fn nullable(ty: &str) -> serde_json::Value {
    serde_json::json!({ "type": [ty, "null"] })
}

fn build_input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
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
        }),
        &[],
    )
}

fn build_output_schema() -> serde_json::Value {
    object_schema(
        serde_json::json!({
            "success": { "type": "boolean" },
            "items_built": {
                "type": "object",
                "description": "Items rendered per collection",
                "additionalProperties": { "type": "integer" }
            },
            "static_files_copied": { "type": "integer" },
            "public_files_copied": { "type": "integer" },
            "data_files_loaded": { "type": "integer" },
            "duration_ms": { "type": "integer" },
            "output_dir": prop("string", "Output directory relative to the site root"),
            "links_checked": { "type": "integer" },
            "warnings": string_array(),
            "broken_links": {
                "type": "array",
                "items": object_schema(
                    serde_json::json!({ "target": { "type": "string" }, "sources": string_array() }),
                    &["target", "sources"],
                )
            },
            "missing_assets": {
                "type": "array",
                "items": object_schema(
                    serde_json::json!({ "target": { "type": "string" }, "sources": string_array() }),
                    &["target", "sources"],
                )
            },
            "diagnostics": { "type": "array", "items": diagnostic_schema() },
            "subdomain_builds": {
                "type": "array",
                "items": object_schema(
                    serde_json::json!({
                        "collection": { "type": "string" },
                        "subdomain": { "type": "string" },
                        "base_url": { "type": "string" },
                        "output_dir": { "type": "string" }
                    }),
                    &["collection", "subdomain", "base_url", "output_dir"],
                )
            }
        }),
        &[
            "success",
            "items_built",
            "static_files_copied",
            "public_files_copied",
            "data_files_loaded",
            "duration_ms",
            "output_dir",
            "links_checked",
            "warnings",
            "broken_links",
            "missing_assets",
            "diagnostics",
        ],
    )
}

fn diagnostic_schema() -> serde_json::Value {
    object_schema(
        serde_json::json!({
            "severity": { "type": "string", "enum": ["error", "warning"] },
            "code": prop("string", "Stable identifier, e.g. frontmatter-parse, broken-link"),
            "message": { "type": "string" },
            "file": prop("string", "Source file relative to the site root"),
            "line": { "type": "integer" },
            "column": { "type": "integer" },
            "hint": { "type": "string" }
        }),
        &["severity", "code", "message"],
    )
}

fn check_input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
            "drafts": {
                "type": "boolean",
                "description": "Also check draft content",
                "default": false
            },
            "strict": {
                "type": "boolean",
                "description": "Treat warnings (broken links, missing assets, unknown config keys) as failures",
                "default": false
            }
        }),
        &[],
    )
}

fn check_output_schema() -> serde_json::Value {
    object_schema(
        serde_json::json!({
            "ok": { "type": "boolean" },
            "summary": object_schema(
                serde_json::json!({ "errors": { "type": "integer" }, "warnings": { "type": "integer" } }),
                &["errors", "warnings"],
            ),
            "diagnostics": { "type": "array", "items": diagnostic_schema() }
        }),
        &["ok", "summary", "diagnostics"],
    )
}

fn create_content_input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
            "collection": prop("string", "Collection name (e.g., posts, docs, pages, changelog, roadmap). Singular aliases like 'post' work."),
            "title": prop("string", "Title of the content"),
            "slug": prop("string", "Filename slug (lowercase letters, digits, '-', '_'). Defaults to a slug of the title."),
            "description": prop("string", "Frontmatter description (used for meta description and listings)"),
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Tags for the content"
            },
            "body": prop("string", "Markdown body content. Omit to create a file with frontmatter only."),
            "draft": {
                "type": "boolean",
                "description": "Create as draft (excluded from builds unless seite_build drafts=true)",
                "default": false
            },
            "weight": prop("integer", "Ordering weight for non-date collections (lower sorts first)"),
            "extra": {
                "type": "object",
                "description": "Arbitrary frontmatter data exposed to templates as page.extra",
                "additionalProperties": true
            },
            "subdir": prop("string", "Sub-directory inside a nested collection, e.g. 'guides' (docs only; no '..' or absolute paths)"),
            "lang": prop("string", "Language code for a translation (must be configured under [languages]); adds a .{lang}.md suffix"),
            "overwrite": {
                "type": "boolean",
                "description": "Replace the file if it already exists",
                "default": false
            }
        }),
        &["collection", "title"],
    )
}

fn create_content_output_schema() -> serde_json::Value {
    object_schema(
        serde_json::json!({
            "path": prop("string", "Source path relative to the site root"),
            "url": prop("string", "Published URL path"),
            "slug": { "type": "string" },
            "lang": { "type": "string" },
            "collection": { "type": "string" },
            "draft": { "type": "boolean" },
            "overwritten": { "type": "boolean" },
            "notes": string_array()
        }),
        &[
            "path",
            "url",
            "slug",
            "lang",
            "collection",
            "draft",
            "overwritten",
        ],
    )
}

fn search_input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
            "query": prop("string", "Search keywords (case-insensitive substring match)"),
            "collection": prop("string", "Limit search to a specific collection (singular aliases like 'post' work)"),
            "limit": {
                "type": "integer",
                "description": "Maximum results to return (1-100)",
                "default": 20,
                "minimum": 1,
                "maximum": 100
            }
        }),
        &["query"],
    )
}

fn search_output_schema() -> serde_json::Value {
    object_schema(
        serde_json::json!({
            "query": { "type": "string" },
            "total": { "type": "integer" },
            "returned": { "type": "integer" },
            "results": {
                "type": "array",
                "items": object_schema(
                    serde_json::json!({
                        "title": { "type": "string" },
                        "collection": { "type": "string" },
                        "slug": { "type": "string" },
                        "url": { "type": "string" },
                        "path": { "type": "string" },
                        "lang": { "type": "string" },
                        "draft": { "type": "boolean" },
                        "tags": string_array(),
                        "description": nullable("string"),
                        "date": nullable("string"),
                        "matched_in": string_array(),
                        "excerpt": { "type": "string" }
                    }),
                    &["title", "collection", "url", "path", "lang", "draft", "matched_in"],
                )
            },
            "note": { "type": "string" },
            "parse_errors": {
                "type": "array",
                "items": object_schema(
                    serde_json::json!({ "path": { "type": "string" }, "error": { "type": "string" } }),
                    &["path", "error"],
                )
            }
        }),
        &["query", "total", "returned", "results"],
    )
}

fn apply_theme_input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
            "name": prop("string", "Theme name: default, minimal, dark, docs, brutalist, bento, landing, terminal, magazine, academic, or an installed theme (lowercase letters, digits, hyphens)")
        }),
        &["name"],
    )
}

fn apply_theme_output_schema() -> serde_json::Value {
    object_schema(
        serde_json::json!({
            "applied": { "type": "boolean" },
            "theme": { "type": "string" },
            "description": { "type": "string" },
            "source": prop("string", "bundled or installed"),
            "path": prop("string", "The base.html that was written, relative to the site root"),
            "backup": prop("string", "Backup of the previous customized base.html"),
            "note": { "type": "string" }
        }),
        &["applied", "theme", "source", "path"],
    )
}

fn lookup_docs_input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
            "query": prop("string", "Search keywords to find in documentation"),
            "topic": prop("string", "Specific doc topic slug (e.g., configuration, templates, deployment)")
        }),
        &[],
    )
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

pub type ToolResult = Result<serde_json::Value, ToolError>;

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
                if accepted.is_empty() {
                    "(none)".to_string()
                } else {
                    accepted.join(", ")
                }
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

/// A successful result: compact JSON text (read by every client) plus, on
/// revisions that define it, the same object as `structuredContent`.
fn success_result(value: serde_json::Value, proto: Protocol) -> serde_json::Value {
    let text = serde_json::to_string(&value).unwrap_or_default();
    let mut result = serde_json::json!({ "content": [text_block(text)] });
    if proto.structured_output() && value.is_object() {
        result["structuredContent"] = value;
    }
    result
}

/// A failed result: text only (never `structuredContent`, so it cannot fail
/// output-schema validation in strict clients).
fn error_result(err: ToolError) -> serde_json::Value {
    let mut text = err.message;
    if let Some(details) = err.details {
        text.push_str("\n\n");
        text.push_str(&serde_json::to_string(&details).unwrap_or_default());
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
    state: &ServerState,
    proto: Protocol,
    params: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'name' parameter"))?;

    let tool = TOOLS.iter().find(|t| t.name == name).ok_or_else(|| {
        JsonRpcError::invalid_params(format!(
            "Unknown tool: {name}. Valid tools: {}",
            tool_names().join(", ")
        ))
    })?;

    let empty = serde_json::json!({});
    let arguments = match params.get("arguments") {
        None | Some(serde_json::Value::Null) => &empty,
        Some(args) => args,
    };

    Ok(match (tool.handler)(state, arguments) {
        Ok(value) => success_result(value, proto),
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

    let missing_assets: Vec<serde_json::Value> =
        links::group_broken_links(&result.link_check.missing_assets)
            .into_iter()
            .map(|(target, sources)| serde_json::json!({ "target": target, "sources": sources }))
            .collect();
    let diagnostics = result.diagnostics.clone().relative_to(&paths.root);

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
        "missing_assets": missing_assets,
        "diagnostics": diagnostics,
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

    if strict
        && (!result.warnings.is_empty() || !broken_links.is_empty() || !missing_assets.is_empty())
    {
        response["success"] = serde_json::json!(false);
        return Err(ToolError::with_details(
            format!(
                "Build finished with {} warning(s), {} broken link target(s) and {} missing asset(s); failing because strict=true. \
                 Fix the problems below (or call without strict to accept them).",
                result.warnings.len(),
                broken_links.len(),
                missing_assets.len()
            ),
            response,
        ));
    }

    Ok(response)
}

// ---------------------------------------------------------------------------
// seite_check
// ---------------------------------------------------------------------------

fn call_check(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    let args = Args::new("seite_check", arguments, &["drafts", "strict"])?;
    let include_drafts = args.bool("drafts")?.unwrap_or(false);
    let strict = args.bool("strict")?.unwrap_or(false);
    let (_config, paths) = state.site()?;

    let diagnostics = crate::cli::check::check_site(&paths.root, include_drafts)
        .map_err(|e| ToolError::new(format!("Check could not run: {e}")))?;
    let errors = diagnostics.error_count();
    let warnings = diagnostics.warning_count();
    Ok(serde_json::json!({
        "ok": errors == 0 && !(strict && warnings > 0),
        "summary": { "errors": errors, "warnings": warnings },
        "diagnostics": diagnostics,
    }))
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

fn lookup_docs_tool(_state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    call_lookup_docs(arguments)
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
mod test_support;

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::mcp::protocol::{Protocol, LEGACY_VERSIONS, MODERN_VERSIONS};
    use std::fs;
    use tempfile::TempDir;

    /// Newest revision (all optional fields enabled).
    fn newest() -> Protocol {
        Protocol::new(MODERN_VERSIONS[0])
    }

    #[test]
    fn test_list_returns_all_tools() {
        let result = list(newest());
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), TOOLS.len());
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, tool_names());
        for expected in [
            "seite_build",
            "seite_create_content",
            "seite_search",
            "seite_apply_theme",
            "seite_lookup_docs",
            "seite_get_page",
            "seite_list_templates",
            "seite_update_frontmatter",
            "seite_create_collection",
            "seite_content_stats",
        ] {
            assert!(has_tool(expected), "{expected}");
        }
    }

    /// Cursor caps enabled tools (~40 across all servers) and the combined
    /// server + tool name length (~60); tool names must be plain identifiers.
    #[test]
    fn test_tool_names_fit_client_limits() {
        assert!(TOOLS.len() <= 12, "keep the tool count <= 12");
        let mut seen = std::collections::HashSet::new();
        for tool in TOOLS {
            assert!(seen.insert(tool.name), "duplicate tool {}", tool.name);
            assert!(tool.name.starts_with("seite_"), "{}", tool.name);
            assert!(
                tool.name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{}",
                tool.name
            );
            // Claude Code's qualified form is the longest common prefixing.
            let qualified = format!("mcp__seite__{}", tool.name);
            assert!(qualified.len() <= 60, "{qualified} is too long");
            assert!(!tool.title.is_empty() && !tool.description.is_empty());
        }
    }

    fn assert_plain_property(tool: &str, path: &str, schema: &serde_json::Value) {
        let obj = schema
            .as_object()
            .unwrap_or_else(|| panic!("{tool}.{path}: property schema must be an object"));
        for combinator in ["oneOf", "anyOf", "allOf", "not", "$ref", "if"] {
            assert!(
                !obj.contains_key(combinator),
                "{tool}.{path}: no {combinator}"
            );
        }
        let ty = obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or_else(|| panic!("{tool}.{path}: every property needs a single `type`"));
        match ty {
            "object" => {
                if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
                    for (k, v) in props {
                        assert_plain_property(tool, &format!("{path}.{k}"), v);
                    }
                } else {
                    assert_eq!(
                        obj.get("additionalProperties"),
                        Some(&serde_json::Value::Bool(true)),
                        "{tool}.{path}: free-form objects must say additionalProperties: true"
                    );
                }
            }
            "array" => {
                let items = obj
                    .get("items")
                    .unwrap_or_else(|| panic!("{tool}.{path}: arrays need `items`"));
                assert_plain_property(tool, &format!("{path}[]"), items);
            }
            "string" | "integer" | "number" | "boolean" => {}
            other => panic!("{tool}.{path}: unsupported type {other}"),
        }
    }

    /// Input schemas must survive every client's schema conversion (OpenAI
    /// function schemas for Codex, Cursor, OpenCode): a plain closed object
    /// with typed properties and no combinators.
    #[test]
    fn test_input_schema_invariants() {
        for tool in TOOLS {
            let schema = (tool.input_schema)();
            let obj = schema.as_object().unwrap();
            assert_eq!(obj["type"], "object", "{}", tool.name);
            assert!(obj["properties"].is_object(), "{}", tool.name);
            assert_eq!(obj["additionalProperties"], false, "{}", tool.name);
            for key in obj.keys() {
                assert!(
                    ["type", "properties", "required", "additionalProperties"]
                        .contains(&key.as_str()),
                    "{}: unexpected top-level schema key {key}",
                    tool.name
                );
            }
            let props = obj["properties"].as_object().unwrap();
            for (k, v) in props {
                assert_plain_property(tool.name, k, v);
            }
            if let Some(required) = obj.get("required") {
                let required = required.as_array().unwrap();
                assert!(!required.is_empty(), "{}: omit empty required", tool.name);
                for r in required {
                    assert!(props.contains_key(r.as_str().unwrap()), "{}", tool.name);
                }
            }
            // The schema itself must be valid JSON Schema.
            jsonschema::validator_for(&schema).unwrap();
        }
    }

    #[test]
    fn test_output_schemas_are_valid_object_schemas() {
        for tool in TOOLS {
            if let Some(schema) = tool.output_schema {
                let schema = schema();
                assert_eq!(schema["type"], "object", "{}", tool.name);
                jsonschema::validator_for(&schema).unwrap_or_else(|e| panic!("{}: {e}", tool.name));
            }
        }
    }

    #[test]
    fn test_annotations_match_tool_behavior() {
        let get = |name: &str| TOOLS.iter().find(|t| t.name == name).unwrap().annotations;
        for name in [
            "seite_search",
            "seite_lookup_docs",
            "seite_get_page",
            "seite_list_templates",
            "seite_content_stats",
        ] {
            let a = get(name);
            assert!(a.read_only && !a.destructive && a.idempotent, "{name}");
        }
        for name in [
            "seite_build",
            "seite_create_content",
            "seite_apply_theme",
            "seite_update_frontmatter",
            "seite_create_collection",
        ] {
            assert!(!get(name).read_only, "{name}");
        }
        assert!(get("seite_create_content").destructive);
        assert!(!get("seite_update_frontmatter").destructive);
        assert!(get("seite_update_frontmatter").idempotent);
        assert!(!get("seite_create_collection").idempotent);
        assert!(TOOLS.iter().all(|t| !t.annotations.open_world));
    }

    #[test]
    fn test_list_fields_gated_by_protocol() {
        let old = list(Protocol::new("2024-11-05"));
        for tool in old["tools"].as_array().unwrap() {
            assert!(tool.get("annotations").is_none());
            assert!(tool.get("title").is_none());
            assert!(tool.get("outputSchema").is_none());
        }
        let march = list(Protocol::new("2025-03-26"));
        for tool in march["tools"].as_array().unwrap() {
            assert!(tool["annotations"]["title"].is_string());
            assert!(tool["annotations"]["readOnlyHint"].is_boolean());
            assert!(tool.get("title").is_none());
            assert!(tool.get("outputSchema").is_none());
        }
        for version in LEGACY_VERSIONS[..2].iter().chain(MODERN_VERSIONS) {
            let new = list(Protocol::new(version));
            for (tool, def) in new["tools"].as_array().unwrap().iter().zip(TOOLS) {
                assert_eq!(tool["title"], def.title);
                assert_eq!(
                    tool.get("outputSchema").is_some(),
                    def.output_schema.is_some()
                );
                for hint in [
                    "readOnlyHint",
                    "destructiveHint",
                    "idempotentHint",
                    "openWorldHint",
                ] {
                    assert!(tool["annotations"][hint].is_boolean(), "{version} {hint}");
                }
            }
        }
    }

    #[test]
    fn test_structured_content_gated_by_protocol() {
        let state = empty_state();
        let params = serde_json::json!({ "name": "seite_lookup_docs", "arguments": {} });
        let old = call(&state, Protocol::new("2025-03-26"), &params).unwrap();
        assert!(old.get("structuredContent").is_none());
        let new = call(&state, newest(), &params).unwrap();
        assert!(new["structuredContent"]["available_topics"].is_array());
        let text = new["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains('\n'), "text must be compact JSON");
        // Errors never carry structuredContent.
        let err = call(
            &state,
            newest(),
            &serde_json::json!({ "name": "seite_build", "arguments": {} }),
        )
        .unwrap();
        assert_eq!(err["isError"], true);
        assert!(err.get("structuredContent").is_none());
    }

    /// Every tool that declares an outputSchema is exercised on a real site
    /// and its structuredContent validated (see `test_support::call_tool`).
    #[test]
    fn test_every_output_schema_validates_real_outputs() {
        let (tmp, mut state) = site("\n[languages.es]\ntitle = \"Prueba\"\n");
        write(
            tmp.path(),
            "content/posts/2026-01-02-hello.md",
            "---\ntitle: Hello\ndescription: d\ntags: [a]\n---\nHello {{< youtube(id=\"x\") >}} body\n",
        );
        write(
            tmp.path(),
            "content/docs/guides/intro.md",
            "---\ntitle: Intro\n---\n[missing](/docs/nope)\n",
        );
        write(tmp.path(), "content/docs/broken.md", "no frontmatter");
        write(tmp.path(), "data/authors.yaml", "jane:\n  name: Jane\n");

        let mut covered = Vec::new();
        let mut run = |state: &mut ServerState, name: &'static str, args: serde_json::Value| {
            call_ok(state, name, args);
            covered.push(name);
        };
        run(
            &mut state,
            "seite_search",
            serde_json::json!({ "query": "hello" }),
        );
        run(
            &mut state,
            "seite_search",
            serde_json::json!({ "query": "zzz-no-match" }),
        );
        run(
            &mut state,
            "seite_create_content",
            serde_json::json!({ "collection": "docs", "title": "New Doc", "draft": true }),
        );
        run(
            &mut state,
            "seite_get_page",
            serde_json::json!({ "path": "content/posts/2026-01-02-hello.md" }),
        );
        run(
            &mut state,
            "seite_get_page",
            serde_json::json!({ "url": "/docs/guides/intro" }),
        );
        run(
            &mut state,
            "seite_update_frontmatter",
            serde_json::json!({ "path": "content/docs/guides/intro.md", "set": { "tags": ["x"] } }),
        );
        run(&mut state, "seite_content_stats", serde_json::json!({}));
        run(
            &mut state,
            "seite_content_stats",
            serde_json::json!({ "collection": "posts" }),
        );
        run(&mut state, "seite_list_templates", serde_json::json!({}));
        run(
            &mut state,
            "seite_create_collection",
            serde_json::json!({ "preset": "pages" }),
        );
        state.reload_config();
        // Broken site: errors with file/line, plus a broken-link warning.
        run(&mut state, "seite_check", serde_json::json!({}));
        fs::remove_file(tmp.path().join("content/docs/broken.md")).unwrap();
        // Warnings only; strict turns them into ok=false.
        run(
            &mut state,
            "seite_check",
            serde_json::json!({ "strict": true }),
        );
        run(&mut state, "seite_build", serde_json::json!({}));
        run(
            &mut state,
            "seite_get_page",
            serde_json::json!({ "url": "http://localhost:3000/posts/hello/" }),
        );
        run(
            &mut state,
            "seite_apply_theme",
            serde_json::json!({ "name": "dark" }),
        );
        fs::write(
            tmp.path().join("templates/base.html"),
            "{% block content %}{% endblock %}",
        )
        .unwrap();
        run(
            &mut state,
            "seite_apply_theme",
            serde_json::json!({ "name": "minimal" }),
        );
        run(&mut state, "seite_list_templates", serde_json::json!({}));

        for tool in TOOLS.iter().filter(|t| t.output_schema.is_some()) {
            assert!(
                covered.contains(&tool.name),
                "{} declares an outputSchema but is not exercised here",
                tool.name
            );
        }
    }

    #[test]
    fn test_check_reports_errors_with_location_and_ok_flag() {
        let (tmp, state) = site("");
        write(tmp.path(), "content/docs/broken.md", "no frontmatter");
        write(
            tmp.path(),
            "content/docs/intro.md",
            "---\ntitle: Intro\n---\n[missing](/docs/nope)\n",
        );
        let result = call_check(&state, &serde_json::json!({})).unwrap();
        assert_eq!(result["ok"], false);
        assert!(result["summary"]["errors"].as_u64().unwrap() >= 1);
        let diagnostics = result["diagnostics"].as_array().unwrap();
        assert!(diagnostics
            .iter()
            .any(|d| d["severity"] == "error" && d["file"] == "content/docs/broken.md"));
        assert!(
            !tmp.path().join("dist").exists(),
            "check must not write dist/"
        );

        fs::remove_file(tmp.path().join("content/docs/broken.md")).unwrap();
        let lenient = call_check(&state, &serde_json::json!({})).unwrap();
        assert_eq!(lenient["ok"], true);
        let strict = call_check(&state, &serde_json::json!({ "strict": true })).unwrap();
        assert_eq!(strict["ok"], false);
        assert!(strict["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "broken-link" && d["file"] == "content/docs/intro.md"));
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
        let state = empty_state();
        let err = call(&state, newest(), &serde_json::json!({})).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing 'name'"));
    }

    #[test]
    fn test_call_unknown_tool_lists_valid_tools() {
        let state = empty_state();
        let err = call(
            &state,
            newest(),
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
