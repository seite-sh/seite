//! `seite check --external`: check the external links of the rendered site.
//!
//! # Design note
//!
//! - **What is checked.** Every absolute `http(s)://` (and protocol-relative)
//!   reference in the rendered HTML — the same tag-aware extraction as the
//!   internal link check ([`super::links::extract_external_refs`]):
//!   `<a href>`, `<link href>`, `<img src|srcset>`, `<script src>`, media.
//!   Each URL is checked once (fragment dropped), no matter how many pages
//!   use it. Links to the site itself (`base_url`, subdomain base URLs) are
//!   not external. Hosts in the skip list — the built-in hosts that block
//!   bots plus `[check] external_skip` — are never requested (a host also
//!   matches its subdomains).
//! - **How.** Blocking `ureq` requests on 8 worker threads, at most 2 at a
//!   time per host, 10 s timeout (`[check] external_timeout`), up to 5
//!   redirects, user agent `seite-linkcheck/<version> (+https://seite.sh)`.
//!   `HEAD` first; servers that refuse it (405/403/501) get a `GET` (the body
//!   is never read).
//! - **Results.** 404/410, other 4xx and unknown hosts are
//!   `external-link-broken`; timeouts, connection failures, 5xx, 401/403/429
//!   (often bot blocking) and redirect loops are `external-link-unreachable`
//!   (flaky by nature: the hint says how to confirm or skip). Both are
//!   warnings, errors with `--strict`, and are attributed to the file and
//!   line the link was written in, like internal broken links.
//! - **Cache.** `.seite/cache/links.json` (written through `safe_fs`,
//!   gitignored): working links are trusted for 7 days, failures are
//!   re-checked on every run.
//! - **Never by default.** It needs the network, so it only runs with
//!   `seite check --external`, never in `seite build` or the agent hooks.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use super::links::{self, BrokenLink, LinkKind, SourceAttribution};
use crate::diagnostics::Diagnostic;

/// Hosts that block automated requests (always skipped).
pub const DEFAULT_SKIP: &[&str] = &[
    "linkedin.com",
    "twitter.com",
    "x.com",
    "instagram.com",
    "facebook.com",
];

/// Where the results are cached, relative to the site root.
pub const CACHE_PATH: &str = ".seite/cache/links.json";

const OK_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60;

/// Tunables (tests shrink the timeout).
#[derive(Debug, Clone)]
pub struct ExternalOptions {
    pub timeout: Duration,
    pub workers: usize,
    pub per_host: usize,
    pub max_redirects: u32,
    /// Hosts never requested (in addition to [`DEFAULT_SKIP`]).
    pub skip: Vec<String>,
    /// URL prefixes of the site itself (`https://example.com`).
    pub own_origins: Vec<String>,
    /// Report problems as errors instead of warnings.
    pub strict: bool,
    /// Also request hosts that resolve to loopback/private/link-local
    /// addresses. Off unless the user sets `SEITE_EXTERNAL_PRIVATE=1`: links
    /// come from the repository, which must not make this machine send
    /// requests into its network (routers, metadata endpoints, local admin
    /// pages). Refused links are counted as skipped.
    pub allow_private: bool,
}

/// User-level opt-in to checking links to private/internal addresses.
pub const ALLOW_PRIVATE_ENV: &str = "SEITE_EXTERNAL_PRIVATE";

impl ExternalOptions {
    pub fn for_site(config: &crate::config::SiteConfig, strict: bool) -> Self {
        let check = config.check.clone().unwrap_or_default();
        let mut own_origins = vec![origin_of(&config.site.base_url)];
        for c in config.subdomain_collections() {
            own_origins.push(origin_of(&config.subdomain_base_url(c)));
        }
        own_origins.retain(|o| !o.is_empty());
        Self {
            timeout: Duration::from_secs(check.external_timeout.unwrap_or(10).max(1)),
            workers: 8,
            per_host: 2,
            max_redirects: 5,
            skip: check.external_skip,
            own_origins,
            strict,
            allow_private: std::env::var_os(ALLOW_PRIVATE_ENV).is_some_and(|v| v == "1"),
        }
    }
}

/// `scheme://host[:port]` of a URL, lowercased.
fn origin_of(url: &str) -> String {
    let url = url.trim().to_ascii_lowercase();
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let host = rest.split(['/', '?', '#']).next().unwrap_or("");
            format!("{scheme}://{host}")
        }
        None => String::new(),
    }
}

/// The host of a URL (lowercase, no port or credentials).
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = if host.starts_with('[') {
        host.split(']')
            .next()
            .map(|h| format!("{h}]"))
            .unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or("").to_string()
    };
    host.trim_end_matches('.').to_ascii_lowercase()
}

fn is_skipped(host: &str, skip: &[String]) -> bool {
    DEFAULT_SKIP
        .iter()
        .copied()
        .chain(skip.iter().map(String::as_str))
        .map(|s| s.trim().trim_start_matches("*.").to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .any(|s| host == s || host.ends_with(&format!(".{s}")))
}

/// Result of checking one URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    /// Definitely broken (404/410, unknown host, …): reason.
    Broken(String),
    /// Could not be confirmed (timeout, 5xx, blocked, …): reason.
    Unreachable(String),
    /// Not requested: the host resolves to a private/internal address.
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    ok: bool,
    /// Unix seconds.
    checked_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    #[serde(default)]
    links: BTreeMap<String, CacheEntry>,
}

/// Counts for the `--json` summary.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Summary {
    /// Distinct external URLs found in the output.
    pub found: usize,
    /// Requested over the network this run.
    pub checked: usize,
    /// Answered from the cache (worked within the last 7 days).
    pub cached: usize,
    /// Not requested (skip list or the site's own URLs).
    pub skipped: usize,
    pub broken: usize,
    pub unreachable: usize,
    /// Set when the cache could not be written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_error: Option<String>,
}

/// External URLs in the HTML under `output_root`, each with the pages
/// (relative to `output_root`, `/`-separated) that reference it.
pub fn collect(output_root: &Path) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in WalkDir::new(output_root)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "html"))
    {
        let Ok(html) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let page = links::relative_display(output_root, entry.path());
        for r in links::extract_external_refs(&html) {
            let pages = out.entry(r.url).or_default();
            if !pages.contains(&page) {
                pages.push(page.clone());
            }
        }
    }
    out
}

/// Check every URL (bounded concurrency, per-host limit) and return the
/// outcome of each.
pub fn check_urls(urls: &[String], opts: &ExternalOptions) -> HashMap<String, Outcome> {
    struct State {
        queue: VecDeque<String>,
        active: HashMap<String, usize>,
    }
    let config = ureq::config::Config::builder()
        .timeout_global(Some(opts.timeout))
        .max_redirects(opts.max_redirects)
        .http_status_as_error(false)
        .user_agent(format!(
            "seite-linkcheck/{} (+https://seite.sh)",
            env!("CARGO_PKG_VERSION")
        ));
    let agent = if opts.allow_private {
        ureq::Agent::new_with_config(config.build())
    } else {
        // No proxy: the resolver must see each target host (redirects
        // included), and it connects only to the addresses it approved.
        ureq::Agent::with_parts(
            config.proxy(None).build(),
            ureq::unversioned::transport::DefaultConnector::default(),
            crate::cli::import::images::PublicOnlyResolver::strict(),
        )
    };
    let state = Mutex::new(State {
        queue: urls.iter().cloned().collect(),
        active: HashMap::new(),
    });
    let wake = Condvar::new();
    let results = Mutex::new(HashMap::new());
    let per_host = opts.per_host.max(1);
    let workers = opts.workers.clamp(1, urls.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let url = {
                    let mut st = state.lock().unwrap_or_else(PoisonError::into_inner);
                    loop {
                        if st.queue.is_empty() {
                            return;
                        }
                        let State { queue, active } = &mut *st;
                        let ready = queue
                            .iter()
                            .position(|u| active.get(&host_of(u)).copied().unwrap_or(0) < per_host);
                        match ready {
                            Some(i) => {
                                let url = queue.remove(i).unwrap_or_default();
                                *active.entry(host_of(&url)).or_default() += 1;
                                break url;
                            }
                            None => {
                                st = wake.wait(st).unwrap_or_else(PoisonError::into_inner);
                            }
                        }
                    }
                };
                let outcome = check_one(&agent, &url, opts.timeout);
                {
                    let mut st = state.lock().unwrap_or_else(PoisonError::into_inner);
                    if let Some(n) = st.active.get_mut(&host_of(&url)) {
                        *n = n.saturating_sub(1);
                    }
                }
                wake.notify_all();
                results
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(url, outcome);
            });
        }
    });
    results.into_inner().unwrap_or_else(PoisonError::into_inner)
}

fn check_one(agent: &ureq::Agent, url: &str, timeout: Duration) -> Outcome {
    let head = agent.head(url).call().map(|r| r.status().as_u16());
    let result = match head {
        Ok(405 | 403 | 501) => agent.get(url).call().map(|r| r.status().as_u16()),
        other => other,
    };
    classify(result, timeout)
}

fn classify(result: Result<u16, ureq::Error>, timeout: Duration) -> Outcome {
    match result {
        Ok(status) if status < 400 => Outcome::Ok,
        Ok(404) => Outcome::Broken("HTTP 404 Not Found".into()),
        Ok(410) => Outcome::Broken("HTTP 410 Gone".into()),
        Ok(status @ (401 | 403 | 429)) => Outcome::Unreachable(format!(
            "HTTP {status} (the server refused the check; many sites block bots)"
        )),
        Ok(status) if status >= 500 => {
            Outcome::Unreachable(format!("HTTP {status} (server error)"))
        }
        Ok(status) => Outcome::Broken(format!("HTTP {status}")),
        Err(ureq::Error::HostNotFound) => {
            Outcome::Broken("host not found (DNS lookup failed)".into())
        }
        Err(ureq::Error::BadUri(e)) => Outcome::Broken(format!("invalid URL ({e})")),
        Err(ureq::Error::Timeout(_)) => {
            Outcome::Unreachable(format!("timed out after {}s", timeout.as_secs().max(1)))
        }
        Err(ureq::Error::TooManyRedirects) => {
            Outcome::Unreachable("too many redirects (more than 5)".into())
        }
        Err(ureq::Error::ConnectionFailed) => Outcome::Unreachable("connection failed".into()),
        Err(ureq::Error::Io(e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Outcome::Private
        }
        Err(ureq::Error::Io(e)) => Outcome::Unreachable(format!("connection error: {e}")),
        Err(e) => Outcome::Unreachable(e.to_string()),
    }
}

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

fn load_cache(root: &Path) -> CacheFile {
    std::fs::read_to_string(root.join(CACHE_PATH))
        .ok()
        .and_then(|s| serde_json::from_str::<CacheFile>(&s).ok())
        .filter(|c| c.version == 1)
        .unwrap_or_default()
}

/// Inputs for [`run`].
pub struct ExternalCheck<'a> {
    /// Site root (cache location, attribution root).
    pub root: &'a Path,
    /// Directory holding the rendered output; reported pages are relative
    /// to it (e.g. `dist/posts/a.html`).
    pub output_root: &'a Path,
    /// `{output_root}-relative html path` → markdown source file.
    pub page_sources: &'a HashMap<String, PathBuf>,
    /// Searched for links not found in a page's own source (templates, data,
    /// content).
    pub fallback_dirs: &'a [PathBuf],
}

/// Collect, check (with the cache) and turn problems into located
/// diagnostics.
pub fn run(input: &ExternalCheck, opts: &ExternalOptions) -> (Vec<Diagnostic>, Summary) {
    let found = collect(input.output_root);
    let mut summary = Summary {
        found: found.len(),
        ..Summary::default()
    };
    let mut cache = load_cache(input.root);
    let now = now_secs();
    let mut outcomes: HashMap<String, Outcome> = HashMap::new();
    let mut to_check = Vec::new();
    for url in found.keys() {
        let lower = url.to_ascii_lowercase();
        let own = opts.own_origins.iter().any(|o| {
            lower == *o
                || lower.starts_with(&format!("{o}/"))
                || lower.starts_with(&format!("{o}?"))
        });
        if own || is_skipped(&host_of(url), &opts.skip) {
            summary.skipped += 1;
            continue;
        }
        match cache.links.get(url) {
            Some(e) if e.ok && now - e.checked_at < OK_MAX_AGE_SECS => {
                summary.cached += 1;
                outcomes.insert(url.clone(), Outcome::Ok);
            }
            _ => to_check.push(url.clone()),
        }
    }
    summary.checked = to_check.len();
    for (url, outcome) in check_urls(&to_check, opts) {
        let (ok, reason) = match &outcome {
            Outcome::Ok => (true, None),
            Outcome::Broken(r) | Outcome::Unreachable(r) => (false, Some(r.clone())),
            Outcome::Private => {
                summary.checked -= 1;
                summary.skipped += 1;
                continue;
            }
        };
        cache.links.insert(
            url.clone(),
            CacheEntry {
                ok,
                checked_at: now,
                reason,
            },
        );
        outcomes.insert(url, outcome);
    }
    // Forget URLs that are no longer linked anywhere.
    let linked: HashSet<&String> = found.keys().collect();
    cache.links.retain(|url, _| linked.contains(url));
    cache.version = 1;
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let path = input.root.join(CACHE_PATH);
        if let Err(e) = crate::safe_fs::write(input.root, &path, format!("{json}\n")) {
            summary.cache_error = Some(format!("{e:#}"));
        }
    }

    let mut diagnostics = Vec::new();
    for (url, pages) in &found {
        let (code, reason) = match outcomes.get(url) {
            Some(Outcome::Broken(r)) => {
                summary.broken += 1;
                ("external-link-broken", r)
            }
            Some(Outcome::Unreachable(r)) => {
                summary.unreachable += 1;
                ("external-link-unreachable", r)
            }
            _ => continue,
        };
        let mut occurrences: Vec<BrokenLink> = pages
            .iter()
            .map(|page| BrokenLink::new(page.clone(), url.clone(), LinkKind::Page))
            .collect();
        links::attribute_sources(
            &mut occurrences,
            &SourceAttribution {
                root: input.root,
                page_sources: input.page_sources,
                fallback_dirs: input.fallback_dirs,
                output_display: "",
            },
        );
        let mut seen = HashSet::new();
        for link in occurrences {
            let file = link.file();
            if !seen.insert((file.clone(), link.line)) {
                continue;
            }
            let message = if code == "external-link-broken" {
                format!("external link `{url}` is broken: {reason}")
            } else {
                format!("external link `{url}` could not be checked: {reason}")
            };
            let hint = if code == "external-link-broken" {
                "fix the URL or remove the link (if the page moved, link to its new address); failures are re-checked on every `seite check --external`"
            } else {
                "often temporary, or the site blocks bots: open the URL in a browser; if it works, ignore this or add the host to `[check] external_skip` in seite.toml"
            };
            // Literal codes, so the errors.md coverage test can see them.
            let d = match (code == "external-link-broken", opts.strict) {
                (true, false) => Diagnostic::warning("external-link-broken", message),
                (true, true) => Diagnostic::error("external-link-broken", message),
                (false, false) => Diagnostic::warning("external-link-unreachable", message),
                (false, true) => Diagnostic::error("external-link-unreachable", message),
            };
            let mut d = d.with_file(PathBuf::from(file)).with_hint(hint);
            if let Some(line) = link.line {
                d = d.with_line(line);
            }
            diagnostics.push(d);
        }
    }
    (diagnostics, summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A local HTTP server: /ok 200, /missing 404, /gone 410, /moved → /ok,
    /// /loop → /loop, /nohead 405 on HEAD, /down 503, /slow sleeps 2 s.
    fn server() -> (String, Arc<AtomicUsize>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        std::thread::spawn(move || {
            for req in server.incoming_requests() {
                counter.fetch_add(1, Ordering::SeqCst);
                std::thread::spawn(move || {
                    let path = req.url().to_string();
                    let head = *req.method() == tiny_http::Method::Head;
                    let resp = |code: u16| tiny_http::Response::empty(code);
                    let location = |to: &str| {
                        tiny_http::Response::empty(301).with_header(
                            tiny_http::Header::from_bytes(&b"Location"[..], to.as_bytes()).unwrap(),
                        )
                    };
                    let _ = match path.as_str() {
                        "/ok" => req.respond(resp(200)),
                        "/missing" => req.respond(resp(404)),
                        "/gone" => req.respond(resp(410)),
                        "/moved" => req.respond(location("/ok")),
                        "/loop" => req.respond(location("/loop")),
                        "/nohead" if head => req.respond(resp(405)),
                        "/nohead" => req.respond(resp(200)),
                        "/down" => req.respond(resp(503)),
                        "/slow" => {
                            std::thread::sleep(Duration::from_secs(2));
                            req.respond(resp(200))
                        }
                        _ => req.respond(resp(404)),
                    };
                });
            }
        });
        (format!("http://127.0.0.1:{port}"), hits)
    }

    fn opts() -> ExternalOptions {
        ExternalOptions {
            timeout: Duration::from_millis(500),
            workers: 8,
            per_host: 2,
            max_redirects: 5,
            skip: vec![],
            own_origins: vec![],
            strict: false,
            allow_private: true,
        }
    }

    #[test]
    fn test_check_urls_skips_private_addresses_by_default() {
        let (base, hits) = server();
        let mut o = opts();
        o.allow_private = false;
        let urls = vec![format!("{base}/ok")];
        let r = check_urls(&urls, &o);
        assert_eq!(r[&urls[0]], Outcome::Private);
        assert_eq!(hits.load(Ordering::SeqCst), 0, "nothing was requested");
    }

    #[test]
    fn test_check_urls_classifies_responses() {
        let (base, _) = server();
        let urls: Vec<String> = [
            "ok", "missing", "gone", "moved", "loop", "nohead", "down", "slow",
        ]
        .iter()
        .map(|p| format!("{base}/{p}"))
        .collect();
        let r = check_urls(&urls, &opts());
        let get = |p: &str| r.get(&format!("{base}/{p}")).unwrap().clone();
        assert_eq!(get("ok"), Outcome::Ok);
        assert_eq!(get("moved"), Outcome::Ok);
        assert_eq!(get("nohead"), Outcome::Ok);
        assert_eq!(get("missing"), Outcome::Broken("HTTP 404 Not Found".into()));
        assert_eq!(get("gone"), Outcome::Broken("HTTP 410 Gone".into()));
        assert!(matches!(get("loop"), Outcome::Unreachable(r) if r.contains("redirects")));
        assert!(matches!(get("down"), Outcome::Unreachable(r) if r.contains("503")));
        assert!(matches!(get("slow"), Outcome::Unreachable(r) if r.contains("timed out")));
    }

    #[test]
    fn test_extract_external_refs() {
        let html = r#"<link rel="preconnect" href="https://fonts.gstatic.com">
<link rel="stylesheet" href="https://cdn.test/a.css">
<a href="https://Ex.test/p?a=1&amp;b=2#frag">x</a><a href="/local">l</a>
<img src="//img.test/i.png"><script>var u = "https://nope.test";</script>
<a href="mailto:a@b.c">m</a><a href="https://">empty</a>"#;
        let urls: Vec<String> = links::extract_external_refs(html)
            .into_iter()
            .map(|r| r.url)
            .collect();
        assert_eq!(
            urls,
            vec![
                "https://cdn.test/a.css",
                "https://Ex.test/p?a=1&b=2",
                "https://img.test/i.png"
            ]
        );
    }

    #[test]
    fn test_classify_errors() {
        let t = Duration::from_secs(10);
        assert!(
            matches!(classify(Err(ureq::Error::HostNotFound), t), Outcome::Broken(r) if r.contains("DNS"))
        );
        assert!(matches!(
            classify(Err(ureq::Error::ConnectionFailed), t),
            Outcome::Unreachable(_)
        ));
        assert!(matches!(classify(Ok(429), t), Outcome::Unreachable(_)));
        assert!(matches!(classify(Ok(400), t), Outcome::Broken(_)));
        assert_eq!(classify(Ok(204), t), Outcome::Ok);
    }

    #[test]
    fn test_hosts_skip_and_origins() {
        assert_eq!(
            host_of("https://User@WWW.Example.com:8080/a?b"),
            "www.example.com"
        );
        assert_eq!(host_of("http://[::1]:80/x"), "[::1]");
        assert!(is_skipped("www.linkedin.com", &[]));
        assert!(is_skipped("x.com", &[]));
        assert!(!is_skipped("box.com", &[]));
        assert!(is_skipped("docs.internal.dev", &["internal.dev".into()]));
        assert_eq!(
            origin_of("https://Example.com/blog/"),
            "https://example.com"
        );
        assert_eq!(origin_of("not a url"), "");
    }

    #[test]
    fn test_run_attributes_caches_and_skips() {
        let (base, hits) = server();
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let out = root.join("out");
        std::fs::create_dir_all(out.join("dist/posts")).unwrap();
        std::fs::create_dir_all(root.join("content/posts")).unwrap();
        let src = root.join("content/posts/a.md");
        std::fs::write(
            &src,
            format!("---\ntitle: A\n---\nintro\n\n[bad]({base}/missing)\n[good]({base}/ok)\n"),
        )
        .unwrap();
        std::fs::write(
            out.join("dist/posts/a.html"),
            format!(
                "<a href=\"{base}/missing#x\">bad</a><a href=\"{base}/ok\">ok</a>\
                 <a href=\"https://www.linkedin.com/in/me\">li</a>\
                 <a href=\"https://mysite.test/self\">self</a>"
            ),
        )
        .unwrap();
        let mut page_sources = HashMap::new();
        page_sources.insert("dist/posts/a.html".to_string(), src.clone());
        let input = ExternalCheck {
            root,
            output_root: &out,
            page_sources: &page_sources,
            fallback_dirs: &[],
        };
        let mut o = opts();
        o.own_origins = vec!["https://mysite.test".into()];
        let (diags, summary) = run(&input, &o);
        assert_eq!(summary.found, 4);
        assert_eq!(summary.skipped, 2, "{summary:?}");
        assert_eq!(summary.checked, 2);
        assert_eq!(summary.broken, 1);
        assert_eq!(diags.len(), 1, "{diags:#?}");
        let d = &diags[0];
        assert_eq!(d.code, "external-link-broken");
        assert!(!d.is_error());
        assert_eq!(d.file.as_deref(), Some(Path::new("content/posts/a.md")));
        assert_eq!(d.line, Some(6));
        let cache = std::fs::read_to_string(root.join(CACHE_PATH)).unwrap();
        assert!(cache.contains(&format!("{base}/ok")), "{cache}");

        // Second run: the working link comes from the cache, the failure is
        // re-checked; strict turns it into an error.
        let before = hits.load(Ordering::SeqCst);
        o.strict = true;
        let (diags, summary) = run(&input, &o);
        assert_eq!(summary.cached, 1);
        assert_eq!(summary.checked, 1);
        assert!(diags[0].is_error());
        assert_eq!(
            hits.load(Ordering::SeqCst) - before,
            1,
            "only /missing is requested again"
        );
    }
}
