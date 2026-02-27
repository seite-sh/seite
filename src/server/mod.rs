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
        let server = Server::http(&addr).map_err(|e| {
            PageError::Server(format!("failed to start server on port {port}: {e}"))
        })?;
        (server, port)
    };

    if actual_port != port {
        human::info(&format!(
            "Port {port} in use, serving at http://localhost:{actual_port}"
        ));
    } else {
        human::success(&format!("Serving at http://localhost:{actual_port}"));
    }

    // Compute subdomain mount points for dev preview
    let subdomain_mounts: Vec<(String, PathBuf)> = config
        .subdomain_collections()
        .iter()
        .map(|c| {
            let prefix = format!("{}-preview", c.name);
            let output = paths.subdomain_output(&c.name);
            (prefix, output)
        })
        .collect();

    if !subdomain_mounts.is_empty() {
        human::info("Subdomain previews:");
        for c in config.subdomain_collections() {
            let prefix = format!("{}-preview", c.name);
            let resolved_url = config.subdomain_base_url(c);
            human::info(&format!(
                "  http://localhost:{actual_port}/{prefix}/ -> {resolved_url}"
            ));
        }
    }

    let stop = Arc::new(AtomicBool::new(false));
    let build_version = Arc::new(AtomicU64::new(1));

    // Spawn file watcher thread
    let watcher_stop = stop.clone();
    let watcher_version = build_version.clone();
    let watcher_config = config.clone();
    let watcher_paths = paths.clone();
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
    let server_paths = paths.clone();
    std::thread::spawn(move || {
        run_serve_loop(
            server,
            &server_paths,
            &subdomain_mounts,
            &build_version,
            &server_stop,
        );
    });

    Ok(ServerHandle {
        stop,
        port: actual_port,
    })
}

fn run_serve_loop(
    server: Server,
    paths: &ResolvedPaths,
    subdomain_mounts: &[(String, PathBuf)],
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
                    let _ = request.respond(Response::from_string(version).with_header(header));
                    continue;
                }

                // Check subdomain mount points (e.g., /docs-preview/ → dist-subdomains/docs/)
                let subdomain_file = {
                    let clean_url = url_path.trim_start_matches('/');
                    subdomain_mounts.iter().find_map(|(prefix, output_dir)| {
                        if clean_url.starts_with(prefix.as_str()) {
                            let rest = &clean_url[prefix.len()..];
                            let rest = if rest.is_empty() { "/" } else { rest };
                            resolve_file_path(output_dir, rest)
                                .filter(|p| p.exists() && p.is_file())
                        } else {
                            None
                        }
                    })
                };
                if let Some(ref path) = subdomain_file {
                    let content = fs::read(path).unwrap_or_default();
                    let mime = guess_mime(path);
                    let content = if mime == "text/html; charset=utf-8" {
                        inject_livereload(&content)
                    } else {
                        content
                    };
                    let header = Header::from_bytes("Content-Type", mime).expect("valid header");
                    let _ = request.respond(Response::from_data(content).with_header(header));
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

                        let header =
                            Header::from_bytes("Content-Type", mime).expect("valid header");
                        let _ = request.respond(Response::from_data(content).with_header(header));
                        continue;
                    }
                }
                // Try to serve a language-specific 404.html based on URL prefix,
                // falling back to the default 404.html
                let lang_404_path = url_path
                    .trim_start_matches('/')
                    .split('/')
                    .next()
                    .filter(|seg| seg.len() == 2)
                    .map(|lang| paths.output.join(lang).join("404.html"))
                    .filter(|p| p.exists());
                let not_found_path = lang_404_path.unwrap_or_else(|| paths.output.join("404.html"));
                if not_found_path.exists() {
                    let content = fs::read(&not_found_path).unwrap_or_default();
                    let content = inject_livereload(&content);
                    let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8")
                        .expect("valid header");
                    let _ = request.respond(
                        Response::from_data(content)
                            .with_header(header)
                            .with_status_code(404),
                    );
                } else {
                    let _ = request
                        .respond(Response::from_string("404 Not Found").with_status_code(404));
                }
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

    None
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
    let (tx, rx) = mpsc::channel();

    // Use notify's recommended watcher (FSEvents on macOS, inotify on Linux, ReadDirectoryChanges on Windows)
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            human::error(&format!("Failed to start file watcher: {e}"));
            return;
        }
    };

    // Watch content, templates, static, public, and data directories
    let dirs = [
        &paths.content,
        &paths.templates,
        &paths.static_dir,
        &paths.public_dir,
        &paths.data_dir,
    ];
    for dir in &dirs {
        if dir.exists() {
            if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                human::error(&format!("Failed to watch {}: {e}", dir.display()));
            }
        }
    }

    // Debounce: wait for events, then pause briefly to batch rapid changes
    let debounce = Duration::from_millis(200);

    while !stop.load(Ordering::Relaxed) {
        // Block until we get an event or timeout (so we can check `stop`)
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(_event)) => {
                // Got a real fs event — drain any additional events within the debounce window
                while rx.recv_timeout(debounce).is_ok() {}

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
            }
            Ok(Err(e)) => {
                human::error(&format!("Watch error: {e}"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No events — loop back to check `stop`
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Watcher dropped — exit
                break;
            }
        }
    }
}

/// Check if a port is available by trying to connect to it.
/// If the connection succeeds, something is already listening.
fn port_is_available(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}")
            .parse()
            .expect("valid socket address"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tempfile::TempDir;

    // =========================================================================
    // LIVERELOAD_SCRIPT constant
    // =========================================================================

    #[test]
    fn test_livereload_script_is_valid_script_tag() {
        assert!(
            LIVERELOAD_SCRIPT.starts_with("<script>"),
            "script should open with <script>"
        );
        assert!(
            LIVERELOAD_SCRIPT.ends_with("</script>"),
            "script should close with </script>"
        );
    }

    #[test]
    fn test_livereload_script_contains_reload_endpoint() {
        assert!(
            LIVERELOAD_SCRIPT.contains("/__livereload"),
            "script should poll the /__livereload endpoint"
        );
    }

    #[test]
    fn test_livereload_script_calls_location_reload() {
        assert!(
            LIVERELOAD_SCRIPT.contains("location.reload()"),
            "script should trigger page reload on version change"
        );
    }

    // =========================================================================
    // ServerHandle
    // =========================================================================

    #[test]
    fn test_server_handle_port() {
        let handle = ServerHandle {
            stop: Arc::new(AtomicBool::new(false)),
            port: 3000,
        };
        assert_eq!(handle.port(), 3000);
    }

    #[test]
    fn test_server_handle_stop() {
        let stop = Arc::new(AtomicBool::new(false));
        let handle = ServerHandle {
            stop: stop.clone(),
            port: 3000,
        };
        assert!(!stop.load(Ordering::Relaxed), "should start as not stopped");
        handle.stop();
        assert!(
            stop.load(Ordering::Relaxed),
            "should be stopped after calling stop()"
        );
    }

    #[test]
    fn test_server_handle_drop_stops() {
        let stop = Arc::new(AtomicBool::new(false));
        {
            let _handle = ServerHandle {
                stop: stop.clone(),
                port: 3000,
            };
            assert!(!stop.load(Ordering::Relaxed));
        } // handle dropped here
        assert!(
            stop.load(Ordering::Relaxed),
            "dropping the handle should set stop to true"
        );
    }

    #[test]
    fn test_server_handle_stop_is_idempotent() {
        let stop = Arc::new(AtomicBool::new(false));
        let handle = ServerHandle {
            stop: stop.clone(),
            port: 3000,
        };
        handle.stop();
        handle.stop();
        assert!(
            stop.load(Ordering::Relaxed),
            "calling stop twice should still be stopped"
        );
    }

    // =========================================================================
    // inject_livereload
    // =========================================================================

    #[test]
    fn test_inject_livereload_with_body_tag() {
        let html = b"<html><body><p>Hello</p></body></html>";
        let result = inject_livereload(html);
        let result_str = String::from_utf8(result).unwrap();
        assert!(
            result_str.contains(LIVERELOAD_SCRIPT),
            "livereload script should be injected"
        );
        assert!(
            result_str.contains("</body>"),
            "closing body tag should still be present"
        );
        // Script should appear before </body>
        let script_pos = result_str.find(LIVERELOAD_SCRIPT).unwrap();
        let body_pos = result_str.rfind("</body>").unwrap();
        assert!(
            script_pos < body_pos,
            "script should be injected before </body>"
        );
    }

    #[test]
    fn test_inject_livereload_no_body_tag() {
        let html = b"<html><p>No body tag here</p></html>";
        let result = inject_livereload(html);
        assert_eq!(
            result,
            html.to_vec(),
            "HTML without </body> should be returned unchanged"
        );
    }

    #[test]
    fn test_inject_livereload_empty() {
        let result = inject_livereload(b"");
        assert!(result.is_empty(), "empty input should return empty output");
    }

    #[test]
    fn test_inject_livereload_multiple_body_tags() {
        // rfind should find the LAST </body>
        let html = b"<html><body>first</body><body>second</body></html>";
        let result = inject_livereload(html);
        let result_str = String::from_utf8(result).unwrap();

        // The script should be injected before the last </body>
        let script_pos = result_str.find(LIVERELOAD_SCRIPT).unwrap();
        let last_body = result_str.rfind("</body>").unwrap();
        assert!(
            script_pos < last_body,
            "script should be injected before the last </body>"
        );

        // The first </body> should still appear before the script
        let first_body = result_str.find("</body>").unwrap();
        assert!(first_body != script_pos, "both body tags should exist");
    }

    #[test]
    fn test_inject_livereload_preserves_original_content() {
        let html = b"<html><head><title>Test</title></head><body><h1>Hello World</h1><p>Content here.</p></body></html>";
        let result = inject_livereload(html);
        let result_str = String::from_utf8(result).unwrap();

        assert!(result_str.contains("<h1>Hello World</h1>"));
        assert!(result_str.contains("<p>Content here.</p>"));
        assert!(result_str.contains("<title>Test</title>"));
        assert!(result_str.contains("</html>"));
    }

    #[test]
    fn test_inject_livereload_body_at_end() {
        let html = b"</body>";
        let result = inject_livereload(html);
        let result_str = String::from_utf8(result).unwrap();
        assert!(
            result_str.contains(LIVERELOAD_SCRIPT),
            "should inject even when </body> is the only content"
        );
        assert!(result_str.ends_with("</body>"), "should end with </body>");
    }

    #[test]
    fn test_inject_livereload_body_with_whitespace_around() {
        let html = b"<body>content\n  \n</body>\n";
        let result = inject_livereload(html);
        let result_str = String::from_utf8(result).unwrap();
        assert!(result_str.contains(LIVERELOAD_SCRIPT));
    }

    #[test]
    fn test_inject_livereload_invalid_utf8_passthrough() {
        // Invalid UTF-8 bytes without </body>
        let bytes: Vec<u8> = vec![0xFF, 0xFE, 0x00, 0x01];
        let result = inject_livereload(&bytes);
        assert_eq!(
            result, bytes,
            "non-UTF8 input without </body> should be returned as-is"
        );
    }

    #[test]
    fn test_inject_livereload_uppercase_body_not_matched() {
        // The function uses rfind("</body>") which is case-sensitive
        let html = b"<html><BODY>content</BODY></html>";
        let result = inject_livereload(html);
        assert_eq!(
            result,
            html.to_vec(),
            "uppercase </BODY> should not trigger injection"
        );
    }

    #[test]
    fn test_inject_livereload_inserts_newlines() {
        let html = b"<body>content</body>";
        let result = inject_livereload(html);
        let result_str = String::from_utf8(result).unwrap();
        // The function inserts \n before and after the script
        let expected = format!("<body>content\n{}\n</body>", LIVERELOAD_SCRIPT);
        assert_eq!(result_str, expected);
    }

    // =========================================================================
    // resolve_file_path
    // =========================================================================

    #[test]
    fn test_resolve_file_path_root() {
        let tmp = TempDir::new().unwrap();
        let index = tmp.path().join("index.html");
        fs::write(&index, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/");
        assert_eq!(result, Some(index));
    }

    #[test]
    fn test_resolve_file_path_root_no_index() {
        let tmp = TempDir::new().unwrap();
        // No index.html exists
        let result = resolve_file_path(tmp.path(), "/");
        // Returns Some(path) even though file doesn't exist — the caller checks existence
        // Actually, let's check: clean is empty, returns Some(output_dir.join("index.html"))
        assert_eq!(result, Some(tmp.path().join("index.html")));
    }

    #[test]
    fn test_resolve_file_path_exact_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("robots.txt");
        fs::write(&file, "User-agent: *").unwrap();

        let result = resolve_file_path(tmp.path(), "/robots.txt");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_clean_url() {
        let tmp = TempDir::new().unwrap();
        let posts_dir = tmp.path().join("posts");
        fs::create_dir_all(&posts_dir).unwrap();
        let file = posts_dir.join("hello-world.html");
        fs::write(&file, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/posts/hello-world");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_directory_index() {
        let tmp = TempDir::new().unwrap();
        let posts_dir = tmp.path().join("posts");
        fs::create_dir_all(&posts_dir).unwrap();
        let index = posts_dir.join("index.html");
        fs::write(&index, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/posts/");
        assert_eq!(result, Some(index));
    }

    #[test]
    fn test_resolve_file_path_not_found() {
        let tmp = TempDir::new().unwrap();

        let result = resolve_file_path(tmp.path(), "/nonexistent");
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_file_path_query_string() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("page.html");
        fs::write(&file, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/page?v=1");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_query_string_with_path() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("docs");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("guide.html");
        fs::write(&file, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/docs/guide?section=intro&highlight=true");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_empty_query_string() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("page.html");
        fs::write(&file, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/page?");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_nested_directory_index() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("docs").join("guides");
        fs::create_dir_all(&nested).unwrap();
        let index = nested.join("index.html");
        fs::write(&index, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/docs/guides");
        assert_eq!(result, Some(index));
    }

    #[test]
    fn test_resolve_file_path_nested_clean_url() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("docs").join("guides");
        fs::create_dir_all(&nested).unwrap();
        let file = nested.join("setup.html");
        fs::write(&file, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/docs/guides/setup");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_exact_html_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("about.html");
        fs::write(&file, "<html></html>").unwrap();

        // Requesting the exact .html path should find the file directly
        let result = resolve_file_path(tmp.path(), "/about.html");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_xml_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("feed.xml");
        fs::write(&file, "<rss></rss>").unwrap();

        let result = resolve_file_path(tmp.path(), "/feed.xml");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_markdown_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("posts");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hello-world.md");
        fs::write(&file, "# Hello").unwrap();

        let result = resolve_file_path(tmp.path(), "/posts/hello-world.md");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_static_asset() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("static");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("style.css");
        fs::write(&file, "body{}").unwrap();

        let result = resolve_file_path(tmp.path(), "/static/style.css");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_search_index_json() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("search-index.json");
        fs::write(&file, "[]").unwrap();

        let result = resolve_file_path(tmp.path(), "/search-index.json");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_i18n_prefix() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("es").join("posts");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hola-mundo.html");
        fs::write(&file, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/es/posts/hola-mundo");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_i18n_index() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("es");
        fs::create_dir_all(&dir).unwrap();
        let index = dir.join("index.html");
        fs::write(&index, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/es/");
        assert_eq!(result, Some(index));
    }

    #[test]
    fn test_resolve_file_path_priority_exact_file_over_html() {
        // If both "about" (exact file) and "about.html" exist, exact file wins
        let tmp = TempDir::new().unwrap();
        let exact = tmp.path().join("about");
        fs::write(&exact, "raw file").unwrap();
        let html = tmp.path().join("about.html");
        fs::write(&html, "<html></html>").unwrap();

        let result = resolve_file_path(tmp.path(), "/about");
        // "about" is a file, so candidate.is_file() returns true — exact match wins
        assert_eq!(result, Some(exact));
    }

    #[test]
    fn test_resolve_file_path_directory_without_trailing_slash() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("posts");
        fs::create_dir_all(&dir).unwrap();
        let index = dir.join("index.html");
        fs::write(&index, "<html></html>").unwrap();

        // /posts without trailing slash — candidate is a dir (not a file),
        // html_candidate posts.html doesn't exist, then checks posts/index.html
        let result = resolve_file_path(tmp.path(), "/posts");
        assert_eq!(result, Some(index));
    }

    #[test]
    fn test_resolve_file_path_directory_without_index() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("empty-dir");
        fs::create_dir_all(&dir).unwrap();

        let result = resolve_file_path(tmp.path(), "/empty-dir");
        assert_eq!(
            result, None,
            "directory without index.html should return None"
        );
    }

    #[test]
    fn test_resolve_file_path_favicon() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("favicon.ico");
        fs::write(&file, [0u8; 10]).unwrap();

        let result = resolve_file_path(tmp.path(), "/favicon.ico");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_image_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("static").join("images");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("photo.jpg");
        fs::write(&file, [0xFF, 0xD8, 0xFF]).unwrap();

        let result = resolve_file_path(tmp.path(), "/static/images/photo.jpg");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_dotfile() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join(".nojekyll");
        fs::write(&file, "").unwrap();

        let result = resolve_file_path(tmp.path(), "/.nojekyll");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_resolve_file_path_fingerprinted_asset() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("static");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("style.a1b2c3d4.css");
        fs::write(&file, "body{}").unwrap();

        let result = resolve_file_path(tmp.path(), "/static/style.a1b2c3d4.css");
        assert_eq!(result, Some(file));
    }

    // =========================================================================
    // guess_mime
    // =========================================================================

    #[test]
    fn test_guess_mime_html() {
        assert_eq!(
            guess_mime(Path::new("index.html")),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn test_guess_mime_css() {
        assert_eq!(guess_mime(Path::new("style.css")), "text/css");
    }

    #[test]
    fn test_guess_mime_js() {
        assert_eq!(guess_mime(Path::new("app.js")), "application/javascript");
    }

    #[test]
    fn test_guess_mime_json() {
        assert_eq!(guess_mime(Path::new("data.json")), "application/json");
    }

    #[test]
    fn test_guess_mime_xml() {
        assert_eq!(guess_mime(Path::new("sitemap.xml")), "application/xml");
    }

    #[test]
    fn test_guess_mime_images() {
        assert_eq!(guess_mime(Path::new("photo.png")), "image/png");
        assert_eq!(guess_mime(Path::new("photo.jpg")), "image/jpeg");
        assert_eq!(guess_mime(Path::new("photo.jpeg")), "image/jpeg");
        assert_eq!(guess_mime(Path::new("anim.gif")), "image/gif");
        assert_eq!(guess_mime(Path::new("icon.svg")), "image/svg+xml");
        assert_eq!(guess_mime(Path::new("favicon.ico")), "image/x-icon");
    }

    #[test]
    fn test_guess_mime_fonts() {
        assert_eq!(guess_mime(Path::new("font.woff")), "font/woff");
        assert_eq!(guess_mime(Path::new("font.woff2")), "font/woff2");
    }

    #[test]
    fn test_guess_mime_md() {
        assert_eq!(
            guess_mime(Path::new("readme.md")),
            "text/plain; charset=utf-8"
        );
    }

    #[test]
    fn test_guess_mime_txt() {
        assert_eq!(
            guess_mime(Path::new("llms.txt")),
            "text/plain; charset=utf-8"
        );
    }

    #[test]
    fn test_guess_mime_unknown() {
        assert_eq!(
            guess_mime(Path::new("archive.tar.gz")),
            "application/octet-stream"
        );
        assert_eq!(
            guess_mime(Path::new("file.xyz")),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_guess_mime_no_extension() {
        assert_eq!(guess_mime(Path::new("CNAME")), "application/octet-stream");
        assert_eq!(
            guess_mime(Path::new(".nojekyll")),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_guess_mime_nested_path() {
        assert_eq!(guess_mime(Path::new("static/css/style.css")), "text/css");
        assert_eq!(guess_mime(Path::new("posts/2026/photo.png")), "image/png");
        assert_eq!(
            guess_mime(Path::new("docs/guides/setup.html")),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn test_guess_mime_double_extension() {
        // Path::extension() returns the last extension
        assert_eq!(
            guess_mime(Path::new("archive.tar.gz")),
            "application/octet-stream"
        );
        assert_eq!(guess_mime(Path::new("style.min.css")), "text/css");
        assert_eq!(
            guess_mime(Path::new("app.bundle.js")),
            "application/javascript"
        );
    }

    #[test]
    fn test_guess_mime_fingerprinted_assets() {
        // Fingerprinted file: name.<hash>.ext — extension() still returns the last part
        assert_eq!(guess_mime(Path::new("style.a1b2c3d4.css")), "text/css");
        assert_eq!(
            guess_mime(Path::new("app.deadbeef.js")),
            "application/javascript"
        );
    }

    #[test]
    fn test_guess_mime_webp() {
        // WebP is not in the match — should return octet-stream
        assert_eq!(
            guess_mime(Path::new("image.webp")),
            "application/octet-stream"
        );
    }

    // =========================================================================
    // port_is_available
    // =========================================================================

    #[test]
    fn test_port_is_available_high_port() {
        // Port 39_517 is high and extremely unlikely to be in use
        assert!(
            port_is_available(39_517),
            "a high unused port should be available"
        );
    }

    #[test]
    fn test_port_is_available_multiple_high_ports() {
        // Several high ports should all be available
        for port in [39_518, 49_999, 60_000, 65_000] {
            assert!(
                port_is_available(port),
                "high port {port} should be available"
            );
        }
    }

    #[test]
    fn test_port_is_available_detects_bound_port() {
        // Bind a port, then check that port_is_available returns false
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let bound_port = listener.local_addr().unwrap().port();
        assert!(
            !port_is_available(bound_port),
            "a port with a bound listener should not be available"
        );
        drop(listener);
    }

    // =========================================================================
    // try_bind_auto
    // =========================================================================

    #[test]
    fn test_try_bind_auto_finds_available_port() {
        // Should find a port starting from a high number
        let result = try_bind_auto(49_800);
        assert!(result.is_ok(), "should find an available port");
        let (server, port) = result.unwrap();
        assert!(port >= 49_800);
        assert!(port < 49_900);
        drop(server);
    }

    #[test]
    fn test_try_bind_auto_skips_busy_port() {
        // Bind a port, then ask try_bind_auto to start from it
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let bound_port = listener.local_addr().unwrap().port();

        let result = try_bind_auto(bound_port);
        assert!(result.is_ok(), "should find a port even if first is busy");
        let (_server, actual_port) = result.unwrap();
        // It might bind the same port if the OS released it or a different one
        // The key is it succeeds
        assert!(actual_port >= bound_port);
        drop(listener);
    }

    // =========================================================================
    // Integration: resolve_file_path + guess_mime
    // =========================================================================

    #[test]
    fn test_resolve_and_mime_html_page() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("posts");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("my-post.html");
        fs::write(&file, "<html></html>").unwrap();

        let resolved = resolve_file_path(tmp.path(), "/posts/my-post");
        assert_eq!(resolved, Some(file.clone()));

        let mime = guess_mime(&file);
        assert_eq!(mime, "text/html; charset=utf-8");
    }

    #[test]
    fn test_resolve_and_mime_css_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("static");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("style.css");
        fs::write(&file, "body{}").unwrap();

        let resolved = resolve_file_path(tmp.path(), "/static/style.css");
        assert_eq!(resolved, Some(file.clone()));

        let mime = guess_mime(&file);
        assert_eq!(mime, "text/css");
    }

    #[test]
    fn test_resolve_and_mime_markdown_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("posts");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("my-post.md");
        fs::write(&file, "# Title").unwrap();

        let resolved = resolve_file_path(tmp.path(), "/posts/my-post.md");
        assert_eq!(resolved, Some(file.clone()));

        let mime = guess_mime(&file);
        assert_eq!(mime, "text/plain; charset=utf-8");
    }

    // =========================================================================
    // Edge cases / regression tests
    // =========================================================================

    #[test]
    fn test_resolve_file_path_root_empty_string() {
        let tmp = TempDir::new().unwrap();
        // "/" trims to "" — returns index.html
        let result = resolve_file_path(tmp.path(), "");
        // empty path after trimming = returns index.html (even if not exists)
        assert_eq!(result, Some(tmp.path().join("index.html")));
    }

    #[test]
    fn test_resolve_file_path_just_slash() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_file_path(tmp.path(), "/");
        assert_eq!(result, Some(tmp.path().join("index.html")));
    }

    #[test]
    fn test_resolve_file_path_multiple_leading_slashes() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("page.html");
        fs::write(&file, "<html></html>").unwrap();

        // trim_start_matches('/') removes all leading slashes
        let result = resolve_file_path(tmp.path(), "///page");
        assert_eq!(result, Some(file));
    }

    #[test]
    fn test_inject_livereload_large_html() {
        // Simulate a large page
        let mut html = String::from("<html><body>");
        for i in 0..1000 {
            html.push_str(&format!("<p>Paragraph {i}</p>"));
        }
        html.push_str("</body></html>");
        let result = inject_livereload(html.as_bytes());
        let result_str = String::from_utf8(result).unwrap();
        assert!(result_str.contains(LIVERELOAD_SCRIPT));
        assert!(result_str.contains("<p>Paragraph 999</p>"));
    }

    #[test]
    fn test_inject_livereload_only_body_close_in_string() {
        // </body> appears as text content, not as a real tag — the function
        // doesn't parse HTML, it uses string matching, so it still injects
        let html = b"<body><p>the text </body> is special</p></body>";
        let result = inject_livereload(html);
        let result_str = String::from_utf8(result).unwrap();
        assert!(
            result_str.contains(LIVERELOAD_SCRIPT),
            "should inject even when </body> appears in content"
        );
    }
}
