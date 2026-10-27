use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tiny_http::{Header, Response, Server};

use crate::build::{self, BuildOptions};
use crate::error::{PageError, Result};
use crate::output::human;
use crate::output::CommandOutput;
use crate::server::dev::{self, inject_livereload, BuildOutcome, DevState, SiteScope};

use super::{load_site_in_workspace, WorkspaceConfig};

/// Handle to a running workspace dev server.
pub struct WorkspaceServerHandle {
    stop: Arc<AtomicBool>,
    port: u16,
    dev: Arc<DevState>,
}

impl WorkspaceServerHandle {
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The server's runtime state (build outcomes, browser errors).
    pub fn dev(&self) -> &Arc<DevState> {
        &self.dev
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for WorkspaceServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

struct SiteServerInfo {
    name: String,
    root: PathBuf,
    output_dir: PathBuf,
    watch_dirs: Vec<PathBuf>,
}

/// Start a workspace dev server that routes requests by path prefix.
/// `localhost:3000/blog/...` -> sites/blog/dist/...
pub fn start(
    ws_config: &WorkspaceConfig,
    ws_root: &Path,
    host: &str,
    port: u16,
    auto_increment: bool,
) -> Result<WorkspaceServerHandle> {
    let (server, actual_port) = if auto_increment {
        try_bind_auto(host, port)?
    } else {
        if !port_is_available(host, port) {
            return Err(PageError::Server(format!("port {port} is already in use")));
        }
        let server = Server::http(server_addr(host, port)).map_err(|e| {
            PageError::Server(format!("failed to start server on {host}:{port}: {e}"))
        })?;
        (server, port)
    };

    // Collect site info for routing
    let mut sites = Vec::new();
    for ws_site in &ws_config.sites {
        let (_config, paths) = load_site_in_workspace(ws_root, ws_site)?;
        let watch_dirs = vec![
            paths.content.clone(),
            paths.templates.clone(),
            paths.static_dir.clone(),
            paths.public_dir.clone(),
            paths.data_dir.clone(),
        ];
        sites.push(SiteServerInfo {
            name: ws_site.name.clone(),
            root: paths.root.clone(),
            output_dir: paths.output.clone(),
            watch_dirs,
        });
    }

    let display_host = match host {
        "0.0.0.0" | "::" => "localhost",
        h => h,
    };
    let base_url = if display_host.contains(':') {
        format!("http://[{display_host}]:{actual_port}")
    } else {
        format!("http://{display_host}:{actual_port}")
    };
    if actual_port != port {
        human::info(&format!("Port {port} in use, serving at {base_url}"));
    } else {
        human::success(&format!("Serving workspace at {base_url}"));
    }

    // Print site routes
    for site in &sites {
        human::info(&format!(
            "  /{} -> {}",
            site.name,
            site.output_dir.display()
        ));
    }

    let stop = Arc::new(AtomicBool::new(false));
    // No MCP endpoint here (several sites): `seite serve --site <name>` has one.
    let dev = DevState::new(dev::DevOptions {
        port: actual_port,
        bind_host: host.to_string(),
        root: ws_root.to_path_buf(),
        sites: sites
            .iter()
            .map(|s| (s.name.clone(), s.root.clone()))
            .collect(),
        stop: stop.clone(),
        rebuild: None,
        mcp: false,
    });

    // Spawn file watcher thread
    let watcher_stop = stop.clone();
    let watcher_dev = dev.clone();
    let watcher_ws_config = ws_config.clone();
    let watcher_ws_root = ws_root.to_path_buf();
    let watcher_sites: Vec<(String, Vec<PathBuf>)> = sites
        .iter()
        .map(|s| (s.name.clone(), s.watch_dirs.clone()))
        .collect();
    std::thread::spawn(move || {
        watch_and_rebuild_workspace(
            &watcher_ws_config,
            &watcher_ws_root,
            &watcher_sites,
            &watcher_stop,
            &watcher_dev,
        );
    });

    // Spawn HTTP server thread
    let server_stop = stop.clone();
    let server_sites: Vec<(String, PathBuf)> = sites
        .iter()
        .map(|s| (s.name.clone(), s.output_dir.clone()))
        .collect();
    let server_dev = dev.clone();
    std::thread::spawn(move || {
        run_serve_loop(server, &server_sites, &server_dev, &server_stop);
    });

    Ok(WorkspaceServerHandle {
        stop,
        port: actual_port,
        dev,
    })
}

fn run_serve_loop(
    server: Server,
    sites: &[(String, PathBuf)],
    dev: &Arc<DevState>,
    stop: &AtomicBool,
) {
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match server.recv_timeout(Duration::from_secs(1)) {
            Ok(Some(request)) => {
                // Dev endpoints, shared with the single-site server.
                let Some(request) = dev::handle_request(dev, None, request) else {
                    continue;
                };
                let url_path = request.url().to_string();

                // Route by path prefix: /blog/... -> sites/blog/dist/...
                if let Some((file_path, _site_name)) = route_request(&url_path, sites) {
                    if file_path.exists() && file_path.is_file() {
                        dev::respond_file(request, &file_path, guess_mime(&file_path));
                        continue;
                    }
                }

                // Workspace index page
                if url_path == "/" {
                    let index_html = generate_workspace_index(sites);
                    let content = inject_livereload(index_html.as_bytes());
                    let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8")
                        .expect("valid header");
                    let _ = request.respond(Response::from_data(content).with_header(header));
                    continue;
                }

                let _ =
                    request.respond(Response::from_string("404 Not Found").with_status_code(404));
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
}

/// Route a request to a site's output directory based on URL path prefix.
fn route_request<'a>(url_path: &str, sites: &'a [(String, PathBuf)]) -> Option<(PathBuf, &'a str)> {
    let clean = url_path.split('?').next().unwrap_or(url_path);
    let clean = clean.trim_start_matches('/');
    if clean.is_empty() {
        return None;
    }

    let (prefix, rest) = match clean.split_once('/') {
        Some((p, r)) => (p, r),
        None => (clean, ""),
    };

    let (name, output_dir) = sites.iter().find(|(name, _)| name == prefix)?;

    let file_path = resolve_file_path(output_dir, rest);
    file_path.map(|p| (p, name.as_str()))
}

fn resolve_file_path(output_dir: &Path, url_path: &str) -> Option<PathBuf> {
    let clean = url_path.trim_start_matches('/');
    if clean.is_empty() {
        return Some(output_dir.join("index.html"));
    }

    // Exact file match
    let candidate = output_dir.join(clean);
    if candidate.is_file() {
        return Some(candidate);
    }

    // Clean URL -> .html file
    let html_candidate = output_dir.join(format!("{clean}.html"));
    if html_candidate.is_file() {
        return Some(html_candidate);
    }

    // Directory with index.html
    let index = candidate.join("index.html");
    if index.is_file() {
        return Some(index);
    }

    None
}

fn generate_workspace_index(sites: &[(String, PathBuf)]) -> String {
    let mut links = String::new();
    for (name, _) in sites {
        links.push_str(&format!("    <li><a href=\"/{name}/\">{name}</a></li>\n"));
    }
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Workspace</title>
<style>
body {{ font-family: system-ui, sans-serif; max-width: 600px; margin: 4rem auto; padding: 0 1rem; }}
a {{ color: #0057b7; }}
li {{ margin: 0.5rem 0; }}
</style>
</head>
<body>
<h1>Workspace Sites</h1>
<ul>
{links}</ul>
</body>
</html>"#
    )
}

fn guess_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("md") => dev::MARKDOWN_CONTENT_TYPE,
        Some("txt") => "text/plain; charset=utf-8",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn watch_and_rebuild_workspace(
    ws_config: &WorkspaceConfig,
    ws_root: &Path,
    sites: &[(String, Vec<PathBuf>)],
    stop: &AtomicBool,
    dev: &DevState,
) {
    let (tx, rx) = mpsc::channel();

    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            human::error(&format!("Failed to start file watcher: {e}"));
            return;
        }
    };

    // Watch all sites' directories
    for (_name, dirs) in sites {
        for dir in dirs {
            if dir.exists() {
                if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                    human::error(&format!("Failed to watch {}: {e}", dir.display()));
                }
            }
        }
    }

    let debounce = Duration::from_millis(200);

    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(event)) => {
                // Ignore metadata-only events (e.g. atime updates from file reads on
                // strictatime/relatime systems) — only react to content changes.
                if matches!(
                    event.kind,
                    notify::EventKind::Modify(notify::event::ModifyKind::Metadata(_))
                        | notify::EventKind::Access(_)
                ) {
                    continue;
                }

                // Drain additional events within debounce window
                while rx.recv_timeout(debounce).is_ok() {}

                // Determine which site(s) changed based on the event path
                let changed_path = event.paths.first();
                let affected_site = changed_path.and_then(|p| {
                    sites
                        .iter()
                        .find(|(_, dirs)| dirs.iter().any(|d| p.starts_with(d)))
                });

                let targets: Vec<&super::WorkspaceSite> = match affected_site {
                    Some((site_name, _)) => {
                        human::info(&format!("Changes detected in '{site_name}', rebuilding..."));
                        ws_config.find_site(site_name).into_iter().collect()
                    }
                    None => {
                        human::info("Changes detected, rebuilding all sites...");
                        ws_config.sites.iter().collect()
                    }
                };
                {
                    let _guard = dev.build_guard();
                    let mut outcomes = Vec::new();
                    let mut changed = false;
                    for ws_site in targets {
                        let (outcome, site_changed) = rebuild_site(ws_root, ws_site);
                        changed |= site_changed;
                        outcomes.push((ws_site.name.clone(), outcome));
                    }
                    // One merged outcome per batch: pages reload once.
                    dev.record_site_builds(outcomes, changed);
                }

                // Drain events that accumulated during the build so they don't
                // immediately trigger another rebuild.
                while rx.try_recv().is_ok() {}
            }
            Ok(Err(e)) => {
                human::error(&format!("Watch error: {e}"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Rebuild one workspace site the way the dev server does; print and return
/// its outcome (diagnostic paths prefixed with the site's path).
fn rebuild_site(ws_root: &Path, ws_site: &super::WorkspaceSite) -> (BuildOutcome, bool) {
    let scope = SiteScope {
        name: ws_site.name.clone(),
        root: ws_root.join(&ws_site.path),
        prefix: Some(PathBuf::from(&ws_site.path)),
    };
    let started = std::time::Instant::now();
    let result = load_site_in_workspace(ws_root, ws_site).and_then(|(mut config, paths)| {
        // The dev server previews scheduled content, like drafts.
        config.build.future = true;
        let opts = BuildOptions {
            include_drafts: true,
            incremental: true,
        };
        build::build_site(&config, &paths, &opts)
    });
    let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    match &result {
        Ok(r) => human::success(&format!("[{}] {}", ws_site.name, r.stats.human_display())),
        Err(e) => human::error(&format!("Rebuild of '{}' failed: {e}", ws_site.name)),
    }
    BuildOutcome::from_result(&result, &scope, ms)
}

fn port_is_available(host: &str, port: u16) -> bool {
    TcpListener::bind((host, port)).is_ok()
}

/// Build a TCP listen address, bracketing IPv6 literals so they form a valid `SocketAddr`.
fn server_addr(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn try_bind_auto(host: &str, start_port: u16) -> Result<(Server, u16)> {
    for port in start_port..start_port.saturating_add(100) {
        if !port_is_available(host, port) {
            continue;
        }
        match Server::http(server_addr(host, port)) {
            Ok(server) => return Ok((server, port)),
            Err(_) => continue,
        }
    }
    Err(PageError::Server("no available port found".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_port_is_available_high_port() {
        assert!(
            port_is_available("127.0.0.1", 39_519),
            "high unused port should be available"
        );
    }

    #[test]
    fn test_port_is_available_detects_bound_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let bound_port = listener.local_addr().unwrap().port();
        assert!(
            !port_is_available("127.0.0.1", bound_port),
            "port with a bound listener should not be available"
        );
        drop(listener);
    }

    #[test]
    fn test_resolve_file_path_root() {
        let tmp = TempDir::new().unwrap();
        let index = tmp.path().join("index.html");
        fs::write(&index, "<html></html>").unwrap();
        assert_eq!(resolve_file_path(tmp.path(), "/"), Some(index));
    }

    #[test]
    fn test_resolve_file_path_clean_url() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("posts");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hello.html");
        fs::write(&file, "<html></html>").unwrap();
        assert_eq!(resolve_file_path(tmp.path(), "/posts/hello"), Some(file));
    }

    #[test]
    fn test_resolve_file_path_not_found() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(resolve_file_path(tmp.path(), "/nonexistent"), None);
    }

    #[test]
    fn test_route_request_matches_prefix() {
        let tmp = TempDir::new().unwrap();
        let site_dir = tmp.path().join("blog");
        fs::create_dir_all(&site_dir).unwrap();
        let file = site_dir.join("index.html");
        fs::write(&file, "<html></html>").unwrap();
        let sites = vec![("blog".to_string(), site_dir.clone())];
        let result = route_request("/blog/", &sites);
        assert!(result.is_some());
        let (path, name) = result.unwrap();
        assert_eq!(name, "blog");
        assert_eq!(path, site_dir.join("index.html"));
    }

    #[test]
    fn test_route_request_root_returns_none() {
        let tmp = TempDir::new().unwrap();
        let sites = vec![("blog".to_string(), tmp.path().to_path_buf())];
        assert!(route_request("/", &sites).is_none());
    }

    #[test]
    fn test_guess_mime_html() {
        assert_eq!(
            guess_mime(Path::new("index.html")),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn test_guess_mime_webp() {
        assert_eq!(guess_mime(Path::new("photo.webp")), "image/webp");
    }

    #[test]
    fn test_guess_mime_unknown() {
        assert_eq!(
            guess_mime(Path::new("file.xyz")),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_generate_workspace_index_contains_sites() {
        let sites = vec![
            ("blog".to_string(), PathBuf::from("/tmp/blog")),
            ("docs".to_string(), PathBuf::from("/tmp/docs")),
        ];
        let html = generate_workspace_index(&sites);
        assert!(html.contains(r#"href="/blog/""#));
        assert!(html.contains(r#"href="/docs/""#));
        assert!(html.contains("Workspace Sites"));
    }
}
