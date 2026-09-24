//! `seite mcp` — MCP (Model Context Protocol) server over stdio.
//!
//! Implements JSON-RPC 2.0 over stdin/stdout for AI tool integration.
//! Claude Code (and other MCP clients) connect to this server to get
//! structured access to site documentation, configuration, content,
//! themes, and build tools. Claude Code discovers it through the project's
//! `.mcp.json` file.
//!
//! **Critical**: All logging goes to stderr. Never write to stdout
//! except protocol messages — it would corrupt the JSON-RPC stream.
//!
//! Error semantics follow the MCP spec: problems with a *tool's execution*
//! (bad arguments, no site, failed build, ...) are returned as normal tool
//! results with `isError: true`; JSON-RPC errors are reserved for protocol
//! problems (unknown method, malformed request, unknown tool name).

pub mod content_index;
pub mod resources;
pub mod tools;

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{ResolvedPaths, SiteConfig};

/// MCP protocol version we implement.
const PROTOCOL_VERSION: &str = "2024-11-05";

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    /// Requests have an id; notifications do not.
    #[serde(default)]
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
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
        if let Some(resp) = handle_line(&mut state, &line) {
            write_response(&mut writer, &resp)?;
        }
    }

    Ok(())
}

/// Process one line of input. Returns `None` for notifications.
fn handle_line(state: &mut ServerState, line: &str) -> Option<JsonRpcResponse> {
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return Some(error_response(
                serde_json::Value::Null,
                PARSE_ERROR,
                format!("Parse error: {e}"),
            ));
        }
    };
    let id_hint = value.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let request: JsonRpcRequest = match serde_json::from_value(value) {
        Ok(r) => r,
        Err(e) => {
            return Some(error_response(
                id_hint,
                INVALID_REQUEST,
                format!("Invalid request: {e}"),
            ));
        }
    };

    // Notifications (no id) don't get a response
    let Some(id) = request.id.clone() else {
        handle_notification(&request);
        return None;
    };

    Some(match dispatch(state, &request) {
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

fn write_response(writer: &mut impl Write, resp: &JsonRpcResponse) -> io::Result<()> {
    let json = serde_json::to_string(resp).unwrap_or_default();
    writeln!(writer, "{json}")?;
    writer.flush()
}

fn handle_notification(request: &JsonRpcRequest) {
    match request.method.as_str() {
        "notifications/initialized" => {
            // Client acknowledged initialization — no action needed
        }
        "notifications/cancelled" => {
            // Client cancelled a request — no action needed for sync server
        }
        _ => {
            eprintln!("MCP: unknown notification: {}", request.method);
        }
    }
}

fn dispatch(
    state: &mut ServerState,
    request: &JsonRpcRequest,
) -> Result<serde_json::Value, JsonRpcError> {
    match request.method.as_str() {
        "initialize" => handle_initialize(&request.params),
        "ping" => Ok(serde_json::json!({})),
        "resources/list" => {
            state.reload_config();
            resources::list(state)
        }
        "resources/read" => {
            state.reload_config();
            resources::read(state, &request.params)
        }
        "tools/list" => tools::list(),
        "tools/call" => {
            state.reload_config();
            tools::call(state, &request.params)
        }
        _ => Err(JsonRpcError {
            code: METHOD_NOT_FOUND,
            message: format!("Method not found: {}", request.method),
            data: None,
        }),
    }
}

fn handle_initialize(_params: &serde_json::Value) -> Result<serde_json::Value, JsonRpcError> {
    Ok(serde_json::json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "resources": { "listChanged": false },
            "tools": {}
        },
        "serverInfo": {
            "name": "seite",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
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

    #[test]
    fn test_handle_initialize() {
        let result = handle_initialize(&serde_json::json!({})).unwrap();
        assert_eq!(result["serverInfo"]["name"], "seite");
        assert!(result["capabilities"]["resources"].is_object());
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_dispatch_unknown_method() {
        let mut state = empty_state();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "unknown/method".into(),
            params: serde_json::json!({}),
        };
        let result = dispatch(&mut state, &request);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, METHOD_NOT_FOUND);
    }

    #[test]
    fn test_dispatch_ping() {
        let mut state = empty_state();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "ping".into(),
            params: serde_json::json!({}),
        };
        let result = dispatch(&mut state, &request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "initialize");
        assert!(req.id.is_some());
    }

    #[test]
    fn test_parse_notification() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "notifications/initialized");
        assert!(req.id.is_none());
    }

    #[test]
    fn test_handle_line_malformed_json_is_parse_error() {
        let mut state = empty_state();
        let resp = handle_line(&mut state, "{not json").unwrap();
        assert_eq!(resp.error.unwrap().code, PARSE_ERROR);
    }

    #[test]
    fn test_handle_line_missing_method_is_invalid_request() {
        let mut state = empty_state();
        let resp = handle_line(&mut state, r#"{"jsonrpc":"2.0","id":7}"#).unwrap();
        assert_eq!(resp.id, serde_json::json!(7));
        assert_eq!(resp.error.unwrap().code, INVALID_REQUEST);
    }

    #[test]
    fn test_handle_line_notification_has_no_response() {
        let mut state = empty_state();
        assert!(handle_line(
            &mut state,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
        )
        .is_none());
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
        let list_req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "resources/list".into(),
            params: serde_json::json!({}),
        };
        let uris = |v: serde_json::Value| -> Vec<String> {
            v["resources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["uri"].as_str().unwrap().to_string())
                .collect()
        };
        let before = uris(dispatch(&mut state, &list_req).unwrap());
        assert!(!before.contains(&"seite://content/docs".to_string()));

        std::fs::write(
            &config_path,
            "[site]\ntitle = \"T\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"L\"\ndirectory = \"posts\"\ndefault_template = \"page.html\"\n\n[[collections]]\nname = \"docs\"\nlabel = \"L\"\ndirectory = \"docs\"\ndefault_template = \"page.html\"\n",
        )
        .unwrap();
        let after = uris(dispatch(&mut state, &list_req).unwrap());
        assert!(after.contains(&"seite://content/docs".to_string()));
    }
}
