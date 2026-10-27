//! Published-site features for readers' agents: the `llms.txt` pointer in
//! every markdown copy, `sitemap.md`, and `Accept: text/markdown` content
//! negotiation in the generated Cloudflare Pages Workers (the dev server's
//! negotiation is tested in `dev_server.rs`).

use std::path::{Path, PathBuf};

use crate::common::*;

const POINTER: &str = "> Index of this site for agents: [llms.txt](";

fn build(site_dir: &Path) {
    page_cmd()
        .arg("build")
        .current_dir(site_dir)
        .assert()
        .success();
}

/// Set `[deploy] target` and optionally opt in to markdown negotiation.
fn set_deploy(site_dir: &Path, target: &str, negotiation: bool) {
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let mut line = format!("target = \"{target}\"");
    if negotiation {
        line.push_str("\nmarkdown_negotiation = true");
    }
    let replaced = config.replacen("target = \"github-pages\"", &line, 1);
    assert_ne!(
        config, replaced,
        "no [deploy] target in seite.toml:\n{config}"
    );
    fs::write(&toml_path, replaced).unwrap();
}

fn md_files(dir: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect()
}

#[test]
fn test_build_every_markdown_copy_has_llms_pointer_after_frontmatter() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Pointer Site", "posts,docs,pages");
    let site_dir = tmp.path().join("site");
    fs::write(
        site_dir.join("content/pages/index.md"),
        "---\ntitle: Home\n---\n\nWelcome home.\n",
    )
    .unwrap();
    build(&site_dir);
    let dist = site_dir.join("dist");

    let files: Vec<PathBuf> = md_files(&dist)
        .into_iter()
        .filter(|p| !p.ends_with("sitemap.md"))
        .collect();
    assert!(files
        .iter()
        .any(|p| p.ends_with("index.md") && p.parent() == Some(&dist)));
    assert!(files.iter().any(|p| p.ends_with("posts/index.md")));
    assert!(files.len() >= 4, "{files:?}");
    for file in &files {
        let md = fs::read_to_string(file).unwrap();
        let after_frontmatter = match md.strip_prefix("---\n") {
            Some(rest) => {
                let end = rest.find("\n---\n").expect("closed frontmatter");
                &rest[end + 5..]
            }
            None => md.as_str(),
        };
        let after_frontmatter = after_frontmatter.trim_start_matches('\n');
        assert!(
            after_frontmatter.starts_with(POINTER),
            "{} has no pointer after its frontmatter:\n{md}",
            file.display()
        );
        assert_eq!(md.matches(POINTER).count(), 1, "{}", file.display());
        assert!(md.contains("(http://localhost:3000/llms.txt)"), "{md}");
        assert!(md.contains("(http://localhost:3000/sitemap.md)"), "{md}");
    }

    // llms-full.txt carries page bodies, never the pointer.
    let full = fs::read_to_string(dist.join("llms-full.txt")).unwrap();
    assert!(!full.contains("Index of this site for agents"));
    // llms.txt links sitemap.md.
    let llms = fs::read_to_string(dist.join("llms.txt")).unwrap();
    assert!(llms.contains("[sitemap.md](http://localhost:3000/sitemap.md)"));
}

#[test]
fn test_build_pointer_links_language_llms_txt() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Lang Site", "posts,pages");
    let site_dir = tmp.path().join("site");
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config.push_str("\n[languages.es]\ntitle = \"Sitio\"\n");
    fs::write(&toml_path, config).unwrap();
    fs::write(
        site_dir.join("content/pages/about.es.md"),
        "---\ntitle: Acerca\ndescription: Quiénes somos\n---\n\nHola.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/pages/about.md"),
        "---\ntitle: About\n---\n\nHello.\n",
    )
    .unwrap();
    build(&site_dir);
    let dist = site_dir.join("dist");

    let es = fs::read_to_string(dist.join("es/about.md")).unwrap();
    assert!(es.contains("(http://localhost:3000/es/llms.txt)"), "{es}");
    assert!(es.contains("(http://localhost:3000/sitemap.md)"), "{es}");
    let en = fs::read_to_string(dist.join("about.md")).unwrap();
    assert!(en.contains("(http://localhost:3000/llms.txt)"), "{en}");

    let sitemap = fs::read_to_string(dist.join("sitemap.md")).unwrap();
    assert!(sitemap.contains("## Pages (en)"), "{sitemap}");
    assert!(sitemap.contains("## Pages (es)"), "{sitemap}");
    assert!(sitemap.contains("- [Acerca](http://localhost:3000/es/about.md) — Quiénes somos"));
    assert!(sitemap.find("## Pages (en)") < sitemap.find("## Pages (es)"));
}

#[test]
fn test_build_writes_sitemap_md_without_drafts_or_noindex() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Map Site", "posts,docs,pages");
    let site_dir = tmp.path().join("site");
    fs::write(
        site_dir.join("content/docs/visible.md"),
        "---\ntitle: Visible Doc\ndescription: Shown\n---\n\nText.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/hidden.md"),
        "---\ntitle: Hidden Doc\nrobots: noindex\n---\n\nText.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/posts/2024-01-01-draft.md"),
        "---\ntitle: Draft Post\ndraft: true\n---\n\nText.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/pages/about.md"),
        "---\ntitle: About Us\n---\n\nText.\n",
    )
    .unwrap();
    build(&site_dir);

    let sitemap = fs::read_to_string(site_dir.join("dist/sitemap.md")).unwrap();
    assert!(sitemap.starts_with("# Map Site: sitemap\n"), "{sitemap}");
    assert!(sitemap.contains("- [Visible Doc](http://localhost:3000/docs/visible.md) — Shown"));
    // Unlisted but public collections are in, like sitemap.xml.
    assert!(sitemap.contains("- [About Us](http://localhost:3000/about.md)"));
    assert!(!sitemap.contains("Hidden Doc"), "{sitemap}");
    assert!(!sitemap.contains("Draft Post"), "{sitemap}");
    // Collections in seite.toml order.
    let posts = sitemap.find("## Posts").unwrap();
    let docs = sitemap.find("[Visible Doc]").unwrap();
    let pages = sitemap.find("## Pages").unwrap();
    assert!(posts < docs && docs < pages, "{sitemap}");
    // Every listed link is a markdown copy that exists.
    for line in sitemap.lines().filter(|l| l.starts_with("- [")) {
        let url = line.split("](").nth(1).unwrap().split(')').next().unwrap();
        let rel = url.trim_start_matches("http://localhost:3000/");
        assert!(site_dir.join("dist").join(rel).is_file(), "{url}");
    }

    // Drafts stay out even with --drafts.
    page_cmd()
        .args(["build", "--drafts"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let sitemap = fs::read_to_string(site_dir.join("dist/sitemap.md")).unwrap();
    assert!(!sitemap.contains("Draft Post"), "{sitemap}");
}

#[test]
fn test_build_cloudflare_markdown_worker_only_when_opted_in() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Negotiate", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Default: no Worker (every request stays a free static-asset request).
    set_deploy(&site_dir, "cloudflare", false);
    build(&site_dir);
    assert!(!site_dir.join("dist/_worker.js").exists());
    assert!(!site_dir.join("dist/_routes.json").exists());

    // Opted in on Cloudflare: the negotiation-only Worker.
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    fs::write(
        &toml_path,
        config.replacen(
            "target = \"cloudflare\"",
            "target = \"cloudflare\"\nmarkdown_negotiation = true",
            1,
        ),
    )
    .unwrap();
    build(&site_dir);
    let worker = fs::read_to_string(site_dir.join("dist/_worker.js")).unwrap();
    assert!(worker.starts_with("// Generated by seite markdown negotiation. Do not edit."));
    assert!(worker.contains("function prefersMarkdown"));
    assert!(!worker.contains("PASSWORD_BINDINGS"));
    let routes: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site_dir.join("dist/_routes.json")).unwrap())
            .unwrap();
    assert_eq!(routes["include"], serde_json::json!(["/*"]));
    assert_eq!(
        routes["exclude"],
        serde_json::json!(["/static/*", "/og/*", "/search/*"])
    );
}

#[test]
fn test_build_markdown_negotiation_ignored_off_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Pages Site", "posts,pages");
    let site_dir = tmp.path().join("site");
    set_deploy(&site_dir, "github-pages", true);
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "markdown_negotiation only applies to Cloudflare Pages",
        ));
    assert!(!site_dir.join("dist/_worker.js").exists());
}

#[test]
fn test_build_markdown_worker_refuses_custom_worker() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Custom Worker", "posts,pages");
    let site_dir = tmp.path().join("site");
    set_deploy(&site_dir, "cloudflare", true);
    fs::create_dir_all(site_dir.join("public")).unwrap();
    fs::write(site_dir.join("public/_worker.js"), "export default {};").unwrap();
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "markdown_negotiation cannot generate _worker.js",
        ));
}

#[test]
fn test_build_access_worker_negotiates_markdown_without_opt_in() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Access Negotiation", "posts,pages");
    let site_dir = tmp.path().join("site");
    add_collection_line(&site_dir, "posts", "private = true");
    add_collection_line(&site_dir, "posts", "access_group = \"members\"");
    enable_password_access(&site_dir);
    build(&site_dir);
    let worker = fs::read_to_string(site_dir.join("dist/_worker.js")).unwrap();
    assert!(worker.starts_with("// Generated by seite password access. Do not edit."));
    assert!(worker.contains("function prefersMarkdown"));
    assert!(worker.contains("return serveAsset(request, env, requestPath, sameRoute)"));
    assert!(!worker.contains("return env.ASSETS.fetch(request);\n    return"));
}

// --- the generated Workers under Node ---------------------------------------

/// Node on PATH. Locally a missing Node skips the test; in CI it fails it.
fn node() -> Option<PathBuf> {
    let found = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|d| d.join(if cfg!(windows) { "node.exe" } else { "node" }))
            .find(|p| p.is_file())
    });
    if found.is_none() && std::env::var_os("CI").is_some_and(|v| !v.is_empty()) {
        panic!("node is not on PATH in CI: the Worker tests need it");
    }
    if found.is_none() {
        eprintln!("node not on PATH: skipping the Worker test");
    }
    found
}

/// Runs the Worker's `fetch` against a mock `env.ASSETS` that serves `dist`
/// like Pages (clean URLs, `index.html`, 404s) and binds the `members`
/// password group's secrets, one request per `[path, accept, method, cookie,
/// form]` entry. Returns `[{status, type, vary, setCookie, body}]`.
const HARNESS: &str = r#"
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
// Workers-only extension of SubtleCrypto, used by the password Worker.
if (!crypto.subtle.timingSafeEqual) {
  crypto.subtle.timingSafeEqual = (a, b) => {
    const x = ArrayBuffer.isView(a) ? new Uint8Array(a.buffer, a.byteOffset, a.byteLength) : new Uint8Array(a);
    const y = ArrayBuffer.isView(b) ? new Uint8Array(b.buffer, b.byteOffset, b.byteLength) : new Uint8Array(b);
    if (x.length !== y.length) throw new TypeError("length mismatch");
    let diff = 0;
    for (let i = 0; i < x.length; i++) diff |= x[i] ^ y[i];
    return diff === 0;
  };
}
const [dist, workerPath, requestsJson] = process.argv.slice(2);
const worker = (await import(pathToFileURL(workerPath).href)).default;
const TYPES = { ".html": "text/html; charset=utf-8", ".md": "text/plain; charset=utf-8", ".txt": "text/plain; charset=utf-8" };
const env = {
  SEITE_PASSWORD_MEMBERS: "open sesame",
  SEITE_SESSION_SECRET_MEMBERS: "test-session-secret",
  ASSETS: {
    async fetch(request) {
      const p = decodeURIComponent(new URL(request.url).pathname);
      const candidates = p.endsWith("/") ? [p + "index.html"] : [p, p + ".html"];
      for (const c of candidates) {
        const file = path.join(dist, c);
        if (fs.existsSync(file) && fs.statSync(file).isFile()) {
          const body = request.method === "HEAD" ? null : fs.readFileSync(file);
          return new Response(body, { status: 200, headers: { "Content-Type": TYPES[path.extname(file)] || "application/octet-stream" } });
        }
      }
      return new Response("not found", { status: 404, headers: { "Content-Type": "text/html; charset=utf-8" } });
    },
  },
};
const out = [];
for (const [p, accept, method, cookie, form] of JSON.parse(requestsJson)) {
  const headers = accept ? { Accept: accept } : {};
  if (cookie) headers.Cookie = cookie;
  const init = { method: method || "GET", headers };
  if (form) {
    headers["Content-Type"] = "application/x-www-form-urlencoded";
    init.body = new URLSearchParams(form).toString();
  }
  const response = await worker.fetch(new Request("https://example.com" + p, init), env);
  out.push({ status: response.status, type: response.headers.get("Content-Type"), vary: response.headers.get("Vary"), setCookie: response.headers.get("Set-Cookie"), body: (await response.text()).slice(0, 400) });
}
console.log(JSON.stringify(out));
"#;

fn run_worker(node: &Path, dist: &Path, requests: serde_json::Value) -> Vec<serde_json::Value> {
    let dir = TempDir::new().unwrap();
    let worker = dir.path().join("worker.mjs");
    fs::copy(dist.join("_worker.js"), &worker).unwrap();
    let harness = dir.path().join("harness.mjs");
    fs::write(&harness, HARNESS).unwrap();
    let out = std::process::Command::new(node)
        .arg(&harness)
        .arg(dist)
        .arg(&worker)
        .arg(requests.to_string())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn first_post_path(dist: &Path) -> String {
    let md = fs::read_dir(dist.join("posts"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "md") && !p.ends_with("index.md"))
        .expect("starter post");
    format!("/posts/{}", md.file_stem().unwrap().to_string_lossy())
}

#[test]
fn test_build_markdown_worker_negotiates_under_node() {
    let Some(node) = node() else { return };
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Node Worker", "posts,pages");
    let site_dir = tmp.path().join("site");
    set_deploy(&site_dir, "cloudflare", true);
    build(&site_dir);
    let dist = site_dir.join("dist");
    let post = first_post_path(&dist);

    let r = run_worker(
        &node,
        &dist,
        serde_json::json!([
            [post, "text/markdown, text/html;q=0.9"],
            [post, "text/html,application/xhtml+xml,*/*;q=0.8"],
            [post, "*/*"],
            ["/posts/", "text/markdown"],
            ["/does-not-exist", "text/markdown"],
            ["/robots.txt", "text/markdown"],
            [post, "text/markdown", "POST"],
            [post, "text/markdown", "HEAD"],
        ]),
    );
    // Markdown for an agent.
    assert_eq!(r[0]["status"], 200);
    assert_eq!(r[0]["type"], "text/markdown; charset=utf-8");
    assert_eq!(r[0]["vary"], "Accept");
    assert!(r[0]["body"].as_str().unwrap().starts_with("---\n"));
    // HTML for a browser and for `*/*`, marked Vary: Accept.
    for i in [1, 2] {
        assert!(
            r[i]["type"].as_str().unwrap().starts_with("text/html"),
            "{}",
            r[i]
        );
        assert_eq!(r[i]["vary"], "Accept");
        assert!(r[i]["body"].as_str().unwrap().contains("<html"));
    }
    // Collection index → index.md.
    assert_eq!(r[3]["type"], "text/markdown; charset=utf-8");
    assert!(r[3]["body"].as_str().unwrap().contains("# Posts"));
    // A page without a markdown copy falls through to the normal response.
    assert_eq!(r[4]["status"], 404);
    // Files with an extension are never negotiated.
    assert_eq!(r[5]["type"], "text/plain; charset=utf-8");
    assert!(r[5]["vary"].is_null());
    // Only GET/HEAD negotiate.
    assert!(r[6]["type"].as_str().unwrap().starts_with("text/html"));
    assert_eq!(r[7]["type"], "text/markdown; charset=utf-8");
}

#[test]
fn test_build_access_worker_negotiates_only_where_authorized() {
    let Some(node) = node() else { return };
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Node Access", "posts,pages");
    let site_dir = tmp.path().join("site");
    add_collection_line(&site_dir, "posts", "private = true");
    add_collection_line(&site_dir, "posts", "access_group = \"members\"");
    enable_password_access(&site_dir);
    fs::write(
        site_dir.join("content/pages/about.md"),
        "---\ntitle: About\n---\n\nPublic page.\n",
    )
    .unwrap();
    build(&site_dir);
    let dist = site_dir.join("dist");
    let post = first_post_path(&dist);

    let r = run_worker(
        &node,
        &dist,
        serde_json::json!([
            ["/about", "text/markdown"],
            ["/about", "text/html"],
            [post, "text/markdown"],
            [format!("{post}.md"), "text/markdown"],
        ]),
    );
    // Public page: negotiated.
    assert_eq!(r[0]["type"], "text/markdown; charset=utf-8");
    assert_eq!(r[0]["vary"], "Accept");
    assert!(r[0]["body"].as_str().unwrap().contains("Public page."));
    assert_eq!(r[1]["vary"], "Accept");
    // Protected page without a session: the login page, never the markdown.
    for i in [2, 3] {
        assert_eq!(r[i]["status"], 200, "{}", r[i]);
        assert_ne!(r[i]["type"], "text/markdown; charset=utf-8", "{}", r[i]);
        let body = r[i]["body"].as_str().unwrap();
        assert!(body.contains("Private page"), "{body}");
        assert!(!body.contains("---\ntitle"), "{body}");
    }

    // Log in, then the same request negotiates to the markdown copy.
    let login = run_worker(
        &node,
        &dist,
        serde_json::json!([[
            "/_seite/access/login",
            null,
            "POST",
            null,
            {"group": "members", "password": "open sesame", "next": "/"}
        ]]),
    );
    assert_eq!(login[0]["status"], 303, "{}", login[0]);
    let cookie = login[0]["setCookie"]
        .as_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let r = run_worker(
        &node,
        &dist,
        serde_json::json!([
            [post, "text/markdown", "GET", cookie],
            [post, "text/html", "GET", cookie],
        ]),
    );
    assert_eq!(r[0]["status"], 200, "{}", r[0]);
    assert_eq!(r[0]["type"], "text/markdown; charset=utf-8");
    assert_eq!(r[0]["vary"], "Accept");
    assert!(r[0]["body"].as_str().unwrap().starts_with("---\n"));
    assert!(r[1]["type"].as_str().unwrap().starts_with("text/html"));
    assert_eq!(r[1]["vary"], "Accept");
}
