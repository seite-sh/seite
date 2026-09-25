use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::diagnostics::Diagnostic;
use crate::error::Result;

/// What kind of reference a broken link is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    /// A link to a page (`<a href>`, `<link rel=alternate>`, ...).
    Page,
    /// An embedded asset (`<img src>`, `srcset`, stylesheet, script, media).
    Asset,
    /// A relative link to a markdown source file (`[x](../docs/intro.md)`)
    /// that doesn't match any content file.
    Markdown,
}

/// A broken internal link (or missing asset) found during validation.
#[derive(Debug, Clone)]
pub struct BrokenLink {
    /// Generated HTML file containing the broken link, relative to the output
    /// directory (e.g. "posts/hello-world.html").
    pub source_file: String,
    /// The broken reference as authored (base path stripped, fragment and
    /// query removed), e.g. "/posts/nonexistent".
    pub href: String,
    pub kind: LinkKind,
    /// File the link was written in, relative to the site root (e.g.
    /// "content/posts/hello.md", "templates/base.html", "data/nav.yaml").
    /// `None` when it couldn't be traced back to a source file.
    pub source: Option<String>,
    /// 1-based line of the link in `source`.
    pub line: Option<usize>,
    /// True when the link isn't in the page's own markdown: it comes from a
    /// template, a data file (e.g. nav), or a generated listing page.
    pub from_template: bool,
    /// Closest valid URL, for a "did you mean" hint.
    pub suggestion: Option<String>,
    /// Final output directory relative to the site root (e.g. "dist"); used
    /// to display generated pages that have no source file.
    pub output_display: String,
}

impl BrokenLink {
    /// A broken reference found in `source_file` (output-relative HTML path),
    /// not yet attributed to a source file.
    pub fn new(source_file: impl Into<String>, href: impl Into<String>, kind: LinkKind) -> Self {
        Self {
            source_file: source_file.into(),
            href: href.into(),
            kind,
            source: None,
            line: None,
            from_template: false,
            suggestion: None,
            output_display: String::new(),
        }
    }

    /// The file to edit, relative to the site root: the source file when
    /// known, otherwise the generated page.
    pub fn file(&self) -> String {
        match &self.source {
            Some(source) => source.clone(),
            None if self.output_display.is_empty() => self.source_file.clone(),
            None => format!("{}/{}", self.output_display, self.source_file),
        }
    }

    /// Human-readable location, e.g. `content/posts/a.md:12` or
    /// `dist/tags/index.html (from template/listing)`.
    pub fn location(&self) -> String {
        let mut loc = self.file();
        if let Some(line) = self.line {
            loc.push_str(&format!(":{line}"));
        }
        if self.from_template {
            loc.push_str(" (from template/listing)");
        }
        loc
    }
}

/// Result of an internal link check across all HTML files in the output directory.
#[derive(Debug, Default, Clone)]
pub struct LinkCheckResult {
    /// Total number of internal links and asset references checked.
    pub total_links_checked: usize,
    /// Links to pages that don't exist (kinds `Page` and `Markdown`).
    pub broken_links: Vec<BrokenLink>,
    /// References to assets (images, stylesheets, scripts, media) that don't exist.
    pub missing_assets: Vec<BrokenLink>,
}

impl LinkCheckResult {
    /// Number of distinct problems (see [`distinct_links`]) across broken
    /// links and missing assets.
    pub fn problem_count(&self) -> usize {
        distinct_links(&self.broken_links).len() + distinct_links(&self.missing_assets).len()
    }

    /// Add another site's results (e.g. a subdomain build) to these.
    pub fn merge(&mut self, other: &LinkCheckResult) {
        self.total_links_checked += other.total_links_checked;
        self.broken_links.extend(other.broken_links.iter().cloned());
        self.missing_assets
            .extend(other.missing_assets.iter().cloned());
    }
}

/// Convert a link check into `broken-link` / `missing-asset` diagnostics,
/// one per distinct occurrence (see [`distinct_links`]).
///
/// Diagnostics are warnings, or errors when `strict` is set (matching
/// `seite build --strict`). `file` is relative to the site root: the
/// markdown/template/data file the link was written in when it could be
/// traced, otherwise the generated page.
pub fn link_diagnostics(check: &LinkCheckResult, strict: bool) -> Vec<Diagnostic> {
    let links = distinct_links(&check.broken_links)
        .into_iter()
        .chain(distinct_links(&check.missing_assets));
    let mut out = Vec::new();
    for link in links {
        let (code, mut message) = match link.kind {
            LinkKind::Page => (
                "broken-link",
                format!("link to `{}` does not match any page", link.href),
            ),
            LinkKind::Markdown => (
                "broken-link",
                format!("link to `{}` does not match any content file", link.href),
            ),
            LinkKind::Asset => (
                "missing-asset",
                format!("`{}` does not exist in the build output", link.href),
            ),
        };
        if link.from_template {
            if link.source.is_some() {
                message.push_str(&format!(
                    " (from template/listing, on page {})",
                    link.source_file
                ));
            } else {
                message.push_str(" (from template/listing)");
            }
        }
        let mut d = if strict {
            Diagnostic::error(code, message)
        } else {
            Diagnostic::warning(code, message)
        }
        .with_file(PathBuf::from(link.file()));
        if let Some(line) = link.line {
            d = d.with_line(line);
        }
        if let Some(s) = &link.suggestion {
            d = d.with_hint(format!("did you mean `{s}`?"));
        } else if link.kind == LinkKind::Asset {
            d = d.with_hint("add the file under static/ or public/, or fix the path");
        } else if link.kind == LinkKind::Markdown {
            d = d.with_hint(
                "relative .md links must point at a content file (path is relative to this file)",
            );
        }
        out.push(d);
    }
    out
}

/// Check all internal links and asset references in HTML files under `output_dir`.
///
/// Walks every `.html` file, extracts root-relative references, and validates
/// each against the set of files present in `output_dir`.
pub fn check_internal_links(output_dir: &Path) -> Result<LinkCheckResult> {
    let valid_urls = build_valid_urls(output_dir);
    let mut result = LinkCheckResult::default();

    for entry in WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "html")
        })
    {
        let html = fs::read_to_string(entry.path())?;
        let rel_path = entry
            .path()
            .strip_prefix(output_dir)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        let page = check_page_refs(&html, &rel_path, &valid_urls, "");
        result.total_links_checked += page.checked;
        result.broken_links.extend(page.broken_links);
        result.missing_assets.extend(page.missing_assets);
    }
    fill_suggestions(&mut result.broken_links, &valid_urls);
    fill_suggestions(&mut result.missing_assets, &valid_urls);

    Ok(result)
}

/// Build the set of all valid internal URL paths from files in the output directory.
///
/// For each file, computes the URL paths that would resolve to it:
/// - Exact file path: `/feed.xml`, `/static/style.css`
/// - Clean URL for `.html` files: `/posts/hello-world` (from `posts/hello-world.html`)
/// - Directory index variants: `/posts/` and `/posts` (from `posts/index.html`)
fn build_valid_urls(output_dir: &Path) -> HashSet<String> {
    let entries: Vec<_> = WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();
    build_valid_urls_from_entries(output_dir, &entries)
}

/// Build valid URL set from pre-collected WalkDir entries (avoids a redundant walk).
pub fn build_valid_urls_from_entries(
    output_dir: &Path,
    entries: &[walkdir::DirEntry],
) -> HashSet<String> {
    let mut urls = HashSet::new();

    for entry in entries {
        let rel = entry
            .path()
            .strip_prefix(output_dir)
            .unwrap_or(entry.path());
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        // Exact file path is always valid
        urls.insert(format!("/{rel_str}"));

        // For .html files, add clean URL variants
        if let Some(stripped) = rel_str.strip_suffix(".html") {
            if stripped == "index" {
                // Root index
                urls.insert("/".to_string());
            } else if let Some(dir) = stripped.strip_suffix("/index") {
                // Directory index: dist/posts/index.html → /posts/ and /posts
                urls.insert(format!("/{dir}/"));
                urls.insert(format!("/{dir}"));
            } else {
                // Regular page: dist/posts/hello-world.html → /posts/hello-world
                urls.insert(format!("/{stripped}"));
            }
        }
    }

    urls
}

/// Extract all internal link hrefs from an HTML string.
///
/// Finds `href="/..."` values (both single and double quotes), strips fragments
/// and query strings, and returns deduplicated paths. Only paths starting with
/// `/` are considered internal links. External URLs, anchors, and relative paths
/// are ignored. See [`extract_internal_refs`] for the tag-aware variant that
/// also covers asset references.
pub fn extract_internal_links(html: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Find next "href="
        match html[pos..].find("href=") {
            Some(idx) => {
                let attr_start = pos + idx + 5; // position after "href="
                if attr_start >= len {
                    break;
                }
                let quote = bytes[attr_start];
                if quote == b'"' || quote == b'\'' {
                    let val_start = attr_start + 1;
                    if let Some(end_offset) = html[val_start..].find(quote as char) {
                        let href = &html[val_start..val_start + end_offset];
                        if let Some(href) = normalize_internal_ref(href) {
                            if seen.insert(href.clone()) {
                                links.push(href);
                            }
                        }
                        pos = val_start + end_offset + 1;
                        continue;
                    }
                }
                pos = attr_start + 1;
            }
            None => break,
        }
    }

    links
}

/// One root-relative reference found in an HTML page.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InternalRef {
    /// Path with fragment and query stripped, e.g. `/static/logo.png`.
    pub url: String,
    pub kind: LinkKind,
}

/// Normalize an attribute value into an internal path: unescape `&amp;`,
/// require a root-relative URL (not `//host`), and strip fragment + query.
/// `/favicon.ico` is skipped — every bundled theme links it but the file is
/// optional (users add it to `public/` when they have one).
fn normalize_internal_ref(raw: &str) -> Option<String> {
    let value = raw.trim();
    if !value.starts_with('/') || value.starts_with("//") {
        return None;
    }
    let value = value.replace("&amp;", "&");
    let path = value.split(['#', '?']).next().unwrap_or("");
    if path.is_empty() || path == "/favicon.ico" {
        return None;
    }
    Some(path.to_string())
}

/// `rel` values on `<link>` that load a resource (an asset), as opposed to
/// pointing at another document (`alternate`, `canonical`, `next`, ...).
const ASSET_LINK_RELS: &[&str] = &[
    "stylesheet",
    "icon",
    "shortcut",
    "apple-touch-icon",
    "mask-icon",
    "manifest",
    "preload",
    "modulepreload",
    "prefetch",
];

/// Extract root-relative page links and asset references from HTML.
///
/// Tag-aware: pages come from `<a href>`/`<area href>`/`<link href>` (for
/// document rels); assets from `<img src|srcset>`, `<source src|srcset>`,
/// `<link rel=stylesheet|icon|preload|… href>`, `<script src>`,
/// `<video src|poster>`, `<audio src>`, `<track src>`, `<embed src>`, and SVG
/// `<use|image href>`. Comments and `<script>`/`<style>` bodies are skipped;
/// external, protocol-relative, `data:` and relative URLs are ignored.
/// Results are deduplicated per (url, kind).
pub fn extract_internal_refs(html: &str) -> Vec<InternalRef> {
    let mut out = Vec::new();
    let mut seen: HashSet<(String, LinkKind)> = HashSet::new();
    let mut push = |raw: &str, kind: LinkKind| {
        if let Some(url) = normalize_internal_ref(raw) {
            if seen.insert((url.clone(), kind)) {
                out.push(InternalRef { url, kind });
            }
        }
    };

    let bytes = html.as_bytes();
    let mut pos = 0;
    while let Some(off) = html[pos..].find('<') {
        let start = pos + off;
        let rest = &html[start..];
        if rest.starts_with("<!--") {
            match rest.find("-->") {
                Some(end) => {
                    pos = start + end + 3;
                    continue;
                }
                None => break,
            }
        }
        // Only treat `<` followed by a letter as an opening tag.
        if !bytes
            .get(start + 1)
            .is_some_and(|b| b.is_ascii_alphabetic())
        {
            pos = start + 1;
            continue;
        }
        let Some(end) = find_tag_end(html, start) else {
            break;
        };
        let tag = &html[start + 1..end];
        pos = end + 1;

        let name_end = tag
            .find(|c: char| c.is_ascii_whitespace() || c == '/')
            .unwrap_or(tag.len());
        let name = tag[..name_end].to_ascii_lowercase();
        let attrs = parse_attrs(&tag[name_end..]);
        let attr = |n: &str| {
            attrs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(n))
                .map(|(_, v)| *v)
        };

        match name.as_str() {
            "a" | "area" => {
                if let Some(v) = attr("href") {
                    push(v, LinkKind::Page);
                }
            }
            "link" => {
                if let Some(v) = attr("href") {
                    let rel = attr("rel").unwrap_or("").to_ascii_lowercase();
                    let is_asset = rel
                        .split_ascii_whitespace()
                        .any(|r| ASSET_LINK_RELS.contains(&r));
                    push(
                        v,
                        if is_asset {
                            LinkKind::Asset
                        } else {
                            LinkKind::Page
                        },
                    );
                }
            }
            "img" | "source" => {
                if let Some(v) = attr("src") {
                    push(v, LinkKind::Asset);
                }
                if let Some(v) = attr("srcset") {
                    for url in parse_srcset(v) {
                        push(url, LinkKind::Asset);
                    }
                }
            }
            "script" | "audio" | "track" | "embed" => {
                if let Some(v) = attr("src") {
                    push(v, LinkKind::Asset);
                }
            }
            "video" => {
                if let Some(v) = attr("src") {
                    push(v, LinkKind::Asset);
                }
                if let Some(v) = attr("poster") {
                    push(v, LinkKind::Asset);
                }
            }
            "use" | "image" => {
                if let Some(v) = attr("href") {
                    push(v, LinkKind::Asset);
                }
            }
            "iframe" => {}
            _ => {
                if let Some(v) = attr("href") {
                    push(v, LinkKind::Page);
                }
            }
        }

        // Skip raw-text element bodies so inline JS/CSS isn't parsed as tags.
        if name == "script" || name == "style" {
            let close = format!("</{name}");
            match find_ascii_ci(&html[pos..], &close) {
                Some(i) => pos += i,
                None => break,
            }
        }
    }

    out
}

/// Case-insensitive ASCII substring search.
fn find_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

/// Find the `>` that closes the tag starting at `start`, honoring quotes.
fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    let mut in_quote: Option<u8> = None;
    for (i, &b) in bytes.iter().enumerate().skip(start + 1) {
        match in_quote {
            Some(q) if b == q => in_quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => in_quote = Some(b),
            None if b == b'>' => return Some(i),
            None => {}
        }
    }
    None
}

/// Parse `name="value"` / `name='value'` / `name=value` / bare `name`
/// attributes from the inside of a tag (after the tag name).
fn parse_attrs(s: &str) -> Vec<(&str, &str)> {
    let bytes = s.as_bytes();
    let mut attrs = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'/'
            && bytes[i] != b'>'
        {
            i += 1;
        }
        let name = &s[name_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = "";
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let q = bytes[i];
                let v_start = i + 1;
                let v_end = s[v_start..]
                    .find(q as char)
                    .map(|e| v_start + e)
                    .unwrap_or(bytes.len());
                value = &s[v_start..v_end];
                i = (v_end + 1).min(bytes.len());
            } else {
                let v_start = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                value = &s[v_start..i];
            }
        }
        if !name.is_empty() {
            attrs.push((name, value));
        } else if i == name_start {
            // Unparseable byte; skip it to guarantee progress.
            i += 1;
        }
    }
    attrs
}

/// Extract the URLs from a `srcset` value (`url [descriptor], url [descriptor]`).
///
/// URLs are split on whitespace rather than commas so `data:` URIs (which
/// contain commas) survive; a trailing comma on a URL ends the candidate.
pub fn parse_srcset(srcset: &str) -> Vec<&str> {
    let bytes = srcset.as_bytes();
    let mut urls = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut url = &srcset[start..i];
        let ended_candidate = url.ends_with(',');
        url = url.trim_end_matches(',');
        if !url.is_empty() {
            urls.push(url);
        }
        if !ended_candidate {
            // Skip descriptors up to the next comma.
            while i < bytes.len() && bytes[i] != b',' {
                i += 1;
            }
        }
    }
    urls
}

/// Strip a site base path (e.g. `/repo`) from a root-relative URL so it can be
/// compared against output files. `/repo` → `/`, `/repo/x` → `/x`; URLs that
/// don't carry the prefix are returned unchanged.
pub fn strip_base_path<'a>(url: &'a str, base_path: &str) -> &'a str {
    if base_path.is_empty() {
        return url;
    }
    match url.strip_prefix(base_path) {
        Some("") => "/",
        Some(rest) if rest.starts_with('/') => rest,
        _ => url,
    }
}

/// Per-page result of [`check_page_refs`].
#[derive(Debug, Default)]
pub struct PageRefCheck {
    pub checked: usize,
    pub broken_links: Vec<BrokenLink>,
    pub missing_assets: Vec<BrokenLink>,
}

/// Validate every internal reference in one generated page against the set
/// of valid output URLs. `rel_path` is the page's path relative to the output
/// directory; `base_path` is stripped from references before lookup.
pub fn check_page_refs(
    html: &str,
    rel_path: &str,
    valid_urls: &HashSet<String>,
    base_path: &str,
) -> PageRefCheck {
    let refs = extract_internal_refs(html);
    let mut result = PageRefCheck {
        checked: refs.len(),
        ..Default::default()
    };
    for r in refs {
        let path = strip_base_path(&r.url, base_path);
        // Optional favicon (see `normalize_internal_ref`), after base-path rewriting.
        if path == "/favicon.ico" || url_exists(path, valid_urls) {
            continue;
        }
        let link = BrokenLink::new(rel_path, path, r.kind);
        match r.kind {
            LinkKind::Asset => result.missing_assets.push(link),
            _ => result.broken_links.push(link),
        }
    }
    result
}

fn url_exists(path: &str, valid_urls: &HashSet<String>) -> bool {
    if valid_urls.contains(path) {
        return true;
    }
    if path.contains('%') {
        if let Ok(decoded) = urlencoding::decode(path) {
            return valid_urls.contains(decoded.as_ref());
        }
    }
    false
}

/// Fill `suggestion` on each link with the closest valid URL (by edit
/// distance) when one is close enough to be a likely typo. Pages are matched
/// against clean page URLs, assets against file URLs.
pub fn fill_suggestions(links: &mut [BrokenLink], valid_urls: &HashSet<String>) {
    if links.is_empty() {
        return;
    }
    let mut pages: Vec<&str> = Vec::new();
    let mut files: Vec<&str> = Vec::new();
    for url in valid_urls {
        let last = url.rsplit('/').next().unwrap_or("");
        if last.contains('.') {
            if !url.ends_with(".html") && !url.ends_with(".md") {
                files.push(url);
            }
        } else {
            pages.push(url);
        }
    }
    // Deterministic tie-breaking.
    pages.sort_unstable();
    files.sort_unstable();

    let mut cache: HashMap<(String, LinkKind), Option<String>> = HashMap::new();
    for link in links.iter_mut() {
        if link.kind == LinkKind::Markdown {
            continue;
        }
        let key = (link.href.clone(), link.kind);
        let suggestion = cache
            .entry(key)
            .or_insert_with(|| {
                let candidates = if link.kind == LinkKind::Asset {
                    &files
                } else {
                    &pages
                };
                closest_url(&link.href, candidates)
            })
            .clone();
        link.suggestion = suggestion;
    }
}

fn closest_url(href: &str, candidates: &[&str]) -> Option<String> {
    let trimmed = href.trim_end_matches('/');
    let max = (trimmed.chars().count() / 4).clamp(1, 3);
    let mut best: Option<(usize, &str)> = None;
    for &c in candidates {
        let ct = c.trim_end_matches('/');
        if ct.len().abs_diff(trimmed.len()) > max || ct == trimmed {
            continue;
        }
        let d = strsim::levenshtein(trimmed, ct);
        if d <= max && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c.to_string())
}

/// Where to look when attributing broken links to source files.
pub struct SourceAttribution<'a> {
    /// Site root; reported paths are relative to it.
    pub root: &'a Path,
    /// Output-relative HTML path (e.g. "posts/a.html") → markdown source file.
    pub page_sources: &'a HashMap<String, PathBuf>,
    /// Searched in order for links that aren't in the page's own markdown
    /// (the build passes user templates, data files such as `data/nav.yaml`,
    /// then the content dir for post excerpts shown on listing pages).
    pub fallback_dirs: &'a [PathBuf],
    /// Final output directory relative to the site root (e.g. "dist").
    pub output_display: &'a str,
}

/// Largest file searched for link text during attribution.
const MAX_ATTRIBUTION_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Trace each broken link back to where it was written.
///
/// First looks for the link in the markdown source of the page it was found
/// on; failing that, marks it `from_template` and searches user templates and
/// data files (first match wins). Links that already have a `source` (e.g.
/// unresolved `.md` links found during rendering) are left as they are.
pub fn attribute_sources(links: &mut [BrokenLink], ctx: &SourceAttribution) {
    let mut texts: HashMap<PathBuf, Option<String>> = HashMap::new();
    let mut fallback_files: Option<Vec<PathBuf>> = None;

    fn read_text<'c>(
        cache: &'c mut HashMap<PathBuf, Option<String>>,
        path: &Path,
    ) -> Option<&'c str> {
        cache
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let meta = fs::metadata(path).ok()?;
                if !meta.is_file() || meta.len() > MAX_ATTRIBUTION_FILE_BYTES {
                    return None;
                }
                fs::read_to_string(path).ok()
            })
            .as_deref()
    }

    for link in links.iter_mut() {
        link.output_display = ctx.output_display.to_string();
        if link.source.is_some() {
            continue;
        }
        if let Some(src) = ctx.page_sources.get(&link.source_file) {
            if let Some(line) =
                read_text(&mut texts, src).and_then(|t| find_link_line(t, &link.href))
            {
                link.source = Some(relative_display(ctx.root, src));
                link.line = Some(line);
                link.from_template = false;
                continue;
            }
        }
        link.from_template = true;
        let files = fallback_files.get_or_insert_with(|| {
            let mut files = Vec::new();
            for dir in ctx.fallback_dirs {
                if !dir.is_dir() {
                    continue;
                }
                let mut in_dir: Vec<PathBuf> = WalkDir::new(dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .map(|e| e.into_path())
                    .collect();
                in_dir.sort();
                files.extend(in_dir);
            }
            files
        });
        for file in files.iter() {
            if let Some(line) =
                read_text(&mut texts, file).and_then(|t| find_link_line(t, &link.href))
            {
                link.source = Some(relative_display(ctx.root, file));
                link.line = Some(line);
                break;
            }
        }
    }
}

/// `path` relative to `root`, with forward slashes.
pub fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 1-based line of the first occurrence of `href` in `text` that stands on
/// its own as a URL (not a prefix of a longer path such as `/docs/a` inside
/// `/docs/about`, nor a suffix such as `./x.md` inside `../x.md`).
/// Percent-encoded hrefs are also searched in decoded form.
pub fn find_link_line(text: &str, href: &str) -> Option<usize> {
    let mut needles = vec![href.to_string()];
    if href.contains('%') {
        if let Ok(decoded) = urlencoding::decode(href) {
            if decoded != href {
                needles.push(decoded.into_owned());
            }
        }
    }
    needles
        .iter()
        .filter_map(|n| find_standalone(text, n))
        .min()
        .map(|offset| text[..offset].matches('\n').count() + 1)
}

fn find_standalone(text: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let bytes = text.as_bytes();
    let is_path_byte =
        |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'~' | b'%');
    let mut from = 0;
    while let Some(off) = text[from..].find(needle) {
        let start = from + off;
        let end = start + needle.len();
        let prev_ok = start == 0 || {
            let p = bytes[start - 1];
            !(is_path_byte(p) || p == b'.')
        };
        let next_ok = match bytes.get(end) {
            None => true,
            Some(&b'.') => !bytes
                .get(end + 1)
                .is_some_and(|b| b.is_ascii_alphanumeric()),
            Some(&b) => !is_path_byte(b),
        };
        if prev_ok && next_ok {
            return Some(start);
        }
        from = start + 1;
        while !text.is_char_boundary(from) {
            from += 1;
        }
    }
    None
}

/// Rewrite internal links that target subdomain collections to absolute URLs.
///
/// Given a map of URL path prefixes to absolute base URLs, scans `href="..."` values
/// and rewrites matching paths. For example, with `{"/docs" => "https://docs.example.com"}`:
/// - `href="/docs/setup"` → `href="https://docs.example.com/setup"`
/// - `href="/docs/"` → `href="https://docs.example.com/"`
/// - `href="/docs"` → `href="https://docs.example.com"`
///
/// Fragments and query strings are preserved. Only `href` attributes are rewritten.
pub fn rewrite_subdomain_links(html: &str, rewrites: &HashMap<String, String>) -> String {
    if rewrites.is_empty() {
        return html.to_string();
    }

    let mut result = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        match html[pos..].find("href=") {
            Some(idx) => {
                let attr_start = pos + idx + 5;
                // Copy everything up to and including "href="
                result.push_str(&html[pos..attr_start]);

                if attr_start >= len {
                    break;
                }

                let quote = bytes[attr_start];
                if quote == b'"' || quote == b'\'' {
                    result.push(quote as char);
                    let val_start = attr_start + 1;
                    if let Some(end_offset) = html[val_start..].find(quote as char) {
                        let href = &html[val_start..val_start + end_offset];

                        if let Some(rewritten) = rewrite_href(href, rewrites) {
                            result.push_str(&rewritten);
                        } else {
                            result.push_str(href);
                        }

                        result.push(quote as char);
                        pos = val_start + end_offset + 1;
                    } else {
                        // No closing quote, copy as-is
                        pos = val_start;
                    }
                } else {
                    pos = attr_start;
                }
            }
            None => {
                result.push_str(&html[pos..]);
                break;
            }
        }
    }

    result
}

/// Try to rewrite a single href value using the subdomain rewrite map.
/// Returns `Some(rewritten)` if the href matched a prefix, `None` otherwise.
fn rewrite_href(href: &str, rewrites: &HashMap<String, String>) -> Option<String> {
    // Only rewrite root-relative paths, skip absolute URLs
    if !href.starts_with('/') || href.starts_with("//") {
        return None;
    }

    // Split off fragment and query for preservation
    let (path, suffix) = split_href_suffix(href);

    for (prefix, base_url) in rewrites {
        if path == prefix {
            // Exact match: /docs → https://docs.example.com
            return Some(format!("{base_url}{suffix}"));
        }
        if let Some(rest) = path.strip_prefix(prefix) {
            if rest.starts_with('/') {
                // Path match: /docs/setup → https://docs.example.com/setup
                return Some(format!("{base_url}{rest}{suffix}"));
            }
        }
    }

    None
}

/// Split an href into the path portion and the suffix (fragment + query).
/// Returns (path, suffix) where suffix includes the leading `#` or `?`.
fn split_href_suffix(href: &str) -> (&str, &str) {
    // Find the earliest fragment or query marker
    let frag_pos = href.find('#');
    let query_pos = href.find('?');

    let split_pos = match (frag_pos, query_pos) {
        (Some(f), Some(q)) => Some(f.min(q)),
        (Some(f), None) => Some(f),
        (None, Some(q)) => Some(q),
        (None, None) => None,
    };

    match split_pos {
        Some(pos) => (&href[..pos], &href[pos..]),
        None => (href, ""),
    }
}

/// Distinct occurrences: one per (href, file, line), so a broken link in a
/// shared template (or a post excerpt shown on listing pages) is reported
/// once rather than once per generated page. Occurrences in a page's own
/// markdown sort before template/listing ones. Sorted by href.
pub fn distinct_links(links: &[BrokenLink]) -> Vec<&BrokenLink> {
    let mut sorted: Vec<(&BrokenLink, String)> = links.iter().map(|l| (l, l.file())).collect();
    sorted.sort_by(|(a, af), (b, bf)| {
        (a.href.as_str(), a.from_template, af, a.line).cmp(&(
            b.href.as_str(),
            b.from_template,
            bf,
            b.line,
        ))
    });
    let mut seen: HashSet<(&str, String, Option<usize>)> = HashSet::new();
    sorted
        .into_iter()
        .filter(|(l, file)| seen.insert((l.href.as_str(), file.clone(), l.line)))
        .map(|(l, _)| l)
        .collect()
}

/// Group broken links by href. Each group lists the distinct places the
/// target is linked from (see [`BrokenLink::location`] and [`distinct_links`]).
pub fn group_broken_links(broken: &[BrokenLink]) -> Vec<(String, Vec<String>)> {
    group_by_target(broken)
        .into_iter()
        .map(|(href, links)| (href, links.iter().map(|l| l.location()).collect()))
        .collect()
}

/// Group [`distinct_links`] by href (sorted by href).
pub fn group_by_target(broken: &[BrokenLink]) -> Vec<(String, Vec<&BrokenLink>)> {
    let mut grouped: Vec<(String, Vec<&BrokenLink>)> = Vec::new();
    for link in distinct_links(broken) {
        match grouped.last_mut() {
            Some((href, group)) if *href == link.href => group.push(link),
            _ => grouped.push((link.href.clone(), vec![link])),
        }
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_internal_links_basic() {
        let html = r#"<a href="/posts/hello">Hello</a> <a href="/about">About</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/posts/hello", "/about"]);
    }

    #[test]
    fn test_extract_internal_links_single_quotes() {
        let html = "<a href='/posts/hello'>Hello</a>";
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/posts/hello"]);
    }

    #[test]
    fn test_extract_internal_links_strips_fragment() {
        let html = r#"<a href="/posts/hello#section-1">Hello</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/posts/hello"]);
    }

    #[test]
    fn test_extract_internal_links_strips_query() {
        let html = r#"<a href="/search?q=test">Search</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/search"]);
    }

    #[test]
    fn test_extract_internal_links_ignores_external() {
        let html = r#"<a href="https://example.com">External</a> <a href="/internal">Internal</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/internal"]);
    }

    #[test]
    fn test_extract_internal_links_ignores_protocol_relative() {
        let html = r#"<a href="//cdn.example.com/lib.js">CDN</a>"#;
        let links = extract_internal_links(html);
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_internal_links_ignores_relative() {
        let html = r#"<a href="relative-page">Relative</a>"#;
        let links = extract_internal_links(html);
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_internal_links_deduplicates() {
        let html = r#"<a href="/about">About</a> <a href="/about">About again</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/about"]);
    }

    #[test]
    fn test_extract_internal_links_includes_non_anchor_hrefs() {
        // link tags for stylesheets, canonical, etc. also have href
        let html = r#"<link rel="stylesheet" href="/static/style.css"><a href="/about">About</a>"#;
        let links = extract_internal_links(html);
        assert_eq!(links, vec!["/static/style.css", "/about"]);
    }

    #[test]
    fn test_build_valid_urls() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = tmp.path();

        // Create file structure
        fs::create_dir_all(out.join("posts")).unwrap();
        fs::create_dir_all(out.join("tags/rust")).unwrap();
        fs::create_dir_all(out.join("static")).unwrap();

        fs::write(out.join("index.html"), "<html></html>").unwrap();
        fs::write(out.join("posts/hello-world.html"), "<html></html>").unwrap();
        fs::write(out.join("posts/index.html"), "<html></html>").unwrap();
        fs::write(out.join("tags/rust/index.html"), "<html></html>").unwrap();
        fs::write(out.join("feed.xml"), "<rss></rss>").unwrap();
        fs::write(out.join("static/style.css"), "body{}").unwrap();

        let urls = build_valid_urls(out);

        // Root index
        assert!(urls.contains("/"));
        assert!(urls.contains("/index.html"));

        // Regular page (clean URL)
        assert!(urls.contains("/posts/hello-world"));
        assert!(urls.contains("/posts/hello-world.html"));

        // Directory index
        assert!(urls.contains("/posts/"));
        assert!(urls.contains("/posts"));
        assert!(urls.contains("/posts/index.html"));

        // Nested directory index
        assert!(urls.contains("/tags/rust/"));
        assert!(urls.contains("/tags/rust"));

        // Non-HTML files
        assert!(urls.contains("/feed.xml"));
        assert!(urls.contains("/static/style.css"));
    }

    #[test]
    fn test_check_internal_links_all_valid() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = tmp.path();

        fs::create_dir_all(out.join("posts")).unwrap();
        fs::write(out.join("index.html"), r#"<a href="/posts/hello">link</a>"#).unwrap();
        fs::write(out.join("posts/hello.html"), r#"<a href="/">home</a>"#).unwrap();

        let result = check_internal_links(out).unwrap();
        assert_eq!(result.total_links_checked, 2);
        assert!(result.broken_links.is_empty());
    }

    #[test]
    fn test_check_internal_links_detects_broken() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = tmp.path();

        fs::write(
            out.join("index.html"),
            r#"<a href="/nonexistent">broken</a> <a href="/also-missing">also broken</a>"#,
        )
        .unwrap();

        let result = check_internal_links(out).unwrap();
        assert_eq!(result.total_links_checked, 2);
        assert_eq!(result.broken_links.len(), 2);
    }

    #[test]
    fn test_group_broken_links() {
        let broken = vec![
            BrokenLink::new("index.html", "/missing", LinkKind::Page),
            BrokenLink::new("about.html", "/missing", LinkKind::Page),
            BrokenLink::new("index.html", "/other", LinkKind::Page),
        ];
        let grouped = group_broken_links(&broken);
        assert_eq!(grouped.len(), 2);
        // Sorted alphabetically by href
        assert_eq!(grouped[0].0, "/missing");
        assert_eq!(grouped[0].1.len(), 2);
        assert_eq!(grouped[1].0, "/other");
        assert_eq!(grouped[1].1.len(), 1);
    }

    fn make_rewrites(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_rewrite_subdomain_links_basic() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/docs/setup">Setup</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(
            result,
            r#"<a href="https://docs.example.com/setup">Setup</a>"#
        );
    }

    #[test]
    fn test_rewrite_subdomain_links_index() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);

        let html = r#"<a href="/docs/">Docs</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, r#"<a href="https://docs.example.com/">Docs</a>"#);

        let html = r#"<a href="/docs">Docs</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, r#"<a href="https://docs.example.com">Docs</a>"#);
    }

    #[test]
    fn test_rewrite_subdomain_links_preserves_fragment() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/docs/setup#section-1">Setup</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(
            result,
            r#"<a href="https://docs.example.com/setup#section-1">Setup</a>"#
        );
    }

    #[test]
    fn test_rewrite_subdomain_links_preserves_query() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/docs/search?q=test">Search</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(
            result,
            r#"<a href="https://docs.example.com/search?q=test">Search</a>"#
        );
    }

    #[test]
    fn test_rewrite_subdomain_links_skips_non_matching() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/posts/hello">Hello</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_subdomain_links_empty_map() {
        let rewrites = HashMap::new();
        let html = r#"<a href="/docs/setup">Setup</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_subdomain_links_multiple_prefixes() {
        let rewrites = make_rewrites(&[
            ("/docs", "https://docs.example.com"),
            ("/blog", "https://blog.example.com"),
        ]);
        let html = r#"<a href="/docs/setup">Docs</a> <a href="/blog/hello">Blog</a> <a href="/about">About</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(
            result,
            r#"<a href="https://docs.example.com/setup">Docs</a> <a href="https://blog.example.com/hello">Blog</a> <a href="/about">About</a>"#
        );
    }

    #[test]
    fn test_rewrite_subdomain_links_skips_external() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="https://example.com/docs/setup">External</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_subdomain_links_single_quotes() {
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = "<a href='/docs/setup'>Setup</a>";
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, "<a href='https://docs.example.com/setup'>Setup</a>");
    }

    #[test]
    fn test_rewrite_subdomain_links_no_false_prefix_match() {
        // /docs-extra should NOT match /docs prefix
        let rewrites = make_rewrites(&[("/docs", "https://docs.example.com")]);
        let html = r#"<a href="/docs-extra/page">Page</a>"#;
        let result = rewrite_subdomain_links(html, &rewrites);
        assert_eq!(result, html);
    }

    fn refs(html: &str) -> Vec<(String, LinkKind)> {
        extract_internal_refs(html)
            .into_iter()
            .map(|r| (r.url, r.kind))
            .collect()
    }

    #[test]
    fn test_extract_internal_refs_classifies_pages_and_assets() {
        let html = r#"<html><head>
<link rel="stylesheet" href="/static/style.css">
<link rel="alternate" type="application/rss+xml" href="/feed.xml">
<link rel="icon" href="/static/icon.svg">
<script src="/static/app.js"></script>
</head><body>
<a href="/about#team">About</a>
<img src="/static/a.png" alt="a">
<picture><source srcset="/static/a-480w.webp 480w, /static/a-800w.webp 800w" type="image/webp"></picture>
<video src="/media/v.mp4" poster="/static/poster.jpg"></video>
<audio src="/media/a.mp3"></audio>
</body></html>"#;
        assert_eq!(
            refs(html),
            vec![
                ("/static/style.css".into(), LinkKind::Asset),
                ("/feed.xml".into(), LinkKind::Page),
                ("/static/icon.svg".into(), LinkKind::Asset),
                ("/static/app.js".into(), LinkKind::Asset),
                ("/about".into(), LinkKind::Page),
                ("/static/a.png".into(), LinkKind::Asset),
                ("/static/a-480w.webp".into(), LinkKind::Asset),
                ("/static/a-800w.webp".into(), LinkKind::Asset),
                ("/media/v.mp4".into(), LinkKind::Asset),
                ("/static/poster.jpg".into(), LinkKind::Asset),
                ("/media/a.mp3".into(), LinkKind::Asset),
            ]
        );
    }

    #[test]
    fn test_extract_internal_refs_ignores_external_data_and_script_bodies() {
        let html = r#"<img src="https://cdn.example.com/x.png">
<img src="data:image/png;base64,AAAA">
<img src="//cdn.example.com/y.png">
<img src="relative.png">
<!-- <img src="/static/commented.png"> -->
<script>var s = '<img src="/static/in-js.png">';</script>
<style>.a{background:url('/static/bg.png')}</style>
<pre><code>&lt;img src=&quot;/static/code.png&quot;&gt;</code></pre>
<link rel="icon" href="/favicon.ico">
<IMG SRC='/static/upper.png'>"#;
        assert_eq!(
            refs(html),
            vec![("/static/upper.png".into(), LinkKind::Asset)]
        );
    }

    #[test]
    fn test_parse_srcset() {
        assert_eq!(
            parse_srcset("/a-480w.webp 480w, /a-800w.webp 800w"),
            vec!["/a-480w.webp", "/a-800w.webp"]
        );
        assert_eq!(parse_srcset("/a.png"), vec!["/a.png"]);
        assert_eq!(parse_srcset("/a.png, /b.png 2x"), vec!["/a.png", "/b.png"]);
        assert_eq!(
            parse_srcset("data:image/png;base64,AA,BB 1x, /hi.png 2x"),
            vec!["data:image/png;base64,AA,BB", "/hi.png"]
        );
        assert!(parse_srcset("  ").is_empty());
    }

    #[test]
    fn test_strip_base_path() {
        assert_eq!(strip_base_path("/repo/docs/x", "/repo"), "/docs/x");
        assert_eq!(strip_base_path("/repo", "/repo"), "/");
        assert_eq!(strip_base_path("/repository/x", "/repo"), "/repository/x");
        assert_eq!(strip_base_path("/docs/x", ""), "/docs/x");
    }

    fn valid(urls: &[&str]) -> HashSet<String> {
        urls.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_check_page_refs_splits_links_and_assets_with_base_path() {
        let html = r#"<a href="/repo/about">ok</a><a href="/repo/gone">x</a>
<img src="/repo/static/ok.png"><img src="/repo/static/nope.png" srcset="/repo/static/ok.png 1x, /repo/static/nope-2x.png 2x">
<link rel="icon" href="/repo/favicon.ico">"#;
        let urls = valid(&["/about", "/static/ok.png"]);
        let result = check_page_refs(html, "posts/a.html", &urls, "/repo");
        let broken: Vec<_> = result
            .broken_links
            .iter()
            .map(|l| l.href.as_str())
            .collect();
        let missing: Vec<_> = result
            .missing_assets
            .iter()
            .map(|l| l.href.as_str())
            .collect();
        assert_eq!(broken, vec!["/gone"]);
        assert_eq!(missing, vec!["/static/nope.png", "/static/nope-2x.png"]);
        assert!(result
            .missing_assets
            .iter()
            .all(|l| l.kind == LinkKind::Asset && l.source_file == "posts/a.html"));
    }

    #[test]
    fn test_check_page_refs_accepts_percent_encoded_files() {
        let urls = valid(&["/static/my photo.png"]);
        let result = check_page_refs(r#"<img src="/static/my%20photo.png">"#, "a.html", &urls, "");
        assert!(result.missing_assets.is_empty());
    }

    #[test]
    fn test_fill_suggestions_did_you_mean() {
        let urls = valid(&[
            "/docs/getting-started",
            "/docs/getting-started.html",
            "/static/logo.png",
            "/",
        ]);
        let mut links = vec![
            BrokenLink::new("a.html", "/docs/getting-startd", LinkKind::Page),
            BrokenLink::new("a.html", "/totally/unrelated/path", LinkKind::Page),
            BrokenLink::new("a.html", "/static/lgo.png", LinkKind::Asset),
        ];
        fill_suggestions(&mut links, &urls);
        assert_eq!(
            links[0].suggestion.as_deref(),
            Some("/docs/getting-started")
        );
        assert_eq!(links[1].suggestion, None);
        assert_eq!(links[2].suggestion.as_deref(), Some("/static/logo.png"));
    }

    #[test]
    fn test_find_link_line() {
        let text = "---\ntitle: T\n---\n\nSee [about](/docs/about).\n\n[a](/docs/a#x)\n";
        assert_eq!(find_link_line(text, "/docs/about"), Some(5));
        // `/docs/a` must not match inside `/docs/about`.
        assert_eq!(find_link_line(text, "/docs/a"), Some(7));
        assert_eq!(find_link_line(text, "/missing"), None);

        // `./x.md` must not match inside `../x.md`.
        let text = "[up](../x.md)\n[here](./x.md)\n";
        assert_eq!(find_link_line(text, "./x.md"), Some(2));
        assert_eq!(find_link_line(text, "../x.md"), Some(1));

        // Sentence punctuation after a bare URL still matches; a longer
        // file name does not.
        assert_eq!(find_link_line("go to /about.\n", "/about"), Some(1));
        assert_eq!(find_link_line("/about.html\n", "/about"), None);

        // Percent-encoded hrefs match their decoded source text.
        assert_eq!(
            find_link_line("x\n![p](/static/my photo.png)", "/static/my%20photo.png"),
            Some(2)
        );
    }

    #[test]
    fn test_attribute_sources_prefers_page_source_then_templates() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("content/posts")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("data")).unwrap();
        let post = root.join("content/posts/a.md");
        fs::write(&post, "---\ntitle: A\n---\n\nIntro\n\n[x](/in-content)\n").unwrap();
        fs::write(
            root.join("templates/base.html"),
            "<html>\n<body>\n<a href=\"/in-template\">x</a>\n",
        )
        .unwrap();
        fs::write(root.join("data/nav.yaml"), "- title: X\n  url: /in-nav\n").unwrap();

        let page_sources: HashMap<String, PathBuf> =
            [("posts/a.html".to_string(), post.clone())].into();
        let fallback = [root.join("templates"), root.join("data")];
        let ctx = SourceAttribution {
            root,
            page_sources: &page_sources,
            fallback_dirs: &fallback,
            output_display: "dist",
        };
        let mut links = vec![
            BrokenLink::new("posts/a.html", "/in-content", LinkKind::Page),
            BrokenLink::new("posts/a.html", "/in-template", LinkKind::Page),
            BrokenLink::new("tags/index.html", "/in-nav", LinkKind::Page),
            BrokenLink::new("tags/index.html", "/nowhere", LinkKind::Page),
        ];
        attribute_sources(&mut links, &ctx);

        assert_eq!(links[0].location(), "content/posts/a.md:7");
        assert!(!links[0].from_template);
        assert_eq!(
            links[1].location(),
            "templates/base.html:3 (from template/listing)"
        );
        assert_eq!(
            links[2].location(),
            "data/nav.yaml:2 (from template/listing)"
        );
        assert_eq!(
            links[3].location(),
            "dist/tags/index.html (from template/listing)"
        );
    }

    #[test]
    fn test_link_diagnostics_codes_severity_and_location() {
        let mut page = BrokenLink::new("posts/a.html", "/docs/getting-startd", LinkKind::Page);
        page.source = Some("content/posts/a.md".into());
        page.line = Some(8);
        page.suggestion = Some("/docs/getting-started".into());
        let mut md = BrokenLink::new("posts/a.html", "./nope.md", LinkKind::Markdown);
        md.source = Some("content/posts/a.md".into());
        md.line = Some(12);
        let mut asset = BrokenLink::new("tags/index.html", "/static/x.png", LinkKind::Asset);
        asset.from_template = true;
        asset.output_display = "dist".into();
        let check = LinkCheckResult {
            total_links_checked: 3,
            broken_links: vec![page, md],
            missing_assets: vec![asset],
        };

        let warnings = link_diagnostics(&check, false);
        assert_eq!(warnings.len(), 3);
        assert!(warnings.iter().all(|d| !d.is_error()));
        assert_eq!(
            warnings[1].to_string(),
            "content/posts/a.md:8: warning[broken-link]: link to `/docs/getting-startd` does not match any page\n  hint: did you mean `/docs/getting-started`?"
        );
        assert!(warnings[0]
            .message
            .contains("does not match any content file"));
        assert_eq!(warnings[0].line, Some(12));
        assert_eq!(warnings[2].code, "missing-asset");
        assert_eq!(
            warnings[2].file.as_deref(),
            Some(Path::new("dist/tags/index.html"))
        );
        assert!(warnings[2].message.contains("from template/listing"));

        let errors = link_diagnostics(&check, true);
        assert!(errors.iter().all(|d| d.is_error()));
    }

    #[test]
    fn test_distinct_links_prefers_page_occurrence_over_listing() {
        // A post excerpt shown on a listing page attributes to the same
        // file + line as the post's own page: report it once, as the post.
        let mut listing = BrokenLink::new("index.html", "/gone", LinkKind::Page);
        listing.source = Some("content/posts/a.md".into());
        listing.line = Some(6);
        listing.from_template = true;
        let mut own = listing.clone();
        own.source_file = "posts/a.html".into();
        own.from_template = false;
        let links = [listing, own];
        let distinct = distinct_links(&links);
        assert_eq!(distinct.len(), 1);
        assert!(!distinct[0].from_template);
        let check = LinkCheckResult {
            total_links_checked: 2,
            broken_links: links.to_vec(),
            missing_assets: Vec::new(),
        };
        assert_eq!(check.problem_count(), 1);
        assert_eq!(link_diagnostics(&check, false).len(), 1);
    }

    #[test]
    fn test_group_broken_links_dedupes_template_locations() {
        let mut a = BrokenLink::new("a.html", "/gone", LinkKind::Page);
        a.source = Some("templates/base.html".into());
        a.line = Some(3);
        a.from_template = true;
        let mut b = a.clone();
        b.source_file = "b.html".into();
        let grouped = group_broken_links(&[a, b]);
        assert_eq!(grouped.len(), 1);
        assert_eq!(
            grouped[0].1,
            vec!["templates/base.html:3 (from template/listing)".to_string()]
        );
    }
}
