//! `seite mcp` — MCP (Model Context Protocol) server over stdio.
//!
//! Implements JSON-RPC 2.0 over stdin/stdout for AI tool integration.
//! Coding agents (Claude Code, Codex CLI, OpenCode, Cursor, ...) connect to
//! this server to get structured access to site documentation,
//! configuration, content, templates, themes, and build tools. Claude Code
//! discovers it through the project's `.mcp.json` file.
//!
//! **Critical**: All logging goes to stderr. Never write to stdout
//! except protocol messages — it would corrupt the JSON-RPC stream.
//!
//! Error semantics follow the MCP spec: problems with a *tool's execution*
//! (bad arguments, no site, failed build, ...) are returned as normal tool
//! results with `isError: true`; JSON-RPC errors are reserved for protocol
//! problems (unknown method, malformed request, unknown tool name).
//!
//! Protocol versions: see [`protocol`] — legacy revisions are negotiated
//! through `initialize`, modern (stateless) revisions are declared per
//! request in `_meta` and advertised by `server/discover`.

pub mod content_index;
pub mod protocol;
pub mod resources;
pub mod tools;

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{ResolvedPaths, SiteConfig};
use protocol::Protocol;

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

/// A validated JSON-RPC request or notification.
struct JsonRpcRequest {
    /// `None` for notifications (no `id` member).
    id: Option<serde_json::Value>,
    method: String,
    params: serde_json::Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// Standard JSON-RPC error codes
const PARSE_ERROR: i32 = -32700;
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;

impl JsonRpcError {
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: INVALID_PARAMS,
            message: msg.into(),
            data: None,
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: INTERNAL_ERROR,
            message: msg.into(),
            data: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Server state
// ---------------------------------------------------------------------------

/// State shared across all handlers. The site config is re-read from disk
/// before every resource/tool request (it is cheap), so edits to
/// `seite.toml` — new collections, fixed syntax errors — are seen immediately.
pub struct ServerState {
    /// Site configuration (None if not in a seite project or it failed to load).
    pub config: Option<SiteConfig>,
    /// Resolved directory paths (None if config not loaded).
    pub paths: Option<ResolvedPaths>,
    /// Project root: the directory containing `seite.toml` (found by walking
    /// up from the launch directory), or the launch directory if none exists.
    pub cwd: PathBuf,
    /// Why `seite.toml` exists but could not be loaded (verbatim load error).
    pub config_error: Option<String>,
}

impl ServerState {
    /// Create state rooted at `root` and load its config.
    pub fn load(root: PathBuf) -> Self {
        let mut state = Self {
            config: None,
            paths: None,
            cwd: root,
            config_error: None,
        };
        state.reload_config();
        state
    }

    /// Re-read `seite.toml` from disk, recording the load error (if any)
    /// instead of discarding it.
    pub fn reload_config(&mut self) {
        let config_path = self.cwd.join("seite.toml");
        self.config = None;
        self.paths = None;
        self.config_error = None;
        if !config_path.exists() {
            return;
        }
        match SiteConfig::load(&config_path) {
            Ok(config) => {
                self.paths = Some(config.resolve_paths(&self.cwd));
                self.config = Some(config);
            }
            Err(e) => {
                self.config_error = Some(format!("Failed to load {}: {e}", config_path.display()));
            }
        }
    }

    /// Actionable explanation of why no site config is available.
    pub fn no_config_message(&self) -> String {
        match &self.config_error {
            Some(err) => format!(
                "{err}\nFix seite.toml and retry — the MCP server re-reads it on every request."
            ),
            None => format!(
                "Not in a seite project: no seite.toml found in {} or any parent directory. \
                 Run `seite init <name>` to create a site, or start `seite mcp` from inside a site directory.",
                self.cwd.display()
            ),
        }
    }

    /// The loaded config and paths, or an actionable error message.
    pub fn site(&self) -> std::result::Result<(&SiteConfig, &ResolvedPaths), String> {
        match (&self.config, &self.paths) {
            (Some(config), Some(paths)) => Ok((config, paths)),
            (Some(_), None) => Err("No project paths configured".to_string()),
            _ => Err(self.no_config_message()),
        }
    }
}

/// Per-connection protocol state: the version negotiated by a legacy
/// `initialize` handshake (modern requests carry their own version).
#[derive(Default)]
struct Session {
    negotiated: Option<&'static str>,
}

/// Find the nearest directory at or above `start` that contains `seite.toml`.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join("seite.toml").is_file())
        .map(Path::to_path_buf)
}

// ---------------------------------------------------------------------------
// Main server loop
// ---------------------------------------------------------------------------

/// Run the MCP server over stdio. Reads JSON-RPC messages from stdin,
/// dispatches to handlers, writes responses to stdout.
pub fn serve() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let root = find_project_root(&cwd).unwrap_or(cwd);
    let mut state = ServerState::load(root);
    let mut session = Session::default();
    if let Some(ref err) = state.config_error {
        eprintln!("MCP: {err}");
    }

    let stdin = io::stdin();
    let reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(resp) = handle_line(&mut state, &mut session, &line) {
            let json = serde_json::to_string(&resp).unwrap_or_default();
            writeln!(writer, "{json}")?;
            writer.flush()?;
        }
    }

    Ok(())
}

/// Process one line of input: a single message or (2025-03-26) a batch.
/// Returns `None` when nothing must be written (notifications only).
fn handle_line(
    state: &mut ServerState,
    session: &mut Session,
    line: &str,
) -> Option<serde_json::Value> {
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return to_value(error_response(
                serde_json::Value::Null,
                PARSE_ERROR,
                format!("Parse error: {e}"),
            ));
        }
    };
    match value {
        serde_json::Value::Array(batch) => {
            if batch.is_empty() {
                return to_value(error_response(
                    serde_json::Value::Null,
                    INVALID_REQUEST,
                    "Invalid request: empty batch".to_string(),
                ));
            }
            let responses: Vec<serde_json::Value> = batch
                .into_iter()
                .filter_map(|msg| handle_message(state, session, msg))
                .filter_map(to_value)
                .collect();
            (!responses.is_empty()).then_some(serde_json::Value::Array(responses))
        }
        msg => handle_message(state, session, msg).and_then(to_value),
    }
}

fn to_value(resp: JsonRpcResponse) -> Option<serde_json::Value> {
    serde_json::to_value(resp).ok()
}

/// Validate the JSON-RPC envelope of one message. On failure, returns the id
/// to echo (only when it is itself valid) and the error message.
fn parse_request(value: serde_json::Value) -> Result<JsonRpcRequest, (serde_json::Value, String)> {
    let serde_json::Value::Object(mut map) = value else {
        return Err((
            serde_json::Value::Null,
            "Invalid request: expected a JSON object".to_string(),
        ));
    };
    let id = map.remove("id");
    let echo_id = match &id {
        Some(v @ (serde_json::Value::String(_) | serde_json::Value::Number(_))) => v.clone(),
        _ => serde_json::Value::Null,
    };
    if map.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return Err((
            echo_id,
            "Invalid request: 'jsonrpc' must be \"2.0\"".to_string(),
        ));
    }
    let method = match map.remove("method") {
        Some(serde_json::Value::String(m)) => m,
        Some(_) => return Err((echo_id, "Invalid request: 'method' must be a string".into())),
        None => return Err((echo_id, "Invalid request: missing field `method`".into())),
    };
    if let Some(ref id) = id {
        if !(id.is_string() || id.is_number()) {
            return Err((
                echo_id,
                "Invalid request: 'id' must be a string or number (MCP forbids null ids)".into(),
            ));
        }
    }
    let params = map.remove("params").unwrap_or(serde_json::Value::Null);
    if !(params.is_null() || params.is_object()) {
        return Err((
            echo_id,
            "Invalid request: 'params' must be an object".into(),
        ));
    }
    Ok(JsonRpcRequest { id, method, params })
}

/// Handle one message. Returns `None` for notifications.
fn handle_message(
    state: &mut ServerState,
    session: &mut Session,
    value: serde_json::Value,
) -> Option<JsonRpcResponse> {
    let request = match parse_request(value) {
        Ok(r) => r,
        Err((id, message)) => return Some(error_response(id, INVALID_REQUEST, message)),
    };

    // Notifications (no id) never get a response, not even an error.
    let Some(id) = request.id.clone() else {
        handle_notification(&request);
        return None;
    };

    Some(match dispatch(state, session, &request) {
        Ok(result) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        },
        Err(err) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(err),
        },
    })
}

fn error_response(id: serde_json::Value, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message,
            data: None,
        }),
    }
}

fn handle_notification(request: &JsonRpcRequest) {
    match request.method.as_str() {
        // Client acknowledged initialization / cancelled a request: nothing to
        // do for a synchronous server (responses are already written in order).
        "notifications/initialized" | "notifications/cancelled" => {}
        _ => {
            eprintln!("MCP: ignoring notification: {}", request.method);
        }
    }
}

/// The protocol version a request is served under: the modern version it
/// declares in `_meta`, else the version negotiated by `initialize`.
fn request_protocol(session: &Session, request: &JsonRpcRequest) -> Result<Protocol, JsonRpcError> {
    let declared = request
        .params
        .get("_meta")
        .and_then(|m| m.get(protocol::META_PROTOCOL_VERSION));
    match declared {
        // `initialize` is legacy-only; any `_meta` on it is ignored.
        Some(_) if request.method == "initialize" => Ok(Protocol::default()),
        Some(v) => protocol::modern_version(v.as_str().unwrap_or_default())
            .map(Protocol::new)
            .ok_or_else(|| JsonRpcError {
                code: protocol::UNSUPPORTED_PROTOCOL_VERSION,
                message: format!(
                    "Unsupported protocol version: {v}. Supported per-request versions: {}; \
                     legacy clients can use `initialize` with one of: {}",
                    protocol::MODERN_VERSIONS.join(", "),
                    protocol::LEGACY_VERSIONS.join(", ")
                ),
                data: Some(serde_json::json!({
                    "supported": protocol::MODERN_VERSIONS,
                    "requested": v,
                })),
            }),
        None => Ok(session.negotiated.map(Protocol::new).unwrap_or_default()),
    }
}

fn dispatch(
    state: &mut ServerState,
    session: &mut Session,
    request: &JsonRpcRequest,
) -> Result<serde_json::Value, JsonRpcError> {
    let proto = request_protocol(session, request)?;
    let result = match request.method.as_str() {
        "initialize" => {
            let requested = request
                .params
                .get("protocolVersion")
                .and_then(|v| v.as_str());
            let version = protocol::negotiate_legacy(requested);
            session.negotiated = Some(version);
            return Ok(initialize_result(Protocol::new(version)));
        }
        "server/discover" => discover_result(),
        "ping" => serde_json::json!({}),
        "resources/list" => {
            state.reload_config();
            resources::list(state)?
        }
        "resources/templates/list" => resources::templates_list(proto),
        "resources/read" => {
            state.reload_config();
            resources::read(state, &request.params)?
        }
        "tools/list" => tools::list(proto),
        "tools/call" => {
            state.reload_config();
            tools::call(state, proto, &request.params)?
        }
        _ => {
            return Err(JsonRpcError {
                code: METHOD_NOT_FOUND,
                message: format!("Method not found: {}", request.method),
                data: None,
            })
        }
    };
    Ok(
        if proto.is_modern() || request.method == "server/discover" {
            decorate_modern(result, &request.method, &request.params)
        } else {
            result
        },
    )
}

/// Add the fields every 2026-07-28 result carries: `resultType`, the server
/// identity in `_meta`, and caching hints on list/read results.
fn decorate_modern(
    mut result: serde_json::Value,
    method: &str,
    params: &serde_json::Value,
) -> serde_json::Value {
    let Some(obj) = result.as_object_mut() else {
        return result;
    };
    obj.insert("resultType".into(), "complete".into());
    let meta = obj.entry("_meta").or_insert_with(|| serde_json::json!({}));
    if let Some(meta) = meta.as_object_mut() {
        meta.insert(protocol::META_SERVER_INFO.into(), server_info_basic());
    }
    // Tool/template lists and the embedded docs only change with the binary;
    // site-derived resources can change at any moment.
    let embedded_docs = method == "resources/read"
        && params
            .get("uri")
            .and_then(|u| u.as_str())
            .is_some_and(|u| u == "seite://docs" || u.starts_with("seite://docs/"));
    let cache = match method {
        "server/discover" | "tools/list" | "resources/templates/list" => {
            Some((3_600_000, "public"))
        }
        "resources/read" if embedded_docs => Some((3_600_000, "public")),
        "resources/list" | "resources/read" => Some((0, "private")),
        _ => None,
    };
    if let Some((ttl, scope)) = cache {
        obj.insert("ttlMs".into(), ttl.into());
        obj.insert("cacheScope".into(), scope.into());
    }
    result
}

fn server_info_basic() -> serde_json::Value {
    serde_json::json!({ "name": "seite", "version": env!("CARGO_PKG_VERSION") })
}

fn server_info(proto: Protocol) -> serde_json::Value {
    let mut info = server_info_basic();
    if proto.titles() {
        info["title"] = "seite".into();
    }
    if proto.implementation_description() {
        info["description"] = "Structured access to a seite static site: content, config, \
                               templates, themes, docs, and builds."
            .into();
    }
    info
}

/// Server capabilities — only what is implemented (no prompts, no resource
/// subscriptions, no list-changed notifications, no logging).
fn capabilities() -> serde_json::Value {
    serde_json::json!({
        "resources": {},
        "tools": {}
    })
}

/// Guidance for the agent, returned by `initialize` and `server/discover`.
pub fn instructions() -> String {
    let verify = if tools::has_tool("seite_check") {
        "Verify changes with seite_check (diagnostics) and seite_build (strict=true fails on \
         warnings and broken links) instead of shelling out to `seite build`."
    } else {
        "Verify changes with seite_build (strict=true fails on warnings and broken links) \
         instead of shelling out to `seite build`."
    };
    [
        "seite MCP server: live, structured access to this seite static site (seite.toml, content, templates, themes, docs).",
        "Prefer it over reading files whenever you need the CURRENT site state: it resolves URLs, languages, and defaults exactly as the build does.",
        "- Before creating or editing content, check what exists: seite_search, seite_content_stats, or the seite://content/{collection} resource.",
        "- Create pages with seite_create_content (valid frontmatter, correct path and URL) instead of hand-writing .md files; change metadata with seite_update_frontmatter.",
        "- Inspect a page as the build sees it (resolved frontmatter, URL, rendered HTML, output file) with seite_get_page.",
        "- Before editing templates or shortcodes, call seite_list_templates (overrides, blocks, context variables, shortcode params, data keys).",
        &format!("- {verify}"),
        "- Read seite://config for the real configuration (it may differ from documented defaults); use seite_lookup_docs for seite features and syntax.",
        "Tool failures come back as results with isError: true and an actionable message; fix the arguments and retry.",
    ]
    .join("\n")
}

fn initialize_result(proto: Protocol) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": proto.version,
        "capabilities": capabilities(),
        "serverInfo": server_info(proto),
        "instructions": instructions(),
    })
}

fn discover_result() -> serde_json::Value {
    serde_json::json!({
        "supportedVersions": protocol::MODERN_VERSIONS,
        "capabilities": capabilities(),
        "instructions": instructions(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_state() -> ServerState {
        ServerState {
            config: None,
            paths: None,
            cwd: std::path::PathBuf::from("."),
            config_error: None,
        }
    }

    fn request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
        JsonRpcRequest {
            id: Some(serde_json::json!(1)),
            method: method.into(),
            params,
        }
    }

    /// Run one line through the server and return the parsed response.
    fn roundtrip(
        state: &mut ServerState,
        session: &mut Session,
        msg: serde_json::Value,
    ) -> serde_json::Value {
        handle_line(state, session, &msg.to_string()).expect("expected a response")
    }

    fn initialize(session: &mut Session, version: Option<&str>) -> serde_json::Value {
        let mut state = empty_state();
        let mut params =
            serde_json::json!({ "capabilities": {}, "clientInfo": {"name": "t", "version": "1"} });
        if let Some(v) = version {
            params["protocolVersion"] = v.into();
        }
        roundtrip(
            &mut state,
            session,
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": params}),
        )["result"]
            .clone()
    }

    #[test]
    fn test_initialize_negotiates_requested_version() {
        for v in protocol::LEGACY_VERSIONS {
            let mut session = Session::default();
            let result = initialize(&mut session, Some(v));
            assert_eq!(result["protocolVersion"], *v);
            assert_eq!(session.negotiated, Some(*v));
            assert_eq!(result["serverInfo"]["name"], "seite");
            assert!(result["capabilities"]["resources"].is_object());
            assert!(result["capabilities"]["tools"].is_object());
            assert!(result["capabilities"].get("prompts").is_none());
            let instructions = result["instructions"].as_str().unwrap();
            assert!(instructions.contains("seite_build"), "{instructions}");
            let lines = instructions.lines().count();
            assert!((5..=15).contains(&lines), "{lines} lines");
        }
    }

    #[test]
    fn test_initialize_unknown_version_answers_newest_legacy() {
        let mut session = Session::default();
        assert_eq!(
            initialize(&mut session, Some("1999-01-01"))["protocolVersion"],
            protocol::LEGACY_VERSIONS[0]
        );
        let mut session = Session::default();
        assert_eq!(
            initialize(&mut session, None)["protocolVersion"],
            protocol::LEGACY_VERSIONS[0]
        );
    }

    #[test]
    fn test_server_info_fields_gated_by_version() {
        let mut session = Session::default();
        let old = initialize(&mut session, Some("2024-11-05"));
        assert!(old["serverInfo"].get("title").is_none());
        let mut session = Session::default();
        let new = initialize(&mut session, Some("2025-11-25"));
        assert_eq!(new["serverInfo"]["title"], "seite");
        assert!(new["serverInfo"]["description"].is_string());
    }

    #[test]
    fn test_negotiated_version_gates_tool_fields() {
        let mut state = empty_state();
        let list = serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});

        let mut session = Session::default();
        initialize(&mut session, Some("2024-11-05"));
        let tools = roundtrip(&mut state, &mut session, list.clone());
        let first = &tools["result"]["tools"][0];
        assert!(first.get("annotations").is_none());
        assert!(first.get("title").is_none());
        assert!(first.get("outputSchema").is_none());

        let mut session = Session::default();
        initialize(&mut session, Some("2025-06-18"));
        let tools = roundtrip(&mut state, &mut session, list);
        let first = &tools["result"]["tools"][0];
        assert!(first["annotations"].is_object());
        assert!(first["title"].is_string());
    }

    #[test]
    fn test_modern_request_is_served_statelessly() {
        let mut state = empty_state();
        let mut session = Session::default();
        let resp = roundtrip(
            &mut state,
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": "d", "method": "server/discover",
                "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}}
            }),
        );
        let result = &resp["result"];
        assert_eq!(
            result["supportedVersions"],
            serde_json::json!(["2026-07-28"])
        );
        assert_eq!(result["resultType"], "complete");
        assert_eq!(
            result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "seite"
        );
        assert!(result["ttlMs"].as_u64().is_some());
        assert_eq!(result["cacheScope"], "public");
        assert!(result["instructions"].is_string());

        // A modern tools/list without any initialize gets modern fields.
        let resp = roundtrip(
            &mut state,
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/list",
                "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}}
            }),
        );
        assert_eq!(resp["result"]["resultType"], "complete");
        assert!(resp["result"]["tools"][0]["annotations"].is_object());
        assert!(session.negotiated.is_none());
    }

    #[test]
    fn test_unsupported_modern_version_is_structured_error() {
        let mut state = empty_state();
        let mut session = Session::default();
        let resp = roundtrip(
            &mut state,
            &mut session,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/list",
                "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "2099-01-01"}}
            }),
        );
        assert_eq!(
            resp["error"]["code"],
            protocol::UNSUPPORTED_PROTOCOL_VERSION
        );
        assert_eq!(resp["error"]["data"]["requested"], "2099-01-01");
        assert_eq!(
            resp["error"]["data"]["supported"],
            serde_json::json!(protocol::MODERN_VERSIONS)
        );
    }

    #[test]
    fn test_modern_cache_hints() {
        let v = decorate_modern(
            serde_json::json!({"contents": []}),
            "resources/read",
            &serde_json::json!({"uri": "seite://content/posts"}),
        );
        assert_eq!(v["ttlMs"], 0);
        assert_eq!(v["cacheScope"], "private");
        let v = decorate_modern(
            serde_json::json!({"contents": []}),
            "resources/read",
            &serde_json::json!({"uri": "seite://docs/templates"}),
        );
        assert_eq!(v["cacheScope"], "public");
        let v = decorate_modern(
            serde_json::json!({"content": []}),
            "tools/call",
            &serde_json::json!({}),
        );
        assert!(v.get("ttlMs").is_none());
        assert_eq!(v["resultType"], "complete");
    }

    #[test]
    fn test_dispatch_unknown_method() {
        let mut state = empty_state();
        let mut session = Session::default();
        for method in ["unknown/method", "prompts/list", "logging/setLevel"] {
            let err = dispatch(
                &mut state,
                &mut session,
                &request(method, serde_json::json!({})),
            )
            .unwrap_err();
            assert_eq!(err.code, METHOD_NOT_FOUND, "{method}");
        }
    }

    #[test]
    fn test_dispatch_ping() {
        let mut state = empty_state();
        let mut session = Session::default();
        let result = dispatch(
            &mut state,
            &mut session,
            &request("ping", serde_json::json!({})),
        );
        assert_eq!(result.unwrap(), serde_json::json!({}));
    }

    #[test]
    fn test_parse_request_and_notification() {
        let req = parse_request(serde_json::json!(
            {"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
        ))
        .ok()
        .unwrap();
        assert_eq!(req.method, "initialize");
        assert!(req.id.is_some());
        let note = parse_request(serde_json::json!(
            {"jsonrpc":"2.0","method":"notifications/initialized"}
        ))
        .ok()
        .unwrap();
        assert!(note.id.is_none());
    }

    #[test]
    fn test_handle_line_malformed_json_is_parse_error() {
        let mut state = empty_state();
        let mut session = Session::default();
        let resp = handle_line(&mut state, &mut session, "{not json").unwrap();
        assert_eq!(resp["error"]["code"], PARSE_ERROR);
        assert!(resp["id"].is_null());
    }

    #[test]
    fn test_handle_line_invalid_envelopes() {
        let mut state = empty_state();
        let mut session = Session::default();
        let cases = [
            (r#"{"jsonrpc":"2.0","id":7}"#, serde_json::json!(7)),
            (
                r#"{"jsonrpc":"1.0","id":8,"method":"ping"}"#,
                serde_json::json!(8),
            ),
            (r#"{"id":9,"method":"ping"}"#, serde_json::json!(9)),
            (
                r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#,
                serde_json::Value::Null,
            ),
            (
                r#"{"jsonrpc":"2.0","id":{"a":1},"method":"ping"}"#,
                serde_json::Value::Null,
            ),
            (
                r#"{"jsonrpc":"2.0","id":10,"method":"ping","params":[1]}"#,
                serde_json::json!(10),
            ),
            (r#""just a string""#, serde_json::Value::Null),
        ];
        for (line, id) in cases {
            let resp = handle_line(&mut state, &mut session, line).unwrap();
            assert_eq!(resp["error"]["code"], INVALID_REQUEST, "{line}");
            assert_eq!(resp["id"], id, "{line}");
        }
    }

    #[test]
    fn test_handle_line_string_ids_are_echoed() {
        let mut state = empty_state();
        let mut session = Session::default();
        let resp = handle_line(
            &mut state,
            &mut session,
            r#"{"jsonrpc":"2.0","id":"abc-1","method":"ping"}"#,
        )
        .unwrap();
        assert_eq!(resp["id"], "abc-1");
        assert_eq!(resp["result"], serde_json::json!({}));
    }

    #[test]
    fn test_handle_line_notifications_have_no_response() {
        let mut state = empty_state();
        let mut session = Session::default();
        for line in [
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/unknown"}"#,
            // A notification for a request-only method is still silent.
            r#"{"jsonrpc":"2.0","method":"tools/list"}"#,
        ] {
            assert!(
                handle_line(&mut state, &mut session, line).is_none(),
                "{line}"
            );
        }
    }

    #[test]
    fn test_handle_line_batch() {
        let mut state = empty_state();
        let mut session = Session::default();
        let resp = handle_line(
            &mut state,
            &mut session,
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":2,"method":"nope"}]"#,
        )
        .unwrap();
        let arr = resp.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["id"], 1);
        assert_eq!(arr[1]["error"]["code"], METHOD_NOT_FOUND);
        assert!(handle_line(
            &mut state,
            &mut session,
            r#"[{"jsonrpc":"2.0","method":"notifications/initialized"}]"#
        )
        .is_none());
        let empty = handle_line(&mut state, &mut session, "[]").unwrap();
        assert_eq!(empty["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn test_resource_templates_list() {
        let mut state = empty_state();
        let mut session = Session::default();
        let result = dispatch(
            &mut state,
            &mut session,
            &request("resources/templates/list", serde_json::json!({})),
        )
        .unwrap();
        let templates: Vec<&str> = result["resourceTemplates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["uriTemplate"].as_str().unwrap())
            .collect();
        assert!(templates.contains(&"seite://content/{collection}"));
        assert!(templates.contains(&"seite://docs/{slug}"));
    }

    #[test]
    fn test_find_project_root_walks_up() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("seite.toml"), "").unwrap();
        let nested = tmp.path().join("content/posts");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(find_project_root(&nested).unwrap(), tmp.path());
        let other = tempfile::TempDir::new().unwrap();
        // No seite.toml anywhere up the temp tree (the system temp dir has none).
        assert!(find_project_root(other.path()).is_none_or(|p| !p.starts_with(other.path())));
    }

    #[test]
    fn test_reload_config_surfaces_toml_error_and_recovers() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        std::fs::write(&config_path, "[site\ntitle = \"x\"\n").unwrap();
        let mut state = ServerState::load(tmp.path().to_path_buf());
        assert!(state.config.is_none());
        let err = state.site().err().unwrap();
        assert!(err.contains("Failed to load"), "{err}");
        assert!(err.contains("Invalid config"), "{err}");
        assert!(!err.contains("Not in a seite project"), "{err}");

        // Fixing the file is picked up on the next reload (no restart needed).
        std::fs::write(
            &config_path,
            "[site]\ntitle = \"Fixed\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"L\"\ndirectory = \"posts\"\ndefault_template = \"page.html\"\n",
        )
        .unwrap();
        state.reload_config();
        let (config, _) = state.site().unwrap();
        assert_eq!(config.site.title, "Fixed");
        assert!(state.config_error.is_none());
    }

    #[test]
    fn test_dispatch_reloads_config_per_request() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        std::fs::write(
            &config_path,
            "[site]\ntitle = \"T\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"L\"\ndirectory = \"posts\"\ndefault_template = \"page.html\"\n",
        )
        .unwrap();
        let mut state = ServerState::load(tmp.path().to_path_buf());
        let mut session = Session::default();
        let list_req = request("resources/list", serde_json::json!({}));
        let uris = |v: serde_json::Value| -> Vec<String> {
            v["resources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["uri"].as_str().unwrap().to_string())
                .collect()
        };
        let before = uris(dispatch(&mut state, &mut session, &list_req).unwrap());
        assert!(!before.contains(&"seite://content/docs".to_string()));

        std::fs::write(
            &config_path,
            "[site]\ntitle = \"T\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"L\"\ndirectory = \"posts\"\ndefault_template = \"page.html\"\n\n[[collections]]\nname = \"docs\"\nlabel = \"L\"\ndirectory = \"docs\"\ndefault_template = \"page.html\"\n",
        )
        .unwrap();
        let after = uris(dispatch(&mut state, &mut session, &list_req).unwrap());
        assert!(after.contains(&"seite://content/docs".to_string()));
    }
}
