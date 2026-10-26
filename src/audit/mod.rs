//! `seite audit`: a free, offline SEO / GEO (AI visibility) / accessibility /
//! content / performance audit of the built site, for agents and CI.
//!
//! The site is built into a scratch directory exactly like `seite check`
//! ([`crate::cli::check::scratch_build`]; `dist/` is never touched), or an
//! existing output directory is read (`--dist`). Every page is scanned once
//! ([`html::scan`]) and the checks in [`seo`], [`geo`], [`a11y`],
//! [`content`] and [`perf`] turn the facts into [`Finding`]s, each with a
//! stable code (documented in `seite-sh/content/docs/errors.md`, listed in
//! [`CODES`]) and — where possible — the source file and line to edit.
//!
//! Findings a template produces on many pages are collapsed into one
//! ("… and N more pages"), so a fix in one file reads as one finding.
//!
//! **Score**: `100 - Σ penalty(category)`, clamped to 0..=100. Each code
//! costs its severity weight (error 10, warning 3, notice 0: notices are
//! reported but never lower the score) per finding, counting at most
//! [`PENALTY_CAP`] findings per code, and a category costs at most
//! [`CATEGORY_CAP`]. One kind of problem
//! can't sink the score on its own, and fixing a kind of problem always
//! shows.

pub mod a11y;
pub mod content;
pub mod geo;
pub mod html;
pub mod perf;
pub mod seo;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::build::links;
use crate::config::{ResolvedPaths, SiteConfig};
use crate::content::schedule::HiddenPage;
use crate::error::{PageError, Result};

/// How serious a finding is (ordered: `Error` < `Warning` < `Notice`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Notice,
}

impl Severity {
    /// Score penalty per finding (see the module docs).
    pub fn weight(self) -> u32 {
        match self {
            Severity::Error => 10,
            Severity::Warning => 3,
            Severity::Notice => 0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Notice => "notice",
        }
    }
}

/// Findings counted per code in the score.
pub const PENALTY_CAP: u32 = 3;

/// Most a category's findings can take off the score.
pub const CATEGORY_CAP: u32 = 30;

/// What a finding is about (`--category`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Seo,
    Geo,
    A11y,
    Content,
    Perf,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Seo,
        Category::Geo,
        Category::A11y,
        Category::Content,
        Category::Perf,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Seo => "seo",
            Category::Geo => "geo",
            Category::A11y => "a11y",
            Category::Content => "content",
            Category::Perf => "perf",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Seo => "SEO",
            Category::Geo => "GEO (AI visibility)",
            Category::A11y => "Accessibility",
            Category::Content => "Content",
            Category::Perf => "Performance",
        }
    }
}

/// A finding code with its fixed category, severity and fix hint.
pub struct CodeInfo {
    pub code: &'static str,
    pub category: Category,
    pub severity: Severity,
    pub hint: &'static str,
}

macro_rules! codes {
    ($($code:literal, $cat:ident, $sev:ident, $hint:literal;)*) => {
        /// Every audit code. Each has a `## <code>` section in the error
        /// reference (enforced by a test).
        pub static CODES: &[CodeInfo] = &[
            $(CodeInfo { code: $code, category: Category::$cat, severity: Severity::$sev, hint: $hint },)*
        ];
    };
}

codes! {
    // SEO
    "seo-title-missing", Seo, Error, "give the page a `title:` in its frontmatter (the theme puts it in <title>)";
    "seo-title-duplicate", Seo, Warning, "make each page's title unique; search engines show one result per title";
    "seo-title-long", Seo, Warning, "keep the <title> (page title plus site name) within 60 characters so it isn't truncated";
    "seo-description-missing", Seo, Warning, "add `description:` (50–160 characters) to the frontmatter; it is the search snippet";
    "seo-description-duplicate", Seo, Warning, "write a description specific to this page";
    "seo-description-long", Seo, Warning, "shorten `description:` to at most 160 characters";
    "seo-description-short", Seo, Notice, "expand `description:` to at least 50 characters that summarize the page";
    "seo-canonical-missing", Seo, Warning, "add <link rel=\"canonical\" href=\"{{ page_url }}\"> to the <head> of the layout template";
    "seo-h1-missing", Seo, Warning, "give the page exactly one <h1> (usually the page title in the template)";
    "seo-h1-multiple", Seo, Warning, "keep one <h1> in <main>; demote the others to <h2>";
    "seo-heading-skip", Seo, Notice, "don't skip heading levels (an <h2> before an <h3>); use `##` before `###`";
    "seo-thin-content", Seo, Warning, "expand the page to at least 150 words, merge it into a related page, or mark it a draft";
    "seo-orphan-page", Seo, Warning, "link to the page from a related page, a listing, or the navigation";
    "seo-broken-anchor", Seo, Warning, "point the link at an existing heading id (see the target page's table of contents) or remove the #fragment";
    "seo-noindex-in-sitemap", Seo, Error, "remove `robots: noindex` from the page, or keep it out of the sitemap";
    "seo-img-dimensions", Seo, Warning, "give the <img> width and height attributes (local images processed by seite get them automatically)";
    "seo-og-image-missing", Seo, Notice, "add `[og]` with `generate = true` to seite.toml (generated cards), or set `image:` in the frontmatter";
    "seo-hreflang", Seo, Warning, "every translation must link back with a matching hreflang; check the translation files exist and the language switcher template";
    "seo-sitemap-missing", Seo, Error, "build the site with seite (it writes sitemap.xml) or restore sitemap.xml in the output";
    "seo-sitemap-stale-url", Seo, Error, "the sitemap lists a URL with no built page; rebuild, or check the page's slug/url";
    "seo-page-not-in-sitemap", Seo, Warning, "rebuild; if a custom sitemap.xml in static/ or public/ replaces the generated one, remove it or add the page";
    // GEO (AI visibility)
    "geo-llms-txt-missing", Geo, Error, "build with seite (it writes llms.txt) or add llms.txt to the output";
    "geo-llms-txt-unlisted", Geo, Warning, "content pages are listed in llms.txt unless their collection is private; check the collection's `listed`/`private` settings";
    "geo-llms-no-summary", Geo, Notice, "add `description:` to the frontmatter; llms.txt uses it as the page summary";
    "geo-markdown-alternate-missing", Geo, Warning, "add <link rel=\"alternate\" type=\"text/markdown\" href=\"{{ page.markdown_url }}\"> to the layout <head>";
    "geo-jsonld-missing", Geo, Warning, "add a <script type=\"application/ld+json\"> block (the bundled themes emit one per page)";
    "geo-jsonld-invalid", Geo, Error, "fix the JSON-LD block so it parses as JSON (escape values with `| json_encode`)";
    "geo-jsonld-no-type", Geo, Warning, "give every JSON-LD object an \"@type\" (e.g. \"BlogPosting\", \"WebPage\")";
    "geo-robots-missing", Geo, Warning, "build with seite (it writes robots.txt) or add robots.txt to the output";
    "geo-robots-blocks-ai-search", Geo, Warning, "allow AI search crawlers (OAI-SearchBot, ChatGPT-User, PerplexityBot, Claude-SearchBot) in robots.txt; block training crawlers instead if needed";
    "geo-content-signal-missing", Geo, Notice, "add a `Content-Signal: search=yes, ai-input=yes, ai-train=no` line to robots.txt (seite writes one by default)";
    // Accessibility
    "a11y-html-lang", A11y, Error, "set <html lang=\"{{ lang }}\"> in the layout template";
    "a11y-img-alt", A11y, Error, "describe the image: `![what it shows](src)` in markdown or alt=\"…\" in templates (alt=\"\" only for decorative images)";
    "a11y-link-empty", A11y, Error, "give the link visible text, or an aria-label for icon-only links";
    "a11y-link-ambiguous", A11y, Warning, "use link text that names the target (\"Read the install guide\"), or add an aria-label";
    "a11y-input-label", A11y, Error, "add a <label for=\"id\">, wrap the control in <label>, or give it an aria-label";
    "a11y-button-name", A11y, Error, "give the button text or an aria-label";
    "a11y-skip-link", A11y, Notice, "add <a href=\"#main\" class=\"skip-link\">Skip to main content</a> as the first element of <body> and id=\"main\" on <main>";
    "a11y-duplicate-id", A11y, Warning, "make every id on the page unique (duplicate heading text gets duplicate ids)";
    // Content
    "content-link-to-draft", Content, Error, "publish the target (remove `draft: true`) or remove the link; drafts are not built";
    "content-link-to-scheduled", Content, Warning, "the target goes live later; the link is broken until then (or point it elsewhere)";
    "content-missing-date", Content, Warning, "add `date: YYYY-MM-DD` to the frontmatter (or a YYYY-MM-DD- filename prefix)";
    // Performance
    "perf-image-large", Perf, Warning, "compress or resize the image below 300 KB (seite makes resized WebP copies of local images when [images] is set)";
    "perf-page-weight", Perf, Warning, "reduce the page's images, scripts and styles below 1.5 MB in total";
    "perf-render-blocking-script", Perf, Warning, "add `defer` (or `async`) to the <script src> in <head>, or move it to the end of <body>";
    "perf-img-lazy", Perf, Notice, "add loading=\"lazy\" to images below the fold ([images] lazy_loading = true does it for markdown images)";
}

/// Details of `code` (`None` for an unknown code).
pub fn code_info(code: &str) -> Option<&'static CodeInfo> {
    CODES.iter().find(|c| c.code == code)
}

/// One audit finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    pub category: Category,
    /// Site-relative URL of the page (`/posts/hello`); `None` for site-wide
    /// findings (robots.txt, sitemap, llms.txt).
    pub page_url: Option<String>,
    /// File to edit, relative to the site root: the content source or
    /// template when it could be traced, else the generated page
    /// (`dist/...`) or site file.
    pub file: Option<String>,
    pub line: Option<usize>,
    pub message: String,
    pub hint: String,
    /// Attributed to a template/data file (shared by many pages).
    pub shared: bool,
}

impl Finding {
    /// A finding with `code`'s category, severity and hint.
    pub fn new(code: &'static str, page_url: Option<&str>, message: impl Into<String>) -> Self {
        let info = code_info(code);
        Self {
            code,
            severity: info.map_or(Severity::Warning, |i| i.severity),
            category: info.map_or(Category::Seo, |i| i.category),
            page_url: page_url.map(str::to_string),
            file: None,
            line: None,
            message: message.into(),
            hint: info.map_or("", |i| i.hint).to_string(),
            shared: false,
        }
    }

    pub fn at(mut self, loc: Loc) -> Self {
        self.file = loc.file;
        self.line = loc.line;
        self.shared = loc.shared;
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    fn sort_key(&self) -> impl Ord + '_ {
        (
            self.category,
            self.code,
            self.page_url.clone().unwrap_or_default(),
            self.file.clone().unwrap_or_default(),
            self.line.unwrap_or(0),
            self.message.clone(),
        )
    }
}

impl Serialize for Finding {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(9))?;
        map.serialize_entry("code", self.code)?;
        map.serialize_entry("severity", &self.severity)?;
        map.serialize_entry("category", &self.category)?;
        map.serialize_entry("page_url", &self.page_url)?;
        map.serialize_entry("file", &self.file)?;
        map.serialize_entry("line", &self.line)?;
        map.serialize_entry("message", &self.message)?;
        map.serialize_entry("hint", &self.hint)?;
        map.serialize_entry("docs_url", &crate::diagnostics::docs_url(self.code))?;
        map.end()
    }
}

/// Where a finding is located.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loc {
    pub file: Option<String>,
    pub line: Option<usize>,
    pub shared: bool,
}

/// Facts about the content item a page was rendered from (build mode).
#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub collection: String,
    pub word_count: usize,
    /// The collection is dated (`has_date`).
    pub dated: bool,
    pub date: Option<chrono::NaiveDate>,
    pub is_index: bool,
    pub description: Option<String>,
    pub lang: String,
}

/// One generated HTML page.
pub struct Page {
    /// Output-relative path (`posts/hello.html`).
    pub rel: String,
    /// Site-relative URL (`/posts/hello`, `/` for the homepage).
    pub url: String,
    pub html: String,
    pub facts: html::PageFacts,
    pub bytes: u64,
    /// Absolute path of the markdown source (build mode).
    pub source: Option<PathBuf>,
    pub item: Option<ItemInfo>,
}

impl Page {
    /// An alias redirect page or the 404 page: not audited as content.
    pub fn is_auxiliary(&self) -> bool {
        self.facts.refresh || self.rel == "404.html"
    }

    /// The homepage, a collection index, a listing (tags, categories,
    /// series), or a paginated page: an `index.html`.
    pub fn is_listing(&self) -> bool {
        self.rel == "index.html"
            || self.rel.ends_with("/index.html")
            || self.item.as_ref().is_some_and(|i| i.is_index)
    }

    /// A page rendered from a content file (build mode), or — reading an
    /// existing output dir — a non-listing page with a markdown alternate.
    pub fn is_content(&self, build_mode: bool) -> bool {
        if self.is_auxiliary() {
            return false;
        }
        if build_mode {
            self.item.as_ref().is_some_and(|i| !i.is_index)
        } else {
            !self.is_listing() && self.facts.markdown_alternate.is_some()
        }
    }

    pub fn noindex(&self) -> bool {
        self.facts
            .robots
            .as_deref()
            .is_some_and(|r| r.to_ascii_lowercase().contains("noindex"))
    }
}

/// The audited site.
pub struct Site {
    pub root: PathBuf,
    pub output: PathBuf,
    /// Output dir as shown in findings (`dist`).
    pub output_display: String,
    pub base_url: String,
    pub base_path: String,
    pub pages: Vec<Page>,
    by_key: HashMap<String, usize>,
    pub build_mode: bool,
    pub templates_dir: PathBuf,
    pub data_dir: PathBuf,
    /// URL → source of draft pages (build mode).
    pub drafts: Vec<(String, PathBuf)>,
    pub hidden: Vec<HiddenPage>,
    /// Collections with `private = true` (kept out of sitemap and llms.txt).
    pub private_collections: Vec<String>,
    /// `[site] language` (llms.txt lists this language's pages).
    pub default_lang: String,
    /// Collections as `(name, url_prefix)`, for pages without an item
    /// (`--dist`), longest prefix first.
    pub collections: Vec<(String, String)>,
    /// Every configured language code.
    pub languages: Vec<String>,
    /// `[site] description`: the themes' fallback meta description, so in
    /// `--dist` mode a content page showing it has no description of its own.
    pub site_description: String,
    texts: std::cell::RefCell<HashMap<PathBuf, Option<std::rc::Rc<str>>>>,
    fallback_files: Vec<PathBuf>,
}

/// Normalize a site-relative path for page lookup: no query/fragment,
/// percent-decoded, no trailing `/`, `.html` or `/index`; `/` for the root.
pub fn page_key(path: &str) -> String {
    let path = path.split(['#', '?']).next().unwrap_or("");
    let decoded = urlencoding::decode(path)
        .map(|d| d.into_owned())
        .unwrap_or_else(|_| path.to_string());
    let mut p = decoded.trim_end_matches('/').to_string();
    if let Some(s) = p.strip_suffix(".html") {
        p = s.to_string();
    }
    if p == "/index" || p == "index" {
        p.clear();
    } else if let Some(s) = p.strip_suffix("/index") {
        p = s.to_string();
    }
    if !p.starts_with('/') {
        p.insert(0, '/');
    }
    p
}

/// Site-relative URL of an output-relative HTML path.
fn url_of(rel: &str) -> String {
    if rel == "index.html" {
        return "/".into();
    }
    if let Some(dir) = rel.strip_suffix("/index.html") {
        return format!("/{dir}/");
    }
    format!("/{}", rel.strip_suffix(".html").unwrap_or(rel))
}

/// Largest page read (bigger files are skipped).
const MAX_PAGE_BYTES: u64 = 8 * 1024 * 1024;

impl Site {
    /// Read every HTML page under `output`.
    pub fn load(
        root: &Path,
        output: &Path,
        output_display: &str,
        config: &SiteConfig,
        paths: &ResolvedPaths,
    ) -> Self {
        let mut pages = Vec::new();
        let mut files: Vec<PathBuf> = walkdir::WalkDir::new(output)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|x| x == "html"))
            .map(|e| e.into_path())
            .collect();
        files.sort();
        for path in files {
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            if meta.len() > MAX_PAGE_BYTES {
                continue;
            }
            let Ok(html) = fs::read_to_string(&path) else {
                continue;
            };
            let rel = links::relative_display(output, &path);
            let facts = html::scan(&html);
            pages.push(Page {
                url: url_of(&rel),
                rel,
                facts,
                bytes: meta.len(),
                html,
                source: None,
                item: None,
            });
        }
        let by_key = pages
            .iter()
            .enumerate()
            .map(|(i, p)| (page_key(&p.url), i))
            .collect();
        let mut fallback_files = Vec::new();
        for dir in [&paths.templates, &paths.data_dir] {
            if !dir.is_dir() {
                continue;
            }
            let mut in_dir: Vec<PathBuf> = walkdir::WalkDir::new(dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.into_path())
                .collect();
            in_dir.sort();
            fallback_files.extend(in_dir);
        }
        Site {
            root: root.to_path_buf(),
            output: output.to_path_buf(),
            output_display: output_display.to_string(),
            base_url: config.site.base_url.trim_end_matches('/').to_string(),
            base_path: config.base_path(),
            pages,
            by_key,
            build_mode: false,
            templates_dir: paths.templates.clone(),
            data_dir: paths.data_dir.clone(),
            drafts: Vec::new(),
            hidden: Vec::new(),
            private_collections: config
                .collections
                .iter()
                .filter(|c| c.private)
                .map(|c| c.name.clone())
                .collect(),
            default_lang: config.site.language.clone(),
            collections: {
                let mut c: Vec<(String, String)> = config
                    .collections
                    .iter()
                    .map(|c| {
                        (
                            c.name.clone(),
                            c.url_prefix.trim_end_matches('/').to_string(),
                        )
                    })
                    .collect();
                c.sort_by_key(|(_, prefix)| std::cmp::Reverse(prefix.len()));
                c
            },
            languages: config.all_languages(),
            site_description: config.site.description.trim().to_string(),
            texts: Default::default(),
            fallback_files,
        }
    }

    /// The collection `page` belongs to: its content item's, else (`--dist`)
    /// the collection whose `url_prefix` its URL starts with (after an
    /// optional `/<lang>` prefix). Collections with an empty prefix
    /// (`pages`) match only when nothing else does.
    pub fn collection_of<'a>(&'a self, page: &'a Page) -> Option<&'a str> {
        if let Some(item) = &page.item {
            return Some(&item.collection);
        }
        let url = page.url.as_str();
        let unprefixed = self.languages.iter().find_map(|lang| {
            url.strip_prefix(&format!("/{lang}"))
                .filter(|rest| rest.is_empty() || rest.starts_with('/'))
        });
        let matches = |u: &str, prefix: &str| {
            !prefix.is_empty()
                && u.strip_prefix(prefix)
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        };
        self.collections
            .iter()
            .find(|(_, prefix)| {
                matches(url, prefix) || unprefixed.is_some_and(|u| matches(u, prefix))
            })
            .or_else(|| {
                self.collections
                    .iter()
                    .find(|(_, prefix)| prefix.is_empty())
            })
            .map(|(name, _)| name.as_str())
    }

    /// Index of the page at site-relative `path`.
    pub fn page_at(&self, path: &str) -> Option<usize> {
        self.by_key.get(&page_key(path)).copied()
    }

    /// The site-relative path an `href` on `page` points at, with its
    /// fragment, when it is internal (same site): root-relative, relative,
    /// a bare `#fragment`, or an absolute URL under `base_url`. `None` for
    /// external, `mailto:`, `javascript:`, ... links.
    pub fn internal_target(&self, page: &Page, href: &str) -> Option<(String, Option<String>)> {
        self.resolve_href(&page.url, href)
    }

    /// [`Site::internal_target`] for an `href` found at site-relative URL
    /// `from` (`/` for site-wide files such as llms.txt and the sitemap).
    pub fn resolve_href(&self, from: &str, href: &str) -> Option<(String, Option<String>)> {
        let href = href.trim();
        if href.is_empty() {
            return None;
        }
        let (path, frag) = match href.split_once('#') {
            Some((p, f)) => (p, Some(f.to_string())),
            None => (href, None),
        };
        let path = path.split('?').next().unwrap_or("");
        let resolved = if path.is_empty() {
            from.to_string()
        } else if let Some(rest) = path
            .strip_prefix(&self.base_url)
            .filter(|_| !self.base_url.is_empty() && self.base_url.contains("://"))
        {
            if rest.is_empty() {
                "/".to_string()
            } else if rest.starts_with('/') {
                rest.to_string()
            } else {
                return None;
            }
        } else if path.starts_with("//") || path.contains(':') {
            return None;
        } else if path.starts_with('/') {
            path.to_string()
        } else {
            // Relative to the page's directory.
            let dir = match from.rfind('/') {
                Some(i) => &from[..=i],
                None => "/",
            };
            let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
            for seg in path.split('/') {
                match seg {
                    "." | "" => {}
                    ".." => {
                        parts.pop();
                    }
                    s => parts.push(s),
                }
            }
            format!("/{}", parts.join("/"))
        };
        let resolved = links::strip_base_path(&resolved, &self.base_path).to_string();
        Some((resolved, frag))
    }

    /// Source text of `path` (cached; `None` when unreadable or too big).
    fn text(&self, path: &Path) -> Option<std::rc::Rc<str>> {
        self.texts
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let meta = fs::metadata(path).ok()?;
                if !meta.is_file() || meta.len() > MAX_PAGE_BYTES {
                    return None;
                }
                fs::read_to_string(path).ok().map(std::rc::Rc::from)
            })
            .clone()
    }

    fn display(&self, path: &Path) -> String {
        links::relative_display(&self.root, path)
    }

    /// The generated page itself (`dist/posts/a.html`), at the line of
    /// byte `offset` when given.
    pub fn output_loc(&self, page: &Page, offset: Option<usize>) -> Loc {
        Loc {
            file: Some(if self.output_display.is_empty() {
                page.rel.clone()
            } else {
                format!("{}/{}", self.output_display, page.rel)
            }),
            line: offset.map(|o| html::line_of(&page.html, o)),
            shared: false,
        }
    }

    /// Locate an element of `page`: the first `needles` match in the page's
    /// markdown source, else in a template or data file (a shared finding),
    /// else the generated page at `offset`. URL needles match as whole URLs
    /// (see [`links::find_link_line`]); text needles as substrings.
    pub fn locate(&self, page: &Page, needles: &[Needle], offset: Option<usize>) -> Loc {
        let find = |text: &str| {
            needles.iter().find_map(|n| match n {
                Needle::Url(u) if !u.is_empty() => links::find_link_line(text, u),
                Needle::Text(t) if !t.trim().is_empty() => {
                    text.find(t).map(|i| text[..i].matches('\n').count() + 1)
                }
                _ => None,
            })
        };
        if let Some(src) = &page.source {
            if let Some(line) = self.text(src).as_deref().and_then(find) {
                return Loc {
                    file: Some(self.display(src)),
                    line: Some(line),
                    shared: false,
                };
            }
        }
        for file in &self.fallback_files {
            if let Some(line) = self.text(file).as_deref().and_then(find) {
                return Loc {
                    file: Some(self.display(file)),
                    line: Some(line),
                    shared: true,
                };
            }
        }
        self.output_loc(page, offset)
    }

    /// A page-level problem (title, description, words, ...): the markdown
    /// source at its `key:` frontmatter line when set, else the source,
    /// else the generated page.
    pub fn locate_page(&self, page: &Page, key: Option<&str>) -> Loc {
        match &page.source {
            Some(src) => {
                let line = key.and_then(|k| {
                    let text = self.text(src)?;
                    let prefix = format!("{k}:");
                    text.lines()
                        .position(|l| l.starts_with(&prefix))
                        .map(|i| i + 1)
                });
                Loc {
                    file: Some(self.display(src)),
                    line: line.or(Some(1)),
                    shared: false,
                }
            }
            None => self.output_loc(page, None),
        }
    }

    /// A problem in the layout (`<head>`, `<html>`): the first template
    /// containing `needle`, else the generated page at `offset`.
    pub fn locate_layout(&self, page: &Page, needle: &str, offset: Option<usize>) -> Loc {
        for file in &self.fallback_files {
            let Some(text) = self.text(file) else {
                continue;
            };
            if let Some(i) = text.find(needle) {
                return Loc {
                    file: Some(self.display(file)),
                    line: Some(text[..i].matches('\n').count() + 1),
                    shared: true,
                };
            }
        }
        self.output_loc(page, offset)
    }

    /// A site-wide output file (`dist/robots.txt`).
    pub fn site_file(&self, name: &str) -> Loc {
        Loc {
            file: Some(if self.output_display.is_empty() {
                name.to_string()
            } else {
                format!("{}/{name}", self.output_display)
            }),
            line: None,
            shared: false,
        }
    }
}

/// What [`Site::locate`] searches for.
#[derive(Debug, Clone, Copy)]
pub enum Needle<'a> {
    Url(&'a str),
    Text(&'a str),
}

/// Summary counts of a report.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Summary {
    pub score: u32,
    pub errors: usize,
    pub warnings: usize,
    pub notices: usize,
    pub pages: usize,
    pub by_category: BTreeMap<&'static str, CategoryCounts>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct CategoryCounts {
    pub errors: usize,
    pub warnings: usize,
    pub notices: usize,
}

/// The result of an audit (the `--json` `data` document).
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Counts and score of every finding, including omitted ones.
    pub summary: Summary,
    pub findings: Vec<Finding>,
    /// Findings left out of `findings` by [`Report::limit`].
    pub omitted: usize,
}

impl Report {
    /// Keep only findings of `min_severity` or worse, most severe first
    /// (stable within a severity), at most `top` of them. The summary keeps
    /// counting everything; `omitted` says how many were left out.
    pub fn limit(&mut self, min_severity: Severity, top: Option<usize>) {
        let before = self.findings.len();
        self.findings.retain(|f| f.severity <= min_severity);
        self.findings.sort_by_key(|f| f.severity);
        if let Some(top) = top {
            self.findings.truncate(top);
        }
        self.omitted += before - self.findings.len();
    }
}

/// Score of a set of findings (see the module docs).
pub fn score(findings: &[Finding]) -> u32 {
    let mut per_code: BTreeMap<&str, (Category, u32, u32)> = BTreeMap::new();
    for f in findings {
        let e = per_code
            .entry(f.code)
            .or_insert((f.category, f.severity.weight(), 0));
        e.2 += 1;
    }
    let mut per_category: BTreeMap<Category, u32> = BTreeMap::new();
    for (category, weight, count) in per_code.values() {
        *per_category.entry(*category).or_default() += weight * (*count).min(PENALTY_CAP);
    }
    let penalty: u32 = per_category.values().map(|p| (*p).min(CATEGORY_CAP)).sum();
    100u32.saturating_sub(penalty)
}

/// Run every check on `site` and build the report (findings filtered to
/// `categories` when non-empty, sorted, shared findings collapsed).
pub fn audit(site: &Site, categories: &[Category]) -> Report {
    let mut findings = Vec::new();
    let wanted = |c: Category| categories.is_empty() || categories.contains(&c);
    if wanted(Category::Seo) {
        seo::check(site, &mut findings);
    }
    if wanted(Category::Geo) {
        geo::check(site, &mut findings);
    }
    if wanted(Category::A11y) {
        a11y::check(site, &mut findings);
    }
    if wanted(Category::Content) {
        content::check(site, &mut findings);
    }
    if wanted(Category::Perf) {
        perf::check(site, &mut findings);
    }
    findings.retain(|f| wanted(f.category));
    // One finding per root cause: a page without a description gets
    // `seo-description-missing`, not also `geo-llms-no-summary`.
    let no_description: std::collections::HashSet<String> = findings
        .iter()
        .filter(|f| f.code == "seo-description-missing")
        .filter_map(|f| f.page_url.clone())
        .collect();
    findings.retain(|f| {
        f.code != "geo-llms-no-summary"
            || f.page_url
                .as_ref()
                .is_none_or(|u| !no_description.contains(u))
    });
    let findings = collapse(findings);
    let mut by_category: BTreeMap<&'static str, CategoryCounts> = Category::ALL
        .iter()
        .filter(|c| wanted(**c))
        .map(|c| (c.as_str(), CategoryCounts::default()))
        .collect();
    for f in &findings {
        let counts = by_category.entry(f.category.as_str()).or_default();
        match f.severity {
            Severity::Error => counts.errors += 1,
            Severity::Warning => counts.warnings += 1,
            Severity::Notice => counts.notices += 1,
        }
    }
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    Report {
        summary: Summary {
            score: score(&findings),
            errors: count(Severity::Error),
            warnings: count(Severity::Warning),
            notices: count(Severity::Notice),
            pages: site.pages.iter().filter(|p| !p.is_auxiliary()).count(),
            by_category,
        },
        findings,
        omitted: 0,
    }
}

/// Sort, drop exact duplicates, and merge shared findings (same code, file
/// and line: one template problem shown on many pages) into the first one,
/// noting how many more pages it affects.
fn collapse(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    findings.dedup();
    let mut out: Vec<Finding> = Vec::new();
    let mut extra: HashMap<usize, usize> = HashMap::new();
    let mut index: HashMap<(&'static str, Option<String>, Option<usize>), usize> = HashMap::new();
    for f in findings {
        if f.shared && f.file.is_some() {
            let key = (f.code, f.file.clone(), f.line);
            if let Some(&i) = index.get(&key) {
                *extra.entry(i).or_default() += 1;
                continue;
            }
            index.insert(key, out.len());
        }
        out.push(f);
    }
    for (i, n) in extra {
        out[i].message = format!(
            "{} (and {n} more page{})",
            out[i].message,
            if n == 1 { "" } else { "s" }
        );
    }
    out
}

/// What to audit.
pub struct AuditOptions<'a> {
    pub root: &'a Path,
    /// Audit this existing output directory instead of building.
    pub dist: Option<&'a Path>,
    pub categories: &'a [Category],
}

/// Build the site at `root` into a scratch dir (or read `dist`) and audit it.
pub fn run(opts: &AuditOptions) -> Result<Report> {
    let config = SiteConfig::load(&opts.root.join("seite.toml"))?;
    let paths = config.resolve_paths(opts.root);
    if let Some(dist) = opts.dist {
        let dist = if dist.is_absolute() {
            dist.to_path_buf()
        } else {
            opts.root.join(dist)
        };
        if !dist.is_dir() {
            return Err(PageError::Build(format!(
                "{} is not a directory (build the site first, or drop --dist to audit a fresh build)",
                dist.display()
            )));
        }
        let display = links::relative_display(opts.root, &dist);
        let site = Site::load(opts.root, &dist, &display, &config, &paths);
        return Ok(audit(&site, opts.categories));
    }
    let (scratch, scratch_paths, built) = crate::cli::check::scratch_build(&config, &paths, false)?;
    let result = built.map_err(|e| {
        PageError::Build(format!(
            "the site does not build, so it can't be audited ({e}); run `seite check` and fix the errors first"
        ))
    })?;
    let output_display = links::relative_display(opts.root, &paths.output);
    let mut site = Site::load(
        opts.root,
        &scratch_paths.output,
        &output_display,
        &config,
        &paths,
    );
    let _keep = &scratch;
    site.build_mode = true;
    attach_items(&mut site, &config, &result);
    site.hidden = crate::content::schedule::hidden_pages(&config, &paths);
    site.drafts = draft_pages(&config, &paths);
    Ok(audit(&site, opts.categories))
}

/// Map built pages to their content items (source file, words, dates).
fn attach_items(site: &mut Site, config: &SiteConfig, result: &crate::build::BuildResult) {
    for items in result.collections.values() {
        for item in items {
            let Some(i) = site.page_at(&item.url) else {
                continue;
            };
            let collection = config
                .collections
                .iter()
                .find(|c| c.name == item.collection);
            let dated = collection.is_some_and(|c| c.has_date);
            let date = collection.and_then(|c| {
                crate::build::resolve_item_date(&item.frontmatter, &item.source_path, c)
            });
            let page = &mut site.pages[i];
            page.source = Some(item.source_path.clone());
            page.item = Some(ItemInfo {
                collection: item.collection.clone(),
                word_count: item.word_count,
                dated,
                date,
                is_index: item.slug == "index",
                description: item.frontmatter.description.clone(),
                lang: item.lang.clone(),
            });
        }
    }
}

/// Draft pages (not built): their URL and source file.
fn draft_pages(config: &SiteConfig, paths: &ResolvedPaths) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for collection in &config.collections {
        let dir = paths.content.join(&collection.directory);
        let mut files: Vec<PathBuf> = walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
            .map(|e| e.into_path())
            .collect();
        files.sort();
        for path in files {
            let Ok((fm, _)) = crate::content::parse_content_file(&path) else {
                continue;
            };
            if !fm.draft {
                continue;
            }
            let rel = path.strip_prefix(&dir).unwrap_or(&path).to_path_buf();
            let url = crate::build::resolve_item_location(config, collection, &path, &rel, &fm).url;
            out.push((url, path));
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A site of in-memory pages (`(rel, html)`), no sources.
    pub fn site(pages: &[(&str, &str)]) -> Site {
        let pages: Vec<Page> = pages
            .iter()
            .map(|(rel, html)| Page {
                rel: rel.to_string(),
                url: url_of(rel),
                html: html.to_string(),
                facts: html::scan(html),
                bytes: html.len() as u64,
                source: None,
                item: None,
            })
            .collect();
        let by_key = pages
            .iter()
            .enumerate()
            .map(|(i, p)| (page_key(&p.url), i))
            .collect();
        Site {
            root: PathBuf::from("/site"),
            output: PathBuf::from("/site/dist"),
            output_display: "dist".into(),
            base_url: "https://example.com".into(),
            base_path: String::new(),
            pages,
            by_key,
            build_mode: false,
            templates_dir: PathBuf::from("/nonexistent/templates"),
            data_dir: PathBuf::from("/nonexistent/data"),
            drafts: Vec::new(),
            hidden: Vec::new(),
            private_collections: Vec::new(),
            default_lang: "en".into(),
            collections: Vec::new(),
            languages: vec!["en".into()],
            site_description: String::new(),
            texts: Default::default(),
            fallback_files: Vec::new(),
        }
    }

    /// Codes found by `check` on `site`.
    pub fn codes(site: &Site, check: fn(&Site, &mut Vec<Finding>)) -> Vec<&'static str> {
        let mut out = Vec::new();
        check(site, &mut out);
        let mut codes: Vec<&'static str> = out.iter().map(|f| f.code).collect();
        codes.sort_unstable();
        codes
    }

    /// A minimal well-formed page around `body` (`head` extra in <head>).
    pub fn page(head: &str, body: &str) -> String {
        format!(
            "<!DOCTYPE html><html lang=\"en\"><head><title>A good title</title>{head}</head><body><a href=\"#main\" class=\"skip-link\">Skip to main content</a><main id=\"main\">{body}</main></body></html>"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_key_and_url_of() {
        assert_eq!(page_key("/"), "/");
        assert_eq!(page_key("/index.html"), "/");
        assert_eq!(page_key("/posts/"), "/posts");
        assert_eq!(page_key("/posts/index.html"), "/posts");
        assert_eq!(page_key("/posts/a.html#x"), "/posts/a");
        assert_eq!(page_key("/posts/a?x=1"), "/posts/a");
        assert_eq!(page_key("/caf%C3%A9"), "/café");
        assert_eq!(url_of("index.html"), "/");
        assert_eq!(url_of("posts/index.html"), "/posts/");
        assert_eq!(url_of("posts/a.html"), "/posts/a");
    }

    #[test]
    fn test_internal_target() {
        let site = test_support::site(&[("posts/a.html", "<html></html>")]);
        let page = &site.pages[0];
        let t = |h: &str| site.internal_target(page, h);
        assert_eq!(t("#x"), Some(("/posts/a".into(), Some("x".into()))));
        assert_eq!(t("/b#y"), Some(("/b".into(), Some("y".into()))));
        assert_eq!(t("https://example.com/c"), Some(("/c".into(), None)));
        assert_eq!(t("https://example.com"), Some(("/".into(), None)));
        assert_eq!(t("../docs/x"), Some(("/docs/x".into(), None)));
        assert_eq!(t("b"), Some(("/posts/b".into(), None)));
        assert_eq!(t("https://other.org/"), None);
        assert_eq!(t("mailto:a@b.c"), None);
        assert_eq!(t("//cdn.example/x.js"), None);
    }

    #[test]
    fn test_score_caps_per_code() {
        let f = |code| Finding::new(code, Some("/"), "m");
        assert_eq!(score(&[]), 100);
        assert_eq!(score(&[f("seo-title-missing")]), 90);
        // Capped at 3 findings per code.
        // Notices are reported but cost nothing.
        let many: Vec<Finding> = (0..10).map(|_| f("seo-og-image-missing")).collect();
        assert_eq!(score(&many), 100);
        let warnings: Vec<Finding> = (0..10).map(|_| f("seo-title-long")).collect();
        assert_eq!(score(&warnings), 91);
        // A category costs at most 30.
        let seo: Vec<Finding> = CODES
            .iter()
            .filter(|c| c.category == Category::Seo)
            .flat_map(|c| (0..5).map(|_| f(c.code)).collect::<Vec<_>>())
            .collect();
        assert_eq!(score(&seo), 70);
        let worst: Vec<Finding> = CODES
            .iter()
            .flat_map(|c| (0..5).map(|_| f(c.code)).collect::<Vec<_>>())
            .collect();
        assert_eq!(score(&worst), 0);
    }

    #[test]
    fn test_collapse_merges_shared_findings() {
        let shared = |page: &str| {
            Finding::new("a11y-skip-link", Some(page), "no skip link").at(Loc {
                file: Some("templates/base.html".into()),
                line: Some(3),
                shared: true,
            })
        };
        let own = Finding::new("seo-description-missing", Some("/b"), "x").at(Loc {
            file: Some("content/b.md".into()),
            line: Some(1),
            shared: false,
        });
        let out = collapse(vec![shared("/b"), own.clone(), shared("/a"), shared("/c")]);
        assert_eq!(out.len(), 2);
        let merged = out.iter().find(|f| f.code == "a11y-skip-link").unwrap();
        assert_eq!(merged.page_url.as_deref(), Some("/a"));
        assert!(
            merged.message.ends_with("(and 2 more pages)"),
            "{}",
            merged.message
        );
    }

    #[test]
    fn test_finding_json_shape_is_stable() {
        let f = Finding::new("seo-title-missing", None, "m");
        let v = serde_json::to_value(&f).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "code", "severity", "category", "page_url", "file", "line", "message", "hint",
                "docs_url"
            ]
        );
        assert_eq!(v["severity"], "error");
        assert_eq!(v["category"], "seo");
        assert!(v["page_url"].is_null());
        assert_eq!(
            v["docs_url"],
            "https://seite.sh/docs/errors#seo-title-missing"
        );
    }

    /// Every code a check constructs is in [`CODES`] (so it has a category,
    /// severity, hint and a docs section), and every listed code is used.
    #[test]
    fn test_every_constructed_code_is_registered() {
        let sources = [
            include_str!("seo.rs"),
            include_str!("geo.rs"),
            include_str!("a11y.rs"),
            include_str!("content.rs"),
            include_str!("perf.rs"),
        ];
        let mut used = std::collections::BTreeSet::new();
        for src in sources {
            let src = src.split("#[cfg(test)]").next().unwrap_or("");
            for (i, _) in src.match_indices("Finding::new(") {
                let rest = src[i + "Finding::new(".len()..].trim_start();
                if let Some(rest) = rest.strip_prefix('"') {
                    if let Some(end) = rest.find('"') {
                        used.insert(rest[..end].to_string());
                    }
                }
            }
        }
        for code in &used {
            assert!(code_info(code).is_some(), "{code} is not in CODES");
        }
        for info in CODES {
            assert!(
                used.contains(info.code),
                "{} is never constructed",
                info.code
            );
        }
    }

    #[test]
    fn test_report_limit_keeps_summary() {
        let f = |code| Finding::new(code, Some("/"), "m");
        let site = test_support::site(&[]);
        let mut report = audit(&site, &[Category::Perf]);
        report.findings = vec![
            f("seo-og-image-missing"),
            f("seo-title-long"),
            f("seo-title-missing"),
            f("seo-title-long"),
        ];
        let summary = report.summary.clone();
        report.limit(Severity::Warning, Some(2));
        let codes: Vec<&str> = report.findings.iter().map(|f| f.code).collect();
        assert_eq!(codes, ["seo-title-missing", "seo-title-long"]);
        assert_eq!(report.omitted, 2);
        assert_eq!(report.summary, summary);
    }

    #[test]
    fn test_collection_of_from_url_prefix() {
        let mut site = test_support::site(&[
            ("changelog/v1.html", "<html></html>"),
            ("es/changelog/v1.html", "<html></html>"),
            ("changelogs.html", "<html></html>"),
            ("about.html", "<html></html>"),
        ]);
        site.collections = vec![
            ("changelog".into(), "/changelog".into()),
            ("pages".into(), String::new()),
        ];
        site.languages = vec!["en".into(), "es".into()];
        let of: Vec<Option<&str>> = site.pages.iter().map(|p| site.collection_of(p)).collect();
        assert_eq!(
            of,
            [
                Some("changelog"),
                Some("changelog"),
                Some("pages"),
                Some("pages")
            ]
        );
    }

    #[test]
    fn test_llms_no_summary_is_dropped_when_description_is_missing() {
        let md = "<link rel=\"alternate\" type=\"text/markdown\" href=\"/x.md\">";
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("llms.txt"), "- [A](/a.md)\n").unwrap();
        let mut site = test_support::site(&[("a.html", &test_support::page(md, ""))]);
        site.output = tmp.path().to_path_buf();
        let report = audit(&site, &[Category::Seo, Category::Geo]);
        let codes: Vec<&str> = report.findings.iter().map(|f| f.code).collect();
        assert!(codes.contains(&"seo-description-missing"), "{codes:?}");
        assert!(!codes.contains(&"geo-llms-no-summary"), "{codes:?}");
        // Without the SEO category, the GEO finding stays.
        let report = audit(&site, &[Category::Geo]);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "geo-llms-no-summary"));
    }
}
