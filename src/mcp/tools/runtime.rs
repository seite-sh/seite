//! Runtime tools: what the running dev server (`seite serve`) knows —
//! the last build outcome, browser errors, preview URLs, routes, rebuilds.
//!
//! Served by the dev server's `/__seite/mcp` endpoint, they read its
//! in-process state ([`ServerState::dev`]). Under stdio `seite mcp` there is
//! no dev state: they find the running server for the site through
//! `.seite/dev.json` and ask it over loopback HTTP (`/__seite/status`, and
//! `/__seite/mcp` for `seite_reload`); with no server they fail with
//! "no dev server running; start it with `seite serve`".

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use super::{input_object, prop, Args, ServerState, ToolError, ToolResult};
use crate::server::remote;

const STATUS_TIMEOUT: Duration = Duration::from_secs(3);
const RELOAD_TIMEOUT: Duration = Duration::from_secs(600);
const ROUTES_DEFAULT_LIMIT: i64 = 200;
const ROUTES_MAX_LIMIT: i64 = 2000;

/// Base URL of this site on the running dev server, plus its status.
fn server_for(state: &ServerState) -> Result<(String, Value), ToolError> {
    if let Some(dev) = &state.dev {
        return Ok((dev.base_url(), dev.status_json()));
    }
    let remote = remote::find_for_site(&state.cwd, STATUS_TIMEOUT).map_err(ToolError::new)?;
    Ok((remote.site_url, remote.status))
}

pub fn empty_input_schema() -> Value {
    input_object(json!({}), &[])
}

pub fn call_get_errors(state: &ServerState, arguments: &Value) -> ToolResult {
    Args::new("seite_get_errors", arguments, &[])?;
    let (url, status) = server_for(state).map_err(prefix("seite_get_errors"))?;
    let last = status.get("last_build").cloned().unwrap_or(Value::Null);
    let browser = status.get("browser_errors").cloned().unwrap_or(json!([]));
    let build_failed = last.get("ok").and_then(Value::as_bool) == Some(false);
    let browser_count = browser.as_array().map_or(0, Vec::len);
    Ok(json!({
        "url": url,
        "ok": !build_failed && browser_count == 0,
        "last_build": last,
        "browser_errors": browser,
    }))
}

pub fn get_preview_url_input_schema() -> Value {
    input_object(
        json!({
            "path": prop("string", "Content file (e.g. content/posts/2026-01-02-hello.md): returns the preview URL of its page"),
            "url": prop("string", "Published page path (e.g. /posts/hello): returns its preview URL"),
        }),
        &[],
    )
}

pub fn call_get_preview_url(state: &ServerState, arguments: &Value) -> ToolResult {
    let args = Args::new("seite_get_preview_url", arguments, &["path", "url"])?;
    let path = args.str("path")?;
    let url = args.str("url")?;
    let page = match (path, url) {
        (Some(_), Some(_)) => {
            return Err(ToolError::new(
                "seite_get_preview_url: pass at most one of 'path' or 'url'",
            ))
        }
        (Some(p), None) => {
            let page = super::page::call_get_page(state, &json!({ "path": p }))?;
            Some(
                page.get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("/")
                    .to_string(),
            )
        }
        (None, Some(u)) => Some(page_path(u)),
        (None, None) => None,
    };
    let (base, _) = server_for(state).map_err(prefix("seite_get_preview_url"))?;
    let preview = match &page {
        Some(p) => format!("{base}{p}"),
        None => format!("{base}/"),
    };
    Ok(json!({ "base_url": base, "page": page, "preview_url": preview }))
}

/// `/posts/x` from a path or an absolute URL (origin, query, fragment dropped).
fn page_path(raw: &str) -> String {
    let raw = raw.trim();
    let rest = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
        .map_or(raw, |r| r.find('/').map_or("", |i| &r[i..]));
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    if rest.starts_with('/') {
        rest.to_string()
    } else {
        format!("/{rest}")
    }
}

pub fn list_routes_input_schema() -> Value {
    input_object(
        json!({
            "prefix": prop("string", "Only routes starting with this path (e.g. /posts/)"),
            "limit": prop("integer", "Maximum routes returned (default 200, max 2000)"),
        }),
        &[],
    )
}

pub fn call_list_routes(state: &ServerState, arguments: &Value) -> ToolResult {
    let args = Args::new("seite_list_routes", arguments, &["prefix", "limit"])?;
    let prefix = args.str("prefix")?.unwrap_or("");
    let limit = args
        .int("limit")?
        .unwrap_or(ROUTES_DEFAULT_LIMIT)
        .clamp(1, ROUTES_MAX_LIMIT);
    let (_, paths) = state.site()?;
    if !paths.output.is_dir() {
        return Err(ToolError::new(format!(
            "seite_list_routes: {} does not exist yet; build the site (seite_build) or start `seite serve`",
            super::content_index::relative_path(&paths.root, &paths.output)
        )));
    }
    let mut routes: Vec<String> = html_routes(&paths.output)
        .into_iter()
        .filter(|r| r.starts_with(prefix))
        .collect();
    routes.sort();
    let total = routes.len();
    routes.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    // The preview base is a bonus: routes come from disk, server or not.
    let preview_base = server_for(state).ok().map(|(base, _)| base);
    Ok(json!({
        "output_dir": super::content_index::relative_path(&paths.root, &paths.output),
        "total": total,
        "returned": routes.len(),
        "routes": routes,
        "preview_base": preview_base,
    }))
}

/// Page URLs of every `.html` file under `output` (`index.html` → `/dir/`,
/// `a.html` → `/a`), skipping symlinks.
fn html_routes(output: &Path) -> Vec<String> {
    walkdir::WalkDir::new(output)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(output).ok()?;
            if rel.extension().is_none_or(|x| x != "html") {
                return None;
            }
            let rel = rel.to_string_lossy().replace('\\', "/");
            Some(if rel == "index.html" {
                "/".to_string()
            } else if let Some(dir) = rel.strip_suffix("/index.html") {
                format!("/{dir}/")
            } else {
                format!("/{}", rel.trim_end_matches(".html"))
            })
        })
        .collect()
}

pub fn call_reload(state: &ServerState, arguments: &Value) -> ToolResult {
    Args::new("seite_reload", arguments, &[])?;
    if let Some(dev) = &state.dev {
        let outcome = dev
            .reload()
            .map_err(|e| ToolError::new(format!("seite_reload: {e}")))?;
        let mut value = serde_json::to_value(&outcome)
            .map_err(|e| ToolError::new(format!("seite_reload: {e}")))?;
        value["url"] = dev.base_url().into();
        return Ok(value);
    }
    let remote = remote::find_for_site(&state.cwd, STATUS_TIMEOUT)
        .map_err(|e| ToolError::new(format!("seite_reload: {e}")))?;
    if remote.status.get("workspace").and_then(Value::as_bool) == Some(true) {
        return Err(ToolError::new(
            "seite_reload: the running workspace dev server rebuilds on file changes only; save a file, or serve one site with `seite serve --site <name>`",
        ));
    }
    remote::call_tool(
        remote.server.port,
        "seite_reload",
        json!({}),
        RELOAD_TIMEOUT,
    )
    .map_err(|e| ToolError::new(format!("seite_reload: {e}")))
}

fn prefix(tool: &'static str) -> impl Fn(ToolError) -> ToolError {
    move |e| ToolError::new(format!("{tool}: {}", e.message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::dev::{BuildOutcome, DevOptions, DevState};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    #[test]
    fn test_page_path() {
        assert_eq!(page_path("/posts/x"), "/posts/x");
        assert_eq!(page_path("posts/x"), "/posts/x");
        assert_eq!(page_path("http://localhost:3000/a?b#c"), "/a");
        assert_eq!(page_path("https://example.com"), "/");
    }

    #[test]
    fn test_html_routes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = tmp.path();
        std::fs::create_dir_all(out.join("posts")).unwrap();
        for f in [
            "index.html",
            "posts/index.html",
            "posts/a.html",
            "posts/a.md",
            "x.css",
        ] {
            std::fs::write(out.join(f), "").unwrap();
        }
        let mut r = html_routes(out);
        r.sort();
        assert_eq!(r, ["/", "/posts/", "/posts/a"]);
    }

    fn dev_state() -> (tempfile::TempDir, ServerState) {
        let (tmp, mut state) = super::super::test_support::site("");
        let dev = DevState::new(DevOptions {
            port: 4321,
            bind_host: "127.0.0.1".into(),
            root: tmp.path().to_path_buf(),
            sites: vec![],
            stop: Arc::new(AtomicBool::new(false)),
            rebuild: None,
            mcp: true,
        });
        dev.record_build(BuildOutcome::failure_message("boom", 3), false);
        state.dev = Some(dev);
        (tmp, state)
    }

    #[test]
    fn test_runtime_tools_read_in_process_state() {
        let (tmp, state) = dev_state();
        let errors = call_get_errors(&state, &json!({})).unwrap();
        assert_eq!(errors["url"], "http://127.0.0.1:4321");
        assert_eq!(errors["ok"], false);
        assert_eq!(errors["last_build"]["message"], "boom");

        let url = call_get_preview_url(&state, &json!({"url": "/posts/hi"})).unwrap();
        assert_eq!(url["preview_url"], "http://127.0.0.1:4321/posts/hi");
        let site = call_get_preview_url(&state, &json!({})).unwrap();
        assert_eq!(site["preview_url"], "http://127.0.0.1:4321/");
        assert!(call_get_preview_url(&state, &json!({"url": "/", "path": "x"})).is_err());

        let err = call_list_routes(&state, &json!({})).unwrap_err();
        assert!(err.message.contains("does not exist"), "{}", err.message);
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(dist.join("posts")).unwrap();
        std::fs::write(dist.join("index.html"), "").unwrap();
        std::fs::write(dist.join("posts/a.html"), "").unwrap();
        let routes = call_list_routes(&state, &json!({"prefix": "/posts"})).unwrap();
        assert_eq!(routes["routes"], json!(["/posts/a"]));
        assert_eq!(routes["preview_base"], "http://127.0.0.1:4321");

        // No rebuild function in this state: a clear tool error.
        assert!(call_reload(&state, &json!({})).is_err());
    }

    #[test]
    fn test_runtime_tools_without_server_explain_how_to_start_one() {
        let (_tmp, state) = super::super::test_support::site("");
        for (name, f) in [
            (
                "seite_get_errors",
                call_get_errors as fn(&ServerState, &Value) -> ToolResult,
            ),
            ("seite_reload", call_reload),
            ("seite_get_preview_url", call_get_preview_url),
        ] {
            let err = f(&state, &json!({})).unwrap_err();
            assert!(
                err.message.contains("seite serve"),
                "{name}: {}",
                err.message
            );
            assert!(err.message.starts_with(name), "{name}: {}", err.message);
        }
    }
}
