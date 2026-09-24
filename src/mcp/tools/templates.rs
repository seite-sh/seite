//! `seite_list_templates` — what an agent needs before editing templates:
//! overrides, base blocks, Tera context variables per template kind,
//! shortcodes with their parameters, and data file keys.

use std::collections::BTreeSet;
use std::path::Path;

use walkdir::WalkDir;

use super::{nullable, object_schema, prop, string_array, Args, ServerState, ToolResult};
use crate::config::SiteConfig;
use crate::mcp::content_index;
use crate::templates::{embedded_default, load_templates_with_warnings, EMBEDDED_TEMPLATE_NAMES};

/// Tera context variables and their meaning. Derived from the contexts built
/// in `src/build/mod.rs` (page templates) and `src/shortcodes/mod.rs`
/// (shortcode templates); `test_context_variables_cover_build` fails if the
/// build starts inserting a variable that is not listed here.
const VARIABLES: &[(&str, &str)] = &[
    ("site", "Site metadata for the page's language: title, description, base_url, base_path, language, author"),
    ("data", "Merged data files from the data directory (data.<file>.<key>)"),
    ("lang", "Language code of the page being rendered"),
    ("lang_prefix", "'' for the default language, '/<lang>' otherwise; prefix internal links with it"),
    ("default_language", "The site's default language code"),
    ("t", "UI strings for this language (defaults merged with data/i18n/<lang>.yaml)"),
    ("math_enabled", "Whether [build] math (KaTeX) is on"),
    ("katex_css_url", "KaTeX stylesheet URL (only set when math is enabled)"),
    ("translations", "[{lang, url}] versions of this page in other languages (empty when none)"),
    ("page", "The page: title, content (rendered HTML), date, updated, description, image, slug, tags, url, collection, robots, word_count, reading_time, excerpt, toc, extra"),
    ("prev_post", "Previous item in the collection {title, url, date, description}, or null"),
    ("next_post", "Next item in the collection {title, url, date, description}, or null"),
    ("nav", "Docs sidebar for nested collections: [{name, label, items: [{title, url, active}]}]; empty otherwise"),
    ("collections", "Collections shown on this listing: [{name, label, items}]"),
    ("items", "Items of this listing: [{title, date, description, slug, tags, url, word_count, reading_time, excerpt}]"),
    ("pagination", "{current_page, total_pages, prev_url, next_url, base_url} on paginated listings"),
    ("tags", "All tags: [{name, url, count}]"),
    ("tag_name", "The tag this page lists"),
    ("tags_url", "URL of the tags index page"),
];

/// Shortcode template variables (besides the shortcode's own arguments).
const SHORTCODE_VARIABLES: &[(&str, &str)] = &[
    (
        "page",
        "{title, slug, collection, tags} of the page using the shortcode",
    ),
    ("site", "{title, base_url, language, contact}"),
    ("t", "UI strings for the page's language"),
    (
        "body",
        "Body shortcodes only: the markdown between the opening tag and {{% end %}}",
    ),
];

const COMMON: &[&str] = &[
    "site",
    "data",
    "page",
    "lang",
    "lang_prefix",
    "default_language",
    "t",
    "math_enabled",
    "katex_css_url",
    "translations",
];

/// Template kinds and the variables (beyond [`COMMON`]) each receives.
const KINDS: &[(&str, &[&str])] = &[
    ("item", &["prev_post", "next_post", "nav"]),
    ("homepage", &["collections", "items", "nav"]),
    (
        "collection_index",
        &["collections", "items", "nav", "pagination"],
    ),
    ("not_found", &["collections"]),
    ("tags", &["tags"]),
    ("tag", &["tag_name", "items", "tags_url"]),
];

pub fn output_schema() -> serde_json::Value {
    let name_desc = object_schema(
        serde_json::json!({ "name": { "type": "string" }, "description": { "type": "string" } }),
        &["name", "description"],
    );
    object_schema(
        serde_json::json!({
            "template_dir": prop("string", "Template directory relative to the site root"),
            "active_theme": prop("string", "Theme matching base.html: a theme name, 'default' (no base.html), or 'custom'"),
            "base_template": object_schema(
                serde_json::json!({
                    "source": prop("string", "user or embedded"),
                    "path": nullable("string"),
                    "blocks": string_array(),
                    "error": { "type": "string" }
                }),
                &["source", "path", "blocks"],
            ),
            "user_templates": {
                "type": "array",
                "items": object_schema(
                    serde_json::json!({
                        "name": prop("string", "Tera template name (path inside the template dir)"),
                        "path": { "type": "string" },
                        "kind": prop("string", "base, homepage, item, collection_index, not_found, tags, tag, shortcode, or other (partials, frontmatter `template:` targets)"),
                        "overrides_default": { "type": "boolean" }
                    }),
                    &["name", "path", "kind", "overrides_default"],
                )
            },
            "embedded_defaults": {
                "type": "array",
                "items": object_schema(
                    serde_json::json!({ "name": { "type": "string" }, "overridden": { "type": "boolean" } }),
                    &["name", "overridden"],
                )
            },
            "context": {
                "type": "array",
                "items": object_schema(
                    serde_json::json!({
                        "kind": { "type": "string" },
                        "templates": string_array(),
                        "variables": { "type": "array", "items": name_desc }
                    }),
                    &["kind", "templates", "variables"],
                )
            },
            "shortcodes": {
                "type": "array",
                "items": object_schema(
                    serde_json::json!({
                        "name": { "type": "string" },
                        "source": prop("string", "builtin, custom, or custom (overrides builtin)"),
                        "syntax": prop("string", "inline or body"),
                        "params": {
                            "type": "array",
                            "items": object_schema(
                                serde_json::json!({ "name": { "type": "string" }, "required": { "type": "boolean" } }),
                                &["name", "required"],
                            )
                        },
                        "usage": { "type": "string" }
                    }),
                    &["name", "source", "syntax", "params", "usage"],
                )
            },
            "data": object_schema(
                serde_json::json!({
                    "dir": { "type": "string" },
                    "keys": {
                        "type": "array",
                        "items": object_schema(
                            serde_json::json!({
                                "key": { "type": "string" },
                                "type": { "type": "string" },
                                "keys": string_array()
                            }),
                            &["key", "type"],
                        )
                    },
                    "error": { "type": "string" }
                }),
                &["dir", "keys"],
            ),
            "warnings": string_array()
        }),
        &[
            "template_dir",
            "active_theme",
            "base_template",
            "user_templates",
            "embedded_defaults",
            "context",
            "shortcodes",
            "data",
            "warnings",
        ],
    )
}

fn describe(names: &[&str], table: &[(&str, &str)]) -> Vec<serde_json::Value> {
    names
        .iter()
        .map(|name| {
            let description = table
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, d)| *d)
                .unwrap_or("");
            serde_json::json!({ "name": name, "description": description })
        })
        .collect()
}

/// All `.html` templates under `dir`, as Tera names (`/`-separated, relative).
fn user_template_names(dir: &Path) -> Vec<String> {
    if !dir.is_dir() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|x| x == "html"))
        .map(|e| content_index::relative_path(dir, e.path()))
        .collect()
}

fn template_kind(config: &SiteConfig, name: &str) -> &'static str {
    if name.starts_with("shortcodes/") {
        return "shortcode";
    }
    match name {
        "base.html" => return "base",
        "index.html" => return "homepage",
        "404.html" => return "not_found",
        "tags.html" => return "tags",
        "tag.html" => return "tag",
        _ => {}
    }
    if config
        .collections
        .iter()
        .any(|c| name == format!("{}-index.html", c.name))
    {
        "collection_index"
    } else if config
        .collections
        .iter()
        .any(|c| c.default_template == name)
    {
        "item"
    } else {
        "other"
    }
}

const TEMPLATE_KEYWORDS: &[&str] = &[
    "if",
    "elif",
    "else",
    "endif",
    "for",
    "endfor",
    "in",
    "set",
    "set_global",
    "and",
    "or",
    "not",
    "is",
    "true",
    "false",
    "True",
    "False",
    "none",
    "None",
    "loop",
    "defined",
    "undefined",
    "odd",
    "even",
    "string",
    "number",
    "iterable",
    "object",
    "divisibleby",
    "starting_with",
    "ending_with",
    "containing",
    "matching",
    "super",
    "__tera_context",
];

/// Parameters a shortcode template reads, in first-use order, with whether
/// each is required (used for output without a `default` filter and never
/// tested in an `if`). A lexical heuristic over the template's tags.
fn shortcode_params(template: &str) -> Vec<(String, bool)> {
    let mut order: Vec<String> = Vec::new();
    let mut bare: BTreeSet<String> = BTreeSet::new();
    let mut optional: BTreeSet<String> = BTreeSet::new();
    let mut locals: BTreeSet<String> = BTreeSet::new();

    let mut rest = template;
    while let Some(start) = rest.find(['{']) {
        let after = &rest[start..];
        let (close, is_output) = if after.starts_with("{{") {
            ("}}", true)
        } else if after.starts_with("{%") {
            ("%}", false)
        } else {
            rest = &rest[start + 1..];
            continue;
        };
        let Some(end) = after[2..].find(close) else {
            break;
        };
        let inner = after[2..2 + end].trim_matches(|c: char| c == '-' || c.is_whitespace());
        rest = &after[2 + end + 2..];

        // Blank out string literals so their contents are not identifiers.
        let mut cleaned = String::with_capacity(inner.len());
        let mut quote: Option<char> = None;
        for c in inner.chars() {
            match quote {
                Some(q) if c == q => {
                    quote = None;
                    cleaned.push(' ');
                }
                Some(_) => cleaned.push(' '),
                None if c == '"' || c == '\'' || c == '`' => {
                    quote = Some(c);
                    cleaned.push(' ');
                }
                None => cleaned.push(c),
            }
        }

        let mut words = cleaned.split_whitespace();
        let (expr, conditional) = if is_output {
            (cleaned.as_str(), false)
        } else {
            match words.next() {
                Some("if") | Some("elif") => (
                    cleaned.trim_start().split_once(' ').map_or("", |x| x.1),
                    true,
                ),
                Some("for") => {
                    let body = cleaned.trim_start().split_once(' ').map_or("", |x| x.1);
                    let (vars, iter) = body.split_once(" in ").unwrap_or(("", body));
                    for v in vars.split(',') {
                        locals.insert(v.trim().to_string());
                    }
                    (iter, true)
                }
                Some("set") | Some("set_global") => {
                    let body = cleaned.trim_start().split_once(' ').map_or("", |x| x.1);
                    let (var, value) = body.split_once('=').unwrap_or(("", body));
                    locals.insert(var.trim().to_string());
                    (value, false)
                }
                _ => continue,
            }
        };

        let mut segments = expr.split('|');
        let head = segments.next().unwrap_or("");
        let has_default = segments.any(|s| s.trim_start().starts_with("default"));

        let bytes = head.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c.is_ascii_alphabetic() || c == '_' {
                let begin = i;
                while i < bytes.len()
                    && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] == b'_')
                {
                    i += 1;
                }
                let ident = &head[begin..i];
                let prev = head[..begin].trim_end().chars().last();
                let next_non_space = head[i..].trim_start();
                let is_attr = prev == Some('.');
                let is_call = next_non_space.starts_with('(');
                let is_kwarg = next_non_space.starts_with('=') && !next_non_space.starts_with("==");
                let reserved = ["page", "site", "t", "body"].contains(&ident)
                    || TEMPLATE_KEYWORDS.contains(&ident)
                    || locals.contains(ident);
                if !(is_attr || is_call || is_kwarg || reserved) {
                    if !order.iter().any(|o| o == ident) {
                        order.push(ident.to_string());
                    }
                    if conditional || has_default {
                        optional.insert(ident.to_string());
                    } else {
                        bare.insert(ident.to_string());
                    }
                }
            } else if c.is_ascii_digit() {
                while i < bytes.len() && (bytes[i] as char).is_ascii_alphanumeric() {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
    }

    order
        .into_iter()
        .map(|name| {
            let required = bare.contains(&name) && !optional.contains(&name);
            (name, required)
        })
        .collect()
}

fn uses_body(template: &str) -> bool {
    shortcode_uses_var(template, "body")
}

fn shortcode_uses_var(template: &str, var: &str) -> bool {
    template.match_indices(var).any(|(i, _)| {
        let before = template[..i].chars().last();
        let after = template[i + var.len()..].chars().next();
        let boundary =
            |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        boundary(before) && boundary(after)
    })
}

fn shortcode_json(name: &str, source: &str, template: &str, is_body: bool) -> serde_json::Value {
    let params = shortcode_params(template);
    let args: Vec<String> = params
        .iter()
        .filter(|(_, required)| *required)
        .map(|(p, _)| format!("{p}=\"…\""))
        .collect();
    let args = args.join(", ");
    let usage = if is_body {
        format!("{{{{% {name}({args}) %}}}}\nmarkdown body\n{{{{% end %}}}}")
    } else {
        format!("{{{{< {name}({args}) >}}}}")
    };
    serde_json::json!({
        "name": name,
        "source": source,
        "syntax": if is_body { "body" } else { "inline" },
        "params": params
            .iter()
            .map(|(p, required)| serde_json::json!({ "name": p, "required": required }))
            .collect::<Vec<_>>(),
        "usage": usage,
    })
}

pub fn call_list_templates(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    Args::new("seite_list_templates", arguments, &[])?;
    let (config, paths) = state.site()?;
    let root = &paths.root;
    let dir = &paths.templates;

    let user = user_template_names(dir);
    let user_set: BTreeSet<&str> = user.iter().map(String::as_str).collect();
    // Same loading as the build, for its warnings (e.g. a template that fails
    // to parse makes the build fall back to the embedded defaults).
    let mut warnings = match load_templates_with_warnings(dir, &config.collections) {
        Ok((_, warnings)) => warnings,
        Err(e) => vec![e.to_string()],
    };

    // Base template and its blocks.
    let (base_source, base_path, base_content) = if user_set.contains("base.html") {
        let path = dir.join("base.html");
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        (
            "user",
            Some(content_index::relative_path(root, &path)),
            content,
        )
    } else {
        (
            "embedded",
            None,
            embedded_default("base.html")
                .unwrap_or_default()
                .to_string(),
        )
    };
    let mut base = serde_json::json!({ "source": base_source, "path": base_path, "blocks": [] });
    match tera::Template::new("base.html", None, &base_content) {
        Ok(tpl) => {
            let blocks: BTreeSet<&String> = tpl.blocks.keys().collect();
            base["blocks"] = serde_json::json!(blocks);
        }
        Err(e) => {
            let msg = format!("base.html does not parse: {e}");
            base["error"] = msg.clone().into();
            warnings.push(msg);
        }
    }

    let user_templates: Vec<serde_json::Value> = user
        .iter()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "path": content_index::relative_path(root, &dir.join(name)),
                "kind": template_kind(config, name),
                "overrides_default": embedded_default(name).is_some(),
            })
        })
        .collect();
    let embedded_defaults: Vec<serde_json::Value> = EMBEDDED_TEMPLATE_NAMES
        .iter()
        .map(|name| serde_json::json!({ "name": name, "overridden": user_set.contains(name) }))
        .collect();

    // Which templates render each kind (as the build selects them).
    let exists = |name: &str| user_set.contains(name) || embedded_default(name).is_some();
    let item_templates: BTreeSet<String> = config
        .collections
        .iter()
        .map(|c| c.default_template.clone())
        .collect();
    let index_templates: BTreeSet<String> = config
        .collections
        .iter()
        .map(|c| {
            let specific = format!("{}-index.html", c.name);
            if exists(&specific) {
                specific
            } else {
                "index.html".to_string()
            }
        })
        .collect();
    let mut context: Vec<serde_json::Value> = KINDS
        .iter()
        .map(|(kind, extra)| {
            let templates: Vec<String> = match *kind {
                "item" => item_templates.iter().cloned().collect(),
                "homepage" => vec!["index.html".into()],
                "collection_index" => index_templates.iter().cloned().collect(),
                "not_found" => vec!["404.html".into()],
                "tags" => vec!["tags.html".into()],
                _ => vec!["tag.html".into()],
            };
            let names: Vec<&str> = COMMON.iter().chain(extra.iter()).copied().collect();
            serde_json::json!({
                "kind": kind,
                "templates": templates,
                "variables": describe(&names, VARIABLES),
            })
        })
        .collect();
    let shortcode_vars: Vec<&str> = SHORTCODE_VARIABLES.iter().map(|(n, _)| *n).collect();
    context.push(serde_json::json!({
        "kind": "shortcode",
        "templates": ["shortcodes/<name>.html"],
        "variables": describe(&shortcode_vars, SHORTCODE_VARIABLES),
    }));

    // Shortcodes: built-ins, then custom ones (which may override built-ins).
    let builtins = crate::shortcodes::builtins::all();
    let custom: Vec<(String, String)> = user
        .iter()
        .filter_map(|n| n.strip_prefix("shortcodes/"))
        .filter(|n| !n.contains('/'))
        .filter_map(|n| {
            let name = n.strip_suffix(".html")?.to_string();
            let content = std::fs::read_to_string(dir.join("shortcodes").join(n)).ok()?;
            Some((name, content))
        })
        .collect();
    let mut shortcodes: Vec<serde_json::Value> = builtins
        .iter()
        .filter(|b| !custom.iter().any(|(n, _)| n == b.name))
        .map(|b| shortcode_json(b.name, "builtin", b.template, b.is_body))
        .collect();
    for (name, content) in &custom {
        let overrides = builtins.iter().any(|b| b.name == name);
        let source = if overrides {
            "custom (overrides builtin)"
        } else {
            "custom"
        };
        shortcodes.push(shortcode_json(name, source, content, uses_body(content)));
    }

    // Data file keys.
    let mut data = serde_json::json!({
        "dir": content_index::relative_path(root, &paths.data_dir),
        "keys": [],
    });
    match crate::data::load_data_dir(&paths.data_dir) {
        Ok(serde_json::Value::Object(map)) => {
            let keys: Vec<serde_json::Value> = map
                .iter()
                .map(|(k, v)| {
                    let mut entry = serde_json::json!({ "key": k, "type": super::type_name(v) });
                    if let Some(obj) = v.as_object() {
                        entry["keys"] = serde_json::json!(obj.keys().take(50).collect::<Vec<_>>());
                    }
                    entry
                })
                .collect();
            data["keys"] = serde_json::json!(keys);
        }
        Ok(_) => {}
        Err(e) => {
            data["error"] = e.to_string().into();
            warnings.push(format!("data files: {e}"));
        }
    }

    Ok(serde_json::json!({
        "template_dir": content_index::relative_path(root, dir),
        "active_theme": crate::themes::active_theme(root, dir),
        "base_template": base,
        "user_templates": user_templates,
        "embedded_defaults": embedded_defaults,
        "context": context,
        "shortcodes": shortcodes,
        "data": data,
        "warnings": warnings,
    }))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    /// Names inserted into Tera contexts by `receiver.insert("name", ...)` in
    /// non-test source, for receivers whose name contains `ctx`.
    fn inserted_names(source: &str) -> BTreeSet<String> {
        let code = source.split("#[cfg(test)]").next().unwrap_or(source);
        let mut names = BTreeSet::new();
        for (idx, _) in code.match_indices(".insert(") {
            let receiver: String = code[..idx]
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            if !receiver.contains("ctx") {
                continue;
            }
            let rest = code[idx + ".insert(".len()..].trim_start();
            if let Some(stripped) = rest.strip_prefix('"') {
                if let Some(end) = stripped.find('"') {
                    names.insert(stripped[..end].to_string());
                }
            }
        }
        names
    }

    #[test]
    fn test_context_variables_cover_build() {
        let build_vars = inserted_names(include_str!("../../build/mod.rs"));
        assert!(build_vars.contains("page") && build_vars.contains("nav"));
        let listed: BTreeSet<&str> = COMMON
            .iter()
            .chain(KINDS.iter().flat_map(|(_, v)| v.iter()))
            .copied()
            .collect();
        for var in &build_vars {
            assert!(
                listed.contains(var.as_str()),
                "build inserts Tera context variable `{var}` — add it to VARIABLES/KINDS in src/mcp/tools/templates.rs"
            );
        }
        for var in &listed {
            assert!(
                VARIABLES.iter().any(|(n, _)| n == var),
                "{var} has no description"
            );
        }
        let shortcode_vars = inserted_names(include_str!("../../shortcodes/mod.rs"));
        for var in &shortcode_vars {
            assert!(
                SHORTCODE_VARIABLES.iter().any(|(n, _)| n == var),
                "shortcode context variable `{var}` is not listed"
            );
        }
    }

    #[test]
    fn test_shortcode_params_for_builtins() {
        let expect: &[(&str, &[(&str, bool)])] = &[
            (
                "youtube",
                &[("id", true), ("start", false), ("title", false)],
            ),
            ("vimeo", &[("id", true), ("title", false)]),
            ("gist", &[("user", true), ("id", true)]),
            ("callout", &[("type", false), ("title", false)]),
            (
                "figure",
                &[
                    ("class", false),
                    ("src", true),
                    ("alt", false),
                    ("width", false),
                    ("height", false),
                    ("caption", false),
                ],
            ),
        ];
        for (name, params) in expect {
            let builtin = crate::shortcodes::builtins::all()
                .into_iter()
                .find(|b| b.name == *name)
                .unwrap();
            let got = shortcode_params(builtin.template);
            let want: Vec<(String, bool)> =
                params.iter().map(|(p, r)| (p.to_string(), *r)).collect();
            assert_eq!(got, want, "{name}");
        }
        let form = crate::shortcodes::builtins::all()
            .into_iter()
            .find(|b| b.name == "contact_form")
            .unwrap();
        let params = shortcode_params(form.template);
        assert!(params.iter().all(|(_, required)| !required), "{params:?}");
        assert!(params.iter().any(|(p, _)| p == "subject"));
    }

    #[test]
    fn test_shortcode_params_locals_and_calls() {
        let tpl = "{% set n = count | default(value=3) %}{% for x in items %}{{ x.name }}{{ now() }}{{ n }}{% endfor %}{{ label }}";
        assert_eq!(
            shortcode_params(tpl),
            vec![
                ("count".to_string(), false),
                ("items".to_string(), false),
                ("label".to_string(), true)
            ]
        );
        assert!(uses_body("<div>{{ body | markdown }}</div>"));
        assert!(!uses_body("{{ page.body }}{{ somebody }}"));
    }

    #[test]
    fn test_list_templates_reports_overrides_blocks_and_shortcodes() {
        let (tmp, mut state) = site("");
        write(
            tmp.path(),
            "templates/base.html",
            "<html>{% block head %}{% endblock %}{% block content %}{% endblock %}</html>",
        );
        write(
            tmp.path(),
            "templates/post.html",
            "{% extends \"base.html\" %}",
        );
        write(tmp.path(), "templates/partials/nav.html", "<nav></nav>");
        write(
            tmp.path(),
            "templates/shortcodes/note.html",
            "<aside class=\"{{ kind | default(value='x') }}\">{{ body }}</aside>",
        );
        write(
            tmp.path(),
            "templates/shortcodes/youtube.html",
            "<a href=\"{{ id }}\">video</a>",
        );
        write(tmp.path(), "data/authors.yaml", "jane:\n  name: Jane\n");
        write(tmp.path(), "data/nav.json", "[1, 2]");

        let out = call_ok(&mut state, "seite_list_templates", serde_json::json!({}));
        assert_eq!(out["template_dir"], "templates");
        assert_eq!(out["active_theme"], "custom");
        assert_eq!(out["base_template"]["source"], "user");
        assert_eq!(
            out["base_template"]["blocks"],
            serde_json::json!(["content", "head"])
        );
        let user = out["user_templates"].as_array().unwrap();
        let find = |name: &str| user.iter().find(|t| t["name"] == name).unwrap().clone();
        assert_eq!(find("post.html")["kind"], "item");
        assert_eq!(find("post.html")["overrides_default"], true);
        assert_eq!(find("partials/nav.html")["kind"], "other");
        assert_eq!(find("partials/nav.html")["overrides_default"], false);
        assert_eq!(find("shortcodes/note.html")["kind"], "shortcode");
        let defaults = out["embedded_defaults"].as_array().unwrap();
        assert!(defaults
            .iter()
            .any(|d| d["name"] == "post.html" && d["overridden"] == true));
        assert!(defaults
            .iter()
            .any(|d| d["name"] == "doc.html" && d["overridden"] == false));

        let item = out["context"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["kind"] == "item")
            .unwrap();
        assert_eq!(
            item["templates"],
            serde_json::json!(["doc.html", "post.html"])
        );
        let vars: Vec<&str> = item["variables"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["name"].as_str().unwrap())
            .collect();
        assert!(vars.contains(&"page") && vars.contains(&"prev_post"));

        let shortcodes = out["shortcodes"].as_array().unwrap();
        let sc = |name: &str| {
            shortcodes
                .iter()
                .find(|s| s["name"] == name)
                .unwrap()
                .clone()
        };
        assert_eq!(sc("note")["source"], "custom");
        assert_eq!(sc("note")["syntax"], "body");
        assert_eq!(sc("note")["params"][0]["name"], "kind");
        assert_eq!(sc("youtube")["source"], "custom (overrides builtin)");
        assert_eq!(sc("callout")["source"], "builtin");
        assert_eq!(sc("callout")["syntax"], "body");
        assert_eq!(sc("gist")["usage"], "{{< gist(user=\"…\", id=\"…\") >}}");

        let keys = out["data"]["keys"].as_array().unwrap();
        let authors = keys.iter().find(|k| k["key"] == "authors").unwrap();
        assert_eq!(authors["type"], "object");
        assert_eq!(authors["keys"], serde_json::json!(["jane"]));
        let nav = keys.iter().find(|k| k["key"] == "nav").unwrap();
        assert_eq!(nav["type"], "array");
    }

    #[test]
    fn test_list_templates_defaults_and_bad_args() {
        let (_tmp, mut state) = site("");
        let out = call_ok(&mut state, "seite_list_templates", serde_json::json!({}));
        assert_eq!(out["base_template"]["source"], "embedded");
        assert!(out["base_template"]["path"].is_null());
        let blocks = out["base_template"]["blocks"].as_array().unwrap();
        assert!(blocks.iter().any(|b| b == "content"), "{blocks:?}");
        assert_eq!(out["user_templates"], serde_json::json!([]));
        assert_eq!(out["active_theme"], "default");
        let err = call_err(
            &mut state,
            "seite_list_templates",
            serde_json::json!({ "x": 1 }),
        );
        assert!(err.contains("unknown argument"), "{err}");
    }
}
