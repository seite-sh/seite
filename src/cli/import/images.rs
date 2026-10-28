//! Image localization for `seite import`: download (WordPress, Ghost) or
//! copy (Jekyll, Hugo) every image a body or featured image references into
//! `static/images/imported/<slug>/`, and rewrite the references to
//! `/static/images/imported/<slug>/<name>`.
//!
//! Downloads: http/https only; the host (with port) must be the export's own
//! site or an `--allow-host`; redirects are followed by hand (max 5) and
//! each hop must pass the same host check; `Content-Type: image/*`; 20 MB per
//! file and a total budget; 30 s per request. The export names its own site
//! (untrusted), so a host that was not given on the command line
//! (`--allow-host`, `--site-url`) must resolve only to public addresses —
//! never loopback, private, link-local (cloud metadata), CGNAT, ULA or
//! multicast — checked before every request, redirect hops included.
//! Local copies must resolve
//! (symlinks included) inside the source directory. Identical bytes are
//! stored once (SHA-256); names come from the source file name, `-2`, `-3`
//! on clashes, in processing order, so repeated imports name files alike.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::Warning;

/// Largest single image.
pub(crate) const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp", "ico", "tif", "tiff",
];

pub(crate) struct Options {
    pub site_root: PathBuf,
    pub static_dir: PathBuf,
    pub site_url: Option<url::Url>,
    /// `--site-url`: the user named this host, so it may be private.
    pub explicit_site_url: Option<url::Url>,
    pub allow_hosts: Vec<String>,
    pub local_roots: Vec<PathBuf>,
    pub confine_root: Option<PathBuf>,
    pub enabled: bool,
    pub dry_run: bool,
    pub force: bool,
    pub max_total: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Stats {
    pub downloaded: usize,
    pub copied: usize,
    /// Identical to an image already stored (not written again).
    pub deduplicated: usize,
    pub failed: usize,
    /// Not fetched: host not allowed, or not http(s).
    pub refused: usize,
    /// `--dry-run`: would be downloaded or copied.
    pub planned: usize,
}

/// The item whose references are being localized.
pub(crate) struct ItemImages<'a> {
    pub slug: &'a str,
    pub item_id: &'a str,
    pub source_ref: &'a str,
    pub base_dirs: &'a [PathBuf],
}

/// Where an image reference points.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Target {
    Remote(url::Url),
    Local(PathBuf),
}

/// `host` or `host:port` an allowed host matches.
#[derive(Debug, Clone)]
struct HostSpec {
    host: String,
    port: Option<u16>,
    /// Given on the command line (may be a private address).
    explicit: bool,
}

impl HostSpec {
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_end_matches('/');
        let s = s
            .strip_prefix("https://")
            .or_else(|| s.strip_prefix("http://"))
            .unwrap_or(s);
        let parsed = url::Url::parse(&format!("http://{s}")).ok()?;
        Some(Self {
            host: parsed.host_str()?.to_ascii_lowercase(),
            port: parsed.port(),
            explicit: true,
        })
    }

    fn from_url(u: &url::Url, explicit: bool) -> Option<Self> {
        Some(Self {
            host: u.host_str()?.to_ascii_lowercase(),
            port: u.port(),
            explicit,
        })
    }

    /// Same host; the same explicit port, or no port on either side.
    fn matches(&self, u: &url::Url) -> bool {
        u.host_str()
            .is_some_and(|h| h.eq_ignore_ascii_case(&self.host))
            && self.port == u.port()
    }
}

pub(crate) struct Images {
    opts: Options,
    allowed: Vec<HostSpec>,
    agent: ureq::Agent,
    confine_canonical: Option<PathBuf>,
    /// Resolved target → new URL (`None`: failed, don't retry).
    done: HashMap<Target, Option<String>>,
    by_hash: HashMap<[u8; 32], String>,
    /// Names given in this run per directory.
    names: HashMap<PathBuf, Vec<String>>,
    total: u64,
    stats: Stats,
}

impl Images {
    pub(crate) fn new(opts: Options) -> anyhow::Result<Self> {
        let mut allowed = Vec::new();
        if let Some(spec) = opts
            .explicit_site_url
            .as_ref()
            .and_then(|u| HostSpec::from_url(u, true))
        {
            allowed.push(spec);
        }
        if let Some(spec) = opts
            .site_url
            .as_ref()
            .and_then(|u| HostSpec::from_url(u, false))
        {
            allowed.push(spec);
        }
        for h in &opts.allow_hosts {
            match HostSpec::parse(h) {
                Some(spec) => allowed.push(spec),
                None => anyhow::bail!(
                    "--allow-host '{h}' is not a host (use e.g. cdn.example.com or 127.0.0.1:8080)"
                ),
            }
        }
        let exempt = allowed
            .iter()
            .filter(|h| h.explicit)
            .map(|h| (bare_host(&h.host), h.port))
            .collect();
        let agent = build_agent(exempt);
        let confine_canonical = opts
            .confine_root
            .as_ref()
            .and_then(|r| std::fs::canonicalize(r).ok());
        Ok(Self {
            opts,
            allowed,
            agent,
            confine_canonical,
            done: HashMap::new(),
            by_hash: HashMap::new(),
            names: HashMap::new(),
            total: 0,
            stats: Stats::default(),
        })
    }

    pub(crate) fn stats(&self) -> Stats {
        self.stats.clone()
    }

    /// Rewrite image references in a Markdown body: `![](x)`, links to
    /// image files `[..](x.jpg)`, and `src="x"` in `<img>` / `figure`.
    pub(crate) fn localize_body(
        &mut self,
        body: &str,
        item: &ItemImages,
        warnings: &mut Vec<Warning>,
    ) -> String {
        if !self.opts.enabled {
            return body.to_string();
        }
        static IMAGE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"!\[[^\]\n]*\]\(\s*(<[^>\n]+>|[^)\s]+)").expect("valid regex")
        });
        static LINK: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(\]\(\s*)(<[^>\n]+>|[^)\s]+)").expect("valid regex"));
        static SRC: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r#"(\bsrc\s*=\s*)("[^"\n]*"|'[^'\n]*')"#).expect("valid regex")
        });
        let unwrap = |s: &str| -> String {
            s.trim_start_matches('<')
                .trim_end_matches('>')
                .trim_matches(['"', '\''])
                .to_string()
        };
        let fences = super::convert::fenced_ranges(body);
        let in_fence = |pos: usize| fences.iter().any(|r| r.contains(&pos));

        // Collect references in document order.
        let mut wanted: Vec<String> = Vec::new();
        let mut add = |r: String| {
            if !wanted.contains(&r) {
                wanted.push(r);
            }
        };
        let mut found: Vec<(usize, String, bool)> = Vec::new();
        for c in IMAGE.captures_iter(body) {
            let m = c.get(1).expect("group 1");
            found.push((m.start(), unwrap(m.as_str()), true));
        }
        for c in LINK.captures_iter(body) {
            let m = c.get(2).expect("group 2");
            let r = unwrap(m.as_str());
            if has_image_extension(&r) {
                found.push((m.start(), r, false));
            }
        }
        for c in SRC.captures_iter(body) {
            let m = c.get(2).expect("group 2");
            let r = unwrap(m.as_str());
            let before = &body[..c.get(0).expect("match").start()];
            let tag_start = before.rfind('<').map(|i| &body[i..]).unwrap_or("");
            let in_image_tag = tag_start.starts_with("<img") || tag_start.starts_with("< figure");
            if in_image_tag || has_image_extension(&r) {
                found.push((m.start(), r, true));
            }
        }
        found.sort();
        for (pos, r, _) in found {
            if !in_fence(pos) {
                add(r);
            }
        }

        let mut map: HashMap<String, String> = HashMap::new();
        for r in wanted {
            if let Some(new) = self.localize(&r, item, false, warnings) {
                map.insert(r, new);
            }
        }
        if map.is_empty() {
            return body.to_string();
        }
        let replace_in = |text: &str, re: &Regex, group: usize| -> String {
            re.replace_all(text, |c: &regex::Captures| {
                let whole = c.get(0).expect("match");
                let m = c.get(group).expect("group");
                if in_fence(whole.start()) {
                    return whole.as_str().to_string();
                }
                let raw = m.as_str();
                let key = unwrap(raw);
                match map.get(&key) {
                    Some(new) => {
                        let replaced = if raw.starts_with('"') {
                            format!("\"{new}\"")
                        } else if raw.starts_with('\'') {
                            format!("'{new}'")
                        } else if raw.starts_with('<') {
                            format!("<{new}>")
                        } else {
                            new.clone()
                        };
                        format!(
                            "{}{}{}",
                            &whole.as_str()[..m.start() - whole.start()],
                            replaced,
                            &whole.as_str()[m.end() - whole.start()..]
                        )
                    }
                    None => whole.as_str().to_string(),
                }
            })
            .into_owned()
        };
        // LINK covers image targets too (`![a](x)` ends in `](x`).
        let body = replace_in(body, &LINK, 2);
        replace_in(&body, &SRC, 2)
    }

    /// Localize one reference; the new `/static/...` URL, or `None` when it
    /// stays as it is (a warning says why, where that is useful).
    pub(crate) fn localize(
        &mut self,
        reference: &str,
        item: &ItemImages,
        featured: bool,
        warnings: &mut Vec<Warning>,
    ) -> Option<String> {
        if !self.opts.enabled {
            return None;
        }
        let reference = reference.trim();
        if reference.is_empty()
            || reference.starts_with("data:")
            || reference.starts_with('#')
            || reference.starts_with("/static/images/imported/")
        {
            return None;
        }
        let warn = |warnings: &mut Vec<Warning>, code: &'static str, msg: String| {
            let mut w = Warning::new(item.source_ref, code, msg);
            w.item = Some(item.item_id.to_string());
            warnings.push(w);
        };
        let target = match self.resolve(reference, item) {
            Ok(t) => t,
            Err(Some(msg)) => {
                self.stats.refused += 1;
                warn(warnings, "image-not-imported", msg);
                return None;
            }
            Err(None) => return None,
        };
        if let Some(done) = self.done.get(&target) {
            return done.clone();
        }
        if self.opts.dry_run {
            self.stats.planned += 1;
            self.done.insert(target, None);
            return None;
        }
        let fetched = match &target {
            Target::Remote(u) => self.download(u).map(|bytes| (bytes, basename_of_url(u))),
            Target::Local(p) => self.read_local(p).map(|bytes| (bytes, basename_of_path(p))),
        };
        // The format (and so the extension) comes from the bytes, never from
        // the name or Content-Type; SVG (can carry script) and anything else
        // unrecognised is not stored.
        let fetched = fetched.and_then(|(bytes, raw)| match sniff_image(&bytes) {
            Some(ext) => Ok((bytes, safe_name(&raw, ext))),
            None => Err(
                "not a PNG, JPEG, GIF, WebP or AVIF image (SVG and other formats are not imported)"
                    .to_string(),
            ),
        });
        let result = match fetched {
            Ok((bytes, name)) => match self.store(&bytes, &name, item.slug) {
                Ok((url, new)) => {
                    if !new {
                        self.stats.deduplicated += 1;
                    } else if matches!(target, Target::Remote(_)) {
                        self.stats.downloaded += 1;
                    } else {
                        self.stats.copied += 1;
                    }
                    Some(url)
                }
                Err(e) => {
                    self.stats.failed += 1;
                    warn(
                        warnings,
                        "image-failed",
                        format!("{reference}: cannot store it: {e:#}"),
                    );
                    None
                }
            },
            Err(e) => {
                self.stats.failed += 1;
                let what = if featured { "featured image " } else { "" };
                warn(
                    warnings,
                    "image-failed",
                    format!("{what}{reference}: {e}; reference left as it was"),
                );
                None
            }
        };
        self.done.insert(target, result.clone());
        result
    }

    /// Where `reference` points. `Err(Some(msg))`: refused (reported);
    /// `Err(None)`: left alone silently (an external image on a local site).
    fn resolve(&self, reference: &str, item: &ItemImages) -> Result<Target, Option<String>> {
        let local = !self.opts.local_roots.is_empty() || !item.base_dirs.is_empty();
        let absolute = if reference.starts_with("//") {
            url::Url::parse(&format!("https:{reference}")).ok()
        } else {
            url::Url::parse(reference).ok()
        };
        if let Some(u) = absolute {
            if !matches!(u.scheme(), "http" | "https") {
                return Err(Some(format!(
                    "{reference}: only http(s) images are downloaded"
                )));
            }
            let on_site = self
                .opts
                .site_url
                .as_ref()
                .and_then(|u| HostSpec::from_url(u, false))
                .is_some_and(|s| s.matches(&u));
            if local && on_site {
                // The old site's own URL for a file that is in the source.
                return self.resolve_local_path(u.path(), item);
            }
            if self.allowed.iter().any(|h| h.matches(&u)) {
                return Ok(Target::Remote(u));
            }
            if local {
                return Err(None);
            }
            return Err(Some(format!(
                "{reference}: not downloaded, {} is not the export's site (pass --allow-host {} to download it)",
                host_port(&u),
                host_port(&u)
            )));
        }
        if local {
            return self.resolve_local_path(reference, item);
        }
        // Root-relative or relative URL on the old site.
        match &self.opts.site_url {
            Some(base) => match base.join(reference) {
                Ok(u) => Ok(Target::Remote(u)),
                Err(_) => Err(Some(format!("{reference}: not a URL"))),
            },
            None => Err(Some(format!(
                "{reference}: relative image path but the export names no site URL (pass --site-url https://old-site.example)"
            ))),
        }
    }

    fn resolve_local_path(
        &self,
        reference: &str,
        item: &ItemImages,
    ) -> Result<Target, Option<String>> {
        let path = reference.split(['?', '#']).next().unwrap_or("");
        let path = urlencoding::decode(path)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| path.to_string());
        if path.is_empty() {
            return Err(None);
        }
        let candidates: Vec<PathBuf> = if let Some(rel) = path.strip_prefix('/') {
            self.opts.local_roots.iter().map(|r| r.join(rel)).collect()
        } else {
            item.base_dirs
                .iter()
                .chain(self.opts.local_roots.iter())
                .map(|d| d.join(&path))
                .collect()
        };
        for c in &candidates {
            if c.is_file() {
                return match self.confined(c) {
                    Some(canonical) => Ok(Target::Local(canonical)),
                    None => Err(Some(format!(
                        "{reference}: resolves outside the source directory (symlink?); not copied"
                    ))),
                };
            }
        }
        Err(Some(format!(
            "{reference}: file not found in the source; reference left as it was"
        )))
    }

    /// The canonical path when it stays inside the source directory.
    fn confined(&self, path: &Path) -> Option<PathBuf> {
        let canonical = std::fs::canonicalize(path).ok()?;
        match &self.confine_canonical {
            Some(root) if canonical.starts_with(root) => Some(canonical),
            _ => None,
        }
    }

    fn read_local(&self, path: &Path) -> Result<Vec<u8>, String> {
        if !has_image_extension(&path.to_string_lossy()) {
            return Err("not an image file (by extension)".into());
        }
        let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        if len > MAX_IMAGE_BYTES {
            return Err(format!(
                "larger than {} MB",
                MAX_IMAGE_BYTES / (1024 * 1024)
            ));
        }
        std::fs::read(path).map_err(|e| e.to_string())
    }

    /// GET an image, following redirects only to allowed hosts. Returns the
    /// bytes and an extension derived from the Content-Type.
    fn download(&mut self, url: &url::Url) -> Result<Vec<u8>, String> {
        let mut current = url.clone();
        for _ in 0..=MAX_REDIRECTS {
            if !matches!(current.scheme(), "http" | "https") {
                return Err(format!("redirected to a non-http URL ({current})"));
            }
            if !self.allowed.iter().any(|h| h.matches(&current)) {
                return Err(format!(
                    "redirected to {}, which is not an allowed host",
                    host_port(&current)
                ));
            }
            // Public-address checks happen in the agent's resolver
            // (`PublicOnlyResolver`), on the very addresses it connects to.
            let response = self
                .agent
                .get(current.as_str())
                .call()
                .map_err(|e| format!("download failed: {e}"))?;
            let status = response.status().as_u16();
            if (300..400).contains(&status) {
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| format!("HTTP {status} without a Location header"))?;
                current = current
                    .join(location)
                    .map_err(|_| format!("bad redirect Location '{location}'"))?;
                continue;
            }
            if status != 200 {
                return Err(format!("HTTP {status}"));
            }
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !content_type.starts_with("image/") {
                return Err(format!(
                    "not an image (Content-Type: {})",
                    if content_type.is_empty() {
                        "none"
                    } else {
                        &content_type
                    }
                ));
            }
            let remaining = self.opts.max_total.saturating_sub(self.total);
            if remaining == 0 {
                return Err(format!(
                    "total download budget of {} MB used up (raise it with --max-download-mb)",
                    self.opts.max_total / (1024 * 1024)
                ));
            }
            if let Some(len) = response
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
            {
                if len > MAX_IMAGE_BYTES {
                    return Err(format!(
                        "larger than {} MB",
                        MAX_IMAGE_BYTES / (1024 * 1024)
                    ));
                }
                if len > remaining {
                    return Err("would exceed the total download budget (--max-download-mb)".into());
                }
            }
            let limit = MAX_IMAGE_BYTES.min(remaining);
            let bytes = response
                .into_body()
                .with_config()
                .limit(limit)
                .read_to_vec()
                .map_err(|e| match e {
                    ureq::Error::BodyExceedsLimit(_) if limit == MAX_IMAGE_BYTES => {
                        format!("larger than {} MB", MAX_IMAGE_BYTES / (1024 * 1024))
                    }
                    ureq::Error::BodyExceedsLimit(_) => {
                        "would exceed the total download budget (--max-download-mb)".to_string()
                    }
                    other => format!("download failed: {other}"),
                })?;
            self.total += bytes.len() as u64;
            return Ok(bytes);
        }
        Err(format!("more than {MAX_REDIRECTS} redirects"))
    }

    /// Write `bytes` as `static/images/imported/<slug>/<name>` (or reuse an
    /// identical image). Returns (URL, newly written).
    fn store(&mut self, bytes: &[u8], name: &str, slug: &str) -> anyhow::Result<(String, bool)> {
        let hash: [u8; 32] = Sha256::digest(bytes).into();
        if let Some(url) = self.by_hash.get(&hash) {
            return Ok((url.clone(), false));
        }
        let dir = self
            .opts
            .static_dir
            .join("images")
            .join("imported")
            .join(slug);
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) => (s.to_string(), format!(".{e}")),
            None => (name.to_string(), String::new()),
        };
        let taken = self.names.entry(dir.clone()).or_default();
        let mut n = 1;
        let filename = loop {
            let candidate = if n == 1 {
                format!("{stem}{ext}")
            } else {
                format!("{stem}-{n}{ext}")
            };
            n += 1;
            if taken.contains(&candidate) {
                continue;
            }
            let path = dir.join(&candidate);
            match std::fs::read(&path) {
                Ok(existing) if existing == bytes => break candidate,
                Ok(_) if !self.opts.force => continue,
                _ => break candidate,
            }
        };
        taken.push(filename.clone());
        let path = dir.join(&filename);
        let unchanged = std::fs::read(&path).is_ok_and(|existing| existing == bytes);
        if !unchanged {
            crate::safe_fs::write(&self.opts.site_root, &path, bytes)?;
        }
        let rel = path
            .strip_prefix(&self.opts.static_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let url = format!("/static/{rel}");
        self.by_hash.insert(hash, url.clone());
        Ok((url, true))
    }
}

/// The download agent: no proxy (the resolver must see the target host),
/// no automatic redirects (each hop is checked by hand), and
/// [`PublicOnlyResolver`], so the socket ureq connects to is exactly an
/// address that passed the public-address check (no DNS-rebinding window).
fn build_agent(exempt: Vec<(String, Option<u16>)>) -> ureq::Agent {
    let config = ureq::config::Config::builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .max_redirects(0)
        .http_status_as_error(false)
        .proxy(None)
        .user_agent(concat!("seite-import/", env!("CARGO_PKG_VERSION")))
        .build();
    ureq::Agent::with_parts(
        config,
        ureq::unversioned::transport::DefaultConnector::default(),
        PublicOnlyResolver {
            exempt,
            inner: ureq::unversioned::resolver::DefaultResolver::default(),
        },
    )
}

/// `[::1]` → `::1`, lowercase.
fn bare_host(host: &str) -> String {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase()
}

/// Resolves like ureq's default resolver, then refuses any non-public
/// address unless the host (and port) was given on the command line
/// (`--allow-host`, `--site-url`).
#[derive(Debug)]
struct PublicOnlyResolver {
    /// (host without brackets, explicit port)
    exempt: Vec<(String, Option<u16>)>,
    inner: ureq::unversioned::resolver::DefaultResolver,
}

impl ureq::unversioned::resolver::Resolver for PublicOnlyResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: ureq::unversioned::transport::NextTimeout,
    ) -> Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        let addrs = self.inner.resolve(uri, config, timeout)?;
        let host = bare_host(uri.host().unwrap_or(""));
        let port = uri.port_u16();
        if self.exempt.iter().any(|(h, p)| *h == host && *p == port) {
            return Ok(addrs);
        }
        if let Some(bad) = addrs.iter().find(|a| !is_public_ip(a.ip())) {
            let shown = match port {
                Some(p) => format!("{host}:{p}"),
                None => host.clone(),
            };
            return Err(ureq::Error::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "{shown} is a private/internal address ({}); pass --allow-host {shown} to download from it",
                    bad.ip()
                ),
            )));
        }
        Ok(addrs)
    }
}

/// Globally routable unicast (not loopback, private, link-local, CGNAT,
/// ULA, multicast, unspecified, documentation or benchmarking ranges).
fn is_public_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || o[0] == 0
                || (o[0] == 100 && (64..128).contains(&o[1]))
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                || o[0] >= 240)
        }
        std::net::IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_ip(v4.into());
            }
            let seg = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg[0] & 0xfe00) == 0xfc00
                || (seg[0] & 0xffc0) == 0xfe80
                || (seg[0] == 0x2001 && seg[1] == 0x0db8)
                || (seg[0] == 0x64 && seg[1] == 0xff9b)
                // 6to4, Teredo and IPv4-compatible (::a.b.c.d) embed IPv4
                || seg[0] == 0x2002
                || (seg[0] == 0x2001 && seg[1] == 0)
                || seg[..6].iter().all(|s| *s == 0))
        }
    }
}

fn host_port(u: &url::Url) -> String {
    match (u.host_str(), u.port()) {
        (Some(h), Some(p)) => format!("{h}:{p}"),
        (Some(h), None) => h.to_string(),
        _ => u.to_string(),
    }
}

pub(crate) fn has_image_extension(reference: &str) -> bool {
    let path = reference.split(['?', '#']).next().unwrap_or("");
    path.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.as_str()))
}

/// A safe file name: slugified stem + lowercase image extension.
fn safe_name(raw: &str, ext: &str) -> String {
    let decoded = urlencoding::decode(raw)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| raw.to_string());
    let stem = match decoded.rsplit_once('.') {
        Some((s, e)) if IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()) => s,
        _ => decoded.as_str(),
    };
    let stem = slug::slugify(stem);
    let stem = if stem.is_empty() {
        "image".to_string()
    } else {
        stem
    };
    format!("{stem}.{ext}")
}

/// The image format by its magic bytes: png, jpg, gif, webp or avif.
fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && matches!(&bytes[8..12], b"avif" | b"avis")
    {
        Some("avif")
    } else {
        None
    }
}

fn basename_of_url(u: &url::Url) -> String {
    u.path_segments()
        .and_then(|mut s| s.next_back())
        .unwrap_or("")
        .to_string()
}

fn basename_of_path(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_spec_matches_port() {
        let spec = HostSpec::parse("127.0.0.1:8080").unwrap();
        assert!(spec.matches(&url::Url::parse("http://127.0.0.1:8080/a.png").unwrap()));
        assert!(!spec.matches(&url::Url::parse("http://127.0.0.1:9090/a.png").unwrap()));
        assert!(!spec.matches(&url::Url::parse("http://localhost:8080/a.png").unwrap()));
        let spec = HostSpec::parse("https://Example.com/").unwrap();
        assert!(spec.matches(&url::Url::parse("https://example.com/x.jpg").unwrap()));
        assert!(!spec.matches(&url::Url::parse("https://cdn.example.com/x.jpg").unwrap()));
    }

    #[test]
    fn test_safe_name() {
        assert_eq!(safe_name("My%20Photo.JPG", "jpg"), "my-photo.jpg");
        assert_eq!(safe_name("download.svg", "png"), "download.png");
        assert_eq!(safe_name("..", "gif"), "image.gif");
        assert_eq!(
            sniff_image(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            None
        );
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
    }

    #[test]
    fn test_is_public_ip() {
        let public = |s: &str| is_public_ip(s.parse().unwrap());
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "224.0.0.1",
            "192.0.0.8",
            "2002:7f00:1::1",
            "2001:0:4136:e378::1",
            "::127.0.0.1",
        ] {
            assert!(!public(private), "{private}");
        }
        assert!(public("93.184.216.34"));
        assert!(public("2606:4700::1111"));
    }

    /// The agent's resolver refuses private addresses before connecting,
    /// unless the host:port was given on the command line.
    #[test]
    fn test_resolver_refuses_private_addresses() {
        // A closed local port: refusal must come from the resolver.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let url = format!("http://127.0.0.1:{port}/x.png");
        let err = build_agent(Vec::new())
            .get(&url)
            .call()
            .unwrap_err()
            .to_string();
        assert!(err.contains("private/internal address"), "{err}");
        let err = build_agent(Vec::new())
            .get("http://169.254.169.254/latest/meta-data")
            .call()
            .unwrap_err()
            .to_string();
        assert!(err.contains("private/internal address"), "{err}");
        // Exempt (explicit) host: the resolver lets it through; the
        // connection itself then fails.
        let err = build_agent(vec![("127.0.0.1".into(), Some(port))])
            .get(&url)
            .call()
            .unwrap_err()
            .to_string();
        assert!(!err.contains("private/internal address"), "{err}");
    }

    #[test]
    fn test_has_image_extension() {
        assert!(has_image_extension("/a/b.PNG?x=1"));
        assert!(!has_image_extension("/a/b.html"));
    }
}
