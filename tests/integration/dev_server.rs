//! `seite serve` runtime endpoints (`/__seite/*`), `seite preview`, and the
//! dev-server MCP tools, against a real server process on a free port.

use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

use crate::common::*;

fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A `seite serve --no-repl` child, killed on drop.
struct Served {
    child: Child,
    port: u16,
    _tmp: TempDir,
    dir: PathBuf,
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Served {
    fn start() -> Served {
        let tmp = TempDir::new().unwrap();
        init_site(&tmp, "site", "Dev Server", "posts,pages");
        let dir = tmp.path().join("site");
        Self::start_in(tmp, dir)
    }

    fn start_in(tmp: TempDir, dir: PathBuf) -> Served {
        for _ in 0..5 {
            let port = free_port();
            let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("seite"));
            for var in AGENT_SESSION_VARS {
                cmd.env_remove(var);
            }
            let mut child = cmd
                .args(["serve", "--no-repl", "--host", "127.0.0.1", "--port"])
                .arg(port.to_string())
                .current_dir(&dir)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(60) && child.try_wait().unwrap().is_none() {
                let host = format!("127.0.0.1:{port}");
                if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
                    && http(port, "GET", "/__seite/status", &[("Host", &host)], "").0 == 200
                {
                    return Served {
                        child,
                        port,
                        _tmp: tmp,
                        dir,
                    };
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            // Port raced away or the server died: retry on a fresh port.
            let _ = child.kill();
            let _ = child.wait();
        }
        panic!("dev server never came up");
    }

    fn host(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    fn get(&self, path: &str) -> (u16, String) {
        http(self.port, "GET", path, &[("Host", &self.host())], "")
    }

    fn status(&self) -> serde_json::Value {
        let (code, body) = self.get("/__seite/status");
        assert_eq!(code, 200, "{body}");
        serde_json::from_str(&body).unwrap()
    }

    fn post(&self, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, String) {
        http(self.port, "POST", path, headers, body)
    }

    fn post_json(&self, path: &str, body: &str) -> (u16, String) {
        let host = self.host();
        self.post(
            path,
            &[("Host", &host), ("Content-Type", "application/json")],
            body,
        )
    }

    /// Wait until `/__seite/status` satisfies `pred`.
    fn wait_status(&self, pred: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
        let start = Instant::now();
        loop {
            let s = self.status();
            if pred(&s) {
                return s;
            }
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "status never matched: {s:#}"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn post_file(&self) -> PathBuf {
        let posts = self.dir.join("content/posts");
        fs::read_dir(&posts)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "md"))
            .expect("init writes a starter post")
    }
}

/// One raw HTTP/1.1 request (full control over Host/Origin), `Connection: close`.
fn http(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (u16, String) {
    let (code, _head, body) = http_full(port, method, path, headers, body);
    (code, body)
}

/// [`http`], also returning the response head (status line and headers).
fn http_full(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (u16, String, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    stream.write_all(req.as_bytes()).unwrap();
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).to_string();
    let code = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    assert!(
        !head.to_ascii_lowercase().contains("access-control-allow"),
        "no CORS headers ever: {head}"
    );
    (code, head.to_string(), body.to_string())
}

/// An open `/__seite/events` stream.
struct Events {
    reader: BufReader<TcpStream>,
}

impl Events {
    fn open(port: u16) -> Events {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(40)))
            .unwrap();
        stream
            .write_all(
                format!("GET /__seite/events HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n")
                    .as_bytes(),
            )
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut status = String::new();
        reader.read_line(&mut status).unwrap();
        assert!(status.contains("200"), "{status}");
        let mut saw_type = false;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line
                .to_ascii_lowercase()
                .starts_with("content-type: text/event-stream")
            {
                saw_type = true;
            }
            if line == "\r\n" {
                break;
            }
        }
        assert!(saw_type);
        Events { reader }
    }

    /// Read until an event named `name` arrives; return its data.
    fn wait_for(&mut self, name: &str) -> String {
        let start = Instant::now();
        let mut current = String::new();
        loop {
            assert!(
                start.elapsed() < Duration::from_secs(40),
                "no `{name}` event"
            );
            let mut line = String::new();
            if self.reader.read_line(&mut line).unwrap_or(0) == 0 {
                panic!("event stream closed before `{name}`");
            }
            let line = line.trim_end();
            if let Some(e) = line.strip_prefix("event: ") {
                current = e.to_string();
            } else if let Some(data) = line.strip_prefix("data: ") {
                if current == name {
                    return data.to_string();
                }
            }
        }
    }
}

fn append(path: &Path, text: &str) {
    let mut s = fs::read_to_string(path).unwrap();
    s.push_str(text);
    fs::write(path, s).unwrap();
}

#[test]
fn test_serve_markdown_content_negotiation() {
    let s = Served::start();
    let host = s.host();
    // The starter post's published URL, from its markdown copy in dist/.
    let md_copy = fs::read_dir(s.dir.join("dist/posts"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "md") && !p.ends_with("index.md"))
        .expect("starter post markdown copy");
    let page = format!("/posts/{}", md_copy.file_stem().unwrap().to_string_lossy());
    let get = |path: &str, accept: Option<&str>| {
        let mut headers = vec![("Host", host.as_str())];
        if let Some(a) = accept {
            headers.push(("Accept", a));
        }
        let (code, head, body) = http_full(s.port, "GET", path, &headers, "");
        (code, head.to_ascii_lowercase(), body)
    };

    // An agent asking for markdown gets the page's .md copy.
    let (code, head, body) = get(&page, Some("text/markdown, text/html;q=0.9"));
    assert_eq!(code, 200, "{head}");
    assert!(
        head.contains("content-type: text/markdown; charset=utf-8"),
        "{head}"
    );
    assert!(head.contains("vary: accept"), "{head}");
    assert!(body.starts_with("---\n"), "{body}");
    assert!(body.contains("Index of this site for agents"), "{body}");
    assert!(!body.contains("<html"), "{body}");

    // A browser gets HTML (with live reload), still marked `Vary: Accept`.
    let (code, head, body) = get(&page, Some("text/html,application/xhtml+xml,*/*;q=0.8"));
    assert_eq!(code, 200, "{head}");
    assert!(head.contains("content-type: text/html"), "{head}");
    assert!(head.contains("vary: accept"), "{head}");
    assert!(body.contains("<html"), "{body}");
    assert!(body.contains("/__seite/events"), "live reload injected");

    // `*/*` alone, or no Accept header → HTML.
    let (_, head, _) = get(&page, Some("*/*"));
    assert!(head.contains("content-type: text/html"), "{head}");
    let (_, head, _) = get(&page, None);
    assert!(head.contains("content-type: text/html"), "{head}");

    // A collection index negotiates to its index.md.
    let (_, head, body) = get("/posts/", Some("text/markdown"));
    assert!(head.contains("content-type: text/markdown"), "{head}");
    assert!(body.contains("# Posts"), "{body}");

    // Explicit .md URLs are served as text/markdown.
    let (code, head, _) = get(&format!("{page}.md"), None);
    assert_eq!(code, 200);
    assert!(
        head.contains("content-type: text/markdown; charset=utf-8"),
        "{head}"
    );

    // Non-HTML files are never negotiated and carry no Vary.
    let (_, head, _) = get("/robots.txt", Some("text/markdown"));
    assert!(head.contains("content-type: text/plain"), "{head}");
    assert!(!head.contains("vary"), "{head}");
}

/// Nothing under `.seite/` (or elsewhere in the project) is ever reachable by
/// URL: the build never writes `.seite` into `dist/`, and the dev server
/// refuses every way of walking out of the output dir.
#[test]
fn test_serve_never_exposes_seite_dir() {
    let s = Served::start();
    const CANARY: &str = "seite-leak-canary-7f3a";
    fs::write(s.dir.join(".seite/canary.txt"), CANARY).unwrap();
    fs::write(s.dir.join("canary.txt"), CANARY).unwrap();

    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&s.dir.join("dist"), &mut files);
    assert!(!files.is_empty());
    for file in &files {
        let rel = file.strip_prefix(s.dir.join("dist")).unwrap();
        assert!(
            !rel.to_string_lossy().contains(".seite"),
            "{} in dist/",
            rel.display()
        );
        if let Ok(text) = fs::read_to_string(file) {
            assert!(
                !text.contains(".seite/"),
                "{} mentions .seite/",
                rel.display()
            );
        }
    }

    for path in [
        "/.seite/canary.txt",
        "/.seite/config.json",
        "/.seite/dev.json",
        "/.seite/docs/errors.md",
        "/../.seite/canary.txt",
        "/../canary.txt",
        "/%2e%2e/.seite/canary.txt",
        "/%2E%2E%2F.seite%2Fcanary.txt",
        "/..%2f.seite/canary.txt",
        "/..%5c.seite%5ccanary.txt",
        "/static/../../.seite/canary.txt",
        "/posts/../../.seite/canary.txt",
        "/og/../../canary.txt",
        "/%2e%2e%2f%2e%2e%2fcanary.txt",
        "//.seite/canary.txt",
        "/seite.toml",
        "/.git/config",
    ] {
        let (code, body) = s.get(path);
        assert_eq!(code, 404, "{path} must not be served");
        assert!(!body.contains(CANARY), "{path} leaked the canary");
        assert!(
            !body.contains("\"initialized_at\""),
            "{path} leaked .seite/config.json"
        );
    }
}

#[test]
fn test_serve_status_reports_server_and_build() {
    let served = Served::start();
    let s = served.status();
    assert_eq!(s["port"], served.port);
    assert_eq!(s["url"], format!("http://127.0.0.1:{}", served.port));
    assert_eq!(s["pid"], served.child.id());
    assert_eq!(s["last_build"]["ok"], true, "{s:#}");
    assert_eq!(s["browser_errors"], serde_json::json!([]));
    assert_eq!(s["mcp"]["enabled"], true);

    // DNS rebinding: a foreign Host can't read it.
    let (code, _) = http(
        served.port,
        "GET",
        "/__seite/status",
        &[("Host", "evil.example:80")],
        "",
    );
    assert_eq!(code, 403);

    // Served HTML carries the dev script; the polling fallback still works.
    let (code, html) = served.get("/");
    assert_eq!(code, 200);
    assert!(html.contains("/__seite/events") && html.contains("/__livereload"));
    let (code, version) = served.get("/__livereload");
    assert_eq!(code, 200);
    assert!(version.trim().parse::<u64>().is_ok(), "{version}");
}

#[test]
fn test_serve_events_reload_and_build_error() {
    let served = Served::start();
    let mut events = Events::open(served.port);
    // Let the watcher settle before changing files.
    std::thread::sleep(Duration::from_millis(500));
    let post = served.post_file();
    append(&post, "\nMore text.\n");
    let version = events.wait_for("reload");
    assert!(version.parse::<u64>().unwrap() > 1);

    // Break the post's template: a render error fails the build.
    let tpl = served.dir.join("templates/post.html");
    fs::create_dir_all(tpl.parent().unwrap()).unwrap();
    fs::write(
        &tpl,
        "{% extends \"base.html\" %}{% block content %}{{ page.title | no_such_filter }}{% endblock %}",
    )
    .unwrap();
    let data = events.wait_for("build-error");
    let outcome: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(outcome["ok"], false, "{outcome:#}");
    let diag = &outcome["diagnostics"][0];
    assert_eq!(diag["severity"], "error", "{outcome:#}");
    assert!(
        diag["code"].is_string() && diag["message"].is_string(),
        "{outcome:#}"
    );
    let s = served.wait_status(|s| s["last_build"]["ok"] == false);
    assert!(s["last_build"]["errors"].as_u64().unwrap() >= 1);

    // `seite preview --json` reports it.
    // ... and exits 1, saying why.
    let (ok, doc) = run_json(&served.dir, &["preview"]);
    assert!(!ok, "{doc:#}");
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("last build failed"));
    assert_eq!(doc["data"]["running"], true);
    assert_eq!(doc["data"]["last_build"]["ok"], false);
    let human = page_cmd()
        .arg("preview")
        .current_dir(&served.dir)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&human.stdout);
    assert!(out.contains("FAILED"), "{out}");

    // `seite context` says so too.
    let (_, ctx) = run_json(&served.dir, &["context"]);
    let names: Vec<&str> = ctx["data"]["directives"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["name"].as_str())
        .collect();
    assert!(names.contains(&"DEV_BUILD_FAILED"), "{names:?}");

    // Fix it: build-ok, and the status recovers.
    fs::remove_file(&tpl).unwrap();
    events.wait_for("build-ok");
    served.wait_status(|s| s["last_build"]["ok"] == true);
}

#[test]
fn test_serve_console_forwarding() {
    let served = Served::start();
    let host = served.host();
    let body = r#"{"kind":"error","message":"\u001b[31mboom\u001b[0m happened","source":"/app.js","line":3,"col":9,"page":"/"}"#;
    let (code, _) = served.post_json("/__seite/console", body);
    assert_eq!(code, 204);
    let s = served.wait_status(|s| !s["browser_errors"].as_array().unwrap().is_empty());
    assert_eq!(s["browser_errors"][0]["message"], "boom happened");
    assert_eq!(s["browser_errors"][0]["line"], 3);

    // Cross-origin, wrong type, oversized: rejected.
    let origin = "https://evil.example".to_string();
    let (code, _) = served.post(
        "/__seite/console",
        &[
            ("Host", &host),
            ("Origin", &origin),
            ("Content-Type", "application/json"),
        ],
        body,
    );
    assert_eq!(code, 403);
    let (code, _) = served.post(
        "/__seite/console",
        &[("Host", &host), ("Content-Type", "text/plain")],
        body,
    );
    assert_eq!(code, 415);
    let big = format!(r#"{{"message":"{}"}}"#, "a".repeat(10_000));
    let (code, _) = served.post_json("/__seite/console", &big);
    assert_eq!(code, 413);

    // `seite preview` prints it, then clears it.
    let out = page_cmd()
        .args(["preview", "--clear-browser-errors"])
        .current_dir(&served.dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("boom happened"), "{text}");
    assert_eq!(served.status()["browser_errors"], serde_json::json!([]));
}

#[test]
fn test_serve_mcp_endpoint_guards() {
    let served = Served::start();
    let host = served.host();
    let list = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let (code, body) = served.post_json("/__seite/mcp", list);
    assert_eq!(code, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> = v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    for tool in [
        "seite_get_errors",
        "seite_get_preview_url",
        "seite_list_routes",
        "seite_reload",
    ] {
        assert!(names.contains(&tool), "{tool}");
    }

    let same_origin = format!("http://{host}");
    let rejected = [
        // Page scripts send Origin even when same-origin: never allowed.
        (
            vec![
                ("Host", host.as_str()),
                ("Origin", same_origin.as_str()),
                ("Content-Type", "application/json"),
            ],
            403,
        ),
        (
            vec![
                ("Host", host.as_str()),
                ("Origin", "https://evil.example"),
                ("Content-Type", "application/json"),
            ],
            403,
        ),
        (
            vec![
                ("Host", "evil.example:3000"),
                ("Content-Type", "application/json"),
            ],
            403,
        ),
        (
            vec![("Host", host.as_str()), ("Content-Type", "text/plain")],
            415,
        ),
    ];
    for (headers, want) in rejected {
        let (code, _) = served.post("/__seite/mcp", &headers, list);
        assert_eq!(code, want, "{headers:?}");
    }
    assert_eq!(served.get("/__seite/mcp").0, 405);

    // Notifications: 202, no body.
    let (code, body) = served.post_json(
        "/__seite/mcp",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );
    assert_eq!(code, 202);
    assert!(body.is_empty());

    // Runtime tools read the live server.
    let call = |name: &str, args: &str| -> serde_json::Value {
        let (code, body) = served.post_json(
            "/__seite/mcp",
            &format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#
            ),
        );
        assert_eq!(code, 200, "{body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_ne!(v["result"]["isError"], true, "{v:#}");
        serde_json::from_str(v["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    };
    let errors = call("seite_get_errors", "{}");
    assert_eq!(errors["url"], format!("http://127.0.0.1:{}", served.port));
    assert_eq!(errors["last_build"]["ok"], true);
    let routes = call("seite_list_routes", "{}");
    assert!(routes["routes"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("/")));
    let preview = call("seite_get_preview_url", r#"{"url":"/posts/"}"#);
    assert_eq!(
        preview["preview_url"],
        format!("http://127.0.0.1:{}/posts/", served.port)
    );
    let reload = call("seite_reload", "{}");
    assert_eq!(reload["ok"], true, "{reload:#}");
    // seite_build through the dev server keeps the dev base URL in dist/.
    let built = call("seite_build", "{}");
    assert_eq!(built["success"], true, "{built:#}");
    let index = fs::read_to_string(served.dir.join("dist/index.html")).unwrap();
    assert!(
        index.contains(&format!("http://127.0.0.1:{}", served.port)),
        "dist/ was rebuilt with the production base_url"
    );

    // stdio `seite mcp` proxies the runtime tools to the running server.
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"seite_get_errors","arguments":{}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"seite_reload","arguments":{}}}"#,
        "\n"
    );
    let out = page_cmd()
        .arg("mcp")
        .current_dir(&served.dir)
        .write_stdin(input)
        .timeout(Duration::from_secs(120))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<serde_json::Value> = stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    for line in &lines {
        assert_ne!(line["result"]["isError"], true, "{line:#}");
    }
    assert!(lines[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains(&format!("127.0.0.1:{}", served.port)));
}

#[test]
fn test_preview_json_without_server() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Server", "posts");
    let (ok, doc) = run_json(&tmp.path().join("site"), &["preview"]);
    assert!(!ok);
    assert_eq!(doc["data"]["running"], false);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("seite serve"));
    // stdio runtime tools say the same.
    let out = page_cmd()
        .arg("mcp")
        .current_dir(tmp.path().join("site"))
        .write_stdin(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"seite_get_errors","arguments":{}}}"#
                .to_string()
                + "\n",
        )
        .output()
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert_eq!(v["result"]["isError"], true);
    assert!(v["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("no dev server running"));
}
