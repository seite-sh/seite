//! `seite mcp` — MCP (Model Context Protocol) server over stdio.
//!
//! Implements JSON-RPC 2.0 over stdin/stdout for AI tool integration.
//! Claude Code (and other MCP clients) connect to this server to get
//! structured access to site documentation, configuration, content,
//! themes, and build tools.
//!
//! **Critical**: All logging goes to stderr. Never write to stdout
//! except protocol messages — it would corrupt the JSON-RPC stream.

pub mod resources;
pub mod tools;

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

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

/// State shared across all handlers, loaded once at startup.
pub struct ServerState {
    /// Site configuration (None if not in a page project).
    pub config: Option<crate::config::SiteConfig>,
    /// Resolved directory paths (None if config not loaded).
    pub paths: Option<crate::config::ResolvedPaths>,
    /// Current working directory.
    pub cwd: std::path::PathBuf,
}

impl ServerState {
    /// Reload config from disk (for tools that mutate state, like build).
    pub fn reload_config(&mut self) {
        let config_path = self.cwd.join("seite.toml");
        self.config = crate::config::SiteConfig::load(&config_path).ok();
        self.paths = self.config.as_ref().map(|c| c.resolve_paths(&self.cwd));
    }
}

// ---------------------------------------------------------------------------
// Main server loop
// ---------------------------------------------------------------------------

/// Run the MCP server over stdio. Reads JSON-RPC messages from stdin,
/// dispatches to handlers, writes responses to stdout.
pub fn serve() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("seite.toml");
    let config = crate::config::SiteConfig::load(&config_path).ok();
    let paths = config.as_ref().map(|c| c.resolve_paths(&cwd));

    let mut state = ServerState { config, paths, cwd };

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

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: serde_json::Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: PARSE_ERROR,
                        message: format!("Parse error: {e}"),
                        data: None,
                    }),
                };
                write_response(&mut writer, &resp)?;
                continue;
            }
        };

        // Notifications (no id) don't get a response
        if request.id.is_none() {
            handle_notification(&request);
            continue;
        }

        let id = request.id.clone().unwrap_or(serde_json::Value::Null);
        let response = dispatch(&mut state, &request);

        let resp = match response {
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
        };

        write_response(&mut writer, &resp)?;
    }

    Ok(())
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
        "resources/list" => resources::list(state),
        "resources/read" => resources::read(state, &request.params),
        "tools/list" => tools::list(),
        "tools/call" => tools::call(state, &request.params),
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

    #[test]
    fn test_handle_initialize() {
        let result = handle_initialize(&serde_json::json!({})).unwrap();
        assert_eq!(result["serverInfo"]["name"], "seite");
        assert!(result["capabilities"]["resources"].is_object());
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_dispatch_unknown_method() {
        let mut state = ServerState {
            config: None,
            paths: None,
            cwd: std::path::PathBuf::from("."),
        };
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
        let mut state = ServerState {
            config: None,
            paths: None,
            cwd: std::path::PathBuf::from("."),
        };
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
}
