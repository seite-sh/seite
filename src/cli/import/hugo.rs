//! Hugo site directory.
//!
//! Content from `contentDir` (default `content/`): top-level files are
//! pages, `posts`/`post`/`blog`/`articles`/`news` sections are posts, a
//! section named like a seite collection goes there, other sections go to
//! posts (reported). `_index.md` list pages and `headless` bundles are
//! skipped; leaf bundles (`<dir>/index.md`) take their slug from the
//! directory and their images from the bundle. Frontmatter: YAML, TOML or
//! JSON. Old URLs: `url`, else `[permalinks]` for the section, else
//! `/<section path>/<slug>/`, plus `aliases`. Shortcodes `youtube`,
//! `vimeo`, `gist`, `figure`, `highlight`, `ref`/`relref` are converted;
//! others are kept inside inline code (so seite doesn't try to run them)
//! and reported.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::bail;
use chrono::Datelike;
use regex::Regex;
use serde_json::Value;
use walkdir::WalkDir;

use super::convert;
use super::{json_str, terms_from_json, Item, Kind, ParseCtx, Parsed, RefLink, Skipped, Warning};

const POST_SECTIONS: &[&str] = &["posts", "post", "blog", "articles", "news"];
const CONFIG_NAMES: &[&str] = &[
    "hugo.toml",
    "hugo.yaml",
    "hugo.yml",
    "hugo.json",
    "config.toml",
    "config.yaml",
    "config.yml",
    "config.json",
    "config/_default/hugo.toml",
    "config/_default/hugo.yaml",
    "config/_default/config.toml",
    "config/_default/config.yaml",
];

pub(crate) fn parse(dir: &Path, ctx: &ParseCtx) -> anyhow::Result<Parsed> {
    if !dir.is_dir() {
        bail!(
            "{} is not a directory (pass the Hugo site's root folder)",
            ctx.display
        );
    }
    let (config, _config_name) = read_config(dir, &ctx.display)?;
    let content_dir = confined_subdir(dir, &config, "contentDir", "content", &ctx.display)?;
    if !content_dir.is_dir() {
        bail!(
            "{} has no content directory ({}): is it a Hugo site?",
            ctx.display,
            content_dir.display()
        );
    }
    let static_dir = confined_subdir(dir, &config, "staticDir", "static", &ctx.display)?;
    let permalinks = permalink_patterns(&config);
    let site_url = ctx.site_url.clone().or_else(|| {
        json_str(config.get("baseURL").or_else(|| config.get("baseurl")))
            .and_then(|u| url::Url::parse(&u).ok())
            .filter(|u| matches!(u.scheme(), "http" | "https"))
    });

    let mut parsed = Parsed {
        site_url,
        local_roots: vec![static_dir, dir.join("assets")],
        confine_root: Some(dir.to_path_buf()),
        ..Default::default()
    };

    let mut files: Vec<PathBuf> = WalkDir::new(&content_dir)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| {
            p.extension().is_some_and(|e| {
                matches!(
                    e.to_string_lossy().to_ascii_lowercase().as_str(),
                    "md" | "markdown"
                )
            })
        })
        .collect();
    files.sort();
    if files.len() > super::MAX_ITEMS {
        return Err(super::too_many_items(&ctx.display));
    }
    let display_root = ctx.display.trim_end_matches('/').to_string();

    for path in files {
        let rel = path.strip_prefix(&content_dir).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let display = format!(
            "{display_root}/{}",
            path.strip_prefix(dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/")
        );
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let components: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let dirs = components[..components.len() - 1].to_vec();
        let text = match super::read_source_file(&path) {
            Ok(t) => t,
            Err(e) => {
                parsed.skipped.push(skipped(
                    &rel_str,
                    &display,
                    None,
                    "",
                    format!("cannot read: {e}"),
                ));
                continue;
            }
        };
        let text = convert::defuse_markers(&text);
        let (fm, body, body_line) = match super::split_frontmatter(&text) {
            Ok(Some(parts)) => parts,
            Ok(None) => (Value::Object(Default::default()), text.as_str(), 1),
            Err(e) => {
                parsed.skipped.push(skipped(
                    &rel_str,
                    &display,
                    None,
                    "",
                    format!("{e}; fix it and re-run"),
                ));
                parsed
                    .warnings
                    .push(Warning::new(format!("{display}:1"), "frontmatter", e));
                continue;
            }
        };
        let title = json_str(fm.get("title")).unwrap_or_default();
        if stem == "_index" {
            parsed.skipped.push(skipped(
                &rel_str,
                &display,
                None,
                &title,
                "section/home list page (_index.md): seite generates collection indexes (put intro text in content/<collection>/index.md)".into(),
            ));
            continue;
        }
        if fm.get("headless") == Some(&Value::Bool(true)) {
            parsed.skipped.push(skipped(
                &rel_str,
                &display,
                None,
                &title,
                "headless bundle".into(),
            ));
            continue;
        }
        // `about.fr.md`: a translation.
        if let Some((_, lang)) = stem.rsplit_once('.') {
            if lang.len() <= 5 && lang.chars().all(|c| c.is_ascii_alphabetic() || c == '-') {
                parsed.skipped.push(skipped(
                    &rel_str,
                    &display,
                    None,
                    &title,
                    format!(
                        "translation ({lang}): import translations by hand as <name>.{lang}.md"
                    ),
                ));
                continue;
            }
        }

        let is_bundle = stem == "index" && !dirs.is_empty();
        let section = dirs.first().cloned().unwrap_or_default();
        let (kind, hint) = if dirs.is_empty() {
            (Kind::Page, None)
        } else if POST_SECTIONS.contains(&section.to_ascii_lowercase().as_str()) {
            (Kind::Post, None)
        } else if crate::config::find_collection(&section, &ctx.config.collections).is_some() {
            (Kind::Post, Some(section.clone()))
        } else if is_bundle && dirs.len() == 1 {
            // content/about/index.md: a page bundle at the top level.
            (Kind::Page, None)
        } else {
            parsed.warnings.push(Warning::new(
                display.clone(),
                "hugo-section",
                format!("section '{section}' has no seite collection; imported into posts (or add one with `seite collection add {section}` and re-import with --force)"),
            ));
            (Kind::Post, None)
        };
        let name_slug = if is_bundle {
            dirs.last().cloned().unwrap_or_default()
        } else {
            stem.clone()
        };
        let mut item = Item {
            source_id: rel_str.clone(),
            source_ref: display.clone(),
            kind: Some(kind),
            collection_hint: hint,
            title: if title.is_empty() {
                name_slug.replace(['-', '_'], " ")
            } else {
                title
            },
            slug: json_str(fm.get("slug")).unwrap_or_else(|| name_slug.clone()),
            ..Default::default()
        };
        item.date = json_str(fm.get("date"))
            .or_else(|| json_str(fm.get("publishDate")))
            .as_deref()
            .and_then(convert::leading_date);
        item.updated = json_str(fm.get("lastmod"))
            .as_deref()
            .and_then(convert::leading_date);
        item.expires = json_str(fm.get("expiryDate"))
            .as_deref()
            .and_then(convert::leading_date);
        item.description = json_str(fm.get("description")).or_else(|| json_str(fm.get("summary")));
        item.tags = terms_from_json(fm.get("tags"), false);
        item.categories = terms_from_json(fm.get("categories"), false);
        item.series = terms_from_json(fm.get("series"), false).into_iter().next();
        item.weight = fm
            .get("weight")
            .and_then(Value::as_i64)
            .and_then(|w| i32::try_from(w).ok())
            .filter(|_| kind == Kind::Page);
        item.author = json_str(fm.get("author"))
            .or_else(|| fm.get("author").and_then(|a| json_str(a.get("name"))))
            .or_else(|| terms_from_json(fm.get("authors"), false).into_iter().next());
        if fm.get("draft") == Some(&Value::Bool(true)) {
            item.draft = Some("draft".into());
        }
        item.image = fm
            .get("images")
            .and_then(|i| i.get(0))
            .and_then(|i| json_str(Some(i)))
            .or_else(|| json_str(fm.get("image")))
            .or_else(|| json_str(fm.get("featured_image")))
            .or_else(|| json_str(fm.get("featureImage")))
            .or_else(|| fm.get("cover").and_then(|c| json_str(c.get("image"))))
            .or_else(|| json_str(fm.get("cover")));
        let item_dir = path.parent().unwrap_or(&content_dir).to_path_buf();
        if is_bundle && item.image.is_none() {
            item.image = bundle_cover(&item_dir);
        }
        item.base_dirs = vec![item_dir];

        // Old URLs.
        let slug_for_url = item.slug.clone();
        let old = match json_str(fm.get("url")) {
            Some(u) => u,
            None => {
                let pattern = permalinks
                    .get(section.as_str())
                    .filter(|_| !dirs.is_empty());
                match (pattern, item.date) {
                    (Some(p), date) => {
                        expand_permalink(p, date, &slug_for_url, &item.title, &section, &name_slug)
                    }
                    (None, _) => {
                        let parents = if is_bundle {
                            &dirs[..dirs.len() - 1]
                        } else {
                            &dirs[..]
                        };
                        let mut parts: Vec<String> = parents.to_vec();
                        parts.push(slug_for_url.clone());
                        format!("/{}/", parts.join("/")).to_lowercase()
                    }
                }
            }
        };
        item.aliases.push(old);
        let page_dir_url = format!("/{}/", dirs.join("/"));
        for a in terms_from_json(fm.get("aliases"), false) {
            item.aliases
                .push(if a.starts_with('/') || a.contains("://") {
                    a
                } else {
                    format!("{page_dir_url}{a}")
                });
        }

        // Keys for ref/relref.
        let no_ext = rel_str
            .rsplit_once('.')
            .map_or(rel_str.as_str(), |(s, _)| s)
            .to_string();
        item.keys.push(rel_str.clone());
        item.keys.push(no_ext.clone());
        if is_bundle {
            let bundle = dirs.join("/");
            item.keys.push(bundle.clone());
            item.keys.push(dirs.last().cloned().unwrap_or_default());
        } else {
            item.keys.push(
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            );
            item.keys.push(stem.clone());
        }

        let mut warnings = Vec::new();
        // Markdown is kept as written; HTML in it that can run script is
        // reported, not rewritten.
        for (line, what) in convert::untrusted_html_lines(body) {
            warnings.push(Warning::for_item(
                &item,
                Some(body_line + line - 1),
                "untrusted-html",
                format!("{what} in the body: review it before publishing"),
            ));
        }
        item.body = shortcodes(body, body_line, &mut item, &mut warnings);
        parsed.warnings.extend(warnings);
        parsed.items.push(item);
    }
    Ok(parsed)
}

fn skipped(id: &str, source: &str, kind: Option<Kind>, title: &str, reason: String) -> Skipped {
    Skipped {
        source_id: id.to_string(),
        source: source.to_string(),
        kind,
        title: title.to_string(),
        reason,
    }
}

/// A directory named by the (untrusted) Hugo config, kept inside the source:
/// relative, no `..`, and (when it exists) its real path under the source's.
fn confined_subdir(
    dir: &Path,
    config: &Value,
    key: &str,
    default: &str,
    display: &str,
) -> anyhow::Result<PathBuf> {
    let value = json_str(config.get(key)).unwrap_or_else(|| default.to_string());
    let rel = Path::new(&value);
    let escapes = rel.is_absolute()
        || rel.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        });
    if escapes {
        bail!("{display}: `{key} = \"{value}\"` points outside the site directory; refusing to read it");
    }
    let path = dir.join(rel);
    if path.exists() {
        let root = std::fs::canonicalize(dir)?;
        let real = std::fs::canonicalize(&path)?;
        if !real.starts_with(&root) {
            bail!("{display}: `{key}` ({value}) resolves outside the site directory (symlink?); refusing to read it");
        }
        return Ok(path);
    }
    Ok(path)
}

/// The site config as JSON (empty when there is none).
fn read_config(dir: &Path, display: &str) -> anyhow::Result<(Value, Option<String>)> {
    for name in CONFIG_NAMES {
        let path = dir.join(name);
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let value = if name.ends_with(".toml") {
            toml::from_str::<toml::Value>(&text)
                .map(super::toml_to_json)
                .map_err(|e| anyhow::anyhow!("{display}/{name}: invalid TOML: {e}"))?
        } else if name.ends_with(".json") {
            serde_json::from_str(&text)
                .map_err(|e| anyhow::anyhow!("{display}/{name}:{}: invalid JSON: {e}", e.line()))?
        } else {
            serde_yaml_ng::from_str(&text)
                .map_err(|e| anyhow::anyhow!("{display}/{name}: invalid YAML: {e}"))?
        };
        return Ok((value, Some(name.to_string())));
    }
    if dir.join("content").is_dir() {
        return Ok((Value::Object(Default::default()), None));
    }
    bail!("{display} has no hugo.toml/config.toml (or .yaml/.json): is it a Hugo site?");
}

/// `[permalinks]` section → pattern (old `posts = "…"` and new
/// `[permalinks.page] posts = "…"` forms).
fn permalink_patterns(config: &Value) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(Value::Object(map)) = config.get("permalinks") else {
        return out;
    };
    for (k, v) in map {
        match v {
            Value::String(p) => {
                out.insert(k.clone(), p.clone());
            }
            Value::Object(inner) if k == "page" => {
                for (section, p) in inner {
                    if let Value::String(p) = p {
                        out.insert(section.clone(), p.clone());
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn expand_permalink(
    pattern: &str,
    date: Option<chrono::NaiveDate>,
    slug: &str,
    title: &str,
    section: &str,
    filename: &str,
) -> String {
    let d = date.unwrap_or_default();
    let replacements: [(&str, String); 11] = [
        (":contentbasename", filename.to_string()),
        (":filename", filename.to_string()),
        (":monthname", d.format("%B").to_string().to_lowercase()),
        (":sections", section.to_string()),
        (":section", section.to_string()),
        (":year", format!("{:04}", d.year())),
        (":month", format!("{:02}", d.month())),
        (":day", format!("{:02}", d.day())),
        (":slugorfilename", slug.to_string()),
        (":slug", slug.to_string()),
        (":title", crate::content::slug_from_title(title)),
    ];
    let mut url = pattern.to_string();
    for (k, v) in &replacements {
        url = url.replace(k, v);
    }
    if !url.starts_with('/') {
        url.insert(0, '/');
    }
    url
}

/// A bundle resource named like a cover image.
fn bundle_cover(dir: &Path) -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| super::images::has_image_extension(n))
        .collect();
    names.sort();
    names.into_iter().find(|n| {
        let lower = n.to_ascii_lowercase();
        ["cover", "feature", "featured", "thumbnail", "banner"]
            .iter()
            .any(|p| lower.starts_with(p))
    })
}

/// One Hugo shortcode argument list → (positional, named).
fn hugo_args(raw: &str) -> (Vec<String>, HashMap<String, String>) {
    static ARG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?:([A-Za-z_][\w-]*)\s*=\s*)?(?:"((?:[^"\\]|\\.)*)"|`([^`]*)`|([^\s"`]+))"#)
            .expect("valid regex")
    });
    let mut positional = Vec::new();
    let mut named = HashMap::new();
    for c in ARG.captures_iter(raw) {
        let value = c
            .get(2)
            .map(|m| m.as_str().replace("\\\"", "\""))
            .or_else(|| c.get(3).or(c.get(4)).map(|m| m.as_str().to_string()))
            .unwrap_or_default();
        match c.get(1) {
            Some(k) => {
                named.insert(k.as_str().to_string(), value);
            }
            None => positional.push(value),
        }
    }
    (positional, named)
}

/// Convert Hugo shortcodes (outside fenced code) in a body.
fn shortcodes(
    body: &str,
    first_line: usize,
    item: &mut Item,
    warnings: &mut Vec<Warning>,
) -> String {
    static TAG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)\{\{([<%])\s*(/\*)?\s*(/?)\s*([\w./-]+)(.*?)\s*(\*/)?\s*[>%]\}\}")
            .expect("valid regex")
    });
    let fences = convert::fenced_ranges(body);
    let in_fence = |pos: usize| fences.iter().any(|r| r.contains(&pos));
    let line_at = |offset: usize| first_line + convert::line_of(body, offset) - 1;

    let tags: Vec<_> = TAG
        .captures_iter(body)
        .filter(|c| !in_fence(c.get(0).expect("m").start()))
        .collect();
    let mut tokens = convert::Tokens::default();
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    let mut i = 0;
    while i < tags.len() {
        let c = &tags[i];
        let whole = c.get(0).expect("match");
        if whole.start() < last {
            i += 1;
            continue;
        }
        out.push_str(&body[last..whole.start()]);
        last = whole.end();
        i += 1;
        let escaped = c.get(2).is_some();
        let closing = !c[3].is_empty();
        let name = c[4].to_string();
        let (positional, named) = hugo_args(&c[5]);
        let arg = |key: &str, pos: usize| {
            named
                .get(key)
                .cloned()
                .or_else(|| positional.get(pos).cloned())
                .unwrap_or_default()
        };
        if escaped {
            // `{{</* x */>}}` shows a shortcode literally.
            let shown = whole.as_str().replace("/*", "").replace("*/", "");
            let shown = shown.split_whitespace().collect::<Vec<_>>().join(" ");
            out.push_str(&tokens.push(inline_code(&shown)));
            continue;
        }
        let converted = match (closing, name.as_str()) {
            (false, "youtube") => Some(convert::shortcode(
                "youtube",
                &[
                    ("id", arg("id", 0)),
                    ("start", named.get("start").cloned().unwrap_or_default()),
                ],
            )),
            (false, "vimeo") => Some(convert::shortcode("vimeo", &[("id", arg("id", 0))])),
            (false, "gist") => Some(convert::shortcode(
                "gist",
                &[("user", arg("user", 0)), ("id", arg("id", 1))],
            )),
            (false, "figure") => Some(convert::figure(
                &arg("src", 0),
                &named.get("alt").cloned().unwrap_or_default(),
                &named
                    .get("caption")
                    .or_else(|| named.get("title"))
                    .cloned()
                    .unwrap_or_default(),
                &named.get("width").cloned().unwrap_or_default(),
            )),
            (false, "ref" | "relref") => {
                let target = arg("path", 0);
                let key = target
                    .trim_start_matches('/')
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .to_string();
                let anchor = target
                    .split_once('#')
                    .map(|(_, a)| format!("#{a}"))
                    .unwrap_or_default();
                let token = convert::marker('R', item.refs.len());
                item.refs.push(RefLink {
                    token: token.clone(),
                    key,
                    original: whole.as_str().to_string(),
                    line: line_at(whole.start()),
                });
                Some(format!(
                    "{token}{}",
                    anchor.replace(
                        |c: char| !(c.is_ascii_alphanumeric()
                            || matches!(c, '#' | '-' | '_' | '.')),
                        ""
                    )
                ))
            }
            (false, "highlight") => {
                // Body runs to the matching `{{< /highlight >}}`.
                let close = tags[i..]
                    .iter()
                    .position(|t| !t[3].is_empty() && &t[4] == "highlight");
                match close {
                    Some(offset) => {
                        let close_match = tags[i + offset].get(0).expect("match");
                        let code =
                            body[whole.end()..close_match.start()].trim_matches(['\n', '\r']);
                        let lang = positional.first().cloned().unwrap_or_default();
                        last = close_match.end();
                        i += offset + 1;
                        Some(convert::code_fence(&lang, code))
                    }
                    None => None,
                }
            }
            _ => None,
        };
        match converted {
            Some(text) => out.push_str(&tokens.push(text)),
            None => {
                out.push_str(&tokens.push(inline_code(whole.as_str())));
                if !closing {
                    warnings.push(Warning::for_item(
                        item,
                        Some(line_at(whole.start())),
                        "hugo-shortcode",
                        format!(
                            "Hugo shortcode `{name}` kept as inline code: replace it with Markdown or a seite shortcode (templates/shortcodes/{name}.html)"
                        ),
                    ));
                }
            }
        }
    }
    out.push_str(&body[last..]);
    // Any other `{{<` / `{{%` (malformed tags, text) is escaped; only the
    // generated shortcodes and code spans/fences (tokens) keep delimiters.
    convert::finish_markdown(&out, &tokens, false, &mut Vec::new())
}

/// `text` as an inline code span (backtick fence longer than any run inside).
fn inline_code(text: &str) -> String {
    let flat = text.replace('\n', " ");
    let longest = flat.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    let pad = if flat.starts_with('`') || flat.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{flat}{pad}{fence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hugo_shortcodes() {
        let mut item = Item::default();
        let mut warnings = Vec::new();
        let body = "{{< youtube w7Ft2ymGmfc >}}\n{{< vimeo id=\"146022717\" >}}\n{{< gist spf13 7896402 >}}\n{{< figure src=\"cat.jpg\" caption=\"A \\\"cat\\\"\" >}}\n[about]({{< ref \"about.md\" >}})\n{{< highlight go \"linenos=table\" >}}\nfunc main() {}\n{{< /highlight >}}\n{{< tweet user=\"x\" id=\"1\" >}}\n{{% notice note %}}\nHi\n{{% /notice %}}\n```\n{{< keep >}}\n```\n";
        let out = shortcodes(body, 3, &mut item, &mut warnings);
        assert!(
            out.contains(r#"{{< youtube(id="w7Ft2ymGmfc") >}}"#),
            "{out}"
        );
        assert!(out.contains(r#"{{< vimeo(id="146022717") >}}"#), "{out}");
        assert!(
            out.contains(r#"{{< gist(user="spf13", id="7896402") >}}"#),
            "{out}"
        );
        assert!(
            out.contains(r#"{{< figure(src="cat.jpg", caption="A \"cat\"") >}}"#),
            "{out}"
        );
        assert!(
            out.contains(&format!("[about]({})", convert::marker('R', 0))),
            "{out}"
        );
        assert!(out.contains("```go\nfunc main() {}\n```"), "{out}");
        assert!(out.contains(r#"`{{< tweet user="x" id="1" >}}`"#), "{out}");
        assert!(out.contains("`{{% notice note %}}`"), "{out}");
        assert!(out.contains("```\n{{< keep >}}\n```"), "{out}");
        assert_eq!(item.refs[0].key, "about.md");
        let codes: Vec<_> = warnings
            .iter()
            .map(|w| (w.code, w.source.clone()))
            .collect();
        assert_eq!(codes.len(), 2, "{codes:?}");
        assert!(codes[0].1.ends_with(":11"), "{codes:?}");
        // seite's parser accepts the result.
        let (calls, errors) =
            crate::shortcodes::parser::parse_shortcodes_recovering(&out, Path::new("x.md"));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(calls.len(), 4);
    }

    #[test]
    fn test_hugo_hostile_content_stays_inert() {
        let mut item = Item::default();
        let mut warnings = Vec::new();
        let body = "{{< highlight md >}}\n```\n{{< contact_form() >}}\n```\n{{< /highlight >}}\n\nText {{< contact_form() and an unclosed {{% tag\n\n{{< figure src=\"a.png\" caption=\"{{< contact_form() >}}\" >}}\n";
        let out = shortcodes(body, 1, &mut item, &mut warnings);
        assert!(out.starts_with("````md\n```\n"), "{out}");
        let (calls, errors) =
            crate::shortcodes::parser::parse_shortcodes_recovering(&out, Path::new("x.md"));
        assert!(errors.is_empty(), "{errors:?}\n{out}");
        let names: Vec<&str> = calls.iter().map(|c| c.name.as_str()).collect();
        assert!(!names.contains(&"contact_form"), "{out}");
    }

    #[test]
    fn test_confined_subdir_rejects_escapes() {
        let tmp = tempfile::TempDir::new().unwrap();
        for bad in ["../outside", "/etc", "content/../../x"] {
            let config = serde_json::json!({ "contentDir": bad });
            assert!(
                confined_subdir(tmp.path(), &config, "contentDir", "content", "site").is_err(),
                "{bad}"
            );
        }
        let config = serde_json::json!({});
        assert!(confined_subdir(tmp.path(), &config, "contentDir", "content", "site").is_ok());
        #[cfg(unix)]
        {
            let outside = tempfile::TempDir::new().unwrap();
            std::os::unix::fs::symlink(outside.path(), tmp.path().join("linked")).unwrap();
            let config = serde_json::json!({ "contentDir": "linked" });
            assert!(confined_subdir(tmp.path(), &config, "contentDir", "content", "site").is_err());
        }
    }

    #[test]
    fn test_expand_permalink() {
        let d = chrono::NaiveDate::from_ymd_opt(2021, 5, 6);
        assert_eq!(
            expand_permalink("/:year/:month/:slug/", d, "hi", "Hi", "posts", "hi"),
            "/2021/05/hi/"
        );
        assert_eq!(
            expand_permalink("/:section/:title/", d, "x", "Hello World", "blog", "x"),
            "/blog/hello-world/"
        );
    }

    #[test]
    fn test_inline_code() {
        assert_eq!(inline_code("a `b` c"), "``a `b` c``");
        assert_eq!(inline_code("x"), "`x`");
    }
}
