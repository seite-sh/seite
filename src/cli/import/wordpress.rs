//! WordPress WXR export (`Tools → Export` → `.xml`).
//!
//! Parsed with quick-xml over the whole file (capped at 200 MB): no DTD,
//! no external or custom entity expansion (unknown `&name;` references are
//! kept as text). Attachments are read first so `_thumbnail_id` resolves to
//! a featured image. Bodies: block comments stripped, `wpautop` for classic
//! (non-block) content, `[caption]` → `figure`, `[embed]` / YouTube / Vimeo
//! URLs → `youtube` / `vimeo`, `[gallery]` and unknown shortcodes left in
//! place and reported with their line.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::bail;
use quick_xml::events::Event;
use regex::Regex;

use super::convert::{self, Tokens};
use super::{Item, Kind, ParseCtx, Parsed, Skipped, Warning};

/// One `<item>` as read.
#[derive(Default, Debug)]
struct RawItem {
    line: usize,
    content_line: usize,
    fields: HashMap<String, String>,
    /// (domain, nicename, name)
    terms: Vec<(String, String, String)>,
    meta: Vec<(String, String)>,
}

impl RawItem {
    fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(String::as_str).unwrap_or("")
    }
}

#[derive(Default, Debug)]
struct Channel {
    link: String,
    base_site_url: String,
    base_blog_url: String,
    /// login → display name
    authors: HashMap<String, String>,
    items: Vec<RawItem>,
}

pub(crate) fn parse(path: &Path, ctx: &ParseCtx) -> anyhow::Result<Parsed> {
    let text = super::read_export(path, &ctx.display)?;
    let channel = read_wxr(&text, &ctx.display)?;

    let site_url = ctx
        .site_url
        .clone()
        .or_else(|| http_url(&channel.base_blog_url))
        .or_else(|| http_url(&channel.link))
        .or_else(|| http_url(&channel.base_site_url));
    let mut parsed = Parsed {
        site_url: site_url.clone(),
        ..Default::default()
    };

    let attachments: HashMap<&str, &str> = channel
        .items
        .iter()
        .filter(|i| i.get("wp:post_type") == "attachment")
        .map(|i| (i.get("wp:post_id"), i.get("wp:attachment_url")))
        .filter(|(id, url)| !id.is_empty() && !url.is_empty())
        .collect();

    for raw in &channel.items {
        let post_type = raw.get("wp:post_type");
        let id = raw.get("wp:post_id");
        let source_id = format!("wp:{}", if id.is_empty() { "?" } else { id });
        let source = format!("{}:{}", ctx.display, raw.line);
        let title = convert::decode_entities(raw.get("title").trim());
        let kind = match post_type {
            "post" => Kind::Post,
            "page" => Kind::Page,
            "attachment"
            | "revision"
            | "nav_menu_item"
            | "customize_changeset"
            | "custom_css"
            | "oembed_cache"
            | "user_request"
            | "wp_block"
            | "wp_template"
            | "wp_template_part"
            | "wp_global_styles"
            | "wp_navigation"
            | "wp_font_family"
            | "wp_font_face" => continue,
            other => {
                parsed.skipped.push(Skipped {
                    source_id,
                    source,
                    kind: None,
                    title,
                    reason: format!("post type '{other}' is not imported (only posts and pages)"),
                });
                continue;
            }
        };
        let status = raw.get("wp:status");
        let draft = match status {
            "publish" if !raw.get("wp:post_password").is_empty() => {
                Some("password-protected in WordPress".to_string())
            }
            "publish" => None,
            "draft" | "pending" | "private" | "future" => Some(match status {
                "future" => "scheduled".to_string(),
                s => s.to_string(),
            }),
            s => {
                parsed.skipped.push(Skipped {
                    source_id,
                    source,
                    kind: Some(kind),
                    title,
                    reason: format!(
                        "status '{}' is never imported",
                        if s.is_empty() { "none" } else { s }
                    ),
                });
                continue;
            }
        };

        let mut item = Item {
            source_id: source_id.clone(),
            source_ref: source.clone(),
            kind: Some(kind),
            title: if title.is_empty() {
                "Untitled".to_string()
            } else {
                title
            },
            slug: raw.get("wp:post_name").to_string(),
            draft,
            ..Default::default()
        };
        item.date =
            wp_date(raw.get("wp:post_date")).or_else(|| wp_date(raw.get("wp:post_date_gmt")));
        item.updated = wp_date(raw.get("wp:post_modified")).filter(|u| Some(*u) > item.date);
        let excerpt = convert::strip_tags(raw.get("excerpt:encoded"));
        item.description = (!excerpt.is_empty()).then_some(excerpt);
        let creator = raw.get("dc:creator").trim();
        if !creator.is_empty() {
            item.author = Some(
                channel
                    .authors
                    .get(creator)
                    .cloned()
                    .unwrap_or_else(|| creator.to_string()),
            );
        }
        for (domain, nicename, name) in &raw.terms {
            let name = convert::decode_entities(name.trim());
            match domain.as_str() {
                "post_tag" => push_term(&mut item.tags, &name),
                "category" if nicename != "uncategorized" => push_term(&mut item.categories, &name),
                _ => {}
            }
        }
        for (key, value) in &raw.meta {
            match key.as_str() {
                "_thumbnail_id" => {
                    if let Some(url) = attachments.get(value.trim()) {
                        item.image = Some(url.to_string());
                    }
                }
                "_wp_old_slug" => {
                    if let Some(alias) = old_slug_alias(raw.get("link"), value, site_url.as_ref()) {
                        item.aliases.push(alias);
                    }
                }
                _ => {}
            }
        }
        if let Some(alias) = same_site_path(raw.get("link"), site_url.as_ref()) {
            item.aliases.insert(0, alias);
        }

        let mut warnings = Vec::new();
        item.body = convert_body(
            raw.get("content:encoded"),
            raw.content_line,
            &item,
            &mut warnings,
        );
        parsed.warnings.extend(warnings);
        parsed.items.push(item);
    }
    Ok(parsed)
}

fn push_term(terms: &mut Vec<String>, name: &str) {
    if !name.is_empty() && !terms.iter().any(|t| t.eq_ignore_ascii_case(name)) {
        terms.push(name.to_string());
    }
}

fn http_url(s: &str) -> Option<url::Url> {
    url::Url::parse(s.trim())
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
}

/// `2020-01-02 10:00:00` → the date as written (WordPress stores the site's
/// local time); `0000-00-00 …` (unpublished) → None.
fn wp_date(s: &str) -> Option<chrono::NaiveDate> {
    convert::leading_date(s)
}

/// The path of an item's permalink when it is on the export's own site and
/// not a `?p=ID` fallback.
fn same_site_path(link: &str, site: Option<&url::Url>) -> Option<String> {
    let u = url::Url::parse(link.trim()).ok()?;
    let site = site?;
    if u.host_str() != site.host_str() || u.port() != site.port() || u.query().is_some() {
        return None;
    }
    Some(u.path().to_string())
}

/// The permalink with its last segment replaced by an old slug.
fn old_slug_alias(link: &str, old_slug: &str, site: Option<&url::Url>) -> Option<String> {
    let path = same_site_path(link, site)?;
    let trailing = path.ends_with('/');
    let trimmed = path.trim_end_matches('/');
    let (parent, _) = trimmed.rsplit_once('/')?;
    let old = old_slug.trim();
    if old.is_empty() {
        return None;
    }
    Some(format!("{parent}/{old}{}", if trailing { "/" } else { "" }))
}

/// 1-based line of a byte offset, counted incrementally from the last
/// query (the reader only moves forward), so no per-line index is stored.
struct LineCounter<'a> {
    bytes: &'a [u8],
    /// (byte offset, line at that offset)
    cursor: std::cell::Cell<(usize, usize)>,
}

impl<'a> LineCounter<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            cursor: std::cell::Cell::new((0, 1)),
        }
    }

    fn line_at(&self, pos: u64) -> usize {
        let pos = usize::try_from(pos)
            .unwrap_or(usize::MAX)
            .min(self.bytes.len());
        let (mut from, mut line) = self.cursor.get();
        if pos < from {
            (from, line) = (0, 1);
        }
        line += self.bytes[from..pos]
            .iter()
            .filter(|b| **b == b'\n')
            .count();
        self.cursor.set((pos, line));
        line
    }
}

/// Read the channel and its items.
fn read_wxr(text: &str, display: &str) -> anyhow::Result<Channel> {
    let lines = LineCounter::new(text);
    let line_at = |pos: u64| lines.line_at(pos);

    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(false);
    let mut channel = Channel::default();
    let mut stack: Vec<String> = Vec::new();
    let mut text_buf = String::new();
    let mut current: Option<RawItem> = None;
    let mut author: (String, String) = Default::default();
    let mut meta: (String, String) = Default::default();
    let mut term_attrs: (String, String) = Default::default();
    let mut saw_rss = false;
    let mut saw_wp = false;

    loop {
        let pos = reader.buffer_position();
        let event = reader.read_event().map_err(|e| {
            anyhow::anyhow!(
                "{display}:{}: malformed XML: {e}",
                line_at(reader.error_position())
            )
        })?;
        match event {
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if name == "rss" {
                    saw_rss = true;
                }
                if name.starts_with("wp:") {
                    saw_wp = true;
                }
                let depth = stack.len();
                if name == "item" && depth == 2 {
                    current = Some(RawItem {
                        line: line_at(pos),
                        ..Default::default()
                    });
                }
                if name == "category" && current.is_some() {
                    let get = |key: &[u8]| {
                        e.attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == key)
                            .and_then(|a| {
                                a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                    .ok()
                                    .map(|v| v.into_owned())
                            })
                            .unwrap_or_default()
                    };
                    term_attrs = (get(b"domain"), get(b"nicename"));
                }
                if name == "content:encoded" {
                    if let Some(item) = current.as_mut() {
                        item.content_line = line_at(reader.buffer_position());
                    }
                }
                stack.push(name);
                text_buf.clear();
            }
            Event::Empty(_) => {}
            Event::Text(t) => {
                text_buf.push_str(
                    &t.decode()
                        .map_err(|e| anyhow::anyhow!("{display}:{}: {e}", line_at(pos)))?,
                );
            }
            Event::CData(t) => {
                text_buf.push_str(
                    &t.decode()
                        .map_err(|e| anyhow::anyhow!("{display}:{}: {e}", line_at(pos)))?,
                );
            }
            Event::GeneralRef(r) => {
                let resolved = if r.is_char_ref() {
                    r.resolve_char_ref().ok().flatten().map(String::from)
                } else {
                    let name: &[u8] = &r;
                    match name {
                        b"amp" => Some("&".into()),
                        b"lt" => Some("<".into()),
                        b"gt" => Some(">".into()),
                        b"quot" => Some("\"".into()),
                        b"apos" => Some("'".into()),
                        _ => None,
                    }
                };
                match resolved {
                    Some(s) => text_buf.push_str(&s),
                    // Not expanded: kept as written.
                    None => {
                        text_buf.push('&');
                        text_buf.push_str(&String::from_utf8_lossy(r.as_ref()));
                        text_buf.push(';');
                    }
                }
            }
            Event::End(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let parent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|i| stack.get(i))
                    .cloned();
                stack.pop();
                let value = std::mem::take(&mut text_buf);
                match (parent.as_deref(), name.as_str()) {
                    (Some("channel"), "item") => {
                        if let Some(item) = current.take() {
                            if channel.items.len() >= super::MAX_ITEMS {
                                return Err(super::too_many_items(display));
                            }
                            channel.items.push(item);
                        }
                    }
                    (Some("channel"), "link") => channel.link = value,
                    (Some("channel"), "wp:base_site_url") => channel.base_site_url = value,
                    (Some("channel"), "wp:base_blog_url") => channel.base_blog_url = value,
                    (Some("wp:author"), "wp:author_login") => author.0 = value,
                    (Some("wp:author"), "wp:author_display_name") => author.1 = value,
                    (Some("channel"), "wp:author") => {
                        let (login, display_name) = std::mem::take(&mut author);
                        if !login.is_empty() && !display_name.trim().is_empty() {
                            channel
                                .authors
                                .insert(login, display_name.trim().to_string());
                        }
                    }
                    (Some("wp:postmeta"), "wp:meta_key") => meta.0 = value,
                    (Some("wp:postmeta"), "wp:meta_value") => meta.1 = value,
                    (Some("item"), "wp:postmeta") => {
                        if let Some(item) = current.as_mut() {
                            item.meta.push(std::mem::take(&mut meta));
                        }
                    }
                    (Some("item"), "category") => {
                        if let Some(item) = current.as_mut() {
                            let (domain, nicename) = std::mem::take(&mut term_attrs);
                            item.terms.push((domain, nicename, value));
                        }
                    }
                    (Some("item"), field) => {
                        if let Some(item) = current.as_mut() {
                            item.fields.insert(field.to_string(), value);
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            // Declarations, comments, processing instructions and the DTD
            // (never expanded) carry nothing we need.
            _ => {}
        }
    }
    if !saw_rss || !saw_wp {
        bail!(
            "{display} is not a WordPress export (WXR): expected an <rss> document with wp: elements \
             (export it from WordPress under Tools → Export)"
        );
    }
    Ok(channel)
}

/// Convert a `content:encoded` body to Markdown.
fn convert_body(html: &str, first_line: usize, item: &Item, warnings: &mut Vec<Warning>) -> String {
    let html = &convert::defuse_markers(html);
    static BLOCK_COMMENT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)<!--\s*/?wp:.*?-->").expect("valid regex"));
    static EMBED_WRAPPER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?is)<div class="wp-block-embed__wrapper">\s*(\S+?)\s*</div>"#)
            .expect("valid regex")
    });
    let is_blocks = html.contains("<!-- wp:");
    // Replacements keep the newline count so warning lines stay right.
    let html = BLOCK_COMMENT.replace_all(html, |c: &regex::Captures| {
        "\n".repeat(c[0].matches('\n').count())
    });
    let html = EMBED_WRAPPER.replace_all(&html, |c: &regex::Captures| {
        let newlines = c[0].matches('\n').count().max(2);
        format!("\n{}{}", &c[1], "\n".repeat(newlines - 1))
    });
    let mut tokens = Tokens::default();
    let html = shortcodes(&html, first_line, item, &mut tokens, warnings);
    let html = embed_lines(&html, &mut tokens);
    let html = if is_blocks { html } else { autop(&html) };
    let mut notes = Vec::new();
    let html = convert::prepare_html(&html, &mut tokens, &mut notes);
    let md = convert::finish_markdown(&convert::html_to_markdown(&html), &tokens, true, &mut notes);
    for note in notes {
        warnings.push(Warning::for_item(item, None, "untrusted-html", note));
    }
    md
}

/// WordPress shortcodes we know, plus anything shaped like one.
const KNOWN_SHORTCODES: &[&str] = &[
    "gallery",
    "audio",
    "video",
    "playlist",
    "embed",
    "caption",
    "wp_caption",
    "youtube",
    "vimeo",
    "soundcloud",
    "contact-form-7",
    "contact-form",
    "gravityform",
    "su_button",
    "et_pb_section",
    "vc_row",
    "vc_column",
    "instagram",
    "tweet",
    "googlemaps",
    "spotify",
];

/// Convert or protect `[shortcodes]`.
fn shortcodes(
    html: &str,
    first_line: usize,
    item: &Item,
    tokens: &mut Tokens,
    warnings: &mut Vec<Warning>,
) -> String {
    static CAPTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)\[(?:wp_)?caption\b([^\]]*)\](.*?)\[/(?:wp_)?caption\]")
            .expect("valid regex")
    });
    static EMBED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)\[embed\b[^\]]*\](.*?)\[/embed\]").expect("valid regex")
    });
    static VIDEO_SC: LazyLock<Regex> = LazyLock::new(|| {
        // Jetpack: [youtube https://…] / [youtube=https://…] / [vimeo 123]
        Regex::new(r"(?i)\[(youtube|vimeo)[=\s]+([^\]\s]+)[^\]]*\]").expect("valid regex")
    });
    static ANY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\[(/?)([a-zA-Z][a-zA-Z0-9_-]*)((?:\s[^\]\[]*)?)\]").expect("valid regex")
    });
    static IMG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?is)<img\b[^>]*>").expect("valid regex"));

    let line_at =
        |text: &str, offset: usize| first_line.max(1) + convert::line_of(text, offset) - 1;

    let html = CAPTION.replace_all(html, |c: &regex::Captures| {
        let attrs = &c[1];
        let inner = &c[2];
        let img = IMG.find(inner).map(|m| m.as_str()).unwrap_or("");
        let src = convert::attr(img, "src").unwrap_or_default();
        if src.is_empty() {
            return c[0].to_string();
        }
        let alt = convert::attr(img, "alt").unwrap_or_default();
        let caption = convert::attr(&format!(" {attrs}"), "caption")
            .unwrap_or_else(|| convert::strip_tags(&IMG.replace_all(inner, "")));
        let width = convert::attr(img, "width")
            .or_else(|| convert::attr(&format!(" {attrs}"), "width"))
            .unwrap_or_default();
        keep_lines(
            &c[0],
            tokens.push(convert::figure(&src, &alt, &caption, &width)),
        )
    });
    let html = EMBED.replace_all(&html, |c: &regex::Captures| {
        let url = c[1].trim();
        match convert::embed_for_url(url) {
            Some(sc) => keep_lines(&c[0], tokens.push(sc)),
            None => keep_lines(&c[0], format!("<a href=\"{url}\">{url}</a>")),
        }
    });
    let html = VIDEO_SC.replace_all(&html, |c: &regex::Captures| {
        let arg = c[2].trim();
        let url = if c[1].eq_ignore_ascii_case("vimeo") && arg.chars().all(|ch| ch.is_ascii_digit())
        {
            format!("https://vimeo.com/{arg}")
        } else {
            arg.to_string()
        };
        match convert::embed_for_url(&url) {
            Some(sc) => keep_lines(&c[0], tokens.push(sc)),
            None => c[0].to_string(),
        }
    });

    // Everything else that looks like a shortcode is kept verbatim.
    let html = html.into_owned();
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    for c in ANY.captures_iter(&html) {
        let whole = c.get(0).expect("match");
        let name = c[2].to_ascii_lowercase();
        let closing = !c[1].is_empty();
        let attrs = &c[3];
        let shaped = KNOWN_SHORTCODES.contains(&name.as_str())
            || attrs.contains('=')
            || closing
            || html.contains(&format!("[/{}]", &c[2]));
        if !shaped {
            continue;
        }
        out.push_str(&html[last..whole.start()]);
        out.push_str(&tokens.push_untrusted(whole.as_str()));
        last = whole.end();
        if !closing {
            let what = if name == "gallery" {
                "gallery: list its images in Markdown (or a custom shortcode)".to_string()
            } else {
                format!(
                    "WordPress shortcode [{}] left in place: convert or remove it",
                    &c[2]
                )
            };
            warnings.push(Warning::for_item(
                item,
                Some(line_at(&html, whole.start())),
                "wp-shortcode",
                what,
            ));
        }
    }
    out.push_str(&html[last..]);
    out
}

/// `<p>replacement</p>` followed by as many newlines as `original` had.
fn keep_lines(original: &str, replacement: String) -> String {
    format!(
        "<p>{replacement}</p>{}",
        "\n".repeat(original.matches('\n').count())
    )
}

/// A YouTube/Vimeo URL alone on a line (WordPress auto-embeds those).
fn embed_lines(html: &str, tokens: &mut Tokens) -> String {
    static URL_LINE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^[ \t]*(?:<p>)?[ \t]*(https?://[^\s<]+)[ \t]*(?:</p>)?[ \t]*$")
            .expect("valid regex")
    });
    URL_LINE
        .replace_all(html, |c: &regex::Captures| {
            match convert::embed_for_url(&c[1]) {
                Some(sc) => format!("<p>{}</p>", tokens.push(sc)),
                None => c[0].to_string(),
            }
        })
        .into_owned()
}

/// A simplified `wpautop`: blank-line separated chunks become paragraphs
/// (single newlines → `<br>`) unless they start with a block element;
/// `<pre>` contents are left alone.
fn autop(html: &str) -> String {
    static PRE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?is)<pre\b.*?</pre>").expect("valid regex"));
    static PARAS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\n[ \t]*\n").expect("valid regex"));
    static BLOCK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^<(?:/?(?:p|div|h[1-6]|ul|ol|li|dl|dd|dt|blockquote|pre|table|thead|tbody|tr|td|th|figure|figcaption|hr|form|section|article|aside|header|footer|nav|iframe|video|audio|address|details|summary|style|script)\b|!--)")
            .expect("valid regex")
    });
    let mut pres = Vec::new();
    let html = PRE.replace_all(html, |c: &regex::Captures| {
        pres.push(c[0].to_string());
        format!("<pre>{}</pre>", convert::marker('P', pres.len() - 1))
    });
    let html = html.replace("\r\n", "\n");
    let mut out = Vec::new();
    for chunk in PARAS.split(&html) {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        if BLOCK.is_match(chunk) {
            out.push(chunk.to_string());
        } else {
            out.push(format!("<p>{}</p>", chunk.replace('\n', "<br>")));
        }
    }
    let mut joined = out.join("\n\n");
    for (i, pre) in pres.iter().enumerate() {
        joined = joined.replace(&format!("<pre>{}</pre>", convert::marker('P', i)), pre);
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> Item {
        Item {
            source_id: "wp:1".into(),
            source_ref: "export.xml:10".into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_convert_body_classic_with_caption_embed_gallery() {
        let html = "First para\nsecond line\n\n[caption id=\"attachment_5\" align=\"alignnone\" width=\"300\"]<a href=\"https://ex.com/full.jpg\"><img src=\"https://ex.com/a.jpg\" alt=\"Photo\" width=\"300\" /></a> The caption[/caption]\n\n[embed]https://www.youtube.com/watch?v=dQw4w9WgXcQ[/embed]\n\nhttps://vimeo.com/76979871\n\n[gallery ids=\"1,2\"]\n\n<pre>a\n\nb</pre>";
        let mut warnings = Vec::new();
        let md = convert_body(html, 20, &item(), &mut warnings);
        assert!(md.contains("First para\\\nsecond line"), "{md}");
        assert!(md.contains(r#"{{< figure(src="https://ex.com/a.jpg", alt="Photo", caption="The caption", width="300") >}}"#), "{md}");
        assert!(md.contains(r#"{{< youtube(id="dQw4w9WgXcQ") >}}"#), "{md}");
        assert!(md.contains(r#"{{< vimeo(id="76979871") >}}"#), "{md}");
        assert!(md.contains("[gallery ids=\"1,2\"]"), "{md}");
        assert!(md.contains("```\na\n\nb\n```"), "{md}");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "wp-shortcode");
        assert_eq!(warnings[0].source, "export.xml:29");
    }

    #[test]
    fn test_convert_body_forged_tokens_cannot_build_links_or_shortcodes() {
        // Old-format markers, a guessed nonce-less marker, and an iframe that
        // is removed (no token) right before attacker text.
        let real = convert::marker('T', 0);
        let html = format!(
            "<p>[x](SEITEIMPORTTOKEN0Xjavascript:alert(1))</p>\
             <iframe src=\"javascript:x\"></iframe><p>[y](javascript:alert(2))</p>\
             <p>{real} SEITEIMPORTT0X [z]({real}) {{{{SEITEIMPORTT1X contact_form() >}}}}</p>\
             [caption]<img src=\"https://ex.com/a.jpg\" alt=\"a\"/> cap[/caption]"
        );
        let mut warnings = Vec::new();
        let md = convert_body(&html, 1, &item(), &mut warnings);
        assert!(!md.contains("SEITEIMPORT"), "{md}");
        let (rendered, _) = crate::build::markdown::markdown_to_html(&md);
        assert!(
            !rendered.to_ascii_lowercase().contains("javascript:"),
            "{rendered}"
        );
        let (calls, errors) = crate::shortcodes::parser::parse_shortcodes_recovering(
            &md,
            std::path::Path::new("x.md"),
        );
        assert!(errors.is_empty(), "{errors:?}\n{md}");
        let names: Vec<&str> = calls.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["figure"], "{md}");
    }

    #[test]
    fn test_tokens_never_restore_empty_values() {
        let mut tokens = Tokens::default();
        assert_eq!(tokens.push(String::new()), "");
        let t = tokens.push("v".into());
        assert_eq!(tokens.restore(&format!("a{t}b")), "avb");
    }

    #[test]
    fn test_convert_body_hostile_content_cannot_inject_shortcodes() {
        let html = r#"[caption]<img src="https://ex.com/a.jpg" alt="{{< contact_form() >}}" /> {{% contact_form() %}} x >}}[/caption]

<iframe src="https://evil.example/?q={{< contact_form() >}}"></iframe>

<video src="v.mp4">{{< contact_form() >}}</video>

[foo x="{{< contact_form() >}}"]

<p>Plain {{< contact_form() >}} text</p>

<pre><code>{{< contact_form() >}}</code></pre>"#;
        let mut warnings = Vec::new();
        let md = convert_body(html, 1, &item(), &mut warnings);
        let (calls, errors) = crate::shortcodes::parser::parse_shortcodes_recovering(
            &md,
            std::path::Path::new("x.md"),
        );
        assert!(errors.is_empty(), "{errors:?}\n{md}");
        let names: Vec<&str> = calls.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["figure"], "{md}");
    }

    #[test]
    fn test_convert_body_blocks() {
        let html = "<!-- wp:paragraph -->\n<p>Hello <em>there</em></p>\n<!-- /wp:paragraph -->\n\n<!-- wp:embed {\"url\":\"https://youtu.be/dQw4w9WgXcQ\"} -->\n<figure class=\"wp-block-embed\"><div class=\"wp-block-embed__wrapper\">\nhttps://youtu.be/dQw4w9WgXcQ\n</div></figure>\n<!-- /wp:embed -->\n<p>Literal {{< x >}}</p>";
        let mut warnings = Vec::new();
        let md = convert_body(html, 1, &item(), &mut warnings);
        assert!(md.starts_with("Hello *there*"), "{md}");
        assert!(md.contains(r#"{{< youtube(id="dQw4w9WgXcQ") >}}"#), "{md}");
        assert!(md.contains("Literal {{\\< x >}}"), "{md}");
        assert!(!md.contains("wp:"), "{md}");
    }

    #[test]
    fn test_old_slug_alias() {
        let site = url::Url::parse("https://example.com").ok();
        assert_eq!(
            old_slug_alias(
                "https://example.com/2020/01/new-slug/",
                "old-slug",
                site.as_ref()
            )
            .as_deref(),
            Some("/2020/01/old-slug/")
        );
        assert_eq!(
            same_site_path("https://example.com/?p=12", site.as_ref()),
            None
        );
        assert_eq!(same_site_path("https://other.com/a/", site.as_ref()), None);
    }

    #[test]
    fn test_read_wxr_rejects_non_wxr_and_malformed() {
        assert!(read_wxr("<rss><channel></channel></rss>", "x.xml")
            .unwrap_err()
            .to_string()
            .contains("not a WordPress export"));
        let err = read_wxr("<rss>\n<channel>\n<wp:x></channel>", "x.xml")
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("x.xml:"), "{err}");
    }
}
