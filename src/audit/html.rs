//! A small, forgiving HTML scanner for `seite audit`: one pass over a page
//! collects the facts the checks need ([`PageFacts`]). No DOM is built; only
//! the elements the checks look at are tracked (title, headings, links,
//! buttons, labels, form controls, images, scripts, `<main>`, ...).
//!
//! Tags are found with the same quote-aware scanner the link checker uses
//! (`build::links`), comments are skipped, and `<script>`/`<style>` bodies
//! are raw text (kept for JSON-LD).

use crate::build::links::{find_ascii_ci, find_tag_end, parse_attrs};

/// One token of an HTML document. Offsets are byte offsets into the page.
#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    Start {
        name: String,
        attrs: Vec<(String, String)>,
        offset: usize,
        /// Body of a `<script>`/`<style>` element (raw text).
        raw: Option<&'a str>,
    },
    End {
        name: String,
    },
    Text(&'a str),
}

/// Tokenize `html` (see the module docs).
pub fn tokenize(html: &str) -> Vec<Token<'_>> {
    let bytes = html.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0;
    let mut text_start = 0;
    while let Some(off) = html[pos..].find('<') {
        let start = pos + off;
        let rest = &html[start..];
        if rest.starts_with("<!--") {
            push_text(&mut out, &html[text_start..start]);
            match rest.find("-->") {
                Some(end) => {
                    pos = start + end + 3;
                    text_start = pos;
                    continue;
                }
                None => return out,
            }
        }
        let next = bytes.get(start + 1).copied().unwrap_or(b' ');
        let is_end = next == b'/';
        let name_byte = if is_end {
            bytes.get(start + 2).copied().unwrap_or(b' ')
        } else {
            next
        };
        if !(name_byte.is_ascii_alphabetic() || (!is_end && next == b'!')) {
            pos = start + 1;
            continue;
        }
        let Some(end) = find_tag_end(html, start) else {
            break;
        };
        push_text(&mut out, &html[text_start..start]);
        pos = end + 1;
        text_start = pos;
        if next == b'!' {
            continue; // <!DOCTYPE ...>
        }
        let inner = &html[start + 1..end];
        if is_end {
            let name = inner[1..]
                .split(|c: char| c.is_ascii_whitespace() || c == '>')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            out.push(Token::End { name });
            continue;
        }
        let name_end = inner
            .find(|c: char| c.is_ascii_whitespace() || c == '/')
            .unwrap_or(inner.len());
        let name = inner[..name_end].to_ascii_lowercase();
        let attrs = parse_attrs(&inner[name_end..])
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), decode_entities(v)))
            .collect();
        let mut raw = None;
        if name == "script" || name == "style" {
            let close = format!("</{name}");
            match find_ascii_ci(&html[pos..], &close) {
                Some(i) => {
                    raw = Some(&html[pos..pos + i]);
                    pos += i;
                    text_start = pos;
                }
                None => {
                    raw = Some(&html[pos..]);
                    pos = html.len();
                    text_start = pos;
                }
            }
        }
        out.push(Token::Start {
            name,
            attrs,
            offset: start,
            raw,
        });
    }
    push_text(&mut out, &html[text_start.min(html.len())..]);
    out
}

fn push_text<'a>(out: &mut Vec<Token<'a>>, text: &'a str) {
    if !text.is_empty() {
        out.push(Token::Text(text));
    }
}

/// Decode the common named and numeric character references.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(semi) = rest.as_bytes()[..rest.len().min(12)]
            .iter()
            .position(|&b| b == b';')
        else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "middot" => Some('·'),
            "rarr" => Some('→'),
            "larr" => Some('←'),
            "hellip" => Some('…'),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "copy" => Some('©'),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Collapse runs of whitespace and trim.
pub fn normalize_space(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 1-based line of byte `offset` in `text`.
pub fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].matches('\n').count() + 1
}

/// An `<a href>` (or `<area>`) with its accessible name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Link {
    pub href: String,
    /// Text content (plus `alt` of images inside), whitespace-normalized.
    pub text: String,
    pub aria_label: Option<String>,
    pub title: Option<String>,
    pub in_main: bool,
    pub offset: usize,
}

impl Link {
    /// Name a screen reader announces: aria-label, else text, else title.
    pub fn accessible_name(&self) -> String {
        self.aria_label
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(normalize_space)
            .or_else(|| (!self.text.is_empty()).then(|| self.text.clone()))
            .or_else(|| self.title.as_deref().map(normalize_space))
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Img {
    pub src: Option<String>,
    pub srcset: Option<String>,
    pub alt: Option<String>,
    pub width: Option<String>,
    pub height: Option<String>,
    pub loading: Option<String>,
    pub role_presentation: bool,
    pub in_main: bool,
    pub offset: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub in_main: bool,
    pub offset: usize,
}

/// A form control that needs a label (`input`, `select`, `textarea`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Control {
    pub tag: String,
    pub kind: String,
    pub id: Option<String>,
    pub name: Option<String>,
    /// aria-label / aria-labelledby / title, or wrapped in a `<label>`.
    pub labelled: bool,
    pub offset: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Button {
    /// Accessible name (aria-label, text, inner img alt, title).
    pub name: String,
    pub offset: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Script {
    pub src: Option<String>,
    pub kind: Option<String>,
    pub is_async: bool,
    pub defer: bool,
    pub in_head: bool,
    pub body: String,
    pub offset: usize,
}

/// Everything the audit checks read from one page.
#[derive(Debug, Clone, Default)]
pub struct PageFacts {
    /// `<html lang>`: `None` when absent, `Some("")` when empty.
    pub lang: Option<String>,
    pub html_offset: usize,
    pub head_offset: usize,
    pub titles: Vec<String>,
    pub description: Option<String>,
    pub canonical: Option<String>,
    pub robots: Option<String>,
    pub og_image: bool,
    /// `<meta http-equiv="refresh">`: an alias redirect page.
    pub refresh: bool,
    pub markdown_alternate: Option<String>,
    /// `<link rel="alternate" hreflang>`: (lang, href).
    pub hreflangs: Vec<(String, String)>,
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
    pub imgs: Vec<Img>,
    pub controls: Vec<Control>,
    pub buttons: Vec<Button>,
    /// `<label for>` targets.
    pub label_for: Vec<String>,
    /// Every `id` attribute value (plus `<a name>`), in document order,
    /// with its offset.
    pub ids: Vec<(String, usize)>,
    pub scripts: Vec<Script>,
    pub stylesheets: Vec<String>,
    pub has_main: bool,
    /// Words of visible text in `<main>` (whole body without `<main>`),
    /// outside `nav`/`header`/`footer`/`aside`.
    pub main_words: usize,
}

impl PageFacts {
    /// JSON-LD blocks (`<script type="application/ld+json">` bodies).
    pub fn json_ld(&self) -> impl Iterator<Item = &Script> {
        self.scripts.iter().filter(|s| {
            s.kind
                .as_deref()
                .is_some_and(|k| k.trim().eq_ignore_ascii_case("application/ld+json"))
        })
    }

    /// True when the page links to `#…` with "skip" in its text or class
    /// (a skip-to-content link).
    pub fn has_skip_link(&self) -> bool {
        self.links
            .iter()
            .any(|l| l.href.starts_with('#') && l.accessible_name().to_lowercase().contains("skip"))
    }
}

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

#[derive(Debug)]
enum CaptureKind {
    Title,
    Heading(u8),
    Link(usize),
    Button(usize),
    Label,
}

#[derive(Debug)]
struct Capture {
    tag: &'static str,
    kind: CaptureKind,
    text: String,
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Scan a page into [`PageFacts`].
pub fn scan(html: &str) -> PageFacts {
    let mut f = PageFacts::default();
    let mut captures: Vec<Capture> = Vec::new();
    let mut in_head = false;
    let mut seen_body = false;
    let mut main_depth = 0usize;
    let mut chrome_depth = 0usize;
    let mut main_text = String::new();
    let mut body_text = String::new();
    // Button names come from aria-label when set; text is the fallback.
    let mut button_labels: Vec<Option<String>> = Vec::new();

    for token in tokenize(html) {
        match token {
            Token::Text(text) => {
                let decoded = decode_entities(text);
                for c in captures.iter_mut() {
                    c.text.push_str(&decoded);
                    c.text.push(' ');
                }
                if seen_body && chrome_depth == 0 {
                    if main_depth > 0 {
                        main_text.push_str(&decoded);
                        main_text.push(' ');
                    }
                    body_text.push_str(&decoded);
                    body_text.push(' ');
                }
            }
            Token::Start {
                name,
                attrs,
                offset,
                raw,
            } => {
                if let Some(id) = attr(&attrs, "id").filter(|v| !v.is_empty()) {
                    f.ids.push((id.to_string(), offset));
                }
                let in_main = main_depth > 0;
                match name.as_str() {
                    "html" => {
                        f.lang = attr(&attrs, "lang").map(str::to_string);
                        f.html_offset = offset;
                    }
                    "head" => {
                        in_head = true;
                        f.head_offset = offset;
                    }
                    "body" => {
                        in_head = false;
                        seen_body = true;
                    }
                    "main" => {
                        main_depth += 1;
                        f.has_main = true;
                        seen_body = true;
                    }
                    "nav" | "header" | "footer" | "aside" => {
                        chrome_depth += 1;
                        seen_body = true;
                    }
                    "title" if in_head || !seen_body => captures.push(Capture {
                        tag: "title",
                        kind: CaptureKind::Title,
                        text: String::new(),
                    }),
                    "meta" => {
                        let key = attr(&attrs, "name")
                            .or_else(|| attr(&attrs, "property"))
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        let content = attr(&attrs, "content").unwrap_or("");
                        match key.as_str() {
                            "description" => f.description = Some(content.to_string()),
                            "robots" => f.robots = Some(content.to_string()),
                            "og:image" if !content.trim().is_empty() => f.og_image = true,
                            _ => {}
                        }
                        if attr(&attrs, "http-equiv")
                            .is_some_and(|v| v.eq_ignore_ascii_case("refresh"))
                        {
                            f.refresh = true;
                        }
                    }
                    "link" => {
                        let rel = attr(&attrs, "rel").unwrap_or("").to_ascii_lowercase();
                        let href = attr(&attrs, "href").unwrap_or("").to_string();
                        let rels: Vec<&str> = rel.split_ascii_whitespace().collect();
                        if rels.contains(&"canonical") {
                            f.canonical = Some(href.clone());
                        }
                        if rels.contains(&"stylesheet") {
                            f.stylesheets.push(href.clone());
                        }
                        if rels.contains(&"alternate") {
                            if let Some(lang) = attr(&attrs, "hreflang") {
                                f.hreflangs.push((lang.to_string(), href.clone()));
                            } else if attr(&attrs, "type")
                                .is_some_and(|t| t.eq_ignore_ascii_case("text/markdown"))
                            {
                                f.markdown_alternate = Some(href);
                            }
                        }
                    }
                    "script" => f.scripts.push(Script {
                        src: attr(&attrs, "src").map(str::to_string),
                        kind: attr(&attrs, "type").map(str::to_string),
                        is_async: attr(&attrs, "async").is_some(),
                        defer: attr(&attrs, "defer").is_some(),
                        in_head: in_head || !seen_body,
                        body: raw.unwrap_or("").to_string(),
                        offset,
                    }),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = name.as_bytes()[1] - b'0';
                        captures.push(Capture {
                            tag: heading_tag(level),
                            kind: CaptureKind::Heading(level),
                            text: String::new(),
                        });
                        f.headings.push(Heading {
                            level,
                            text: String::new(),
                            in_main,
                            offset,
                        });
                    }
                    "a" | "area" => {
                        if name == "a" {
                            if let Some(n) = attr(&attrs, "name").filter(|v| !v.is_empty()) {
                                f.ids.push((n.to_string(), offset));
                            }
                        }
                        if let Some(href) = attr(&attrs, "href") {
                            f.links.push(Link {
                                href: href.to_string(),
                                text: String::new(),
                                aria_label: attr(&attrs, "aria-label")
                                    .or_else(|| attr(&attrs, "aria-labelledby"))
                                    .map(str::to_string),
                                title: attr(&attrs, "title").map(str::to_string),
                                in_main,
                                offset,
                            });
                            if name == "a" {
                                captures.push(Capture {
                                    tag: "a",
                                    kind: CaptureKind::Link(f.links.len() - 1),
                                    text: String::new(),
                                });
                            } else {
                                // <area> is void: its name is its alt.
                                let last = f.links.len() - 1;
                                f.links[last].text =
                                    normalize_space(attr(&attrs, "alt").unwrap_or(""));
                            }
                        }
                    }
                    "button" => {
                        f.buttons.push(Button {
                            name: String::new(),
                            offset,
                        });
                        button_labels.push(
                            attr(&attrs, "aria-label")
                                .or_else(|| attr(&attrs, "aria-labelledby"))
                                .or_else(|| attr(&attrs, "title"))
                                .filter(|v| !v.trim().is_empty())
                                .map(str::to_string),
                        );
                        captures.push(Capture {
                            tag: "button",
                            kind: CaptureKind::Button(f.buttons.len() - 1),
                            text: String::new(),
                        });
                    }
                    "label" => {
                        if let Some(target) = attr(&attrs, "for") {
                            f.label_for.push(target.to_string());
                        }
                        captures.push(Capture {
                            tag: "label",
                            kind: CaptureKind::Label,
                            text: String::new(),
                        });
                    }
                    "input" | "select" | "textarea" => {
                        let kind = attr(&attrs, "type").unwrap_or("").to_ascii_lowercase();
                        let exempt = name == "input"
                            && matches!(
                                kind.as_str(),
                                "hidden" | "submit" | "reset" | "button" | "image"
                            );
                        if !exempt {
                            let labelled = captures
                                .iter()
                                .any(|c| matches!(c.kind, CaptureKind::Label))
                                || ["aria-label", "aria-labelledby", "title"]
                                    .iter()
                                    .any(|a| attr(&attrs, a).is_some_and(|v| !v.trim().is_empty()));
                            f.controls.push(Control {
                                tag: name.clone(),
                                kind,
                                id: attr(&attrs, "id").map(str::to_string),
                                name: attr(&attrs, "name").map(str::to_string),
                                labelled,
                                offset,
                            });
                        }
                    }
                    "img" => {
                        let alt = attr(&attrs, "alt").map(str::to_string);
                        if let Some(alt) = &alt {
                            for c in captures.iter_mut() {
                                c.text.push_str(alt);
                                c.text.push(' ');
                            }
                        }
                        f.imgs.push(Img {
                            src: attr(&attrs, "src").map(str::to_string),
                            srcset: attr(&attrs, "srcset").map(str::to_string),
                            alt,
                            width: attr(&attrs, "width").map(str::to_string),
                            height: attr(&attrs, "height").map(str::to_string),
                            loading: attr(&attrs, "loading").map(str::to_string),
                            role_presentation: attr(&attrs, "role").is_some_and(|r| {
                                r.eq_ignore_ascii_case("presentation")
                                    || r.eq_ignore_ascii_case("none")
                            }) || attr(&attrs, "aria-hidden") == Some("true"),
                            in_main,
                            offset,
                        });
                    }
                    "svg" => {
                        // An inline icon with a <title> or aria-label names its link/button.
                        if let Some(label) = attr(&attrs, "aria-label") {
                            for c in captures.iter_mut() {
                                c.text.push_str(label);
                                c.text.push(' ');
                            }
                        }
                    }
                    _ => {}
                }
                if VOID.contains(&name.as_str()) {
                    continue;
                }
            }
            Token::End { name } => match name.as_str() {
                "head" => in_head = false,
                "main" => main_depth = main_depth.saturating_sub(1),
                "nav" | "header" | "footer" | "aside" => {
                    chrome_depth = chrome_depth.saturating_sub(1)
                }
                "title" | "a" | "button" | "label" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let Some(i) = captures.iter().rposition(|c| c.tag == name) else {
                        continue;
                    };
                    let capture = captures.remove(i);
                    let text = normalize_space(&capture.text);
                    match capture.kind {
                        CaptureKind::Title => f.titles.push(text),
                        CaptureKind::Heading(level) => {
                            if let Some(h) = f
                                .headings
                                .iter_mut()
                                .rev()
                                .find(|h| h.level == level && h.text.is_empty())
                            {
                                h.text = text;
                            }
                        }
                        CaptureKind::Link(i) => f.links[i].text = text,
                        CaptureKind::Button(i) => {
                            f.buttons[i].name =
                                button_labels.get(i).cloned().flatten().unwrap_or(text);
                        }
                        CaptureKind::Label => {}
                    }
                }
                _ => {}
            },
        }
    }
    // Unclosed captures (malformed HTML): keep what was collected.
    for capture in captures {
        let text = normalize_space(&capture.text);
        match capture.kind {
            CaptureKind::Title => f.titles.push(text),
            CaptureKind::Link(i) => f.links[i].text = text,
            CaptureKind::Button(i) => {
                f.buttons[i].name = button_labels.get(i).cloned().flatten().unwrap_or(text)
            }
            _ => {}
        }
    }
    let words = if f.has_main { &main_text } else { &body_text };
    f.main_words = count_words(words);
    f
}

fn heading_tag(level: u8) -> &'static str {
    ["h1", "h2", "h3", "h4", "h5", "h6"][usize::from(level.clamp(1, 6) - 1)]
}

/// Words in `text`: whitespace-separated tokens containing a letter or digit.
pub fn count_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_skips_comments_and_keeps_script_bodies() {
        let toks = tokenize(
            "<p>a<!-- <b>x</b> --></p><script type=\"application/ld+json\">{\"a\":\"</p>\"}</script>",
        );
        let names: Vec<String> = toks
            .iter()
            .filter_map(|t| match t {
                Token::Start { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(names, ["p", "script"]);
        let raw = toks.iter().find_map(|t| match t {
            Token::Start { raw: Some(r), .. } => Some(*r),
            _ => None,
        });
        assert_eq!(raw, Some("{\"a\":\"</p>\"}"));
    }

    #[test]
    fn test_decode_entities() {
        assert_eq!(
            decode_entities("a &amp; b &#39;c&#x27; &rarr; &bogus;"),
            "a & b 'c' → &bogus;"
        );
        assert_eq!(decode_entities("no entities"), "no entities");
        assert_eq!(decode_entities("&"), "&");
        // Multi-byte text right after a bare `&` (no panic on char boundaries).
        assert_eq!(decode_entities("a & b…c…d;"), "a & b…c…d;");
    }

    #[test]
    fn test_scan_head_facts() {
        let f = scan(
            r#"<!DOCTYPE html><html lang="en"><head><title>Hi &amp; bye</title>
            <meta name="description" content="Desc">
            <meta name="robots" content="noindex">
            <meta property="og:image" content="/x.png">
            <link rel="canonical" href="https://e.com/a">
            <link rel="alternate" type="text/markdown" href="/a.md">
            <link rel="alternate" hreflang="es" href="/es/a">
            <link rel="stylesheet" href="/s.css">
            <script src="/a.js"></script>
            <script type="application/ld+json">{"@type":"X"}</script>
            </head><body></body></html>"#,
        );
        assert_eq!(f.lang.as_deref(), Some("en"));
        assert_eq!(f.titles, ["Hi & bye"]);
        assert_eq!(f.description.as_deref(), Some("Desc"));
        assert_eq!(f.robots.as_deref(), Some("noindex"));
        assert!(f.og_image);
        assert_eq!(f.canonical.as_deref(), Some("https://e.com/a"));
        assert_eq!(f.markdown_alternate.as_deref(), Some("/a.md"));
        assert_eq!(f.hreflangs, [("es".to_string(), "/es/a".to_string())]);
        assert_eq!(f.stylesheets, ["/s.css"]);
        assert!(f.scripts[0].in_head && f.scripts[0].src.is_some());
        assert_eq!(f.json_ld().count(), 1);
    }

    #[test]
    fn test_scan_body_facts() {
        let f = scan(
            r##"<html><body><a href="#main" class="skip">Skip to content</a>
            <header><h1>Site</h1><nav><a href="/">Home</a></nav></header>
            <main id="main"><h1>Title</h1><h3>Deep</h3>
            <p>One two three, four.</p>
            <a href="/x"><img src="/i.png" alt="Logo"></a>
            <a href="/y" aria-label="Read the post">Read more</a>
            <a href="/z"></a>
            <button><svg aria-label="Close"></svg></button><button></button>
            <label for="q">Q</label><input id="q"><label>N <input name="n"></label>
            <input type="hidden" name="h"><textarea></textarea>
            <img src="/b.png" width="10" height="5" loading="lazy">
            <div id="dup"></div><span id="dup"></span>
            </main></body></html>"##,
        );
        assert!(f.has_main);
        assert!(f.has_skip_link());
        let h1s: Vec<_> = f.headings.iter().filter(|h| h.level == 1).collect();
        assert_eq!(h1s.len(), 2);
        assert!(!h1s[0].in_main && h1s[1].in_main);
        assert_eq!(h1s[1].text, "Title");
        let names: Vec<String> = f.links.iter().map(Link::accessible_name).collect();
        assert_eq!(
            names,
            ["Skip to content", "Home", "Logo", "Read the post", ""]
        );
        assert_eq!(f.buttons[0].name, "Close");
        assert_eq!(f.buttons[1].name, "");
        assert_eq!(f.label_for, ["q"]);
        let labelled: Vec<bool> = f.controls.iter().map(|c| c.labelled).collect();
        // #q is labelled by `for` (resolved by the check), `n` by wrapping.
        assert_eq!(labelled, [false, true, false]);
        assert_eq!(f.imgs.len(), 2);
        assert_eq!(f.imgs[0].alt.as_deref(), Some("Logo"));
        assert!(f.imgs[1].alt.is_none());
        assert_eq!(f.ids.iter().filter(|(id, _)| id == "dup").count(), 2);
        // Words in <main>: Title Deep One two three four Read more Q N
        assert_eq!(f.main_words, 10);
    }

    #[test]
    fn test_scan_refresh_and_missing_lang() {
        let f =
            scan(r#"<html><head><meta http-equiv="refresh" content="0; url=/a"></head></html>"#);
        assert!(f.refresh);
        assert!(f.lang.is_none());
    }

    #[test]
    fn test_line_of_and_count_words() {
        assert_eq!(line_of("a\nb\nc", 4), 3);
        assert_eq!(count_words("a - b , c3"), 3);
    }
}
