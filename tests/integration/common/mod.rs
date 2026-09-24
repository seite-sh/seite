pub(crate) use std::fs;
pub(crate) use std::io::Write;
pub(crate) use std::process::Stdio;

pub(crate) use assert_cmd::Command;
pub(crate) use predicates::prelude::*;
pub(crate) use tempfile::TempDir;

pub(crate) fn page_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("seite")
}

/// Helper to init a site with given collections
pub(crate) fn init_site(tmp: &TempDir, name: &str, title: &str, collections: &str) {
    page_cmd()
        .args([
            "init",
            name,
            "--title",
            title,
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            collections,
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
}

// --- multi-language (i18n) ---

/// Helper: add `[languages.es]` to a site's seite.toml
pub(crate) fn add_language(site_dir: &std::path::Path, lang: &str, title: &str) {
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config.push_str(&format!("\n[languages.{lang}]\ntitle = \"{title}\"\n"));
    fs::write(&toml_path, config).unwrap();
}

/// Helper: write the three bundled Trust Center data files with per-language
/// "language map" values for prose fields, plus a plain-string field.
pub(crate) fn write_bilingual_trust_data(site_dir: &std::path::Path) {
    let trust_dir = site_dir.join("data/trust");
    fs::create_dir_all(&trust_dir).unwrap();

    fs::write(
        trust_dir.join("certifications.yaml"),
        "- name: ISO 27001\n\
         \x20 framework: iso27001\n\
         \x20 status: active\n\
         \x20 slug: iso27001\n\
         \x20 description:\n\
         \x20   en: Information security management.\n\
         \x20   de: Informationssicherheitsmanagement.\n\
         \x20 scope:\n\
         \x20   en: All production systems.\n\
         \x20   de: Alle Produktionssysteme.\n",
    )
    .unwrap();

    fs::write(
        trust_dir.join("subprocessors.yaml"),
        "- name: AWS\n\
         \x20 dpa: true\n\
         \x20 purpose:\n\
         \x20   en: Cloud infrastructure\n\
         \x20   de: Cloud-Infrastruktur\n\
         \x20 location:\n\
         \x20   en: United States\n\
         \x20   de: Vereinigte Staaten\n",
    )
    .unwrap();

    fs::write(
        trust_dir.join("faq.yaml"),
        "- question:\n\
         \x20   en: Where is data stored?\n\
         \x20   de: Wo werden Daten gespeichert?\n\
         \x20 answer:\n\
         \x20   en: Data is stored in the EU.\n\
         \x20   de: Daten werden in der EU gespeichert.\n",
    )
    .unwrap();
}

// --- pagination ---

/// Helper: add `paginate = N` to the [[collections]] entry for `collection_name` in seite.toml.
pub(crate) fn add_pagination(site_dir: &std::path::Path, collection_name: &str, page_size: usize) {
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    // Append paginate to the matching collection block by rewriting the file.
    // Simple approach: find the collection section and append paginate after its last field.
    let updated = config.replace(
        &format!("name = \"{collection_name}\""),
        &format!("name = \"{collection_name}\"\npaginate = {page_size}"),
    );
    fs::write(&toml_path, updated).unwrap();
}

/// Helper: create a dated post file in a site's posts directory.
pub(crate) fn create_post(site_dir: &std::path::Path, date: &str, slug: &str, title: &str) {
    let content = format!("---\ntitle: \"{title}\"\ndate: {date}\n---\n\nContent of {title}.\n");
    fs::write(
        site_dir.join(format!("content/posts/{date}-{slug}.md")),
        content,
    )
    .unwrap();
}

// --- asset pipeline ---

/// Helper: write a CSS file into a site's static directory.
pub(crate) fn write_static_css(site_dir: &std::path::Path, name: &str, content: &str) {
    fs::create_dir_all(site_dir.join("static")).unwrap();
    fs::write(site_dir.join("static").join(name), content).unwrap();
}

/// Helper: append lines to seite.toml [build] section.
pub(crate) fn set_build_option(site_dir: &std::path::Path, key: &str, value: &str) {
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    // If the key already exists, replace its value in-place; otherwise insert after [build].
    let existing_pattern = format!("{key} = ");
    let updated = if config.contains(&existing_pattern) {
        config
            .lines()
            .map(|line| {
                if line.trim_start().starts_with(&existing_pattern) {
                    format!("{key} = {value}")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    } else {
        config.replace("[build]", &format!("[build]\n{key} = {value}"))
    };
    fs::write(&toml_path, updated).unwrap();
}

// --- image handling ---

/// Helper: write a minimal valid PNG (1x1 pixel) into a site's static directory.
pub(crate) fn write_test_image(site_dir: &std::path::Path, name: &str) {
    let img_dir = site_dir.join("static/images");
    fs::create_dir_all(&img_dir).unwrap();
    // Create a 100x100 red PNG using the image crate
    let img = image::RgbImage::from_fn(100, 100, |_, _| image::Rgb([255u8, 0, 0]));
    img.save(img_dir.join(name)).unwrap();
}

/// Helper: set [images] section in seite.toml (replaces existing if present).
pub(crate) fn set_images_config(site_dir: &std::path::Path, widths: &str) {
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    // Remove existing [images] section if present
    if let Some(pos) = config.find("\n[images]") {
        config.truncate(pos);
    }
    config.push_str(&format!(
        "\n[images]\nwidths = {widths}\nquality = 80\nlazy_loading = true\nwebp = true\n"
    ));
    fs::write(&toml_path, config).unwrap();
}

// ── coding-agent harness files (--agents) ───────────────────────────

pub(crate) fn read_json_file(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

pub(crate) fn stored_agents(site_dir: &std::path::Path) -> serde_json::Value {
    read_json_file(&site_dir.join(".seite/config.json"))["agents"].clone()
}

pub(crate) fn init_site_with_agents(tmp: &TempDir, name: &str, agents: &str) {
    page_cmd()
        .args([
            "init",
            name,
            "--title",
            "Agents",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages",
            "--agents",
            agents,
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
}

/// Every harness file of a site, relative path → content (for idempotency checks).
pub(crate) fn harness_snapshot(site_dir: &std::path::Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for top in [
        ".claude",
        ".cursor",
        ".codex",
        ".agents",
        ".mcp.json",
        "opencode.json",
        "AGENTS.md",
        "CLAUDE.md",
    ] {
        let path = site_dir.join(top);
        if path.is_file() {
            files.push((top.to_string(), fs::read_to_string(&path).unwrap()));
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(&path) {
                let entry = entry.unwrap();
                if entry.file_type().is_file() {
                    let rel = entry.path().strip_prefix(site_dir).unwrap();
                    files.push((
                        rel.display().to_string(),
                        fs::read_to_string(entry.path()).unwrap(),
                    ));
                }
            }
        }
    }
    files.sort();
    files
}

// ── MCP server ────────────────────────────────────────────────────────

/// Helper: send a single JSON-RPC message to `seite mcp` and return the response.
pub(crate) fn mcp_request(
    dir: &std::path::Path,
    messages: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let bin = assert_cmd::cargo::cargo_bin!("seite");
    let mut child = std::process::Command::new(bin)
        .arg("mcp")
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn page mcp");

    // Write all messages to stdin, then close it
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        for msg in messages {
            let line = serde_json::to_string(msg).unwrap();
            writeln!(stdin, "{}", line).unwrap();
        }
    }
    // stdin drops here, closing the pipe → server exits after processing

    let output = child.wait_with_output().expect("failed to wait on child");
    let stdout = String::from_utf8_lossy(&output.stdout);

    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("invalid JSON from MCP server"))
        .collect()
}

/// Helper: call one MCP tool and return (isError, text).
pub(crate) fn mcp_tool_call(
    dir: &std::path::Path,
    name: &str,
    arguments: serde_json::Value,
) -> (bool, String) {
    let responses = mcp_request(
        dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        })],
    );
    assert_eq!(responses.len(), 1);
    assert!(
        responses[0]["error"].is_null(),
        "tool failures must not be JSON-RPC errors: {}",
        responses[0]
    );
    let result = &responses[0]["result"];
    (
        result["isError"].as_bool().unwrap_or(false),
        result["content"][0]["text"].as_str().unwrap().to_string(),
    )
}

// --- contact form tests ---

/// Helper: add [contact] section to an existing seite.toml
pub(crate) fn add_contact_config(site_dir: &std::path::Path, provider: &str, endpoint: &str) {
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(&format!(
        "\n[contact]\nprovider = \"{provider}\"\nendpoint = \"{endpoint}\"\n"
    ));
    fs::write(&config_path, config).unwrap();
}

pub(crate) fn init_trust_site(tmp: &TempDir, name: &str) {
    page_cmd()
        .args([
            "init",
            name,
            "--title",
            "Trust Test",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,trust",
            "--trust-company",
            "TestCo",
            "--trust-frameworks",
            "soc2",
            "--trust-sections",
            "overview,certifications",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
}

/// Helper: insert a TOML line into the [[collections]] block whose `name` matches.
pub(crate) fn add_collection_line(site_dir: &std::path::Path, collection: &str, line: &str) {
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let needle = format!("name = \"{collection}\"");
    let replaced = config.replacen(&needle, &format!("{needle}\n{line}"), 1);
    assert_ne!(
        config, replaced,
        "collection '{collection}' not found in seite.toml"
    );
    fs::write(&toml_path, replaced).unwrap();
}

/// Helper: enable Cloudflare password access for private collections.
pub(crate) fn enable_password_access(site_dir: &std::path::Path) {
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config.push_str("\n[access]\nmode = \"password\"\nsession_hours = 24\n");
    fs::write(&toml_path, config).unwrap();
}

/// Helper: add plain-string subprocessor + FAQ data so the trust hub renders all
/// of its data-driven sections.
pub(crate) fn write_plain_trust_sections(site_dir: &std::path::Path) {
    let trust_dir = site_dir.join("data/trust");
    fs::create_dir_all(&trust_dir).unwrap();
    fs::write(
        trust_dir.join("subprocessors.yaml"),
        "- name: AWS\n  purpose: Cloud infrastructure\n  location: United States\n  dpa: true\n",
    )
    .unwrap();
    fs::write(
        trust_dir.join("faq.yaml"),
        "- question: Where is data stored?\n  answer: Data is stored in the EU.\n",
    )
    .unwrap();
}

// ═══════════════════════════════════════════════════════════════════════════
// Subdomain support tests
// ═══════════════════════════════════════════════════════════════════════════

/// Helper: create a site and add subdomain config to one collection.
pub(crate) fn init_subdomain_site(tmp: &TempDir, name: &str) -> std::path::PathBuf {
    init_site(tmp, name, "Subdomain Test", "posts,docs,pages");
    let site_dir = tmp.path().join(name);

    // Add subdomain = "docs" to the docs collection and set a real base_url
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "base_url = \"http://localhost:3000\"",
        "base_url = \"https://example.com\"",
    );
    // Add subdomain to the docs collection
    let config = config.replace("name = \"docs\"", "name = \"docs\"\nsubdomain = \"docs\"");
    fs::write(&toml_path, config).unwrap();
    site_dir
}

// --- agent-friendly CLI: non-interactive prompts, --json, --config, serve ---

/// Parse the single JSON document a `--json` run prints on stdout.
pub(crate) fn json_stdout(output: &std::process::Output) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "stdout is not a single JSON document ({e}):\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// Spawn `seite serve` with the given extra args and stdin closed, and check
/// it keeps serving HTTP. Retries on a few ephemeral ports (TOCTOU race).
pub(crate) fn assert_serve_stays_up(extra_args: &[&str]) {
    use std::io::Read;
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Serve Up", "posts");
    let site = tmp.path().join("site");
    let bin = assert_cmd::cargo::cargo_bin!("seite");

    for attempt in 0..5 {
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut child = std::process::Command::new(bin)
            .args(["serve", "--host", "127.0.0.1", "--port", &port.to_string()])
            .args(extra_args)
            .current_dir(&site)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(60);
        let mut response = String::new();
        while Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                break; // exited early: port raced away, or the bug is back
            }
            if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n");
                let _ = stream.read_to_string(&mut response);
                if !response.is_empty() {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }

        let exited = child.try_wait().ok().flatten();
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        if response.starts_with("HTTP/1.") {
            assert!(
                response.contains(" 200 "),
                "unexpected response: {response}"
            );
            assert!(exited.is_none(), "server must still be running");
            return;
        }
        assert!(
            attempt < 4,
            "serve never answered on 5 ports; last stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

// --- diagnostics: one-pass error reporting, `seite check` ---

/// Write `content` to `site/rel`, creating parent directories.
pub(crate) fn write_site_file(site: &std::path::Path, rel: &str, content: &str) {
    let path = site.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Snapshot of every file path + size under `dir` (to prove it is untouched).
pub(crate) fn dir_snapshot(dir: &std::path::Path) -> Vec<(String, u64)> {
    let mut entries: Vec<(String, u64)> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| {
            let len = e.metadata().map(|m| m.len()).unwrap_or(0);
            (e.path().display().to_string(), len)
        })
        .collect();
    entries.sort();
    entries
}
