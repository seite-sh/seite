use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tiny_http::{Header, Response, Server};

use crate::build::{self, BuildOptions};
use crate::error::{PageError, Result};
use crate::output::human;
use crate::output::CommandOutput;

use super::{load_site_in_workspace, WorkspaceConfig};

const LIVERELOAD_SCRIPT: &str = r#"<script>
(function(){
  var v = "0";
  setInterval(function(){
    fetch("/__livereload").then(function(r){ return r.text(); }).then(function(t){
      if (v !== "0" && t !== v) location.reload();
      v = t;
    }).catch(function(){});
  }, 1000);
})();
</script>"#;

/// Handle to a running workspace dev server.
pub struct WorkspaceServerHandle {
    stop: Arc<AtomicBool>,
    port: u16,
}

impl WorkspaceServerHandle {
    pub fn port(&self) -> u16 {
        self.port
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
    output_dir: PathBuf,
    watch_dirs: Vec<PathBuf>,
}

/// Start a workspace dev server that routes requests by path prefix.
/// `localhost:3000/blog/...` -> sites/blog/dist/...
pub fn start(
    ws_config: &WorkspaceConfig,
    ws_root: &Path,
    port: u16,
    auto_increment: bool,
) -> Result<WorkspaceServerHandle> {
    let (server, actual_port) = if auto_increment {
        try_bind_auto(port)?
    } else {
        if !port_is_available(port) {
            return Err(PageError::Server(format!("port {port} is already in use")));
        }
        let addr = format!("127.0.0.1:{port}");
        let server = Server::http(&addr)
            .map_err(|e| PageError::Server(format!("failed to start server on port {port}: {e}")))?;
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
            paths.data_dir.clone(),
        ];
        sites.push(SiteServerInfo {
            name: ws_site.name.clone(),
            output_dir: paths.output.clone(),
            watch_dirs,
        });
    }

    if actual_port != port {
        human::info(&format!(
            "Port {port} in use, serving at http://localhost:{actual_port}"
        ));
    } else {
        human::success(&format!("Serving workspace at http://localhost:{actual_port}"));
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
    let build_version = Arc::new(AtomicU64::new(1));

    // Spawn file watcher thread
    let watcher_stop = stop.clone();
    let watcher_version = build_version.clone();
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
            &watcher_version,
        );
    });

    // Spawn HTTP server thread
    let server_stop = stop.clone();
    let server_sites: Vec<(String, PathBuf)> = sites
        .iter()
        .map(|s| (s.name.clone(), s.output_dir.clone()))
        .collect();
    std::thread::spawn(move || {
        run_serve_loop(server, &server_sites, &build_version, &server_stop);
    });

    Ok(WorkspaceServerHandle {
        stop,
        port: actual_port,
    })
}

fn run_serve_loop(
    server: Server,
    sites: &[(String, PathBuf)],
    build_version: &AtomicU64,
    stop: &AtomicBool,
) {
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match server.recv_timeout(Duration::from_secs(1)) {
            Ok(Some(request)) => {
                let url_path = request.url().to_string();

                // Live reload polling endpoint
                if url_path == "/__livereload" {
                    let version = build_version.load(Ordering::Relaxed).to_string();
                    let header =
                        Header::from_bytes("Content-Type", "text/plain").expect("valid header");
                    let _ = request
                        .respond(Response::from_string(version).with_header(header));
                    continue;
                }

                // Route by path prefix: /blog/... -> sites/blog/dist/...
                if let Some((file_path, _site_name)) = route_request(&url_path, sites) {
                    if file_path.exists() && file_path.is_file() {
                        let content = fs::read(&file_path).unwrap_or_default();
                        let mime = guess_mime(&file_path);

                        let content = if mime == "text/html; charset=utf-8" {
                            inject_livereload(&content)
                        } else {
                            content
                        };

                        let header = Header::from_bytes("Content-Type", mime).expect("valid header");
                        let _ = request
                            .respond(Response::from_data(content).with_header(header));
                        continue;
                    }
                }

                // Workspace index page
                if url_path == "/" {
                    let index_html = generate_workspace_index(sites);
                    let content = inject_livereload(index_html.as_bytes());
                    let header =
                        Header::from_bytes("Content-Type", "text/html; charset=utf-8").expect("valid header");
                    let _ = request
                        .respond(Response::from_data(content).with_header(header));
                    continue;
                }

                let _ = request
                    .respond(Response::from_string("404 Not Found").with_status_code(404));
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
}

/// Route a request to a site's output directory based on URL path prefix.
fn route_request<'a>(
    url_path: &str,
    sites: &'a [(String, PathBuf)],
) -> Option<(PathBuf, &'a str)> {
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

fn inject_livereload(html_bytes: &[u8]) -> Vec<u8> {
    let html = String::from_utf8_lossy(html_bytes);
    if let Some(pos) = html.rfind("</body>") {
        let mut result = String::with_capacity(html.len() + LIVERELOAD_SCRIPT.len() + 1);
        result.push_str(&html[..pos]);
        result.push('\n');
        result.push_str(LIVERELOAD_SCRIPT);
        result.push('\n');
        result.push_str(&html[pos..]);
        result.into_bytes()
    } else {
        html_bytes.to_vec()
    }
}

fn generate_workspace_index(sites: &[(String, PathBuf)]) -> String {
    let mut links = String::new();
    for (name, _) in sites {
        links.push_str(&format!(
            "    <li><a href=\"/{name}/\">{name}</a></li>\n"
        ));
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
        Some("md") => "text/plain; charset=utf-8",
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
    build_version: &AtomicU64,
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
                // Drain additional events within debounce window
                while rx.recv_timeout(debounce).is_ok() {}

                // Determine which site(s) changed based on the event path
                let changed_path = event.paths.first();
                let affected_site = changed_path.and_then(|p| {
                    sites.iter().find(|(_, dirs)| {
                        dirs.iter().any(|d| p.starts_with(d))
                    })
                });

                if let Some((site_name, _)) = affected_site {
                    // Rebuild only the affected site
                    human::info(&format!("Changes detected in '{site_name}', rebuilding..."));
                    if let Some(ws_site) = ws_config.find_site(site_name) {
                        if let Ok((config, paths)) = load_site_in_workspace(ws_root, ws_site) {
                            let opts = BuildOptions {
                                include_drafts: true,
                            };
                            match build::build_site(&config, &paths, &opts) {
                                Ok(result) => {
                                    build_version.fetch_add(1, Ordering::Relaxed);
                                    human::success(&result.stats.human_display());
                                }
                                Err(e) => {
                                    human::error(&format!("Rebuild of '{site_name}' failed: {e}"));
                                }
                            }
                        }
                    }
                } else {
                    // Could not determine site — rebuild all
                    human::info("Changes detected, rebuilding all sites...");
                    for ws_site in &ws_config.sites {
                        if let Ok((config, paths)) = load_site_in_workspace(ws_root, ws_site) {
                            let opts = BuildOptions {
                                include_drafts: true,
                            };
                            match build::build_site(&config, &paths, &opts) {
                                Ok(result) => {
                                    human::success(&format!(
                                        "[{}] {}",
                                        ws_site.name,
                                        result.stats.human_display()
                                    ));
                                }
                                Err(e) => {
                                    human::error(&format!(
                                        "Rebuild of '{}' failed: {e}",
                                        ws_site.name
                                    ));
                                }
                            }
                        }
                    }
                    build_version.fetch_add(1, Ordering::Relaxed);
                }
            }
            Ok(Err(e)) => {
                human::error(&format!("Watch error: {e}"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn port_is_available(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().expect("valid socket address"),
        Duration::from_millis(100),
    )
    .is_err()
}

fn try_bind_auto(start_port: u16) -> Result<(Server, u16)> {
    for port in start_port..start_port.saturating_add(100) {
        if !port_is_available(port) {
            continue;
        }
        match Server::http(format!("127.0.0.1:{port}")) {
            Ok(server) => return Ok((server, port)),
            Err(_) => continue,
        }
    }
    Err(PageError::Server("no available port found".into()))
}
