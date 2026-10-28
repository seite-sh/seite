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
        Regex::new(r"&(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z]+);").expect("valid regex")
    });
    ENT.replace_all(text, |c: &regex::Captures| {
        let name = &c[1];
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

/// Generic HTML preparation before [`html_to_markdown`] (WordPress and
/// Ghost): video iframes → `youtube`/`vimeo` shortcodes, `<figure>` with an
/// image → `figure` shortcode, other iframes/video/audio kept as raw HTML.
pub(crate) fn prepare_html(html: &str, tokens: &mut Tokens) -> String {
    static IFRAME: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)<iframe\b[^>]*>.*?</iframe>|<iframe\b[^>]*/>").expect("valid regex")
    });
    static MEDIA: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)<(video|audio)\b[^>]*>.*?</(?:video|audio)>").expect("valid regex")
    });
    let html = IFRAME.replace_all(html, |c: &regex::Captures| {
        let tag = &c[0];
        let token = match attr(tag, "src").and_then(|src| embed_for_url(&src)) {
            Some(embed) => tokens.push(embed),
            None => tokens.push_untrusted(tag),
        };
        format!("<p>{token}</p>")
    });
    let html = MEDIA.replace_all(&html, |c: &regex::Captures| {
        format!("<p>{}</p>", tokens.push_untrusted(&c[0]))
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
        let prepared = prepare_html(html, &mut tokens);
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
    fn test_decode_entities() {
        assert_eq!(
            decode_entities("a &amp; b &#8217; &#x41; &bogus;"),
            "a & b ’ A &bogus;"
        );
    }
}
