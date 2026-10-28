//! Jekyll site directory.
//!
//! Posts: `_posts/` and `<category>/_posts/` (`YYYY-MM-DD-title.ext`);
//! drafts: `_drafts/`. Pages: Markdown files with frontmatter outside
//! `_`/`.` directories and `exclude` (`index`/`404` and HTML pages are
//! theme-specific and skipped). Old URLs follow `_config.yml` `permalink`
//! (`date`, `pretty`, `ordinal`, `weekdate`, `none`, or a pattern; a
//! post's own `permalink:` wins). Liquid: `{% highlight %}` → fences,
//! `{% raw %}` unwrapped, `{{ site.baseurl }}` / `relative_url` dropped,
//! `{% link %}` / `{% post_url %}` → the imported page's URL; anything else
//! is left and reported.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::bail;
use chrono::{Datelike, NaiveDate};
use regex::Regex;
use serde_json::Value;
use walkdir::WalkDir;

use super::convert::{self, Tokens};
use super::{json_str, terms_from_json, Item, Kind, ParseCtx, Parsed, RefLink, Skipped, Warning};

const SKIP_DIRS: &[&str] = &[
    "_site",
    "node_modules",
    "vendor",
    ".git",
    ".jekyll-cache",
    ".sass-cache",
];
const MARKDOWN_EXT: &[&str] = &["md", "markdown", "mkd", "mkdn"];

struct Config {
    permalink: String,
    url: Option<url::Url>,
    exclude: Vec<String>,
}

pub(crate) fn parse(dir: &Path, ctx: &ParseCtx) -> anyhow::Result<Parsed> {
    if !dir.is_dir() {
        bail!(
            "{} is not a directory (pass the Jekyll site's root folder)",
            ctx.display
        );
    }
    let config_path = ["_config.yml", "_config.yaml"]
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file());
    let Some(config_path) = config_path else {
        bail!("{} has no _config.yml: is it a Jekyll site?", ctx.display);
    };
    let config_text = std::fs::read_to_string(&config_path)?;
    let raw: Value = if config_text.trim().is_empty() {
        Value::Null
    } else {
        serde_yaml_ng::from_str(&config_text)
            .map_err(|e| anyhow::anyhow!("{}/_config.yml: invalid YAML: {e}", ctx.display))?
    };
    let config = Config {
        permalink: json_str(raw.get("permalink")).unwrap_or_else(|| "date".to_string()),
        url: json_str(raw.get("url"))
            .and_then(|u| url::Url::parse(&u).ok())
            .filter(|u| matches!(u.scheme(), "http" | "https")),
        exclude: terms_from_json(raw.get("exclude"), false),
    };

    let mut parsed = Parsed {
        site_url: ctx.site_url.clone().or_else(|| config.url.clone()),
        local_roots: vec![dir.to_path_buf()],
        confine_root: Some(dir.to_path_buf()),
        ..Default::default()
    };

    let mut files: Vec<PathBuf> = WalkDir::new(dir)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            e.depth() == 0 || !(SKIP_DIRS.contains(&name.as_ref()) || name.starts_with('.'))
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    files.sort();
    if files.len() > super::MAX_ITEMS {
        return Err(super::too_many_items(&ctx.display));
    }

    for path in files {
        let rel = path.strip_prefix(dir).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let components: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let dirs = &components[..components.len().saturating_sub(1)];
        let in_posts = dirs.iter().any(|d| d == "_posts");
        let in_drafts = dirs.first().is_some_and(|d| d == "_drafts");
        let is_markdown = MARKDOWN_EXT.contains(&ext.as_str());
        let is_html = ext == "html" || ext == "htm";
        if !(is_markdown || is_html) {
            continue;
        }
        if config
            .exclude
            .iter()
            .any(|x| rel_str == *x || rel_str.starts_with(&format!("{}/", x.trim_end_matches('/'))))
        {
            continue;
        }
        let kind = if in_posts || in_drafts {
            Kind::Post
        } else if dirs.iter().any(|d| d.starts_with('_')) {
            continue; // layouts, includes, collections, data
        } else {
            Kind::Page
        };
        let display = format!("{}/{rel_str}", ctx.display.trim_end_matches('/'));
        let text = match super::read_source_file(&path) {
            Ok(t) => t,
            Err(e) => {
                parsed.skipped.push(skip(
                    &rel_str,
                    &display,
                    kind,
                    "",
                    format!("cannot read: {e}"),
                ));
                continue;
            }
        };
        let text = convert::defuse_markers(&text);
        let (fm, body, body_line) = match super::split_frontmatter(&text) {
            Ok(Some(parts)) => parts,
            Ok(None) if kind == Kind::Post => (Value::Object(Default::default()), text.as_str(), 1),
            Ok(None) => continue, // no frontmatter: a static file, not a page
            Err(e) => {
                parsed.skipped.push(skip(
                    &rel_str,
                    &display,
                    kind,
                    "",
                    format!("{e}; fix it and re-run"),
                ));
                parsed
                    .warnings
                    .push(Warning::new(format!("{display}:1"), "frontmatter", e));
                continue;
            }
        };
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if kind == Kind::Page && (is_html || stem == "index" || stem == "404") {
            let title = json_str(fm.get("title")).unwrap_or_default();
            let why = if is_html {
                "HTML page (usually Liquid layout code): port it by hand"
            } else {
                "home/404 page: seite's theme provides these (put homepage text in content/pages/index.md)"
            };
            parsed
                .skipped
                .push(skip(&rel_str, &display, kind, &title, why.to_string()));
            continue;
        }

        let filename_date =
            convert::leading_date(&stem).filter(|_| stem.as_bytes().get(10) == Some(&b'-'));
        let name_slug = if filename_date.is_some() {
            stem[11..].to_string()
        } else {
            stem.clone()
        };
        let mut item = Item {
            source_id: rel_str.clone(),
            source_ref: display.clone(),
            kind: Some(kind),
            title: json_str(fm.get("title")).unwrap_or_else(|| title_from_slug(&name_slug)),
            slug: json_str(fm.get("slug")).unwrap_or_else(|| name_slug.clone()),
            ..Default::default()
        };
        item.date = json_str(fm.get("date"))
            .as_deref()
            .and_then(convert::leading_date)
            .or(filename_date);
        item.updated = ["last_modified_at", "updated", "modified"]
            .iter()
            .find_map(|k| {
                json_str(fm.get(*k))
                    .as_deref()
                    .and_then(convert::leading_date)
            });
        item.description = json_str(fm.get("description")).or_else(|| json_str(fm.get("excerpt")));
        item.tags = terms_from_json(fm.get("tags"), true);
        for t in terms_from_json(fm.get("tag"), true) {
            if !item.tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                item.tags.push(t);
            }
        }
        let mut categories: Vec<String> = dirs
            .iter()
            .take_while(|d| *d != "_posts")
            .cloned()
            .collect();
        for c in terms_from_json(fm.get("categories"), true)
            .into_iter()
            .chain(terms_from_json(fm.get("category"), true))
        {
            if !categories.iter().any(|x| x.eq_ignore_ascii_case(&c)) {
                categories.push(c);
            }
        }
        item.categories = categories.clone();
        item.author = json_str(fm.get("author"))
            .or_else(|| fm.get("author").and_then(|a| json_str(a.get("name"))));
        item.image = json_str(fm.get("image"))
            .or_else(|| fm.get("image").and_then(|i| json_str(i.get("path"))))
            .or_else(|| {
                fm.get("header")
                    .and_then(|h| json_str(h.get("image")).or_else(|| json_str(h.get("teaser"))))
            })
            .or_else(|| json_str(fm.get("feature_image")))
            .or_else(|| json_str(fm.get("cover")))
            .map(|i| resolve_baseurl(&i));
        if in_drafts {
            item.draft = Some("draft (_drafts)".to_string());
        } else if fm.get("published") == Some(&Value::Bool(false)) {
            item.draft = Some("published: false".to_string());
        }
        item.base_dirs = vec![path.parent().unwrap_or(dir).to_path_buf()];

        // Old URL.
        let permalink = json_str(fm.get("permalink"));
        let old_url = match kind {
            Kind::Post => item.date.map(|d| {
                post_url(
                    permalink.as_deref().unwrap_or(&config.permalink),
                    d,
                    &name_slug,
                    &categories,
                )
            }),
            Kind::Page => Some(match permalink.as_deref() {
                Some(p) => p.to_string(),
                None => page_url(&rel_str, &config.permalink),
            }),
        };
        if let Some(u) = old_url.filter(|_| !in_drafts) {
            item.aliases.push(u);
        }
        if let Some(Value::Array(redirects)) = fm.get("redirect_from") {
            item.aliases.extend(
                redirects
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string)),
            );
        } else if let Some(r) = json_str(fm.get("redirect_from")) {
            item.aliases.push(r);
        }

        // Keys for {% link %} / {% post_url %}.
        item.keys.push(rel_str.clone());
        if kind == Kind::Post {
            item.keys.push(format!("post:{stem}"));
            let under_posts = rel_str
                .split_once("_posts/")
                .map(|(_, r)| r)
                .unwrap_or(&rel_str);
            if let Some(no_ext) = under_posts.rsplit_once('.').map(|(s, _)| s) {
                item.keys.push(format!("post:{no_ext}"));
            }
        }

        let mut warnings = Vec::new();
        let body = if is_html {
            let mut tokens = Tokens::default();
            let liquid = liquid(body, body_line, &mut item, &mut warnings);
            let mut notes = Vec::new();
            let prepared = convert::prepare_html(&liquid, &mut tokens, &mut notes);
            let md = convert::finish_markdown(
                &convert::html_to_markdown(&prepared),
                &tokens,
                true,
                &mut notes,
            );
            for note in notes {
                warnings.push(Warning::for_item(&item, None, "untrusted-html", note));
            }
            md
        } else {
            // Markdown is kept as written; HTML in it that can run script
            // is reported, not rewritten.
            for (line, what) in convert::untrusted_html_lines(body) {
                warnings.push(Warning::for_item(
                    &item,
                    Some(body_line + line - 1),
                    "untrusted-html",
                    format!("{what} in the body: review it before publishing"),
                ));
            }
            convert::neutralize_shortcode_delimiters(&liquid(
                body,
                body_line,
                &mut item,
                &mut warnings,
            ))
        };
        item.body = body;
        parsed.warnings.extend(warnings);
        parsed.items.push(item);
    }
    Ok(parsed)
}

fn skip(id: &str, source: &str, kind: Kind, title: &str, reason: String) -> Skipped {
    Skipped {
        source_id: id.to_string(),
        source: source.to_string(),
        kind: Some(kind),
        title: title.to_string(),
        reason,
    }
}

fn title_from_slug(slug: &str) -> String {
    let words: Vec<String> = slug
        .split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect();
    if words.is_empty() {
        "Untitled".to_string()
    } else {
        words.join(" ")
    }
}

/// `{{ site.baseurl }}/x` and friends → `/x`.
fn resolve_baseurl(s: &str) -> String {
    static BASEURL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{\{-?\s*site\.(?:baseurl|url)\s*-?\}\}").expect("valid regex")
    });
    static RELATIVE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"\{\{-?\s*(?:"([^"]*)"|'([^']*)')\s*\|\s*(?:relative_url|absolute_url)\s*-?\}\}"#,
        )
        .expect("valid regex")
    });
    let s = RELATIVE.replace_all(s, |c: &regex::Captures| {
        c.get(1)
            .or(c.get(2))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    });
    BASEURL.replace_all(&s, "").into_owned()
}

/// Convert Jekyll Liquid in a body; records refs on `item`, warns about
/// what is left.
fn liquid(body: &str, first_line: usize, item: &mut Item, warnings: &mut Vec<Warning>) -> String {
    static HIGHLIGHT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)\{%-?\s*highlight\s+([A-Za-z0-9_+#.-]+)[^%]*-?%\}\r?\n?(.*?)\{%-?\s*endhighlight\s*-?%\}")
            .expect("valid regex")
    });
    static RAW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)\{%-?\s*raw\s*-?%\}(.*?)\{%-?\s*endraw\s*-?%\}").expect("valid regex")
    });
    static LINK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{%-?\s*(link|post_url)\s+([^\s%]+)\s*-?%\}").expect("valid regex")
    });
    static LIQUID: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{%-?\s*([a-z_]+)[^%]*%\}|\{\{-?[^}]*\}\}").expect("valid regex")
    });

    let line_at = |offset: usize| first_line + convert::line_of(body, offset) - 1;
    // Literal regions (raw blocks, highlighted code) and converted tags are
    // located in the original body so reported lines are right.
    let literal: Vec<std::ops::Range<usize>> = RAW
        .find_iter(body)
        .chain(HIGHLIGHT.find_iter(body))
        .map(|m| m.range())
        .collect();
    let outside =
        |r: &std::ops::Range<usize>| !literal.iter().any(|l| l.start <= r.start && r.end <= l.end);
    let link_lines: Vec<usize> = LINK
        .find_iter(body)
        .map(|m| m.range())
        .filter(|r| outside(r))
        .map(|r| line_at(r.start))
        .collect();
    for c in LIQUID.captures_iter(body) {
        let whole = c.get(0).expect("match");
        let tag = c.get(1).map(|m| m.as_str());
        let converted = matches!(
            tag,
            Some("highlight" | "endhighlight" | "raw" | "endraw" | "link" | "post_url")
        ) || resolve_baseurl(whole.as_str()) != whole.as_str();
        if converted || !outside(&whole.range()) {
            continue;
        }
        let what = whole.as_str().chars().take(60).collect::<String>();
        warnings.push(Warning::for_item(
            item,
            Some(line_at(whole.start())),
            "liquid",
            format!("Liquid `{what}` left in place: replace it with plain Markdown or a seite shortcode"),
        ));
    }

    let mut tokens = Tokens::default();
    let converted = RAW.replace_all(body, |c: &regex::Captures| tokens.push(c[1].to_string()));
    let converted = HIGHLIGHT.replace_all(&converted, |c: &regex::Captures| {
        let code = c[2].trim_end_matches(['\n', '\r']);
        tokens.push(convert::code_fence(&c[1], code))
    });
    let converted = resolve_baseurl(&converted);
    let mut out = String::with_capacity(converted.len());
    let mut last = 0;
    for (i, c) in LINK.captures_iter(&converted).enumerate() {
        let whole = c.get(0).expect("match");
        out.push_str(&converted[last..whole.start()]);
        let target = c[2].trim_matches(['"', '\'']);
        let key = if &c[1] == "post_url" {
            format!("post:{}", target.trim_start_matches('/'))
        } else {
            target.trim_start_matches('/').to_string()
        };
        let token = convert::marker('R', item.refs.len());
        item.refs.push(RefLink {
            token: token.clone(),
            key,
            original: whole.as_str().to_string(),
            line: link_lines.get(i).copied().unwrap_or(first_line),
        });
        out.push_str(&token);
        last = whole.end();
    }
    out.push_str(&converted[last..]);
    tokens.restore(&out)
}

/// A post's URL under a Jekyll permalink style or pattern.
fn post_url(style: &str, date: NaiveDate, title: &str, categories: &[String]) -> String {
    let pattern = match style {
        "date" => "/:categories/:year/:month/:day/:title:output_ext",
        "pretty" => "/:categories/:year/:month/:day/:title/",
        "ordinal" => "/:categories/:year/:y_day/:title:output_ext",
        "weekdate" => "/:categories/:year/W:week/:short_day/:title:output_ext",
        "none" => "/:categories/:title:output_ext",
        custom => custom,
    };
    let cats = categories
        .iter()
        .map(|c| c.to_lowercase().replace(' ', "-"))
        .collect::<Vec<_>>()
        .join("/");
    let short_day = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        [date.weekday().num_days_from_monday() as usize];
    // Longest placeholders first (`:i_month` before `:month`).
    let replacements: [(&str, String); 14] = [
        (":categories", cats),
        (":output_ext", ".html".to_string()),
        (":short_year", format!("{:02}", date.year() % 100)),
        (":short_day", short_day.to_string()),
        (":i_month", date.month().to_string()),
        (":i_day", date.day().to_string()),
        (":y_day", format!("{:03}", date.ordinal())),
        (":month", format!("{:02}", date.month())),
        (":title", title.to_string()),
        (":slug", title.to_string()),
        (":year", date.year().to_string()),
        (":week", format!("{:02}", date.iso_week().week())),
        (":day", format!("{:02}", date.day())),
        (":name", title.to_string()),
    ];
    let mut url = pattern.to_string();
    for (k, v) in &replacements {
        url = url.replace(k, v);
    }
    collapse_slashes(&url)
}

/// A page's default URL: `/dir/name.html`, or `/dir/name/` with `pretty`.
fn page_url(rel: &str, style: &str) -> String {
    let no_ext = rel.rsplit_once('.').map_or(rel, |(s, _)| s);
    let pretty = style == "pretty" || style.ends_with('/');
    if pretty {
        format!("/{no_ext}/")
    } else {
        format!("/{no_ext}.html")
    }
}

fn collapse_slashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch == '/' && out.ends_with('/') {
            continue;
        }
        out.push(ch);
    }
    if !out.starts_with('/') {
        out.insert(0, '/');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_url_styles() {
        let d = NaiveDate::from_ymd_opt(2020, 3, 4).unwrap();
        assert_eq!(post_url("date", d, "hello", &[]), "/2020/03/04/hello.html");
        assert_eq!(
            post_url("pretty", d, "hello", &["Rust Lang".into()]),
            "/rust-lang/2020/03/04/hello/"
        );
        assert_eq!(
            post_url("/blog/:year/:i_month/:title/", d, "hello", &[]),
            "/blog/2020/3/hello/"
        );
        assert_eq!(post_url("ordinal", d, "x", &[]), "/2020/064/x.html");
    }

    #[test]
    fn test_page_url() {
        assert_eq!(page_url("about.md", "date"), "/about.html");
        assert_eq!(page_url("docs/setup.md", "pretty"), "/docs/setup/");
    }

    #[test]
    fn test_liquid_conversion() {
        let mut item = Item::default();
        let mut warnings = Vec::new();
        let body = "See {% post_url 2020-01-01-hi %} and [a]({{ site.baseurl }}/about/).\n\n{% highlight ruby linenos %}\nputs 1\n{% endhighlight %}\n\n{% raw %}{{ keep }}{% endraw %}\n\n{% include note.html %}\n";
        let out = liquid(body, 5, &mut item, &mut warnings);
        assert!(
            out.contains(&format!(
                "See {} and [a](/about/).",
                convert::marker('R', 0)
            )),
            "{out}"
        );
        assert!(out.contains("```ruby\nputs 1\n```"), "{out}");
        assert!(out.contains("{{ keep }}"), "{out}");
        assert_eq!(item.refs[0].key, "post:2020-01-01-hi");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].message.contains("include"));
        assert!(
            warnings[0].source.ends_with(":13"),
            "{}",
            warnings[0].source
        );
    }
}
