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
                    heading_level, id, heading_text
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
                let mut highlighted = false;

                if let Some(ref lang) = code_lang {
                    let resolved = resolve_lang_alias(lang);
                    if let Some(syntax) = ss.find_syntax_by_token(resolved) {
                        if let Ok(html) = highlighted_html_for_string(&code_buf, ss, syntax, theme)
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
}
