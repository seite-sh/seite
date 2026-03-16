use std::fs;
use std::time::Duration;

use clap::Args;
use tiny_http::{Header, Method, Response, Server};

use crate::build::{self, BuildOptions};
use crate::config::{self, SiteConfig};
use crate::content::{self, Frontmatter};
use crate::output::human;
use crate::server;

const EDITOR_HTML: &str = include_str!("../editor.html");

#[derive(Args)]
pub struct EditArgs {
    /// Host to bind to
    #[arg(long)]
    pub host: Option<String>,

    /// Port to serve on
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Open the editor in the default browser
    #[arg(long)]
    pub open: bool,
}

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_EDITOR_PORT: u16 = 3001;

pub fn run(args: &EditArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let site_config = SiteConfig::load(&cwd.join("seite.toml"))?;
    let paths = site_config.resolve_paths(&cwd);

    let host = args.host.as_deref().unwrap_or(DEFAULT_HOST);
    let editor_port = args.port.unwrap_or(DEFAULT_EDITOR_PORT);

    // Build the site first
    human::info("Building site...");
    let build_opts = BuildOptions {
        include_drafts: true,
        incremental: false,
    };
    build::build_site(&site_config, &paths, &build_opts)?;

    // Start the preview server (serves the built site for the iframe)
    let preview_handle = server::start(&site_config, &paths, host, 3000, true, true)?;
    let preview_port = preview_handle.port();
    human::info(&format!(
        "Preview server running on http://{host}:{preview_port}/"
    ));

    // Start the editor server
    let server = find_available_server(host, editor_port)?;
    let actual_port = server.1;
    let http_server = server.0;

    print_editor_banner(host, actual_port, preview_port);

    if args.open {
        let url = format!("http://localhost:{actual_port}");
        let _ = open::that(&url);
    }

    // Run the editor server loop (blocks until process is interrupted)
    run_editor_loop(http_server, &site_config, &paths, preview_port, host);

    preview_handle.stop();
    human::info("Editor stopped.");
    Ok(())
}

fn find_available_server(host: &str, start_port: u16) -> anyhow::Result<(Server, u16)> {
    for port in start_port..start_port + 100 {
        let addr = if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        };
        if let Ok(server) = Server::http(&addr) {
            return Ok((server, port));
        }
    }
    anyhow::bail!(
        "could not find an available port in range {start_port}..{}",
        start_port + 100
    )
}

fn print_editor_banner(_host: &str, port: u16, preview_port: u16) {
    use console::style;

    let version = env!("CARGO_PKG_VERSION");
    println!();
    println!(
        "  {} {} {}",
        style("seite edit").bold().cyan(),
        style(format!("v{version}")).dim(),
        style("— visual editor").dim()
    );
    println!();
    println!(
        "  {}  Editor:  {}",
        style("➜").green(),
        style(format!("http://localhost:{port}/"))
            .cyan()
            .underlined()
    );
    println!(
        "  {}  Preview: {}",
        style("➜").dim(),
        style(format!("http://localhost:{preview_port}/")).dim()
    );
    println!();
    println!("  {}", style("Press Ctrl+C to stop").dim());
    println!();
}

fn run_editor_loop(
    server: Server,
    config: &SiteConfig,
    paths: &config::ResolvedPaths,
    preview_port: u16,
    host: &str,
) {
    loop {
        match server.recv_timeout(Duration::from_secs(1)) {
            Ok(Some(request)) => {
                let url = request.url().to_string();
                let method = request.method().clone();

                if url == "/" || url == "/__editor" || url == "/__editor/" {
                    let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8")
                        .expect("valid header");
                    let _ = request.respond(Response::from_string(EDITOR_HTML).with_header(header));
                    continue;
                }

                if url.starts_with("/__editor/api/") {
                    handle_api(request, &url, &method, config, paths, preview_port, host);
                    continue;
                }

                // Fallback: 404
                let _ =
                    request.respond(Response::from_string("404 Not Found").with_status_code(404));
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
}

fn handle_api(
    request: tiny_http::Request,
    url: &str,
    method: &Method,
    config: &SiteConfig,
    paths: &config::ResolvedPaths,
    preview_port: u16,
    host: &str,
) {
    let path = url.strip_prefix("/__editor/api").unwrap_or(url);
    let (route, query) = if let Some(q) = path.find('?') {
        (&path[..q], Some(&path[q + 1..]))
    } else {
        (path, None)
    };

    match (method, route) {
        (&Method::Get, "/collections") => {
            api_get_collections(request, config, paths, preview_port, host);
        }
        (&Method::Get, "/file") => {
            let file_path = extract_query_param(query, "path");
            api_get_file(request, file_path, config, paths);
        }
        (&Method::Put, "/file") => {
            api_save_file(request, config, paths);
        }
        (&Method::Post, "/file") => {
            api_create_file(request, config, paths);
        }
        (&Method::Delete, "/file") => {
            let file_path = extract_query_param(query, "path");
            api_delete_file(request, file_path, paths);
        }
        _ => {
            let _ = request.respond(
                Response::from_string(format!("Unknown API route: {method} {route}"))
                    .with_status_code(404),
            );
        }
    }
}

fn extract_query_param(query: Option<&str>, name: &str) -> Option<String> {
    query.and_then(|q| {
        q.split('&').find_map(|pair| {
            let (key, val) = pair.split_once('=')?;
            if key == name {
                Some(urlencoding::decode(val).unwrap_or_default().into_owned())
            } else {
                None
            }
        })
    })
}

fn json_response(request: tiny_http::Request, json: &str) {
    let header = Header::from_bytes("Content-Type", "application/json; charset=utf-8")
        .expect("valid header");
    let _ = request.respond(Response::from_string(json).with_header(header));
}

fn error_response(request: tiny_http::Request, status: u16, message: &str) {
    let _ = request.respond(Response::from_string(message).with_status_code(status));
}

fn read_body(request: &mut tiny_http::Request) -> Result<String, std::io::Error> {
    let mut body = String::new();
    request.as_reader().read_to_string(&mut body)?;
    Ok(body)
}

// --- API Handlers ---

fn api_get_collections(
    request: tiny_http::Request,
    config: &SiteConfig,
    paths: &config::ResolvedPaths,
    preview_port: u16,
    host: &str,
) {
    let mut collections = Vec::new();

    for col in &config.collections {
        let dir = paths.content.join(&col.directory);
        let mut files = Vec::new();

        if dir.exists() {
            if let Ok(entries) = fs::read_dir(&dir) {
                let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
                entries.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

                for entry in entries {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("md") {
                        continue;
                    }
                    if let Ok((fm, _)) = content::parse_content_file(&path) {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy();
                        let rel_path = path
                            .strip_prefix(&paths.root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();

                        let date_str = fm.date.map(|d| d.format("%Y-%m-%d").to_string());

                        files.push(serde_json::json!({
                            "path": rel_path,
                            "filename": filename,
                            "title": fm.title,
                            "date": date_str,
                            "draft": fm.draft,
                        }));
                    }
                }
            }

            // Also scan subdirectories for nested collections
            if col.nested {
                collect_nested_files(&dir, &dir, &paths.root, &mut files);
            }
        }

        collections.push(serde_json::json!({
            "name": col.name,
            "label": col.label,
            "has_date": col.has_date,
            "url_prefix": col.url_prefix,
            "files": files,
        }));
    }

    let preview_url = format!("http://{}:{}", host, preview_port);
    let result = serde_json::json!({
        "collections": collections,
        "preview_url": preview_url,
    });
    json_response(request, &result.to_string());
}

fn collect_nested_files(
    base_dir: &std::path::Path,
    current_dir: &std::path::Path,
    root: &std::path::Path,
    files: &mut Vec<serde_json::Value>,
) {
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() && path != base_dir {
                collect_nested_files(base_dir, &path, root, files);
            } else if path.is_file()
                && path.extension().and_then(|e| e.to_str()) == Some("md")
                && path.parent() != Some(base_dir)
            {
                // Already collected top-level files above, skip them
                if let Ok((fm, _)) = content::parse_content_file(&path) {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy();
                    let rel_path = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    let date_str = fm.date.map(|d| d.format("%Y-%m-%d").to_string());

                    files.push(serde_json::json!({
                        "path": rel_path,
                        "filename": filename,
                        "title": fm.title,
                        "date": date_str,
                        "draft": fm.draft,
                    }));
                }
            }
        }
    }
}

fn api_get_file(
    request: tiny_http::Request,
    file_path: Option<String>,
    config: &SiteConfig,
    paths: &config::ResolvedPaths,
) {
    let Some(rel_path) = file_path else {
        error_response(request, 400, "Missing 'path' query parameter");
        return;
    };

    let abs_path = paths.root.join(&rel_path);

    // Security: ensure path is within content directory
    if !abs_path.starts_with(&paths.content) {
        error_response(request, 403, "Access denied");
        return;
    }

    if !abs_path.exists() {
        error_response(request, 404, "File not found");
        return;
    }

    match content::parse_content_file(&abs_path) {
        Ok((fm, body)) => {
            // Figure out the URL for preview
            let url = compute_content_url(&abs_path, &fm, config, paths);

            let result = serde_json::json!({
                "path": rel_path,
                "url": url,
                "frontmatter": {
                    "title": fm.title,
                    "date": fm.date.map(|d| d.format("%Y-%m-%d").to_string()),
                    "updated": fm.updated.map(|d| d.format("%Y-%m-%d").to_string()),
                    "description": fm.description,
                    "image": fm.image,
                    "slug": fm.slug,
                    "tags": fm.tags,
                    "draft": fm.draft,
                    "template": fm.template,
                    "weight": fm.weight,
                },
                "body": body,
            });
            json_response(request, &result.to_string());
        }
        Err(e) => {
            error_response(request, 500, &format!("Parse error: {e}"));
        }
    }
}

fn compute_content_url(
    abs_path: &std::path::Path,
    fm: &Frontmatter,
    config: &SiteConfig,
    paths: &config::ResolvedPaths,
) -> String {
    // Find which collection this file belongs to
    for col in &config.collections {
        let col_dir = paths.content.join(&col.directory);
        if abs_path.starts_with(&col_dir) {
            let stem = abs_path.file_stem().unwrap_or_default().to_string_lossy();

            // Strip language suffix if present
            let configured_langs = config.configured_lang_codes();
            let clean_stem = content::strip_lang_suffix(&stem, &configured_langs);

            // For dated collections, strip the date prefix
            let slug = if col.has_date && clean_stem.len() > 11 {
                &clean_stem[11..]
            } else {
                clean_stem
            };

            // Use custom slug from frontmatter if set
            let final_slug = fm.slug.as_deref().unwrap_or(slug);

            let prefix = if col.url_prefix.is_empty() {
                String::new()
            } else {
                col.url_prefix.clone()
            };

            return format!("{prefix}/{final_slug}");
        }
    }

    // Fallback
    String::from("/")
}

fn api_save_file(
    mut request: tiny_http::Request,
    _config: &SiteConfig,
    paths: &config::ResolvedPaths,
) {
    let body = match read_body(&mut request) {
        Ok(b) => b,
        Err(e) => {
            error_response(request, 400, &format!("Failed to read body: {e}"));
            return;
        }
    };

    let data: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            error_response(request, 400, &format!("Invalid JSON: {e}"));
            return;
        }
    };

    let Some(rel_path) = data["path"].as_str() else {
        error_response(request, 400, "Missing 'path' field");
        return;
    };

    let abs_path = paths.root.join(rel_path);
    if !abs_path.starts_with(&paths.content) {
        error_response(request, 403, "Access denied");
        return;
    }

    // Build frontmatter
    let fm_data = &data["frontmatter"];
    let fm = build_frontmatter_from_json(fm_data);
    let content_body = data["body"].as_str().unwrap_or("");

    let file_content = format!(
        "{}\n\n{}\n",
        content::generate_frontmatter(&fm),
        content_body
    );

    match fs::write(&abs_path, &file_content) {
        Ok(_) => json_response(request, r#"{"ok":true}"#),
        Err(e) => error_response(request, 500, &format!("Write failed: {e}")),
    }
}

fn api_create_file(
    mut request: tiny_http::Request,
    config: &SiteConfig,
    paths: &config::ResolvedPaths,
) {
    let body = match read_body(&mut request) {
        Ok(b) => b,
        Err(e) => {
            error_response(request, 400, &format!("Failed to read body: {e}"));
            return;
        }
    };

    let data: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            error_response(request, 400, &format!("Invalid JSON: {e}"));
            return;
        }
    };

    let collection_name = data["collection"].as_str().unwrap_or("");
    let title = data["title"].as_str().unwrap_or("").trim();
    let tags_str = data["tags"].as_str().unwrap_or("");
    let draft = data["draft"].as_bool().unwrap_or(false);

    if title.is_empty() {
        error_response(request, 400, "Title is required");
        return;
    }

    let collection = match config::find_collection(collection_name, &config.collections) {
        Some(c) => c,
        None => {
            error_response(
                request,
                400,
                &format!("Unknown collection: {collection_name}"),
            );
            return;
        }
    };

    let slug = content::slug_from_title(title);
    let tags_vec: Vec<String> = if tags_str.is_empty() {
        Vec::new()
    } else {
        tags_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let date = if collection.has_date {
        Some(chrono::Local::now().date_naive())
    } else {
        None
    };

    let fm = Frontmatter {
        title: title.to_string(),
        date,
        tags: tags_vec,
        draft,
        ..Default::default()
    };

    let filename = if collection.has_date {
        let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
        format!("{date_str}-{slug}.md")
    } else {
        format!("{slug}.md")
    };

    let filepath = paths.content.join(&collection.directory).join(&filename);
    if let Some(parent) = filepath.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let file_content = format!(
        "{}\n\nWrite your content here.\n",
        content::generate_frontmatter(&fm)
    );

    match fs::write(&filepath, &file_content) {
        Ok(_) => {
            let rel_path = filepath
                .strip_prefix(&paths.root)
                .unwrap_or(&filepath)
                .to_string_lossy()
                .to_string();
            let result = serde_json::json!({ "ok": true, "path": rel_path });
            json_response(request, &result.to_string());
        }
        Err(e) => error_response(request, 500, &format!("Create failed: {e}")),
    }
}

fn api_delete_file(
    request: tiny_http::Request,
    file_path: Option<String>,
    paths: &config::ResolvedPaths,
) {
    let Some(rel_path) = file_path else {
        error_response(request, 400, "Missing 'path' query parameter");
        return;
    };

    let abs_path = paths.root.join(&rel_path);
    if !abs_path.starts_with(&paths.content) {
        error_response(request, 403, "Access denied");
        return;
    }

    if !abs_path.exists() {
        error_response(request, 404, "File not found");
        return;
    }

    match fs::remove_file(&abs_path) {
        Ok(_) => json_response(request, r#"{"ok":true}"#),
        Err(e) => error_response(request, 500, &format!("Delete failed: {e}")),
    }
}

fn build_frontmatter_from_json(data: &serde_json::Value) -> Frontmatter {
    let parse_date = |v: &serde_json::Value| -> Option<chrono::NaiveDate> {
        v.as_str()
            .filter(|s| !s.is_empty())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
    };

    let tags: Vec<String> = data["tags"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Frontmatter {
        title: data["title"].as_str().unwrap_or("").to_string(),
        date: parse_date(&data["date"]),
        updated: parse_date(&data["updated"]),
        description: data["description"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(String::from),
        image: data["image"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(String::from),
        slug: data["slug"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(String::from),
        tags,
        draft: data["draft"].as_bool().unwrap_or(false),
        template: data["template"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(String::from),
        weight: data["weight"].as_i64().map(|n| n as i32),
        ..Default::default()
    }
}
