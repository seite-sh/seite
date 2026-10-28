//! Talking to a running `seite serve` from another process (`seite preview`,
//! `seite context`, the runtime tools of stdio `seite mcp`).
//!
//! The server is found through `.seite/dev.json` exactly like
//! `crate::cli::context::running_dev_server` does (live pid + open loopback
//! port; the URL is rebuilt as `http://127.0.0.1:<port>`, never read from the
//! file), first in the site root, then in the enclosing workspace root.
//! Requests go to 127.0.0.1 only, with no proxy, so the Host header is
//! `127.0.0.1:<port>`, which the server's guards accept.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

use crate::cli::context::{dev_server_url, running_dev_server, DevServer};

/// Shown when no dev server is running.
pub const NOT_RUNNING: &str = "no dev server running; start it with `seite serve`";

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .proxy(None)
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_global(Some(timeout))
            .build(),
    )
}

/// `GET http://127.0.0.1:<port><path>` as JSON.
pub fn get_json(port: u16, path: &str, timeout: Duration) -> Result<Value, String> {
    let url = format!("{}{path}", dev_server_url(port));
    let mut response = agent(timeout)
        .get(&url)
        .call()
        .map_err(|e| format!("dev server at {url} did not answer: {e}"))?;
    let status = response.status().as_u16();
    let body: Value = response
        .body_mut()
        .read_json()
        .map_err(|e| format!("dev server at {url} sent invalid JSON: {e}"))?;
    if status != 200 {
        return Err(format!(
            "dev server at {url} answered {status}: {}",
            body.get("error").and_then(Value::as_str).unwrap_or("")
        ));
    }
    Ok(body)
}

/// `POST` a JSON body; returns the status and the JSON reply (if any).
pub fn post_json(
    port: u16,
    path: &str,
    body: &Value,
    timeout: Duration,
) -> Result<(u16, Option<Value>), String> {
    let url = format!("{}{path}", dev_server_url(port));
    let mut response = agent(timeout)
        .post(&url)
        .header("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("dev server at {url} did not answer: {e}"))?;
    let status = response.status().as_u16();
    let reply = response.body_mut().read_json::<Value>().ok();
    Ok((status, reply))
}

/// The running dev server for the site at `site_root`, with its status.
pub struct RemoteDev {
    pub server: DevServer,
    pub status: Value,
    /// The site's base URL on that server (workspace servers mount each
    /// site under `/<name>`).
    pub site_url: String,
}

/// Directories whose `.seite/dev.json` may describe a server for this site.
fn dev_roots(site_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![site_root.to_path_buf()];
    if let Some(ws) = crate::workspace::find_workspace_root(site_root) {
        if ws != site_root {
            roots.push(ws);
        }
    }
    roots
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Any running dev server recorded for `site_root` (or its workspace), with
/// its `/__seite/status` (short `timeout`), whatever site it serves.
fn find_server(site_root: &Path, timeout: Duration) -> Result<(DevServer, Value), String> {
    let server = dev_roots(site_root)
        .iter()
        .find_map(|root| running_dev_server(root))
        .ok_or_else(|| NOT_RUNNING.to_string())?;
    let status = get_json(server.port, "/__seite/status", timeout)?;
    Ok((server, status))
}

/// This site's base URL on the server described by `status`: the server
/// root itself (a site, or the workspace root), or a workspace server's
/// `/<name>` mount. `Err` when the server serves something else.
pub fn site_url_in(status: &Value, base: &str, site_root: &Path) -> Result<String, String> {
    let root = PathBuf::from(status.get("root").and_then(Value::as_str).unwrap_or(""));
    if !root.as_os_str().is_empty() && same_dir(&root, site_root) {
        return Ok(base.to_string());
    }
    if status.get("workspace").and_then(Value::as_bool) == Some(true) {
        let site = status
            .get("sites")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|s| {
                same_dir(
                    Path::new(s.get("root").and_then(Value::as_str).unwrap_or("")),
                    site_root,
                )
            })
            .and_then(|s| s.get("name").and_then(Value::as_str));
        return site.map(|name| format!("{base}/{name}")).ok_or_else(|| {
            format!("the dev server at {base} serves a workspace without this site; start it with `seite serve`")
        });
    }
    Err(format!(
        "the dev server at {base} serves {}, not this site ({}); start one here with `seite serve --force-new`",
        root.display(),
        site_root.display()
    ))
}

/// The running dev server that serves the site (or workspace) at
/// `site_root`. Refuses a server that serves another site (e.g. `seite serve
/// --site other` recorded in the workspace root).
pub fn find_for_site(site_root: &Path, timeout: Duration) -> Result<RemoteDev, String> {
    let (server, status) = find_server(site_root, timeout)?;
    let site_url = site_url_in(&status, &server.url, site_root)?;
    Ok(RemoteDev {
        site_url,
        server,
        status,
    })
}

/// Call an MCP tool on the dev server's `/__seite/mcp`; returns the tool's
/// structured result, or its error text.
pub fn call_tool(
    port: u16,
    name: &str,
    arguments: Value,
    timeout: Duration,
) -> Result<Value, String> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments,
            "_meta": {"io.modelcontextprotocol/protocolVersion": crate::mcp::protocol::MODERN_VERSIONS[0]},
        },
    });
    let (status, reply) = post_json(port, "/__seite/mcp", &request, timeout)?;
    let reply = reply.ok_or_else(|| format!("dev server answered {status} without a JSON body"))?;
    if status != 200 {
        return Err(reply
            .get("error")
            .and_then(Value::as_str)
            .map_or_else(|| format!("dev server answered {status}"), str::to_string));
    }
    if let Some(err) = reply.get("error") {
        return Err(err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("MCP error")
            .to_string());
    }
    let result = reply.get("result").cloned().unwrap_or(Value::Null);
    let text = || {
        result
            .pointer("/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        return Err(text());
    }
    match result.get("structuredContent") {
        Some(v) => Ok(v.clone()),
        None => serde_json::from_str(&text()).map_err(|e| format!("unexpected tool result: {e}")),
    }
}
