//! Full-text search index (`[build] search = "full"`).
//!
//! A prebuilt inverted index that any static host can serve and that the
//! bundled themes' search script (`seite/search-script.html`) loads lazily,
//! in the spirit of Pagefind:
//!
//! ```text
//! search/meta.json         {"version":1,"lang","stem","stop","pages","hash",
//!                           "collections":[..],"pc":[collection index per page],
//!                           "shards":[first term of each shard]}
//! search/terms/<n>.json    {"term":[Δpage, weight, Δpage, weight, ..], ..}
//! search/pages/<id>.json   {"t":title,"u":url,"c":collection,"d":date,"x":description,"b":text}
//! ```
//!
//! - **Shards** hold a contiguous range of terms (sorted by UTF-16 code unit,
//!   the order JavaScript's `<` uses) and are cut at [`SHARD_BYTES`], so a
//!   query downloads one shard per term (a prefix term may span a few).
//!   `meta.shards[n]` is the first term of `terms/<n>.json`.
//! - **Postings** are `[page id delta, weight]` pairs. The weight combines
//!   where the term occurs: title > headings > tags > description > body
//!   (see [`FIELD_WEIGHTS`]); the client multiplies it by an IDF.
//! - **Pages** get one small fragment each (title, URL, collection, date,
//!   description and the first [`TEXT_CHARS`] characters of the text), so
//!   rendering the top results costs a handful of small fetches.
//! - Page ids are positions in URL order; every map is ordered: the output is
//!   byte-identical run to run (`meta.hash` covers every shard and fragment
//!   and is appended to their URLs as a cache-buster).
//!
//! The tokenizer is mirrored line for line in the search script; keep the two
//! in sync (`tests/integration/search_js.rs` runs the script against a built
//! index under Node when it is available):
//! 1. split the original text into runs of word characters (Unicode
//!    Alphabetic, Number or Mark);
//! 2. fold each run: lowercase, NFD, drop U+0300..=U+036F, NFC, then
//!    `ß→ss æ→ae œ→oe ø→o ł→l đ→d ð→d þ→th ı→i ς→σ`;
//! 3. split the run into CJK and non-CJK pieces: CJK pieces become character
//!    bigrams (a lone character stays a unigram); other pieces are one term,
//!    dropped above [`MAX_TERM_CHARS`];
//! 4. English indexes only (`meta.stem`/`meta.stop`): drop [`STOPWORDS`] and
//!    apply [`stem_en`] (light suffix stripping) to all-ASCII terms.

use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Index format version (`meta.version`).
pub const FORMAT_VERSION: u32 = 1;
/// Soft size cap of a term shard (a single larger posting list gets a shard
/// of its own).
pub const SHARD_BYTES: usize = 8 * 1024;
/// A prefix term loads at most this many shards after the one its prefix
/// sorts into (the client's cap: very short prefixes stay cheap).
pub const PREFIX_SHARDS: usize = 3;
/// Characters of page text kept in a page fragment for result snippets.
pub const TEXT_CHARS: usize = 3000;
/// Terms longer than this (hashes, base64, URLs run together) are not indexed.
pub const MAX_TERM_CHARS: usize = 40;

/// English stopwords (Lucene's classic list), dropped from English indexes
/// and queries only.
pub const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it",
    "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there", "these",
    "they", "this", "to", "was", "will", "with",
];

/// (weight, cap on occurrences) per field.
const FIELD_WEIGHTS: [(u32, u32); 5] = [
    (12, 2), // title
    (6, 4),  // headings
    (4, 2),  // tags
    (3, 3),  // description
    (1, 8),  // body
];

/// Tokenizer options for one index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lang {
    /// English: stopwords and light stemming.
    pub english: bool,
}

impl Lang {
    pub fn for_code(code: &str) -> Self {
        let code = code.to_ascii_lowercase();
        Lang {
            english: code == "en" || code.starts_with("en-") || code.starts_with("en_"),
        }
    }
}

/// Fold a word for matching: lowercase, strip Latin combining diacritics
/// (U+0300..=U+036F after NFD), recompose, then map the few letters NFD does
/// not decompose.
pub fn fold(word: &str) -> String {
    let lower = word.to_lowercase();
    let stripped: String = lower
        .nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .nfc()
        .collect();
    let mut out = String::with_capacity(stripped.len());
    for c in stripped.chars() {
        match c {
            'ß' => out.push_str("ss"),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            'ø' => out.push('o'),
            'ł' => out.push('l'),
            'đ' | 'ð' => out.push('d'),
            'þ' => out.push_str("th"),
            'ı' => out.push('i'),
            'ς' => out.push('σ'),
            c => out.push(c),
        }
    }
    out
}

/// A word character: Unicode Alphabetic, Number or Mark.
pub fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || c.is_numeric() || is_combining_mark(c)
}

/// Kana, CJK ideographs and Hangul syllables: indexed as character bigrams.
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
        | 0xAC00..=0xD7AF | 0x20000..=0x2FFFF)
}

/// Light English stemmer (plural, `-ing`, `-ed`), for all-ASCII lowercase
/// words only, so lengths agree with JavaScript's UTF-16 lengths.
pub fn stem_en(word: &str) -> String {
    if word.len() < 4 || !word.bytes().all(|b| b.is_ascii_lowercase()) {
        return word.to_string();
    }
    let mut w = word.to_string();
    if w.ends_with("ies") && w.len() > 4 {
        w.truncate(w.len() - 3);
        w.push('y');
    } else if w.ends_with("sses")
        || w.ends_with("xes")
        || w.ends_with("ches")
        || w.ends_with("shes")
        || w.ends_with("zzes")
    {
        w.truncate(w.len() - 2);
    } else if w.ends_with('s') && !(w.ends_with("ss") || w.ends_with("us") || w.ends_with("is")) {
        w.truncate(w.len() - 1);
    }
    let suffix = if w.ends_with("ing") && w.len() >= 7 {
        3
    } else if w.ends_with("ed") && w.len() >= 6 {
        2
    } else {
        0
    };
    if suffix > 0 {
        w.truncate(w.len() - suffix);
        let b = w.as_bytes();
        let n = b.len();
        if n >= 4 && b[n - 1] == b[n - 2] && !b"aeiouylsz".contains(&b[n - 1]) {
            w.truncate(n - 1);
        }
    }
    w
}

/// One token: its index term and its byte range in the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub term: String,
    pub start: usize,
    pub end: usize,
}

/// Tokenize `text` into index terms (see the module docs for the rules).
pub fn tokenize(text: &str, lang: Lang) -> Vec<Token> {
    let mut out = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        let word = i < text.len() && is_word_char(c);
        match (word, run_start) {
            (true, None) => run_start = Some(i),
            (false, Some(s)) => {
                push_run(&text[s..i], s, i, lang, &mut out);
                run_start = None;
            }
            _ => {}
        }
    }
    out
}

fn push_run(run: &str, start: usize, end: usize, lang: Lang, out: &mut Vec<Token>) {
    let folded = fold(run);
    let chars: Vec<char> = folded.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let cjk = is_cjk(chars[i]);
        let mut j = i;
        while j < chars.len() && is_cjk(chars[j]) == cjk {
            j += 1;
        }
        let piece = &chars[i..j];
        if cjk {
            if piece.len() == 1 {
                out.push(Token {
                    term: piece[0].to_string(),
                    start,
                    end,
                });
            }
            for pair in piece.windows(2) {
                out.push(Token {
                    term: pair.iter().collect(),
                    start,
                    end,
                });
            }
        } else if piece.len() <= MAX_TERM_CHARS {
            let term: String = piece.iter().collect();
            if !(lang.english && STOPWORDS.contains(&term.as_str())) {
                let term = if lang.english { stem_en(&term) } else { term };
                out.push(Token { term, start, end });
            }
        }
        i = j;
    }
}

/// Index terms of `text`, without positions.
pub fn terms(text: &str, lang: Lang) -> Vec<String> {
    tokenize(text, lang).into_iter().map(|t| t.term).collect()
}

/// Visible text of a rendered page body: everything (`text`, for snippets)
/// and the heading text alone (`headings`, weighted higher). Script, style,
/// nav, footer, SVG, template, noscript, MathML and HTML comments are dropped.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PageText {
    pub text: String,
    pub headings: String,
    /// `text` without the headings (the body field).
    pub body: String,
}

const SKIP_ELEMENTS: &[&str] = &[
    "script", "style", "nav", "footer", "svg", "template", "noscript", "math", "button",
];
const INLINE_ELEMENTS: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "cite", "code", "data", "del", "dfn", "em", "i", "ins", "kbd",
    "mark", "q", "s", "samp", "small", "span", "strong", "sub", "sup", "time", "u", "var", "wbr",
];

/// Extract the visible text of an HTML fragment (a rendered markdown body).
pub fn extract_text(html: &str) -> PageText {
    let mut out = PageText::default();
    let mut skip: Option<(String, usize)> = None; // element being skipped, nesting depth
    let mut heading_depth = 0usize;
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut text_start = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // Flush the text before the tag.
        if skip.is_none() {
            push_text(&mut out, &html[text_start..i], heading_depth > 0);
        }
        if html[i..].starts_with("<!--") {
            i = html[i..].find("-->").map_or(bytes.len(), |e| i + e + 3);
            text_start = i;
            continue;
        }
        let tag_end = find_tag_end(bytes, i + 1);
        let inner = &html[i + 1..tag_end.min(bytes.len())];
        i = (tag_end + 1).min(bytes.len());
        text_start = i;
        let closing = inner.starts_with('/');
        let name: String = inner
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        let self_closing = inner.ends_with('/');
        if let Some((skipped, depth)) = skip.as_mut() {
            if *skipped == name && !self_closing {
                if closing {
                    *depth -= 1;
                    if *depth == 0 {
                        skip = None;
                    }
                } else {
                    *depth += 1;
                }
            }
            continue;
        }
        if !closing && !self_closing && SKIP_ELEMENTS.contains(&name.as_str()) {
            skip = Some((name, 1));
            continue;
        }
        let is_heading =
            name.len() == 2 && name.starts_with('h') && (b'1'..=b'6').contains(&name.as_bytes()[1]);
        if is_heading {
            if closing {
                heading_depth = heading_depth.saturating_sub(1);
            } else {
                heading_depth += 1;
            }
        }
        if !INLINE_ELEMENTS.contains(&name.as_str()) {
            push_text(&mut out, " ", heading_depth > 0 || (is_heading && closing));
        }
    }
    if skip.is_none() && text_start < bytes.len() {
        push_text(&mut out, &html[text_start..], heading_depth > 0);
    }
    out.text = collapse_ws(&out.text);
    out.headings = collapse_ws(&out.headings);
    out.body = collapse_ws(&out.body);
    out
}

/// End of a tag starting after `<` at `from`: the next `>` outside quotes.
fn find_tag_end(bytes: &[u8], from: usize) -> usize {
    let mut quote: Option<u8> = None;
    let mut j = from;
    while j < bytes.len() {
        let b = bytes[j];
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b == b'>' => return j,
            None => {}
        }
        j += 1;
    }
    bytes.len()
}

fn push_text(out: &mut PageText, raw: &str, heading: bool) {
    if raw.is_empty() {
        return;
    }
    let text = decode_entities(raw);
    out.text.push_str(&text);
    if heading {
        out.headings.push_str(&text);
        out.body.push(' ');
    } else {
        out.body.push_str(&text);
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Decode the HTML entities pulldown-cmark and syntect emit (named basics and
/// numeric references); anything else is kept verbatim.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.bytes().take(12).position(|b| b == b';') else {
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

/// One page to index.
#[derive(Debug, Clone)]
pub struct SearchDoc<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub tags: &'a [String],
    pub collection: &'a str,
    pub date: Option<String>,
    /// The rendered body HTML (main content only: no theme chrome).
    pub html: &'a str,
}

/// Size figures of a built index (summed over languages by the caller).
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct IndexStats {
    pub pages: usize,
    pub terms: usize,
    pub files: usize,
    pub bytes: usize,
    pub shards: usize,
    pub largest_shard_bytes: usize,
    pub meta_bytes: usize,
    pub largest_page_bytes: usize,
}

/// A built index: files relative to the language root (`search/...`).
pub struct BuiltIndex {
    pub files: BTreeMap<String, String>,
    pub stats: IndexStats,
}

/// Field-weighted score of each term on one page.
fn page_weights(doc: &SearchDoc<'_>, text: &PageText, lang: Lang) -> BTreeMap<String, u32> {
    let tags = doc.tags.join(" ");
    let fields: [&str; 5] = [
        doc.title,
        &text.headings,
        &tags,
        doc.description.unwrap_or(""),
        &text.body,
    ];
    let mut out: BTreeMap<String, u32> = BTreeMap::new();
    for (field, (weight, cap)) in fields.iter().zip(FIELD_WEIGHTS) {
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for term in terms(field, lang) {
            *counts.entry(term).or_default() += 1;
        }
        for (term, n) in counts {
            *out.entry(term).or_default() += weight * n.min(cap);
        }
    }
    out
}

/// Cut `text` to at most `max` characters, at a word boundary when possible.
fn truncate_chars(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        None => text,
        Some((cut, _)) => {
            let head = &text[..cut];
            head.rfind(' ').map_or(head, |sp| &head[..sp])
        }
    }
}

fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Build the full-text index of one language's pages.
pub fn build_index(docs: &[SearchDoc<'_>], lang_code: &str) -> BuiltIndex {
    use rayon::prelude::*;
    use std::fmt::Write;

    let lang = Lang::for_code(lang_code);
    let mut docs: Vec<&SearchDoc<'_>> = docs.iter().collect();
    docs.sort_by(|a, b| a.url.cmp(b.url));
    docs.dedup_by(|a, b| a.url == b.url);

    let analyzed: Vec<(PageText, BTreeMap<String, u32>)> = docs
        .par_iter()
        .map(|doc| {
            let text = extract_text(doc.html);
            let weights = page_weights(doc, &text, lang);
            (text, weights)
        })
        .collect();

    // Inverted index: term -> [(page, weight)] in page order.
    let mut postings: BTreeMap<String, Vec<(usize, u32)>> = BTreeMap::new();
    for (id, (_, weights)) in analyzed.iter().enumerate() {
        for (term, w) in weights {
            postings.entry(term.clone()).or_default().push((id, *w));
        }
    }
    let mut terms_sorted: Vec<(&String, &Vec<(usize, u32)>)> = postings.iter().collect();
    terms_sorted.sort_by(|a, b| utf16_cmp(a.0, b.0));

    let mut files: BTreeMap<String, String> = BTreeMap::new();
    let mut stats = IndexStats {
        pages: docs.len(),
        terms: terms_sorted.len(),
        ..Default::default()
    };

    // Term shards, cut at SHARD_BYTES.
    let mut shard_firsts: Vec<&str> = Vec::new();
    let mut shards: Vec<String> = Vec::new();
    let mut current = String::new();
    for (term, list) in terms_sorted {
        let mut entry = serde_json::to_string(term).unwrap_or_default();
        entry.push_str(":[");
        let mut prev = 0usize;
        for (k, (id, w)) in list.iter().enumerate() {
            if k > 0 {
                entry.push(',');
            }
            let _ = write!(entry, "{},{}", id - prev, w);
            prev = *id;
        }
        entry.push(']');
        if !current.is_empty() && current.len() + entry.len() + 2 > SHARD_BYTES {
            current.push('}');
            shards.push(std::mem::take(&mut current));
        }
        if current.is_empty() {
            current.push('{');
            shard_firsts.push(term);
        } else {
            current.push(',');
        }
        current.push_str(&entry);
    }
    if !current.is_empty() {
        current.push('}');
        shards.push(current);
    }
    for (n, shard) in shards.into_iter().enumerate() {
        stats.largest_shard_bytes = stats.largest_shard_bytes.max(shard.len());
        files.insert(format!("search/terms/{n}.json"), shard);
    }
    stats.shards = shard_firsts.len();

    // Page fragments.
    let mut collections: Vec<&str> = docs.iter().map(|d| d.collection).collect();
    collections.sort_unstable();
    collections.dedup();
    let mut page_collections = Vec::with_capacity(docs.len());
    for (id, (doc, (text, _))) in docs.iter().zip(&analyzed).enumerate() {
        page_collections.push(collections.binary_search(&doc.collection).unwrap_or(0));
        let fragment = serde_json::json!({
            "t": doc.title,
            "u": doc.url,
            "c": doc.collection,
            "d": doc.date,
            "x": doc.description,
            "b": truncate_chars(&text.text, TEXT_CHARS),
        });
        let body = fragment.to_string();
        stats.largest_page_bytes = stats.largest_page_bytes.max(body.len());
        files.insert(format!("search/pages/{id}.json"), body);
    }

    // Content hash over every shard and fragment (cache-buster).
    let mut hasher = Sha256::new();
    for (path, body) in &files {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(body.as_bytes());
        hasher.update([0]);
    }
    let hash: String = hasher
        .finalize()
        .iter()
        .take(5)
        .map(|b| format!("{b:02x}"))
        .collect();

    let meta = serde_json::json!({
        "version": FORMAT_VERSION,
        "lang": lang_code,
        "stem": lang.english,
        "stop": lang.english,
        "pages": docs.len(),
        "hash": hash,
        "collections": collections,
        "pc": page_collections,
        "shards": shard_firsts,
    })
    .to_string();
    stats.meta_bytes = meta.len();
    files.insert("search/meta.json".to_string(), meta);

    stats.files = files.len();
    stats.bytes = files.values().map(String::len).sum();
    BuiltIndex { files, stats }
}

/// Bytes a query for `words` downloads from a built index: meta, the shards
/// that hold each term (the last one as a prefix, like the client), and the
/// fragments of the top `results` pages (the largest ones, as an upper
/// bound). Used by the size-budget tests and `seite build --verbose`.
pub fn query_bytes(index: &BuiltIndex, words: &str, results: usize, lang_code: &str) -> usize {
    let meta: serde_json::Value = index
        .files
        .get("search/meta.json")
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or_default();
    let firsts: Vec<&str> = meta["shards"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let mut total = index.files.get("search/meta.json").map_or(0, String::len);
    let mut seen = std::collections::BTreeSet::new();
    let query_terms = terms(words, Lang::for_code(lang_code));
    let last = query_terms.len().saturating_sub(1);
    for (k, term) in query_terms.iter().enumerate() {
        for n in shards_for(&firsts, term, k == last) {
            if seen.insert(n) {
                total += index
                    .files
                    .get(&format!("search/terms/{n}.json"))
                    .map_or(0, String::len);
            }
        }
    }
    let mut pages: Vec<usize> = index
        .files
        .iter()
        .filter(|(p, _)| p.starts_with("search/pages/"))
        .map(|(_, b)| b.len())
        .collect();
    pages.sort_unstable_by(|a, b| b.cmp(a));
    total + pages.iter().take(results).sum::<usize>()
}

/// Shard numbers holding `term` (exact) or every term starting with it
/// (`prefix`), mirroring the client's lookup.
pub fn shards_for(firsts: &[&str], term: &str, prefix: bool) -> Vec<usize> {
    // Last shard whose first term is <= term.
    let mut lo = 0usize;
    let mut hi = firsts.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if utf16_cmp(firsts[mid], term) != std::cmp::Ordering::Greater {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let mut out = Vec::new();
    if lo > 0 {
        out.push(lo - 1);
    }
    if prefix {
        let mut n = lo;
        while n < firsts.len() && n < lo + PREFIX_SHARDS && firsts[n].starts_with(term) {
            out.push(n);
            n += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: Lang = Lang { english: true };
    const OTHER: Lang = Lang { english: false };

    #[test]
    fn test_fold_diacritics_and_special_letters() {
        assert_eq!(fold("Café"), "cafe");
        assert_eq!(fold("NAÏVE"), "naive");
        assert_eq!(fold("Straße"), "strasse");
        assert_eq!(fold("Ærø"), "aero");
        assert_eq!(fold("Łódź"), "lodz");
        assert_eq!(fold("ΣΟΦΟΣ"), fold("σοφοσ"));
        assert_eq!(fold("İstanbul"), "istanbul");
        // Hangul syllables survive (NFD/NFC round trip).
        assert_eq!(fold("한국어"), "한국어");
        // Japanese voiced marks are not Latin diacritics: kept.
        assert_eq!(fold("が"), "が");
    }

    #[test]
    fn test_word_chars() {
        assert!(is_word_char('a'));
        assert!(is_word_char('é'));
        assert!(is_word_char('7'));
        assert!(is_word_char('\u{301}'));
        assert!(is_word_char('漢'));
        assert!(!is_word_char('-'));
        assert!(!is_word_char(' '));
        assert!(!is_word_char('.'));
    }

    #[test]
    fn test_stem_en() {
        for (word, stem) in [
            ("posts", "post"),
            ("stories", "story"),
            ("classes", "class"),
            ("boxes", "box"),
            ("matches", "match"),
            ("running", "run"),
            ("testing", "test"),
            ("building", "build"),
            ("stopped", "stop"),
            ("called", "call"),
            ("tested", "test"),
            ("string", "string"),
            ("added", "added"),
            ("status", "status"),
            ("analysis", "analysis"),
            ("glass", "glass"),
            ("cats", "cat"),
            ("is", "is"),
            ("café", "café"),
            ("things", "thing"),
        ] {
            assert_eq!(stem_en(word), stem, "{word}");
        }
    }

    #[test]
    fn test_tokenize_english_stems_and_drops_stopwords() {
        assert_eq!(
            terms("The Running of the Cats, naïvely!", EN),
            vec!["run", "cat", "naively"]
        );
        // Other languages: no stemming, no stopwords.
        assert_eq!(
            terms("The running cats", OTHER),
            vec!["the", "running", "cats"]
        );
    }

    #[test]
    fn test_tokenize_splits_on_punctuation_and_keeps_numbers() {
        assert_eq!(
            terms("seite-sh v0.21 don't", OTHER),
            vec!["seite", "sh", "v0", "21", "don", "t"]
        );
        let long = "x".repeat(MAX_TERM_CHARS + 1);
        assert!(terms(&long, OTHER).is_empty());
    }

    #[test]
    fn test_tokenize_cjk_bigrams() {
        assert_eq!(terms("東京都", OTHER), vec!["東京", "京都"]);
        assert_eq!(terms("日", OTHER), vec!["日"]);
        // Mixed runs split into CJK and non-CJK pieces.
        assert_eq!(terms("abc漢字def", OTHER), vec!["abc", "漢字", "def"]);
        assert_eq!(terms("검색 엔진", OTHER), vec!["검색", "엔진"]);
    }

    #[test]
    fn test_tokenize_positions_cover_the_original_run() {
        let text = "Ein Café.";
        let toks = tokenize(text, OTHER);
        assert_eq!(&text[toks[1].start..toks[1].end], "Café");
        assert_eq!(toks[1].term, "cafe");
    }

    #[test]
    fn test_extract_text_skips_chrome_and_collects_headings() {
        let html = "<h2 id=\"x\">Install <code>seite</code></h2><p>Run it &amp; go&#33;</p>\
                    <script>var x = '<p>';</script><nav>menu</nav><style>p{}</style>\
                    <!-- c --><p>Tail<br/>end</p><svg><g><text>no</text></g></svg>";
        let t = extract_text(html);
        assert_eq!(t.headings, "Install seite");
        assert_eq!(t.text, "Install seite Run it & go! Tail end");
        assert_eq!(t.body, "Run it & go! Tail end");
    }

    #[test]
    fn test_extract_text_nested_skip_and_quoted_attrs() {
        let html = "<p title=\"a > b\">One</p><nav><nav>x</nav>y</nav><p>Two</p>";
        assert_eq!(extract_text(html).text, "One Two");
    }

    #[test]
    fn test_decode_entities() {
        assert_eq!(
            decode_entities("a &lt;b&gt; &#x41;&#66; &bogus; &"),
            "a <b> AB &bogus; &"
        );
    }

    fn doc<'a>(url: &'a str, title: &'a str, html: &'a str) -> SearchDoc<'a> {
        SearchDoc {
            url,
            title,
            description: None,
            tags: &[],
            collection: "posts",
            date: None,
            html,
        }
    }

    fn postings(index: &BuiltIndex, term: &str) -> Vec<(usize, u32)> {
        for (path, body) in &index.files {
            if !path.starts_with("search/terms/") {
                continue;
            }
            let shard: serde_json::Value = serde_json::from_str(body).unwrap();
            if let Some(list) = shard[term].as_array() {
                let nums: Vec<u64> = list.iter().map(|v| v.as_u64().unwrap()).collect();
                let mut id = 0usize;
                return nums
                    .chunks(2)
                    .map(|p| {
                        id += p[0] as usize;
                        (id, p[1] as u32)
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    #[test]
    fn test_ranking_weights_title_over_headings_over_body() {
        let docs = vec![
            doc("/a", "Other", "<p>rust</p>"),
            doc("/b", "Rust guide", "<p>text</p>"),
            doc("/c", "Other", "<h2>Rust</h2><p>text</p>"),
        ];
        let index = build_index(&docs, "en");
        let p = postings(&index, "rust");
        let w = |id: usize| p.iter().find(|(i, _)| *i == id).unwrap().1;
        assert!(w(1) > w(2), "{p:?}");
        assert!(w(2) > w(0), "{p:?}");
    }

    #[test]
    fn test_build_index_is_deterministic_and_sharded() {
        let bodies: Vec<String> = (0..300)
            .map(|i| format!("<p>word{i} common shared{} text{}</p>", i % 7, i * 31 % 101))
            .collect();
        let urls: Vec<String> = (0..300).map(|i| format!("/p/{i:03}")).collect();
        let docs: Vec<SearchDoc> = urls
            .iter()
            .zip(&bodies)
            .rev()
            .map(|(u, b)| doc(u, "Page", b))
            .collect();
        let a = build_index(&docs, "en");
        let mut reversed = docs.clone();
        reversed.reverse();
        let b = build_index(&reversed, "en");
        assert_eq!(a.files, b.files);
        assert!(a.stats.shards >= 1);
        assert!(a.stats.largest_shard_bytes <= SHARD_BYTES);
        let meta: serde_json::Value = serde_json::from_str(&a.files["search/meta.json"]).unwrap();
        assert_eq!(meta["pages"], 300);
        assert_eq!(meta["stem"], true);
        // Every page id is listed under "common", in URL order.
        let common = postings(&a, "common");
        assert_eq!(common.len(), 300);
        let page0: serde_json::Value =
            serde_json::from_str(&a.files["search/pages/0.json"]).unwrap();
        assert_eq!(page0["u"], "/p/000");
    }

    #[test]
    fn test_shards_for_exact_and_prefix() {
        let firsts = ["a", "co", "cr", "d"];
        assert_eq!(shards_for(&firsts, "cat", false), vec![0]);
        assert_eq!(shards_for(&firsts, "cow", false), vec![1]);
        assert_eq!(shards_for(&firsts, "c", true), vec![0, 1, 2]);
        assert_eq!(shards_for(&firsts, "0", false), Vec::<usize>::new());
        assert_eq!(shards_for(&firsts, "zz", true), vec![3]);
    }

    #[test]
    fn test_truncate_chars_at_word_boundary() {
        assert_eq!(truncate_chars("hello brave world", 12), "hello brave");
        assert_eq!(truncate_chars("short", 12), "short");
    }

    #[test]
    fn test_query_bytes_counts_meta_shards_and_pages() {
        let docs = vec![
            doc("/a", "Alpha", "<p>beta</p>"),
            doc("/b", "Gamma", "<p>beta</p>"),
        ];
        let index = build_index(&docs, "en");
        let bytes = query_bytes(&index, "beta", 8, "en");
        assert_eq!(bytes, index.stats.bytes);
    }
}
