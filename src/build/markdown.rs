use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use pulldown_cmark::{html, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

/// A single entry in the auto-generated table of contents.
#[derive(Debug, Clone, Serialize)]
pub struct TocEntry {
    /// Heading level (1–6).
    pub level: u8,
    /// Plain-text heading content.
    pub text: String,
    /// Slugified anchor id (injected into the heading element).
    pub id: String,
}

/// Cached syntax set (loaded once per process).
fn syntax_set() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Cached theme set (loaded once per process).
fn theme_set() -> &'static ThemeSet {
    static TS: OnceLock<ThemeSet> = OnceLock::new();
    TS.get_or_init(ThemeSet::load_defaults)
}

/// Convert heading level enum to a numeric value.
fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Generate a URL-safe slug from heading text for use as an HTML id attribute.
fn slugify_heading(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn markdown_to_html(markdown: &str) -> (String, Vec<TocEntry>) {
    markdown_to_html_with(markdown, false)
}

/// Like [`markdown_to_html`], but when `mermaid` is true, fenced ` ```mermaid `
/// blocks are emitted as `<div class="mermaid">` containers for client-side
/// rendering instead of being syntax-highlighted or dumped as plain code.
pub fn markdown_to_html_with(markdown: &str, mermaid: bool) -> (String, Vec<TocEntry>) {
    let ss = syntax_set();
    let ts = theme_set();
    let theme = &ts.themes["base16-ocean.dark"];

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(markdown, options);

    let mut html_output = String::new();
    let mut toc = Vec::new();
    let mut code_buf = String::new();
    let mut in_code_block = false;
    let mut code_lang: Option<String> = None;

    // Heading state
    let mut in_heading = false;
    let mut heading_level: u8 = 0;
    let mut heading_text = String::new();

    // Collect events, intercepting headings (for ToC + id attributes) and
    // code blocks (for syntax highlighting). Everything else is passed through
    // to push_html in batches so that stateful renderers (e.g. tables) work
    // correctly.
    let mut pending: Vec<Event> = Vec::new();

    /// Flush pending events through pulldown-cmark's HTML renderer.
    fn flush_pending<'a>(pending: &mut Vec<Event<'a>>, html_output: &mut String) {
        if !pending.is_empty() {
            html::push_html(html_output, pending.drain(..));
        }
    }

    for event in parser {
        match event {
            // ── Heading events ──
            Event::Start(Tag::Heading { level, .. }) => {
                flush_pending(&mut pending, &mut html_output);
                in_heading = true;
                heading_level = heading_level_to_u8(level);
                heading_text.clear();
            }
            Event::Text(ref text) if in_heading => {
                heading_text.push_str(text);
            }
            Event::Code(ref code) if in_heading => {
                heading_text.push_str(code);
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                let id = slugify_heading(&heading_text);
                toc.push(TocEntry {
                    level: heading_level,
                    text: heading_text.clone(),
                    id: id.clone(),
                });
                html_output.push_str(&format!(
                    "<h{} id=\"{}\">{}",
                    heading_level,
                    id,
                    html_escape(&heading_text)
                ));
                html_output.push_str(&format!("</h{}>\n", heading_level));
            }

            // ── Code block events ──
            Event::Start(Tag::CodeBlock(kind)) => {
                flush_pending(&mut pending, &mut html_output);
                in_code_block = true;
                code_buf.clear();
                code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let l = lang.trim().to_string();
                        if l.is_empty() {
                            None
                        } else {
                            Some(l)
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
            }
            Event::Text(text) if in_code_block => {
                code_buf.push_str(&text);
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;

                // Mermaid diagrams: emit a passthrough container for client-side
                // rendering rather than syntax-highlighting the source as code.
                // A <div> (not <pre>) keeps the code-copy button + syntax
                // highlighter from touching it. The source is HTML-escaped; the
                // browser un-escapes it via textContent for Mermaid.
                let is_mermaid = mermaid
                    && code_lang
                        .as_deref()
                        .is_some_and(|l| l.eq_ignore_ascii_case("mermaid"));

                if is_mermaid {
                    html_output.push_str("<div class=\"mermaid\">");
                    html_output.push_str(&html_escape(&code_buf));
                    html_output.push_str("</div>\n");
                } else {
                    let mut highlighted = false;

                    if let Some(ref lang) = code_lang {
                        let resolved = resolve_lang_alias(lang);
                        if let Some(syntax) = ss.find_syntax_by_token(resolved) {
                            if let Ok(html) =
                                highlighted_html_for_string(&code_buf, ss, syntax, theme)
                            {
                                html_output.push_str(&html);
                                highlighted = true;
                            }
                        }
                    }

                    if !highlighted {
                        html_output.push_str("<pre><code>");
                        html_output.push_str(&html_escape(&code_buf));
                        html_output.push_str("</code></pre>\n");
                    }
                }
            }
            _ if in_code_block => { /* skip non-text events inside code blocks */ }
            other => {
                pending.push(other);
            }
        }
    }

    flush_pending(&mut pending, &mut html_output);

    (html_output, toc)
}

/// Map language tokens that syntect doesn't recognise to ones it does.
///
/// Syntect's default syntax set is missing PowerShell, Nix, HCL, and a few
/// other languages people use in fenced code blocks.  Rather than shipping
/// extra `.sublime-syntax` bundles we alias them to the closest built-in
/// grammar so the output still gets *some* highlighting.
fn resolve_lang_alias(lang: &str) -> &str {
    match lang {
        // PowerShell → bash (pipe-based one-liners look fine)
        "powershell" | "ps1" | "posh" | "pwsh" => "bash",
        // Windows shells
        "batch" | "dos" => "bat",
        // Nix / HCL / Terraform → closest built-in
        "nix" => "bash",
        "hcl" | "terraform" | "tf" => "ruby",
        // Fish shell
        "fish" => "bash",
        // Everything else — pass through as-is
        other => other,
    }
}

/// Escape HTML special characters for plain code blocks.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Maps content source files to their page URLs so that links written as
/// paths to other markdown files (`[x](../docs/intro.md)`, GitHub-style) can
/// be rewritten to the generated page.
#[derive(Debug, Default)]
pub struct SourceLinkMap {
    root: PathBuf,
    content_dir: PathBuf,
    default_lang: String,
    urls: HashMap<PathBuf, String>,
}

/// Outcome of resolving one link against a [`SourceLinkMap`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdLinkTarget {
    /// Not a link to a markdown source file; leave it alone.
    NotSource,
    /// Rewritten page URL (fragment/query preserved).
    Resolved(String),
    /// Looks like a link to a content file, but no content file matches.
    Unresolved,
}

impl SourceLinkMap {
    /// `root` is the site root (for `/content/...md` links) and
    /// `content_dir` the content directory.
    pub fn new(root: &Path, content_dir: &Path, default_lang: &str) -> Self {
        Self {
            root: normalize_path(root),
            content_dir: normalize_path(content_dir),
            default_lang: default_lang.to_string(),
            urls: HashMap::new(),
        }
    }

    /// Register a content file and the URL it renders to. Index pages
    /// (`/docs/index`, `/index`, `/es/index`) map to their directory URL
    /// (`/docs/`, `/`, `/es/`), which is where they are served.
    pub fn insert(&mut self, source: &Path, url: &str) {
        let url = match url.strip_suffix("index") {
            Some(dir) if dir.ends_with('/') => dir.to_string(),
            _ => url.to_string(),
        };
        self.urls.insert(normalize_path(source), url);
    }

    pub fn len(&self) -> usize {
        self.urls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.urls.is_empty()
    }

    /// Resolve `href` as written in `from_source` (a page in language `lang`).
    ///
    /// Rewrites relative links ending in `.md` (optionally with `#fragment`
    /// or `?query`), resolved against the directory of `from_source`, and
    /// root-relative links into the content directory (`/content/...md`).
    /// Other root-relative `.md` links (e.g. `/docs/x.md`) are valid links to
    /// the published markdown copies and are left alone. When the page isn't
    /// in the default language and the target has a translation in `lang`
    /// (`intro.es.md`), the translation's URL is used.
    pub fn resolve(&self, from_source: &Path, href: &str, lang: &str) -> MdLinkTarget {
        let href = href.trim();
        let split = href.find(['#', '?']).unwrap_or(href.len());
        let (path, suffix) = href.split_at(split);
        if !path.to_ascii_lowercase().ends_with(".md") || path.starts_with("//") {
            return MdLinkTarget::NotSource;
        }
        // Absolute URLs and other schemes (https:, mailto:, ...).
        if let Some(colon) = path.find(':') {
            if !path[..colon].contains('/') {
                return MdLinkTarget::NotSource;
            }
        }
        let decoded = urlencoding::decode(path)
            .map(|d| d.into_owned())
            .unwrap_or_else(|_| path.to_string());

        let candidate = if let Some(rest) = decoded.strip_prefix('/') {
            let candidate = normalize_path(&self.root.join(rest));
            if !candidate.starts_with(&self.content_dir) {
                return MdLinkTarget::NotSource;
            }
            candidate
        } else {
            let base = from_source.parent().unwrap_or(Path::new(""));
            normalize_path(&base.join(&decoded))
        };

        match self.lookup(&candidate, lang) {
            Some(url) => MdLinkTarget::Resolved(format!("{url}{suffix}")),
            None => MdLinkTarget::Unresolved,
        }
    }

    fn lookup(&self, candidate: &Path, lang: &str) -> Option<&String> {
        if lang != self.default_lang {
            if let Some(stem) = candidate.file_stem().and_then(|s| s.to_str()) {
                let already_translated = stem
                    .rsplit_once('.')
                    .is_some_and(|(_, suffix)| suffix == lang);
                if !already_translated {
                    let translated = candidate.with_file_name(format!("{stem}.{lang}.md"));
                    if let Some(url) = self.urls.get(&translated) {
                        return Some(url);
                    }
                }
            }
        }
        self.urls.get(candidate)
    }
}

/// Lexically normalize a path: drop `.` components and resolve `..` against
/// preceding components (without touching the filesystem).
fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Rewrite `href` attributes in rendered HTML that point at markdown source
/// files into page URLs (see [`SourceLinkMap::resolve`]).
///
/// Returns the rewritten HTML and the hrefs (as authored) that looked like
/// content-file links but matched no content file, deduplicated in order.
/// Code blocks are safe: their contents are HTML-escaped, so they contain no
/// quoted `href=` attributes.
pub fn rewrite_md_links(
    html: &str,
    from_source: &Path,
    lang: &str,
    map: &SourceLinkMap,
) -> (String, Vec<String>) {
    let mut unresolved: Vec<String> = Vec::new();
    if !html.contains(".md") {
        return (html.to_string(), unresolved);
    }
    let mut out = String::with_capacity(html.len());
    let mut pos = 0;
    while let Some(idx) = html[pos..].find("href=") {
        let value_start = pos + idx + 5;
        let quote = match html.as_bytes().get(value_start) {
            Some(&q) if q == b'"' || q == b'\'' => q as char,
            _ => {
                out.push_str(&html[pos..value_start]);
                pos = value_start;
                continue;
            }
        };
        let Some(len) = html[value_start + 1..].find(quote) else {
            break;
        };
        let raw = &html[value_start + 1..value_start + 1 + len];
        out.push_str(&html[pos..=value_start]);
        let href = raw.replace("&amp;", "&");
        match map.resolve(from_source, &href, lang) {
            MdLinkTarget::Resolved(url) => out.push_str(&html_escape(&url)),
            MdLinkTarget::Unresolved => {
                if !unresolved.contains(&href) {
                    unresolved.push(href);
                }
                out.push_str(raw);
            }
            MdLinkTarget::NotSource => out.push_str(raw),
        }
        out.push(quote);
        pos = value_start + 1 + len + 1;
    }
    out.push_str(&html[pos..]);
    (out, unresolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown() {
        let md = "# Hello\n\nThis is **bold** and *italic*.";
        let (html, toc) = markdown_to_html(md);
        assert!(html.contains("Hello"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].text, "Hello");
        assert_eq!(toc[0].level, 1);
        assert_eq!(toc[0].id, "hello");
    }

    #[test]
    fn test_code_block_highlighted() {
        let md = "```rust\nfn main() {}\n```";
        let (html, _) = markdown_to_html(md);
        // syntect wraps tokens in <span> tags, so check for individual keywords
        assert!(html.contains("fn"));
        assert!(html.contains("main"));
        assert!(html.contains("style=\""));
    }

    #[test]
    fn test_mermaid_fence_emits_div_when_enabled() {
        let md = "```mermaid\ngraph TD\n  A-->B\n```";
        let (html, _) = markdown_to_html_with(md, true);
        assert!(
            html.contains(r#"<div class="mermaid">"#),
            "mermaid fence should become a mermaid div, got: {html}"
        );
        assert!(html.contains("graph TD"));
        // It must NOT be wrapped in a <pre> (so the code-copy button + syntax
        // highlighter both leave it alone).
        assert!(
            !html.contains("<pre"),
            "mermaid block should not be a <pre>: {html}"
        );
    }

    #[test]
    fn test_mermaid_fence_escapes_source() {
        let md = "```mermaid\nflowchart LR\n  A[\"<b>x</b>\"]\n```";
        let (html, _) = markdown_to_html_with(md, true);
        // Angle brackets in the source are HTML-escaped inside the div;
        // the browser un-escapes them via textContent for Mermaid.
        assert!(html.contains("&lt;b&gt;"), "got: {html}");
    }

    #[test]
    fn test_mermaid_fence_plain_when_disabled() {
        let md = "```mermaid\ngraph TD\n  A-->B\n```";
        let (html, _) = markdown_to_html_with(md, false);
        // With mermaid off, an unknown language falls back to plain <pre><code>.
        assert!(!html.contains(r#"class="mermaid""#));
        assert!(html.contains("<pre><code>"));
    }

    #[test]
    fn test_syntax_highlighting_produces_styled_output() {
        let md = "```rust\nlet x = 42;\n```";
        let (html, _) = markdown_to_html(md);
        // syntect with inline styles produces style= attributes
        assert!(html.contains("style=\""));
        assert!(html.contains("let"));
        assert!(html.contains("42"));
    }

    #[test]
    fn test_plain_code_block_no_lang() {
        let md = "```\nplain text\n```";
        let (html, _) = markdown_to_html(md);
        assert!(html.contains("<pre><code>"));
        assert!(html.contains("plain text"));
        // No style attributes for plain code
        assert!(!html.contains("style=\""));
    }

    #[test]
    fn test_unknown_language_falls_back() {
        let md = "```nonsenselangthatdoesnotexist\nhello\n```";
        let (html, _) = markdown_to_html(md);
        assert!(html.contains("hello"));
    }

    #[test]
    fn test_toc_multiple_headings() {
        let md = "## Introduction\n\nText.\n\n### Details\n\nMore text.\n\n## Conclusion";
        let (html, toc) = markdown_to_html(md);
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].text, "Introduction");
        assert_eq!(toc[0].level, 2);
        assert_eq!(toc[0].id, "introduction");
        assert_eq!(toc[1].text, "Details");
        assert_eq!(toc[1].level, 3);
        assert_eq!(toc[2].text, "Conclusion");
        assert_eq!(toc[2].level, 2);
        // Check id attributes in HTML
        assert!(html.contains("id=\"introduction\""));
        assert!(html.contains("id=\"details\""));
        assert!(html.contains("id=\"conclusion\""));
    }

    #[test]
    fn test_slugify_heading() {
        assert_eq!(slugify_heading("Hello World"), "hello-world");
        assert_eq!(slugify_heading("Rust & WebAssembly!"), "rust-webassembly");
        assert_eq!(
            slugify_heading("3.1 Getting Started"),
            "3-1-getting-started"
        );
    }

    #[test]
    fn test_toc_empty_for_no_headings() {
        let md = "Just a paragraph.\n\nAnother one.";
        let (_, toc) = markdown_to_html(md);
        assert!(toc.is_empty());
    }

    #[test]
    fn test_task_list() {
        let md = "- [x] Done\n- [ ] Pending\n- Regular item";
        let (html, _) = markdown_to_html(md);
        assert!(
            html.contains(r#"type="checkbox""#),
            "should render checkboxes"
        );
        assert!(
            html.contains("checked"),
            "checked item should have checked attribute"
        );
        assert!(html.contains("Done"), "should contain task text");
        assert!(html.contains("Pending"), "should contain pending task text");
    }

    #[test]
    fn test_strikethrough() {
        let md = "This is ~~deleted~~ text.";
        let (html, _) = markdown_to_html(md);
        assert!(
            html.contains("<del>deleted</del>"),
            "should render strikethrough"
        );
    }

    #[test]
    fn test_table() {
        let md = "| Name | Value |\n|------|-------|\n| a    | 1     |\n| b    | 2     |";
        let (html, _) = markdown_to_html(md);
        assert!(html.contains("<table"), "should render table");
        assert!(html.contains("<th"), "should render header cells");
        assert!(html.contains("<td"), "should render data cells");
    }

    #[test]
    fn test_footnotes() {
        let md = "Text with a footnote[^1].\n\n[^1]: This is the footnote.";
        let (html, _) = markdown_to_html(md);
        assert!(
            html.contains("footnote"),
            "should render footnote references"
        );
    }

    #[test]
    fn test_resolve_lang_alias() {
        assert_eq!(resolve_lang_alias("powershell"), "bash");
        assert_eq!(resolve_lang_alias("ps1"), "bash");
        assert_eq!(resolve_lang_alias("pwsh"), "bash");
        assert_eq!(resolve_lang_alias("batch"), "bat");
        assert_eq!(resolve_lang_alias("rust"), "rust");
        assert_eq!(resolve_lang_alias("hcl"), "ruby");
    }

    #[test]
    fn test_powershell_code_block_gets_highlighted() {
        let md = "```powershell\nirm https://seite.sh/install.ps1 | iex\n```";
        let (html, _) = markdown_to_html(md);
        // Should produce syntect-highlighted output (style= attributes), not plain <pre><code>
        assert!(
            html.contains("style=\""),
            "powershell block should be syntax-highlighted via bash alias, got: {}",
            html
        );
    }

    /// Site at /site with content in /site/content: posts, nested docs, a
    /// homepage, a docs index, and a Spanish translation.
    fn link_map() -> SourceLinkMap {
        let mut map = SourceLinkMap::new(Path::new("/site"), Path::new("/site/content"), "en");
        for (src, url) in [
            ("posts/2025-01-01-hello.md", "/posts/hello"),
            ("posts/other.md", "/posts/other"),
            ("docs/getting-started.md", "/docs/getting-started"),
            ("docs/getting-started.es.md", "/es/docs/getting-started"),
            ("docs/guides/deep.md", "/docs/guides/deep"),
            ("docs/index.md", "/docs/index"),
            ("pages/index.md", "/index"),
            ("pages/index.es.md", "/es/index"),
        ] {
            map.insert(&Path::new("/site/content").join(src), url);
        }
        map
    }

    fn render_rewrite(md: &str, from: &str, lang: &str) -> (String, Vec<String>) {
        let (html, _) = markdown_to_html(md);
        rewrite_md_links(
            &html,
            &Path::new("/site/content").join(from),
            lang,
            &link_map(),
        )
    }

    #[test]
    fn test_rewrite_md_links_relative_with_fragment() {
        let (html, unresolved) = render_rewrite(
            "[a](./other.md) [b](../docs/getting-started.md#install) [c](../docs/guides/deep.md?x=1)",
            "posts/2025-01-01-hello.md",
            "en",
        );
        assert!(html.contains(r#"href="/posts/other""#), "{html}");
        assert!(
            html.contains(r#"href="/docs/getting-started#install""#),
            "{html}"
        );
        assert!(html.contains(r#"href="/docs/guides/deep?x=1""#), "{html}");
        assert!(unresolved.is_empty());
    }

    #[test]
    fn test_rewrite_md_links_nested_and_index_pages() {
        let (html, _) = render_rewrite(
            "[up](../getting-started.md) [idx](../index.md) [home](../../pages/index.md)",
            "docs/guides/deep.md",
            "en",
        );
        assert!(html.contains(r#"href="/docs/getting-started""#), "{html}");
        assert!(html.contains(r#"href="/docs/""#), "{html}");
        assert!(html.contains(r#"href="/""#), "{html}");
    }

    #[test]
    fn test_rewrite_md_links_content_root_and_published_copies() {
        let (html, unresolved) = render_rewrite(
            "[src](/content/docs/getting-started.md) [copy](/docs/getting-started.md) [ext](https://example.com/README.md) [mail](mailto:a@b.md)",
            "posts/other.md",
            "en",
        );
        assert!(html.contains(r#"href="/docs/getting-started""#), "{html}");
        // Root-relative links outside the content dir point at the published
        // markdown copies and stay as written.
        assert!(
            html.contains(r#"href="/docs/getting-started.md""#),
            "{html}"
        );
        assert!(html.contains(r#"href="https://example.com/README.md""#));
        assert!(html.contains(r#"href="mailto:a@b.md""#));
        assert!(unresolved.is_empty());
    }

    #[test]
    fn test_rewrite_md_links_prefers_translation_for_page_language() {
        let (html, _) = render_rewrite(
            "[start](../docs/getting-started.md) [home](index.md)",
            "pages/index.es.md",
            "es",
        );
        assert!(
            html.contains(r#"href="/es/docs/getting-started""#),
            "{html}"
        );
        assert!(html.contains(r#"href="/es/""#), "{html}");

        // Default-language pages keep the default-language target.
        let (html, _) = render_rewrite(
            "[start](../docs/getting-started.md)",
            "pages/index.md",
            "en",
        );
        assert!(html.contains(r#"href="/docs/getting-started""#), "{html}");
    }

    #[test]
    fn test_rewrite_md_links_reports_unresolved() {
        let (html, unresolved) = render_rewrite(
            "[bad](./nope.md#x) [bad again](./nope.md#x) [content](/content/posts/gone.md) [plain](./not-markdown.txt)",
            "posts/other.md",
            "en",
        );
        assert_eq!(unresolved, vec!["./nope.md#x", "/content/posts/gone.md"]);
        // Unresolved links are left as written.
        assert!(html.contains(r#"href="./nope.md#x""#), "{html}");
    }

    #[test]
    fn test_rewrite_md_links_ignores_code_blocks() {
        let (html, unresolved) = render_rewrite(
            "```html\n<a href=\"./other.md\">x</a>\n```\n\n`[x](./other.md)`",
            "posts/other.md",
            "en",
        );
        assert!(!html.contains(r#"href="/posts/other""#), "{html}");
        assert!(unresolved.is_empty());
    }

    #[test]
    fn test_source_link_map_relative_paths_are_normalized() {
        let mut map = SourceLinkMap::new(Path::new("."), Path::new("./content"), "en");
        map.insert(Path::new("./content/docs/a.md"), "/docs/a");
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.resolve(Path::new("content/posts/x.md"), "../docs/a.md", "en"),
            MdLinkTarget::Resolved("/docs/a".into())
        );
        assert_eq!(
            map.resolve(Path::new("content/posts/x.md"), "/docs/a.md", "en"),
            MdLinkTarget::NotSource
        );
    }
}
