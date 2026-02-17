use std::sync::OnceLock;

use pulldown_cmark::{html, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

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

pub fn markdown_to_html(markdown: &str) -> String {
    let ss = syntax_set();
    let ts = theme_set();
    let theme = &ts.themes["base16-ocean.dark"];

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(markdown, options);

    let mut html_output = String::new();
    let mut code_buf = String::new();
    let mut in_code_block = false;
    let mut code_lang: Option<String> = None;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
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
                    if let Some(syntax) = ss.find_syntax_by_token(lang) {
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
            _ if in_code_block => { /* skip non-text events inside code blocks */ }
            other => {
                html::push_html(&mut html_output, std::iter::once(other));
            }
        }
    }

    html_output
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
        let html = markdown_to_html(md);
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
    }

    #[test]
    fn test_code_block_highlighted() {
        let md = "```rust\nfn main() {}\n```";
        let html = markdown_to_html(md);
        // syntect wraps tokens in <span> tags, so check for individual keywords
        assert!(html.contains("fn"));
        assert!(html.contains("main"));
        assert!(html.contains("style=\""));
    }

    #[test]
    fn test_syntax_highlighting_produces_styled_output() {
        let md = "```rust\nlet x = 42;\n```";
        let html = markdown_to_html(md);
        // syntect with inline styles produces style= attributes
        assert!(html.contains("style=\""));
        assert!(html.contains("let"));
        assert!(html.contains("42"));
    }

    #[test]
    fn test_plain_code_block_no_lang() {
        let md = "```\nplain text\n```";
        let html = markdown_to_html(md);
        assert!(html.contains("<pre><code>"));
        assert!(html.contains("plain text"));
        // No style attributes for plain code
        assert!(!html.contains("style=\""));
    }

    #[test]
    fn test_unknown_language_falls_back() {
        let md = "```nonsenselangthatdoesnotexist\nhello\n```";
        let html = markdown_to_html(md);
        assert!(html.contains("hello"));
    }
}
