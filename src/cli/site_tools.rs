//! Shared plumbing for the agent-facing commands (`seite docs`, `page`,
//! `search`, `stats`, `templates`, `frontmatter`, `context`).
//!
//! Each of those commands is a thin adapter over the MCP tool of the same
//! purpose (`src/mcp/tools.rs`): it builds a [`ServerState`] for the site,
//! calls the tool's handler with a JSON arguments object, and renders the
//! structured result. Coding agents with a shell get exactly what MCP clients
//! get, and the two can never drift apart.

use std::path::PathBuf;

use anyhow::anyhow;
use serde_json::Value;

use crate::mcp::{self, tools, ServerState};
use crate::output::human;
use crate::workspace;

/// The site root a command works on: with `--site <name>` inside a
/// workspace, that site's directory; otherwise the nearest directory at or
/// above the current one that holds a `seite.toml` (falling back to the
/// current directory, which then has no site).
pub fn site_root(site_filter: Option<&str>) -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    if let Some(name) = site_filter {
        if let Some(ws_root) = workspace::find_workspace_root(&cwd) {
            let ws = workspace::WorkspaceConfig::load(&ws_root.join("seite-workspace.toml"))?;
            let site = ws.find_site(name).ok_or_else(|| {
                let names: Vec<&str> = ws.sites.iter().map(|s| s.name.as_str()).collect();
                anyhow!(
                    "unknown workspace site '{name}'. Sites: {}{}",
                    names.join(", "),
                    human::suggest_match(name, &names)
                )
            })?;
            return Ok(ws_root.join(&site.path));
        }
        human::warning("--site flag ignored (not in a workspace)");
    }
    Ok(mcp::find_project_root(&cwd).unwrap_or(cwd))
}

/// Server state for the site at [`site_root`], whether or not it has a
/// (valid) `seite.toml`. For commands that degrade outside a site.
pub fn load_state(site_filter: Option<&str>) -> anyhow::Result<ServerState> {
    Ok(ServerState::load(site_root(site_filter)?))
}

/// Server state for a site whose config loaded; otherwise an actionable
/// error (no `seite.toml`, or the load error of a broken one).
pub fn load_site(site_filter: Option<&str>) -> anyhow::Result<ServerState> {
    let state = load_state(site_filter)?;
    if state.config.is_none() {
        return Err(anyhow!(no_site_message(&state)));
    }
    Ok(state)
}

/// Why `state` has no usable config, phrased for the CLI.
pub fn no_site_message(state: &ServerState) -> String {
    match &state.config_error {
        Some(err) => format!("{err}\nFix seite.toml (`seite check` shows every problem)."),
        None => format!(
            "not in a seite project: no seite.toml in {} or any parent directory. \
             Run `seite init <name>` to create a site, or pass --dir <site>.",
            state.cwd.display()
        ),
    }
}

/// Call the MCP tool `name` with `arguments` and return its structured
/// result. A tool failure becomes an error carrying the tool's message (with
/// the `seite_xxx: ` prefix dropped) and any structured details.
pub fn call_tool(state: &ServerState, name: &str, arguments: Value) -> anyhow::Result<Value> {
    let tool = tools::TOOLS
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| anyhow!("internal error: no MCP tool named {name}"))?;
    (tool.handler)(state, &arguments).map_err(|err| {
        let prefix = format!("{name}: ");
        let mut message = err
            .message
            .strip_prefix(&prefix)
            .unwrap_or(&err.message)
            .to_string();
        if let Some(details) = err.details {
            message.push_str("\n\n");
            message.push_str(&serde_json::to_string(&details).unwrap_or_default());
        }
        anyhow!(message)
    })
}

/// A JSON string field, or `""`.
pub fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// A JSON array field's elements, or an empty slice.
pub fn array_of<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// `n thing` / `n things`.
pub fn plural(n: u64, singular: &str, plural: &str) -> String {
    format!("{n} {}", if n == 1 { singular } else { plural })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_tool_strips_tool_prefix_from_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = ServerState::load(tmp.path().to_path_buf());
        let err = call_tool(&state, "seite_search", serde_json::json!({ "query": "" }))
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("'query' must not be empty"), "{err}");
    }

    #[test]
    fn test_call_tool_unknown_tool_is_internal_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = ServerState::load(tmp.path().to_path_buf());
        assert!(call_tool(&state, "nope", serde_json::json!({})).is_err());
    }

    #[test]
    fn test_no_site_message_outside_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = ServerState::load(tmp.path().to_path_buf());
        assert!(no_site_message(&state).contains("seite init"));
    }

    #[test]
    fn test_helpers() {
        let v = serde_json::json!({ "a": "x", "b": [1, 2] });
        assert_eq!(str_of(&v, "a"), "x");
        assert_eq!(str_of(&v, "zz"), "");
        assert_eq!(array_of(&v, "b").len(), 2);
        assert!(array_of(&v, "a").is_empty());
        assert_eq!(plural(1, "item", "items"), "1 item");
        assert_eq!(plural(2, "item", "items"), "2 items");
    }
}
