use std::collections::HashMap;
use std::fs;
use std::net::TcpStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tiny_http::{Header, Response, Server};
use walkdir::WalkDir;

use crate::build::{self, BuildOptions};
use crate::config::{ResolvedPaths, SiteConfig};
use crate::error::{PageError, Result};
use crate::output::human;
use crate::output::CommandOutput;

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

/// Handle to a running dev server. Drop or call `stop()` to shut down.
pub struct ServerHandle {
    stop: Arc<AtomicBool>,
    port: u16,
}

impl ServerHandle {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start the dev server in background threads. Returns a handle to stop it.
pub fn start(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    port: u16,
    include_drafts: bool,
    auto_increment: bool,
) -> Result<ServerHandle> {
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

    if actual_port != port {
        human::info(&format!(
            "Port {port} in use, serving at http://localhost:{actual_port}"
        ));
    } else {
        human::success(&format!("Serving at http://localhost:{actual_port}"));
    }

    let stop = Arc::new(AtomicBool::new(false));
    let build_version = Arc::new(AtomicU64::new(1));

    // Spawn file watcher thread
    let watcher_stop = stop.clone();
    let watcher_version = build_version.clone();
    let watcher_config = config.clone();
    let watcher_paths = clone_paths(paths);
    std::thread::spawn(move || {
        watch_and_rebuild(
            &watcher_config,
            &watcher_paths,
            include_drafts,
            &watcher_stop,
            &watcher_version,
        );
    });

    // Spawn HTTP server thread
    let server_stop = stop.clone();
    let server_paths = clone_paths(paths);
    std::thread::spawn(move || {
        run_serve_loop(server, &server_paths, &build_version, &server_stop);
    });

    Ok(ServerHandle {
        stop,
        port: actual_port,
    })
}

fn run_serve_loop(
    server: Server,
    paths: &ResolvedPaths,
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
                        Header::from_bytes("Content-Type", "text/plain").unwrap();
                    let _ = request
                        .respond(Response::from_string(version).with_header(header));
                    continue;
                }

                let file_path = resolve_file_path(&paths.output, &url_path);

                if let Some(ref path) = file_path {
                    if path.exists() && path.is_file() {
                        let content = fs::read(path).unwrap_or_default();
                        let mime = guess_mime(path);

                        // Inject livereload script into HTML responses
                        let content = if mime == "text/html; charset=utf-8" {
                            inject_livereload(&content)
                        } else {
                            content
                        };

                        let header = Header::from_bytes("Content-Type", mime).unwrap();
                        let _ = request
                            .respond(Response::from_data(content).with_header(header));
                        continue;
                    }
                }
                let _ =
                    request.respond(Response::from_string("404 Not Found").with_status_code(404));
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
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

fn resolve_file_path(output_dir: &Path, url_path: &str) -> Option<std::path::PathBuf> {
    let clean = url_path.split('?').next().unwrap_or(url_path);
    let clean = clean.trim_start_matches('/');
    if clean.is_empty() {
        return Some(output_dir.join("index.html"));
    }

    // Exact file match (e.g., /posts/hello-world.md, /feed.xml, /robots.txt)
    let candidate = output_dir.join(clean);
    if candidate.is_file() {
        return Some(candidate);
    }

    // Clean URL → .html file (e.g., /posts/hello-world → posts/hello-world.html)
    let html_candidate = output_dir.join(format!("{clean}.html"));
    if html_candidate.is_file() {
        return Some(html_candidate);
    }

    // Directory with index.html (e.g., / → index.html)
    let index = candidate.join("index.html");
    if index.is_file() {
        return Some(index);
    }

    Some(candidate)
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
        _ => "application/octet-stream",
    }
}

fn watch_and_rebuild(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    include_drafts: bool,
    stop: &AtomicBool,
    build_version: &AtomicU64,
) {
    let mut last_mtimes = collect_mtimes(paths);

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_secs(1));

        let current = collect_mtimes(paths);
        if current != last_mtimes {
            human::info("Changes detected, rebuilding...");
            let opts = BuildOptions { include_drafts };
            match build::build_site(config, paths, &opts) {
                Ok(result) => {
                    build_version.fetch_add(1, Ordering::Relaxed);
                    human::success(&result.stats.human_display());
                }
                Err(e) => {
                    human::error(&format!("Rebuild failed: {e}"));
                }
            }
            last_mtimes = current;
        }
    }
}

fn collect_mtimes(paths: &ResolvedPaths) -> HashMap<std::path::PathBuf, SystemTime> {
    let mut map = HashMap::new();
    let dirs = [&paths.content, &paths.templates, &paths.static_dir];
    for dir in dirs {
        if dir.exists() {
            for entry in WalkDir::new(dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        map.insert(entry.path().to_path_buf(), mtime);
                    }
                }
            }
        }
    }
    map
}

/// Check if a port is available by trying to connect to it.
/// If the connection succeeds, something is already listening.
fn port_is_available(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(100),
    )
    .is_err()
}

fn try_bind_auto(start_port: u16) -> Result<(Server, u16)> {
    for port in start_port..start_port.saturating_add(100) {
        if !port_is_available(port) {
            continue;
        }
        match Server::http(&format!("127.0.0.1:{port}")) {
            Ok(server) => return Ok((server, port)),
            Err(_) => continue,
        }
    }
    Err(PageError::Server("no available port found".into()))
}

fn clone_paths(paths: &ResolvedPaths) -> ResolvedPaths {
    ResolvedPaths {
        root: paths.root.clone(),
        output: paths.output.clone(),
        content: paths.content.clone(),
        templates: paths.templates.clone(),
        static_dir: paths.static_dir.clone(),
    }
}
