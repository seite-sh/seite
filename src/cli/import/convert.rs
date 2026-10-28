//! Body conversion shared by the importers: HTML → Markdown (`htmd`), seite
//! shortcode builders, video embeds, placeholder tokens, and small text
//! helpers.
//!
//! Anything that must survive the HTML → Markdown step verbatim (a generated
//! `{{< figure(...) >}}`, a WordPress `[gallery]` left for the user, a raw
//! `<video>`) is swapped for an alphanumeric token first (`htmd` escapes
//! Markdown punctuation but never letters/digits) and restored afterwards.
//!
//! Only shortcodes this module builds may contain `{{<` / `{{%` after
//! conversion: their arguments go through [`sc_string`], which breaks any
//! delimiter inside a value, and source text kept verbatim goes through
//! [`Tokens::push_untrusted`], which escapes delimiters as HTML entities.

use std::sync::LazyLock;

use regex::Regex;

/// Placeholder tokens that survive HTML → Markdown conversion unchanged.
#[derive(Default)]
pub(crate) struct Tokens {
    values: Vec<String>,
}

impl Tokens {
    /// Store `value` and return the token that stands for it.
    pub(crate) fn push(&mut self, value: String) -> String {
        let token = format!("SEITEIMPORTTOKEN{}X", self.values.len());
        self.values.push(value);
        token
    }

    /// Store source-derived text (raw HTML, a shortcode left for the user)
    /// with seite's shortcode delimiters escaped as entities, so restoring it
    /// can never introduce a shortcode call. `&lt;` / `&#37;` render as `<` /
    /// `%` in Markdown text and in HTML.
    pub(crate) fn push_untrusted(&mut self, value: &str) -> String {
        self.push(escape_delimiters_html(value))
    }

    /// Replace every token in `text` with its value (values may contain
    /// earlier tokens, so restore newest first).
    pub(crate) fn restore(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (i, value) in self.values.iter().enumerate().rev() {
            let token = format!("SEITEIMPORTTOKEN{i}X");
            out = out.replace(&token, value);
        }
        out
    }
}

/// Escape a value for a seite shortcode string argument (`"..."`).
pub(crate) fn sc_string(value: &str) -> String {
    let flat: String = value
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    flat.trim()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        // `>}}` / `%}}` would end the tag early; `{{<` / `{{%` would start one.
        .replace(">}}", "> }}")
        .replace("%}}", "% }}")
        .replace("{{<", "{{ <")
        .replace("{{%", "{{ %")
}

/// `{{<` → `{{&lt;`, `{{%` → `{{&#37;`.
pub(crate) fn escape_delimiters_html(text: &str) -> String {
    text.replace("{{<", "{{&lt;").replace("{{%", "{{&#37;")
}

/// A fenced code block whose fence is longer than any backtick run in `code`.
pub(crate) fn code_fence(lang: &str, code: &str) -> String {
    let longest = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    let lang: String = lang
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '-' | '_' | '.'))
        .collect();
    format!("{fence}{lang}\n{code}\n{fence}")
}

/// A seite inline shortcode: `{{< name(k="v", …) >}}` (empty values dropped).
pub(crate) fn shortcode(name: &str, args: &[(&str, String)]) -> String {
    let args: Vec<String> = args
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| format!("{k}=\"{}\"", sc_string(v)))
        .collect();
    format!("{{{{< {name}({}) >}}}}", args.join(", "))
}

/// `{{< figure(src=…, alt=…, caption=…) >}}`.
pub(crate) fn figure(src: &str, alt: &str, caption: &str, width: &str) -> String {
    let src = if is_dangerous_url(src) { "" } else { src };
    shortcode(
        "figure",
        &[
            ("src", src.to_string()),
            ("alt", alt.to_string()),
            ("caption", caption.to_string()),
            ("width", width.to_string()),
        ],
    )
}

/// The seite `youtube` / `vimeo` shortcode for a video URL, if it is one.
pub(crate) fn embed_for_url(url: &str) -> Option<String> {
    static YT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?:https?:)?//(?:www\.|m\.)?(?:youtube\.com/(?:watch\?(?:[^#]*&)?v=|embed/|shorts/|live/|v/)|youtube-nocookie\.com/embed/|youtu\.be/)([A-Za-z0-9_-]{6,})",
        )
        .expect("valid regex")
    });
    static YT_START: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[?&#](?:t|start)=(\d+)s?(?:$|&)").expect("valid regex"));
    static VIMEO: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?:https?:)?//(?:www\.)?(?:player\.)?vimeo\.com/(?:video/|channels/[^/]+/)?(\d+)",
        )
        .expect("valid regex")
    });
    let url = url.trim();
    if let Some(c) = YT.captures(url) {
        let start = YT_START
            .captures(url)
            .map(|s| s[1].to_string())
            .unwrap_or_default();
        return Some(shortcode(
            "youtube",
            &[("id", c[1].to_string()), ("start", start)],
        ));
    }
    VIMEO
        .captures(url)
        .map(|c| shortcode("vimeo", &[("id", c[1].to_string())]))
}

/// HTML → Markdown with fenced code, `-` bullets and inline links.
pub(crate) fn html_to_markdown(html: &str) -> String {
    use htmd::options::{BrStyle, BulletListMarker, HrStyle, Options};
    let converter = htmd::HtmlToMarkdown::builder()
        .options(Options {
            bullet_list_marker: BulletListMarker::Dash,
            ul_bullet_spacing: 1,
            ol_number_spacing: 1,
            hr_style: HrStyle::Dashes,
            // `\` hard breaks survive trailing-whitespace trimming.
            br_style: BrStyle::Backslash,
            ..Default::default()
        })
        .skip_tags(vec!["script", "style", "noscript", "head"])
        .build();
    // htmd only fails on I/O errors of its in-memory writer; fall back to
    // the text with tags stripped rather than losing the body.
    let md = converter.convert(html).unwrap_or_else(|_| strip_tags(html));
    tidy_markdown(&md)
}

/// [`html_to_markdown`] for untrusted HTML: links and images whose
/// destination can run script become their text ([`sanitize_markdown_links`]);
/// a note says how many.
pub(crate) fn markdown_from_html(html: &str, notes: &mut Vec<String>) -> String {
    let (md, replaced) = sanitize_markdown_links(&html_to_markdown(html));
    if replaced > 0 {
        notes.push(format!(
            "{replaced} link(s)/image(s) with a script URL (javascript:, vbscript:, data:) were replaced by their text"
        ));
    }
    md
}

/// Collapse runs of blank lines and trim the ends.
pub(crate) fn tidy_markdown(md: &str) -> String {
    static BLANKS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\n[ \t]*\n(?:[ \t]*\n)+").expect("valid regex"));
    let md = md.replace("\r\n", "\n");
    let md: Vec<&str> = md.lines().map(str::trim_end).collect();
    BLANKS
        .replace_all(&md.join("\n"), "\n\n")
        .trim()
        .to_string()
}

/// Text of an HTML fragment: tags removed, common entities decoded,
/// whitespace collapsed.
pub(crate) fn strip_tags(html: &str) -> String {
    static TAG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").expect("valid regex"));
    let text = TAG.replace_all(html, " ");
    let text = decode_entities(&text);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Decode the handful of entities that appear in titles and excerpts.
pub(crate) fn decode_entities(text: &str) -> String {
    static ENT: LazyLock<Regex> = LazyLock::new(|| {
        // Numeric references may omit the `;` in HTML (`java&#x09script:`).
        Regex::new(r"&(?:(#[0-9]+|#[xX][0-9a-fA-F]+);?|([a-zA-Z]+);)").expect("valid regex")
    });
    ENT.replace_all(text, |c: &regex::Captures| {
        let name = c.get(1).or(c.get(2)).map_or("", |m| m.as_str());
        let decoded = if let Some(hex) = name.strip_prefix("#x").or(name.strip_prefix("#X")) {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else if let Some(dec) = name.strip_prefix('#') {
            dec.parse().ok().and_then(char::from_u32)
        } else {
            match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                "hellip" => Some('…'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "lsquo" => Some('‘'),
                "rsquo" => Some('’'),
                "ldquo" => Some('“'),
                "rdquo" => Some('”'),
                "copy" => Some('©'),
                "colon" => Some(':'),
                "Tab" => Some('\t'),
                "NewLine" => Some('\n'),
                _ => None,
            }
        };
        decoded.map_or_else(|| c[0].to_string(), |ch| ch.to_string())
    })
    .into_owned()
}

/// Value of attribute `name` in an HTML start tag (`<img src="…">`).
pub(crate) fn attr(tag: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(
        r#"(?is)\s{}\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+))"#,
        regex::escape(name)
    ))
    .ok()?;
    let c = re.captures(tag)?;
    let v = c.get(1).or(c.get(2)).or(c.get(3))?.as_str();
    Some(decode_entities(v))
}

/// A URL that can run script when followed or loaded: `javascript:`,
/// `vbscript:`, or `data:` other than PNG/JPEG/GIF/WebP images. Checked
/// after decoding entities and dropping whitespace/control characters, case
/// insensitively (`java&#x09;script:`, ` JaVaScRiPt:`).
pub(crate) fn is_dangerous_url(raw: &str) -> bool {
    let decoded = decode_entities(raw);
    let compact: String = decoded
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control() && *c != '\\')
        .collect::<String>()
        .to_ascii_lowercase();
    if compact.starts_with("javascript:") || compact.starts_with("vbscript:") {
        return true;
    }
    match compact.strip_prefix("data:") {
        Some(rest) => !["image/png", "image/jpeg", "image/gif", "image/webp"]
            .iter()
            .any(|ok| rest.starts_with(ok)),
        None => false,
    }
}

/// Elements kept in sanitized HTML, with their allowed attributes.
/// Anything else is unwrapped (its text and allowed children kept), except
/// [`DROPPED_ELEMENTS`], which go with everything inside them.
const ALLOWED_ELEMENTS: &[(&str, &[&str])] = &[
    (
        "video",
        &[
            "src",
            "poster",
            "width",
            "height",
            "controls",
            "loop",
            "muted",
            "playsinline",
            "preload",
            "title",
        ],
    ),
    (
        "audio",
        &["src", "controls", "loop", "muted", "preload", "title"],
    ),
    ("source", &["src", "type", "media"]),
    ("track", &["src", "kind", "srclang", "label", "default"]),
    (
        "iframe",
        &[
            "src",
            "width",
            "height",
            "title",
            "allow",
            "allowfullscreen",
            "loading",
            "frameborder",
            "referrerpolicy",
        ],
    ),
    (
        "img",
        &["src", "alt", "width", "height", "title", "loading"],
    ),
    ("figure", &[]),
    ("figcaption", &[]),
    ("a", &["href", "title"]),
    ("p", &[]),
    ("br", &[]),
    ("em", &[]),
    ("strong", &[]),
];

/// Elements removed with their whole content.
const DROPPED_ELEMENTS: &[&str] = &[
    "script", "style", "object", "embed", "applet", "svg", "math", "template", "noscript", "base",
    "link", "meta", "form", "input", "button", "textarea", "select", "frame", "frameset",
];

const VOID_ELEMENTS: &[&str] = &["img", "source", "track", "br"];

/// Whether a URL attribute value may be kept: relative, or http(s); `data:`
/// PNG/JPEG/GIF/WebP only on `<img>`. The value is the parser's decoded one.
fn url_allowed(value: &str, element: &str) -> bool {
    let compact: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    let scheme = compact.split_once(':').map(|(s, _)| s).filter(|s| {
        !s.is_empty()
            && !s.contains(['/', '?', '#'])
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    match scheme {
        None => true,
        Some("http" | "https") => true,
        Some("data") if element == "img" => !is_dangerous_url(&compact),
        Some(_) => false,
    }
}

/// Escape text or a double-quoted attribute value for HTML output.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Sanitize an HTML fragment kept verbatim (iframes on allowed hosts,
/// video/audio): it is parsed with html5ever (the same parser browsers'
/// tokenization rules follow) and **rebuilt from the parsed tree** through
/// [`ALLOWED_ELEMENTS`]; event handlers, `style`, unknown attributes and
/// non-http(s) URLs never make it out, however the source spelled them
/// (`<video/onerror=…>`, entity-encoded schemes, …). Iframes keep their
/// `src` only on an allowed player host ([`iframe_allowed`]); an iframe
/// without one is dropped.
pub(crate) fn sanitize_html_fragment(html: &str) -> String {
    let converter = htmd::HtmlToMarkdown::new();
    let Ok(document) = converter.html_to_tree(html) else {
        return String::new();
    };
    let mut out = String::new();
    // html → (head, body); the fragment lives in body (iframes/video may be
    // hoisted into head in odd documents, so walk both).
    for child in document.children.borrow().iter() {
        write_sanitized(child, &mut out, true);
    }
    out
}

fn write_sanitized(node: &markup5ever_rcdom::Handle, out: &mut String, structural: bool) {
    use markup5ever_rcdom::NodeData;
    match &node.data {
        NodeData::Text { contents } => out.push_str(&html_escape(&contents.borrow())),
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref().to_ascii_lowercase();
            if structural && matches!(tag.as_str(), "html" | "head" | "body") {
                for child in node.children.borrow().iter() {
                    write_sanitized(child, out, true);
                }
                return;
            }
            if DROPPED_ELEMENTS.contains(&tag.as_str()) {
                return;
            }
            let Some((_, allowed)) = ALLOWED_ELEMENTS.iter().find(|(t, _)| *t == tag) else {
                for child in node.children.borrow().iter() {
                    write_sanitized(child, out, false);
                }
                return;
            };
            let attrs = attrs.borrow();
            let mut kept = String::new();
            for attr in attrs.iter() {
                let key = attr.name.local.as_ref().to_ascii_lowercase();
                if !attr.name.ns.is_empty() || !allowed.contains(&key.as_str()) {
                    continue;
                }
                let value: &str = &attr.value;
                let is_url = matches!(key.as_str(), "src" | "href" | "poster");
                if is_url && !url_allowed(value, &tag) {
                    continue;
                }
                if tag == "iframe" && key == "src" && !iframe_allowed(value) {
                    continue;
                }
                kept.push_str(&format!(" {key}=\"{}\"", html_escape(value)));
            }
            if tag == "iframe" && !kept.contains(" src=\"") {
                return;
            }
            out.push('<');
            out.push_str(&tag);
            out.push_str(&kept);
            out.push('>');
            if VOID_ELEMENTS.contains(&tag.as_str()) {
                return;
            }
            // Iframe content is not markup; nothing inside is kept.
            if tag != "iframe" {
                for child in node.children.borrow().iter() {
                    write_sanitized(child, out, false);
                }
            }
            out.push_str("</");
            out.push_str(&tag);
            out.push('>');
        }
        // Comments, doctypes, processing instructions: dropped.
        _ => {
            for child in node.children.borrow().iter() {
                write_sanitized(child, out, structural);
            }
        }
    }
}

/// The `src` of the first `<iframe>` in an HTML fragment, as the parser
/// reads it (handles `<iframe/src=…>` and entity-encoded values).
pub(crate) fn iframe_src(html: &str) -> Option<String> {
    fn find(node: &markup5ever_rcdom::Handle) -> Option<String> {
        use markup5ever_rcdom::NodeData;
        if let NodeData::Element { name, attrs, .. } = &node.data {
            if name.local.as_ref().eq_ignore_ascii_case("iframe") {
                return attrs
                    .borrow()
                    .iter()
                    .find(|a| a.name.local.as_ref().eq_ignore_ascii_case("src"))
                    .map(|a| a.value.to_string());
            }
        }
        node.children.borrow().iter().find_map(find)
    }
    let document = htmd::HtmlToMarkdown::new().html_to_tree(html).ok()?;
    find(&document)
}

/// Iframe hosts kept as HTML: YouTube, Vimeo and `player.*` hosts, https only.
fn iframe_allowed(src: &str) -> bool {
    let Ok(u) = url::Url::parse(src.trim()) else {
        return false;
    };
    let Some(host) = u.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    let on = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    u.scheme() == "https"
        && (on("youtube.com")
            || on("youtube-nocookie.com")
            || on("vimeo.com")
            || host.starts_with("player."))
}

/// Links and images whose destination can run script, found the way the
/// build will read the Markdown (pulldown-cmark with the build's options,
/// so escapes, entities and `<…>` destinations are already resolved): each
/// is replaced by its text (an image by its alt text). Returns the new
/// Markdown and how many were replaced.
pub(crate) fn sanitize_markdown_links(md: &str) -> (String, usize) {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let options = build_markdown_options();

    // (span, dangerous, text inside)
    let mut stack: Vec<(std::ops::Range<usize>, bool, String)> = Vec::new();
    let mut replacements: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for (event, range) in Parser::new_ext(md, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. }) => {
                stack.push((range, is_dangerous_url(&dest_url), String::new()));
            }
            Event::End(TagEnd::Link) | Event::End(TagEnd::Image) => {
                let Some((span, dangerous, text)) = stack.pop() else {
                    continue;
                };
                let ancestor_dangerous = stack.iter().any(|s| s.1);
                if let Some(parent) = stack.last_mut() {
                    parent.2.push_str(&text);
                }
                if dangerous && !ancestor_dangerous {
                    replacements.push((span, escape_markdown_text(&text)));
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some(top) = stack.last_mut() {
                    top.2.push_str(&t);
                }
            }
            _ => {}
        }
    }
    let count = replacements.len();
    let mut out = md.to_string();
    replacements.sort_by_key(|(r, _)| std::cmp::Reverse(r.start));
    for (range, text) in replacements {
        if range.end <= out.len()
            && out.is_char_boundary(range.start)
            && out.is_char_boundary(range.end)
        {
            out.replace_range(range, &text);
        }
    }
    (out, count)
}

/// Backslash-escape Markdown punctuation in plain text.
fn escape_markdown_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Lines of a Markdown body (Jekyll, Hugo; kept as written) with HTML that
/// can run script: `<script`, `on*=` attributes, `javascript:` URLs.
pub(crate) fn untrusted_html_lines(body: &str) -> Vec<(usize, &'static str)> {
    static RISKY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)(<script\b)|(<[a-z][^>]*[\s/"']on[a-z]+\s*=)|(javascript\s*:)"#)
            .expect("valid regex")
    });
    let mut out: Vec<(usize, &'static str)> = Vec::new();
    for c in RISKY.captures_iter(body) {
        let what = if c.get(1).is_some() {
            "<script>"
        } else if c.get(2).is_some() {
            "an on* event attribute"
        } else {
            "a javascript: URL"
        };
        out.push((line_of(body, c.get(0).expect("match").start()), what));
    }
    // Obfuscated destinations (`&#106;avascript:`, `java\script:`) as the
    // build will read them.
    for span in dangerous_link_spans(body) {
        out.push((line_of(body, span.start), "a link with a script URL"));
    }
    out.sort();
    out.dedup_by_key(|(l, _)| *l);
    out
}

/// Source spans of links/images whose destination (as pulldown-cmark
/// resolves it) can run script.
fn dangerous_link_spans(md: &str) -> Vec<std::ops::Range<usize>> {
    use pulldown_cmark::{Event, Parser, Tag};
    Parser::new_ext(md, build_markdown_options())
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. })
                if is_dangerous_url(&dest_url) =>
            {
                Some(range)
            }
            _ => None,
        })
        .collect()
}

/// The pulldown-cmark options `src/build/markdown.rs` renders with.
fn build_markdown_options() -> pulldown_cmark::Options {
    use pulldown_cmark::Options;
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options
}

/// Generic HTML preparation before [`html_to_markdown`] (WordPress, Ghost,
/// Jekyll HTML): video iframes → `youtube`/`vimeo` shortcodes, `<figure>`
/// with an image → `figure` shortcode; other iframes on an allow-listed
/// player host and video/audio are kept as sanitized HTML, other iframes
/// become links. `notes` collects what was changed for the report.
pub(crate) fn prepare_html(html: &str, tokens: &mut Tokens, notes: &mut Vec<String>) -> String {
    static IFRAME: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?is)<iframe\b(?:[^>"']|"[^"]*"|'[^']*')*>.*?</iframe>|<iframe\b(?:[^>"']|"[^"]*"|'[^']*')*/>"#).expect("valid regex")
    });
    static MEDIA: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?is)<(video|audio)\b(?:[^>"']|"[^"]*"|'[^']*')*>.*?</(?:video|audio)>"#)
            .expect("valid regex")
    });
    static SCRIPTY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?is)<script\b|\son[a-z]+\s*=|(?:href|src)\s*=\s*["']?\s*(?:javascript|vbscript|data)\s*:"#)
            .expect("valid regex")
    });
    if SCRIPTY.is_match(html) {
        notes.push(
            "the HTML had scripts, on* event handlers or script URLs; they were removed"
                .to_string(),
        );
    }
    let html = IFRAME.replace_all(html, |c: &regex::Captures| {
        let tag = &c[0];
        let src = iframe_src(tag).unwrap_or_default();
        let token = if let Some(embed) = embed_for_url(&src) {
            tokens.push(embed)
        } else if iframe_allowed(&src) {
            tokens.push_untrusted(&sanitize_html_fragment(tag))
        } else if src.trim().is_empty() || !url_allowed(&src, "a") || !src.contains("://") {
            notes.push("an iframe without a usable http(s) src was removed".to_string());
            tokens.push(String::new())
        } else {
            notes.push(format!(
                "iframe from {src} replaced by a link (only YouTube, Vimeo and player.* embeds are kept)"
            ));
            let escaped = escape_delimiters_html(&html_escape(&src));
            tokens.push(format!("<a href=\"{escaped}\">{escaped}</a>"))
        };
        format!("<p>{token}</p>")
    });
    let html = MEDIA.replace_all(&html, |c: &regex::Captures| {
        format!(
            "<p>{}</p>",
            tokens.push_untrusted(&sanitize_html_fragment(&c[0]))
        )
    });
    // `<pre>` without `<code>` (classic editor) would lose its formatting.
    static PRE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?is)<pre\b([^>]*)>(.*?)</pre>").expect("valid regex"));
    let html = PRE.replace_all(&html, |c: &regex::Captures| {
        if c[2].to_ascii_lowercase().contains("<code") {
            c[0].to_string()
        } else {
            format!("<pre{}><code>{}</code></pre>", &c[1], &c[2])
        }
    });
    convert_figures(&html, tokens)
}

/// Replace innermost `<figure>` elements holding one image (or one video
/// embed) with shortcodes; other figures are left to the converter.
fn convert_figures(html: &str, tokens: &mut Tokens) -> String {
    static IMG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?is)<img\b[^>]*>").expect("valid regex"));
    static CAPTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)<figcaption\b[^>]*>(.*?)</figcaption>").expect("valid regex")
    });
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    // Each `</figure>` closes the nearest `<figure` before it that starts
    // after the previous close: the innermost one.
    while let Some(rel_end) = lower[cursor..].find("</figure>") {
        let end = cursor + rel_end;
        let Some(start) = lower[cursor..end].rfind("<figure").map(|s| cursor + s) else {
            out.push_str(&html[cursor..end + 9]);
            cursor = end + 9;
            continue;
        };
        let inner = &html[start..end];
        let imgs: Vec<_> = IMG.find_iter(inner).collect();
        let replacement = if imgs.len() == 1 {
            let tag = imgs[0].as_str();
            let src = attr(tag, "src").unwrap_or_default();
            let alt = attr(tag, "alt").unwrap_or_default();
            let caption = CAPTION
                .captures(inner)
                .map(|c| strip_tags(&c[1]))
                .unwrap_or_default();
            (!src.is_empty()).then(|| figure(&src, &alt, &caption, ""))
        } else {
            None
        };
        out.push_str(&html[cursor..start]);
        match replacement {
            Some(r) => {
                out.push_str("<p>");
                out.push_str(&tokens.push(r));
                out.push_str("</p>");
            }
            None => out.push_str(&html[start..end + 9]),
        }
        cursor = end + 9;
    }
    out.push_str(&html[cursor..]);
    out
}

/// Byte ranges of fenced code blocks (``` or ~~~) in Markdown.
pub(crate) fn fenced_ranges(md: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut open: Option<(usize, char, usize)> = None;
    let mut offset = 0;
    for line in md.split_inclusive('\n') {
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        let fence_char = trimmed.chars().next();
        let run = |c: char| trimmed.chars().take_while(|&x| x == c).count();
        match open {
            None => {
                if indent <= 3 {
                    if let Some(c @ ('`' | '~')) = fence_char {
                        let n = run(c);
                        if n >= 3 {
                            open = Some((offset, c, n));
                        }
                    }
                }
            }
            Some((start, c, n)) => {
                if indent <= 3
                    && fence_char == Some(c)
                    && run(c) >= n
                    && trimmed[run(c)..].trim().is_empty()
                {
                    ranges.push(start..offset + line.len());
                    open = None;
                }
            }
        }
        offset += line.len();
    }
    if let Some((start, _, _)) = open {
        ranges.push(start..md.len());
    }
    ranges
}

/// Keep text that seite would read as a shortcode (`{{<`, `{{%`) from being
/// one: `{{\<` / `{{\%` render the same characters in Markdown. Fenced code
/// is left alone (the shortcode parser skips it).
pub(crate) fn neutralize_shortcode_delimiters(md: &str) -> String {
    if !md.contains("{{<") && !md.contains("{{%") {
        return md.to_string();
    }
    let fences = fenced_ranges(md);
    let mut out = String::with_capacity(md.len() + 8);
    let mut last = 0;
    for range in fences.iter().chain(std::iter::once(&(md.len()..md.len()))) {
        let text = &md[last..range.start];
        out.push_str(&text.replace("{{<", "{{\\<").replace("{{%", "{{\\%"));
        out.push_str(&md[range.start..range.end]);
        last = range.end;
    }
    out
}

/// 1-based line of byte `offset` in `text`.
pub(crate) fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].matches('\n').count() + 1
}

/// `YYYY-MM-DD` at the start of `s` (any time/offset after it is ignored:
/// the calendar date as the author wrote it).
pub(crate) fn leading_date(s: &str) -> Option<chrono::NaiveDate> {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    let head = s.get(..10)?;
    chrono::NaiveDate::parse_from_str(head, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcode_escapes_quotes_and_drops_empty() {
        assert_eq!(
            figure("/a.png", "say \"hi\"", "", ""),
            r#"{{< figure(src="/a.png", alt="say \"hi\"") >}}"#
        );
    }

    #[test]
    fn test_embed_for_url() {
        assert_eq!(
            embed_for_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42s").as_deref(),
            Some(r#"{{< youtube(id="dQw4w9WgXcQ", start="42") >}}"#)
        );
        assert_eq!(
            embed_for_url("https://youtu.be/dQw4w9WgXcQ").as_deref(),
            Some(r#"{{< youtube(id="dQw4w9WgXcQ") >}}"#)
        );
        assert_eq!(
            embed_for_url("https://player.vimeo.com/video/76979871").as_deref(),
            Some(r#"{{< vimeo(id="76979871") >}}"#)
        );
        assert!(embed_for_url("https://example.com/watch?v=x").is_none());
    }

    #[test]
    fn test_prepare_html_figure_and_iframe() {
        let mut tokens = Tokens::default();
        let html = r#"<figure class="wp-block-image"><img src="https://ex.com/a.jpg" alt="A"/><figcaption>The <em>cap</em></figcaption></figure><iframe src="https://www.youtube.com/embed/dQw4w9WgXcQ"></iframe>"#;
        let prepared = prepare_html(html, &mut tokens, &mut Vec::new());
        let md = tokens.restore(&html_to_markdown(&prepared));
        assert!(
            md.contains(
                r#"{{< figure(src="https://ex.com/a.jpg", alt="A", caption="The cap") >}}"#
            ),
            "{md}"
        );
        assert!(md.contains(r#"{{< youtube(id="dQw4w9WgXcQ") >}}"#), "{md}");
    }

    #[test]
    fn test_html_to_markdown_basics() {
        let md = html_to_markdown(
            "<h2>T</h2><p>a <strong>b</strong></p><ul><li>x</li></ul><pre><code class=\"language-rust\">fn x() {}</code></pre>",
        );
        assert_eq!(md, "## T\n\na **b**\n\n- x\n\n```rust\nfn x() {}\n```");
    }

    #[test]
    fn test_neutralize_shortcode_delimiters_skips_fences() {
        let md = "a {{< foo >}}\n\n```\n{{< keep >}}\n```\n";
        assert_eq!(
            neutralize_shortcode_delimiters(md),
            "a {{\\< foo >}}\n\n```\n{{< keep >}}\n```\n"
        );
    }

    #[test]
    fn test_leading_date_ignores_time_and_offset() {
        assert_eq!(
            leading_date("2020-01-02T23:30:00-08:00"),
            chrono::NaiveDate::from_ymd_opt(2020, 1, 2)
        );
        assert_eq!(leading_date("not a date"), None);
    }

    #[test]
    fn test_is_dangerous_url() {
        for bad in [
            "javascript:alert(1)",
            " JaVaScRiPt:alert(1)",
            "java&#x09;script:alert(1)",
            "java&#9script:x",
            "javascript&colon;x",
            "vbscript:msgbox",
            "data:text/html;base64,PHNjcmlwdD4=",
            "data:image/svg+xml,<svg onload=alert(1)>",
        ] {
            assert!(is_dangerous_url(bad), "{bad}");
        }
        for ok in [
            "https://ex.com/a",
            "/posts/x",
            "#top",
            "data:image/png;base64,AAAA",
            "mailto:a@b.c",
        ] {
            assert!(!is_dangerous_url(ok), "{ok}");
        }
    }

    #[test]
    fn test_untrusted_html_is_sanitized() {
        let mut tokens = Tokens::default();
        let mut notes = Vec::new();
        let html = r#"<p><a href="java&#x09;script:alert(1)" onclick="x()">click</a> <img src="x.png" onerror="alert(1)"></p>
<iframe src="https://evil.example/frame" onload="x()"></iframe>
<iframe src="https://player.example.com/embed/1" onload="x()" srcdoc="<script>"></iframe>
<video src="javascript:alert(1)" poster="https://ex.com/p.jpg" onplay="x()"><script>alert(1)</script><source src="v.mp4"></video>"#;
        let prepared = prepare_html(html, &mut tokens, &mut notes);
        let md = tokens.restore(&markdown_from_html(&prepared, &mut notes));
        let lower = md.to_ascii_lowercase();
        assert!(
            !lower.contains("javascript")
                && !lower.contains("onload")
                && !lower.contains("onerror"),
            "{md}"
        );
        assert!(
            !lower.contains("onclick")
                && !lower.contains("onplay")
                && !lower.contains("<script")
                && !lower.contains("srcdoc"),
            "{md}"
        );
        let (rendered, _) = crate::build::markdown::markdown_to_html(&md);
        assert!(
            rendered.contains("click") && !rendered.to_ascii_lowercase().contains("href=\"java"),
            "{rendered}"
        );
        assert!(
            md.contains(r#"<a href="https://evil.example/frame">"#),
            "{md}"
        );
        assert!(
            md.contains(r#"<iframe src="https://player.example.com/embed/1">"#),
            "{md}"
        );
        assert!(
            md.contains(r#"poster="https://ex.com/p.jpg""#)
                && md.contains(r#"<source src="v.mp4">"#),
            "{md}"
        );
        assert!(
            notes.iter().any(|n| n.contains("evil.example")),
            "{notes:?}"
        );
        assert!(notes.iter().any(|n| n.contains("removed")), "{notes:?}");
    }

    /// Parser-differential bypasses of a regex sanitizer.
    #[test]
    fn test_sanitizer_bypasses() {
        for html in [
            "<video/onerror=alert(1) src=x.mp4></video>",
            "<audio src=x onerror\n=alert(1)></audio>",
            "<video src=x.mp4><source src=java&#x09;script:alert(1)></video>",
            "<video poster=\"JaVaScRiPt:alert(1)\" style=\"x:expression(1)\"></video>",
            "<iframe/src=javascript:alert(1)></iframe>",
            "<iframe src=\"https://player.example.com/x\"><script>alert(1)</script></iframe>",
        ] {
            let out = sanitize_html_fragment(html).to_ascii_lowercase();
            assert!(
                !out.contains("onerror")
                    && !out.contains("javascript")
                    && !out.contains("<script")
                    && !out.contains("style="),
                "{html} → {out}"
            );
        }
        assert_eq!(
            sanitize_html_fragment("<video/onerror=alert(1) src=x.mp4 controls></video>"),
            r#"<video src="x.mp4" controls=""></video>"#
        );
        // An iframe off the allow-list loses its src and is dropped.
        assert_eq!(
            sanitize_html_fragment(r#"<iframe src="https://evil.example/x"></iframe>"#),
            ""
        );
        assert_eq!(
            iframe_src("<iframe/src=javascript:alert(1)></iframe>").as_deref(),
            Some("javascript:alert(1)")
        );
        let mut tokens = Tokens::default();
        let mut notes = Vec::new();
        let prepared = prepare_html(
            "<iframe/src=javascript:alert(1)></iframe>",
            &mut tokens,
            &mut notes,
        );
        let out = tokens.restore(&markdown_from_html(&prepared, &mut notes));
        assert!(!out.to_ascii_lowercase().contains("javascript"), "{out}");
    }

    #[test]
    fn test_markdown_links_checked_as_rendered() {
        for md in [
            "[x](java\\script:alert(1))",
            "[x](&#106;avascript:alert(1))",
            "[x](<javascript:alert(1)>)",
            "![pic](JaVa&#x09;Script:alert(1))",
            "[x][r]\n\n[r]: javascript:alert(1)",
            "[x](data:text/html;base64,PHNjcmlwdD4=)",
        ] {
            let (out, n) = sanitize_markdown_links(md);
            assert_eq!(n, 1, "{md} → {out}");
            let (html, _) = crate::build::markdown::markdown_to_html(&out);
            assert!(
                !html.to_ascii_lowercase().contains("href=\"j")
                    && !html.contains("src=\"J")
                    && !html.contains("data:text"),
                "{md} → {html}"
            );
        }
        let (out, n) =
            sanitize_markdown_links("[ok](https://ex.com) ![i](data:image/png;base64,AA)");
        assert_eq!(
            (out.as_str(), n),
            ("[ok](https://ex.com) ![i](data:image/png;base64,AA)", 0)
        );
        assert_eq!(
            untrusted_html_lines("a\n\n[x](&#106;avascript:1)\n").len(),
            1
        );
    }

    #[test]
    fn test_untrusted_html_lines() {
        let body =
            "ok\n<div onclick=\"x()\">a</div>\n[x](javascript:alert(1))\n<script>1</script>\n";
        let lines: Vec<usize> = untrusted_html_lines(body)
            .into_iter()
            .map(|(l, _)| l)
            .collect();
        assert_eq!(lines, vec![2, 3, 4]);
    }

    #[test]
    fn test_decode_entities() {
        assert_eq!(
            decode_entities("a &amp; b &#8217; &#x41; &bogus;"),
            "a & b ’ A &bogus;"
        );
    }
}
