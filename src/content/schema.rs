//! Typed frontmatter schemas: `[collections.fields.<name>]` in `seite.toml`.
//!
//! # Design note
//!
//! - **Config shape.** Each collection may declare fields:
//!
//!   ```toml
//!   [[collections]]
//!   name = "posts"
//!   # ...
//!   strict_fields = true          # optional: warn about undeclared keys
//!
//!   [collections.fields.description]
//!   type = "string"
//!   required = true
//!   max = 160
//!   description = "Summary for search results and social cards"
//!
//!   [collections.fields.author]
//!   type = "string"
//!   default = "Jane Doe"
//!   ```
//!
//!   `type` is one of string, text, number, integer, bool, date, datetime,
//!   url, image, enum, list, object. `enum` (or its alias `values`) lists the
//!   allowed values of an `enum` field, or of each item of a `list` with
//!   `items = "enum"`. `min`/`max` bound numbers, or the length of strings
//!   (characters) and lists (items). `pattern` is a regular expression a
//!   string (or each string item of a list) must contain (unanchored, like
//!   JSON Schema: write `^…$` to match the whole value).
//!
//! - **Where a field lives.** A schema field named like a top-level key — a
//!   built-in (title, date, description, image, tags, draft, slug, template,
//!   robots, weight, updated, aliases, expires, series, series_order, plus
//!   anything `Frontmatter`'s `Deserialize` impl names) or a site-defined
//!   top-level key (`[[taxonomies]]` names, see [`top_level_keys`]) — is
//!   validated at the top level. Every other (custom) field may be written
//!   either at the top level (`author: Ada`) or under `extra:`
//!   (`extra:\n  author: Ada`) and is validated wherever it appears; both at
//!   once is an error naming both lines. Templates read either spelling as
//!   `page.extra.<name>` (see [`page_extra`]), and published `.md` copies keep
//!   declared custom fields where they were written.
//!
//! - **Defaults.** `default` is applied when content is *created*
//!   (`seite new`, MCP `seite_create_content`), never injected at build
//!   time, so what the file says is what the site renders. A required field
//!   without a default is written as an empty value of its type (`''`, `[]`,
//!   `false`, or `null`), so the file exists and `seite check` points at the
//!   exact line to fill in; `seite new` also prints those fields.
//!
//! - **When it runs.** `seite build` and `seite check` validate every
//!   non-draft file (drafts too with `--drafts`) — including future-dated
//!   (scheduled) content, which goes live on its own later. `seite check
//!   --file` and the after-edit hook validate the named files, drafts
//!   included. Violations are `frontmatter-schema` errors; with
//!   `strict_fields = true`, undeclared keys are `frontmatter-schema-unknown`
//!   warnings. Built-in keys are always allowed.

use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value as Yaml};

use crate::config::{CollectionConfig, SiteConfig};
use crate::diagnostics::Diagnostic;

/// The type of a schema field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    String,
    Text,
    Number,
    Integer,
    Bool,
    Date,
    Datetime,
    Url,
    Image,
    Enum,
    List,
    Object,
}

impl FieldType {
    pub fn as_str(self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Text => "text",
            FieldType::Number => "number",
            FieldType::Integer => "integer",
            FieldType::Bool => "bool",
            FieldType::Date => "date",
            FieldType::Datetime => "datetime",
            FieldType::Url => "url",
            FieldType::Image => "image",
            FieldType::Enum => "enum",
            FieldType::List => "list",
            FieldType::Object => "object",
        }
    }

    fn is_stringy(self) -> bool {
        matches!(
            self,
            FieldType::String
                | FieldType::Text
                | FieldType::Url
                | FieldType::Image
                | FieldType::Date
                | FieldType::Datetime
        )
    }

    fn has_bounds(self) -> bool {
        matches!(
            self,
            FieldType::Number
                | FieldType::Integer
                | FieldType::String
                | FieldType::Text
                | FieldType::Url
                | FieldType::Image
                | FieldType::List
        )
    }
}

/// One `[collections.fields.<name>]` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldSchema {
    #[serde(rename = "type")]
    pub field_type: FieldType,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
    /// Allowed values (`type = "enum"`, or list items with `items = "enum"`).
    #[serde(default, rename = "enum", skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<serde_json::Value>,
    /// Alias of `enum`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<serde_json::Value>,
    /// Type of each element of a `list`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<FieldType>,
    /// Lower bound: number value, or string/list length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<serde_json::Number>,
    /// Upper bound: number value, or string/list length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<serde_json::Number>,
    /// Regular expression a string value must contain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Value written by `seite new` / MCP create when the field is not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// What the field is for; shown as the hint of its diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl FieldSchema {
    fn new(field_type: FieldType) -> Self {
        Self {
            field_type,
            required: false,
            enum_values: Vec::new(),
            values: Vec::new(),
            items: None,
            min: None,
            max: None,
            pattern: None,
            default: None,
            description: None,
        }
    }

    /// `enum` and `values` together.
    pub fn allowed_values(&self) -> impl Iterator<Item = &serde_json::Value> {
        self.enum_values.iter().chain(self.values.iter())
    }

    fn min_f64(&self) -> Option<f64> {
        self.min.as_ref().and_then(serde_json::Number::as_f64)
    }

    fn max_f64(&self) -> Option<f64> {
        self.max.as_ref().and_then(serde_json::Number::as_f64)
    }

    /// Plain-words summary of what a valid value looks like.
    pub fn expectation(&self) -> String {
        let mut s = match (self.field_type, self.items) {
            (FieldType::Enum, _) => format!("one of {}", list_values(self.allowed_values())),
            (FieldType::List, Some(FieldType::Enum)) => {
                format!("a list of {}", list_values(self.allowed_values()))
            }
            (FieldType::List, Some(items)) => format!("a list of {}", type_words(items)),
            (t, _) => type_words(t).to_string(),
        };
        let unit = match self.field_type {
            FieldType::Number | FieldType::Integer => "",
            FieldType::List => " items",
            _ => " characters",
        };
        match (self.min_f64(), self.max_f64()) {
            (Some(min), Some(max)) => s.push_str(&format!(
                ", {}{unit} to {}{unit}",
                fmt_num(min),
                fmt_num(max)
            )),
            (Some(min), None) => s.push_str(&format!(", at least {}{unit}", fmt_num(min))),
            (None, Some(max)) => s.push_str(&format!(", at most {}{unit}", fmt_num(max))),
            (None, None) => {}
        }
        if let Some(p) = &self.pattern {
            s.push_str(&format!(", matching `{p}`"));
        }
        s
    }
}

fn type_words(t: FieldType) -> &'static str {
    match t {
        FieldType::String => "a string",
        FieldType::Text => "a string (text)",
        FieldType::Number => "a number",
        FieldType::Integer => "an integer",
        FieldType::Bool => "true or false",
        FieldType::Date => "a date (YYYY-MM-DD)",
        FieldType::Datetime => "a date and time (YYYY-MM-DDTHH:MM:SS, optional offset)",
        FieldType::Url => "a URL (https://… or a site path starting with /)",
        FieldType::Image => "an image path or URL",
        FieldType::Enum => "one of the allowed values",
        FieldType::List => "a list",
        FieldType::Object => "a mapping (key: value pairs)",
    }
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn list_values<'a>(values: impl Iterator<Item = &'a serde_json::Value>) -> String {
    values
        .map(|v| match v {
            serde_json::Value::String(s) => format!("`{s}`"),
            other => format!("`{other}`"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Built-in top-level frontmatter keys, listed explicitly because a
/// `Frontmatter` with a flattened catch-all map hides its field list from
/// the `Deserialize` probe (see [`builtin_keys`]).
const BUILTIN_KEYS: &[&str] = &[
    "title",
    "date",
    "updated",
    "description",
    "image",
    "slug",
    "tags",
    "draft",
    "template",
    "robots",
    "weight",
    "aliases",
    "extra",
    "expires",
    "series",
    "series_order",
];

/// Frontmatter keys seite itself understands (top-level): [`BUILTIN_KEYS`]
/// plus whatever `Frontmatter`'s `Deserialize` impl names, so new built-ins
/// are picked up.
pub fn builtin_keys() -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = BUILTIN_KEYS.to_vec();
    for k in crate::config::unknown_keys::fields_of::<super::Frontmatter>() {
        if !keys.contains(k) {
            keys.push(k);
        }
    }
    keys
}

pub fn is_builtin(name: &str) -> bool {
    BUILTIN_KEYS.contains(&name) || builtin_keys().contains(&name)
}

/// Keys that live at the top level of a site's frontmatter: the built-ins
/// plus site-defined top-level keys (`[[taxonomies]]` names such as
/// `categories`). Schema fields with these names are validated at the top
/// level; every other (custom) field at the top level or under `extra:`.
pub fn top_level_keys(config: &SiteConfig) -> Vec<String> {
    let mut keys: Vec<String> = builtin_keys().into_iter().map(String::from).collect();
    for taxonomy in &config.taxonomies {
        if !keys.contains(&taxonomy.name) {
            keys.push(taxonomy.name.clone());
        }
    }
    keys
}

/// The custom fields of `collection`'s schema: declared names that are not
/// top-level keys of the site (see [`top_level_keys`]). They may be written
/// at the top level or under `extra:`.
pub fn custom_field_names(config: &SiteConfig, collection: &CollectionConfig) -> Vec<String> {
    if collection.fields.is_empty() {
        return Vec::new();
    }
    let top = top_level_keys(config);
    collection
        .fields
        .keys()
        .filter(|name| !top.contains(name))
        .cloned()
        .collect()
}

/// `page.extra` for templates: the page's `extra:` mapping plus every custom
/// schema field (`custom`, from [`custom_field_names`]) written at the top
/// level, so `page.extra.<name>` works for either spelling. `extra:` wins
/// when a field is in both places (a `frontmatter-schema` error anyway).
pub fn page_extra(
    fm: &super::Frontmatter,
    custom: &[String],
) -> indexmap::IndexMap<String, serde_yaml_ng::Value> {
    let mut extra = fm.extra.clone();
    for name in custom {
        if let Some(value) = fm.other.get(name) {
            extra.entry(name.clone()).or_insert_with(|| value.clone());
        }
    }
    extra
}

/// Whether `name` is a top-level key of this site (see [`top_level_keys`]).
pub fn is_top_level(config: &SiteConfig, name: &str) -> bool {
    top_level_keys(config).iter().any(|k| k == name)
}

/// The schema written into new sites' `seite.toml` by `seite init` and
/// `seite collection add` (deliberately nothing `required` and no length
/// bounds, so no existing
/// habit breaks).
pub fn preset_fields(preset: &str) -> IndexMap<String, FieldSchema> {
    let description = || FieldSchema {
        description: Some(
            "Summary for search results and social cards (aim for 50–160 characters)".into(),
        ),
        ..FieldSchema::new(FieldType::String)
    };
    let image = || FieldSchema {
        description: Some("Social preview image (og:image): a path under static/ or a URL".into()),
        ..FieldSchema::new(FieldType::Image)
    };
    let tags = || FieldSchema {
        items: Some(FieldType::String),
        description: Some("Topic tags; each tag gets a page under /tags/".into()),
        ..FieldSchema::new(FieldType::List)
    };
    let weight = || FieldSchema {
        description: Some("Sort order within the collection (lower comes first)".into()),
        ..FieldSchema::new(FieldType::Integer)
    };
    let mut fields = IndexMap::new();
    fields.insert("description".to_string(), description());
    match preset {
        "posts" => {
            fields.insert("image".to_string(), image());
            fields.insert("tags".to_string(), tags());
        }
        "pages" => {
            fields.insert("image".to_string(), image());
        }
        "changelog" => {
            fields.insert("tags".to_string(), tags());
        }
        "docs" | "roadmap" | "trust" => {
            fields.insert("weight".to_string(), weight());
        }
        _ => return IndexMap::new(),
    }
    fields
}

/// Fixed types of built-in keys: a schema may only constrain them with a
/// compatible type.
fn builtin_types(name: &str) -> Option<&'static [FieldType]> {
    use FieldType::*;
    Some(match name {
        "title" | "description" => &[String, Text],
        "slug" | "template" | "robots" => &[String, Text, Enum],
        "image" => &[Image, String, Url],
        "date" | "updated" | "expires" => &[Date],
        "tags" | "aliases" => &[List],
        "draft" => &[Bool],
        "weight" => &[Integer, Number],
        "extra" => &[Object],
        _ => return None,
    })
}

/// Check `[collections.fields.*]` for mistakes the TOML types can't catch.
/// On error: (message, lookup key for [`config_line`]).
pub fn validate_config(config: &SiteConfig) -> Result<(), (String, String)> {
    for collection in &config.collections {
        for (name, spec) in &collection.fields {
            let key = format!("fields.{name}");
            let fail = |msg: String| {
                Err((
                    format!("[collections.fields.{name}] ({}): {msg}", collection.name),
                    key.clone(),
                ))
            };
            let has_values = spec.allowed_values().next().is_some();
            let wants_values = spec.field_type == FieldType::Enum
                || (spec.field_type == FieldType::List && spec.items == Some(FieldType::Enum));
            if wants_values && !has_values {
                return fail(
                    "an enum needs its allowed values: add `enum = [\"a\", \"b\"]`".into(),
                );
            }
            if has_values && !wants_values {
                return fail(
                    "`enum`/`values` only apply to type = \"enum\" (or a list with items = \"enum\")"
                        .into(),
                );
            }
            if let Some(items) = spec.items {
                if spec.field_type != FieldType::List {
                    return fail("`items` only applies to type = \"list\"".into());
                }
                if matches!(items, FieldType::List | FieldType::Object) {
                    return fail(format!(
                        "items = \"{}\" is not supported (list items must be scalar types)",
                        items.as_str()
                    ));
                }
            }
            if (spec.min.is_some() || spec.max.is_some()) && !spec.field_type.has_bounds() {
                return fail(format!(
                    "`min`/`max` do not apply to type = \"{}\" (only number, integer, string, text, url, image, list)",
                    spec.field_type.as_str()
                ));
            }
            if let (Some(min), Some(max)) = (spec.min_f64(), spec.max_f64()) {
                if min > max {
                    return fail(format!(
                        "min ({}) is greater than max ({})",
                        fmt_num(min),
                        fmt_num(max)
                    ));
                }
            }
            if let Some(pattern) = &spec.pattern {
                let applies = spec.field_type.is_stringy()
                    || (spec.field_type == FieldType::List
                        && spec.items.is_none_or(FieldType::is_stringy));
                if !applies {
                    return fail(format!(
                        "`pattern` does not apply to type = \"{}\"",
                        spec.field_type.as_str()
                    ));
                }
                if let Err(e) = regex::Regex::new(pattern) {
                    return fail(format!("invalid `pattern`: {e}"));
                }
            }
            if let Some(allowed) = builtin_types(name) {
                if !allowed.contains(&spec.field_type) {
                    return fail(format!(
                        "`{name}` is a built-in frontmatter key of type {}; type = \"{}\" cannot apply",
                        allowed
                            .iter()
                            .map(|t| format!("\"{}\"", t.as_str()))
                            .collect::<Vec<_>>()
                            .join(" or "),
                        spec.field_type.as_str()
                    ));
                }
            }
            if let Some(default) = &spec.default {
                let yaml = serde_yaml_ng::to_value(default).unwrap_or(Yaml::Null);
                let compiled = Schema::for_site(config, collection);
                if let Some(problem) = compiled.check_value(name, spec, &yaml) {
                    return fail(format!("`default` is invalid: {problem}"));
                }
            }
        }
    }
    Ok(())
}

/// 1-based line of `key` (`timezone`, or `fields.<name>`) in `seite.toml`.
pub fn config_line(contents: &str, key: &str) -> Option<usize> {
    let find =
        |pred: &dyn Fn(&str) -> bool| contents.lines().position(|l| pred(l.trim())).map(|i| i + 1);
    match key.strip_prefix("fields.") {
        Some(name) => find(&|l: &str| {
            l.starts_with('[') && l.trim_end_matches(']').ends_with(&format!("fields.{name}"))
        })
        .or_else(|| find(&|l: &str| l.starts_with(&format!("{name} =")) && l.contains('{'))),
        None => find(&|l: &str| {
            l.strip_prefix(key)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        }),
    }
}

/// What kind of schema problem a diagnostic reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    /// A required field is absent or empty.
    Missing,
    /// The value has the wrong type, is out of bounds, not allowed, …
    Invalid,
    /// A custom field written both at the top level and under `extra:`.
    Duplicate,
    /// Not in the schema (`strict_fields`).
    Unknown,
}

/// One schema problem: the field it concerns plus the ready-made diagnostic.
#[derive(Debug, Clone)]
pub struct Problem {
    pub field: String,
    pub kind: ProblemKind,
    pub diagnostic: Diagnostic,
}

/// A collection's schema with its regular expressions compiled.
pub struct Schema<'a> {
    collection: &'a CollectionConfig,
    patterns: HashMap<&'a str, regex::Regex>,
    /// Keys validated at the top level (see [`top_level_keys`]).
    top_level: Vec<String>,
}

impl<'a> Schema<'a> {
    /// The collection's schema with the site's top-level keys.
    pub fn for_site(config: &SiteConfig, collection: &'a CollectionConfig) -> Self {
        Self {
            top_level: top_level_keys(config),
            ..Self::compile(collection)
        }
    }

    /// Whether `name` is a top-level key (else a custom field, written at the
    /// top level or under `extra:`).
    pub fn is_top(&self, name: &str) -> bool {
        self.top_level.iter().any(|k| k == name)
    }

    /// The collection's schema with only the built-in top-level keys.
    pub fn compile(collection: &'a CollectionConfig) -> Self {
        let patterns = collection
            .fields
            .iter()
            .filter_map(|(name, spec)| {
                let re = regex::Regex::new(spec.pattern.as_deref()?).ok()?;
                Some((name.as_str(), re))
            })
            .collect();
        Self {
            collection,
            patterns,
            top_level: builtin_keys().into_iter().map(String::from).collect(),
        }
    }

    /// True when there is nothing to validate.
    pub fn is_empty(&self) -> bool {
        self.collection.fields.is_empty() && !self.collection.strict_fields
    }

    /// Validate a parsed content file: [`Self::check_text`] on its
    /// frontmatter block. Drafts are skipped unless `include_drafts`.
    pub fn validate(
        &self,
        parsed: &super::ParsedContent,
        path: &Path,
        include_drafts: bool,
    ) -> Vec<Diagnostic> {
        if self.is_empty() || (parsed.frontmatter.draft && !include_drafts) {
            return Vec::new();
        }
        self.check_text(&parsed.frontmatter_text, parsed.frontmatter_line, path)
            .into_iter()
            .map(|p| p.diagnostic)
            .collect()
    }

    /// Validate frontmatter YAML text (without the `---` lines) that starts
    /// on 1-based file line `first_line` of `path`.
    pub fn check_text(&self, yaml: &str, first_line: usize, path: &Path) -> Vec<Problem> {
        if self.is_empty() {
            return Vec::new();
        }
        let map = match serde_yaml_ng::from_str::<Yaml>(yaml) {
            Ok(Yaml::Mapping(m)) => m,
            Ok(Yaml::Null) => Mapping::new(),
            // Not a mapping / unparseable: `frontmatter-parse` reports it.
            _ => return Vec::new(),
        };
        let lines = KeyLines::new(yaml, first_line);
        self.check_mapping(&map, path, &lines)
    }

    fn check_mapping(&self, map: &Mapping, path: &Path, lines: &KeyLines) -> Vec<Problem> {
        let mut out = Vec::new();
        let empty = Mapping::new();
        let extra = match map.get("extra") {
            Some(Yaml::Mapping(m)) => m,
            _ => &empty,
        };
        let coll = &self.collection.name;
        for (name, spec) in &self.collection.fields {
            let builtin = self.is_top(name);
            let hint = |extra_hint: &str| {
                let mut h = match &spec.description {
                    Some(d) => format!("{d} — expected {}", spec.expectation()),
                    None => format!("expected {}", spec.expectation()),
                };
                if !extra_hint.is_empty() {
                    h.push_str("; ");
                    h.push_str(extra_hint);
                }
                h
            };
            let mut push = |kind: ProblemKind, message: String, line: usize, extra_hint: &str| {
                out.push(Problem {
                    field: name.clone(),
                    kind,
                    diagnostic: Diagnostic::error("frontmatter-schema", message)
                        .with_file(path)
                        .with_line(line)
                        .with_hint(hint(extra_hint)),
                });
            };
            // A custom field may be written at the top level or under
            // `extra:`, not both.
            let at_top = !builtin && map.get(name.as_str()).is_some_and(|v| !v.is_null());
            if at_top && extra.get(name.as_str()).is_some_and(|v| !v.is_null()) {
                let top_line = lines.top(name).unwrap_or(lines.start());
                let extra_line = lines.extra(name).unwrap_or(top_line);
                push(
                    ProblemKind::Duplicate,
                    format!(
                        "custom field `{name}` is set twice: at the top level (line {top_line}) and under `extra:` (line {extra_line})"
                    ),
                    top_line,
                    &format!(
                        "keep one of them; templates read either as `page.extra.{name}`"
                    ),
                );
                continue;
            }
            let shown = if builtin || at_top {
                format!("`{name}`")
            } else {
                format!("`extra.{name}`")
            };
            let value = if builtin || at_top {
                map.get(name.as_str())
            } else {
                extra.get(name.as_str())
            };
            let line = if builtin || at_top {
                lines.top(name)
            } else {
                lines.extra(name)
            };
            if value.is_none_or(is_empty_value) {
                let derived_date = name == "date"
                    && self.collection.has_date
                    && path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .and_then(|s| s.get(..10))
                        .is_some_and(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok());
                if spec.required && !derived_date {
                    let shown = format!("`{name}`");
                    let where_ = if builtin {
                        ""
                    } else {
                        " (at the top level or under `extra:`)"
                    };
                    push(
                        ProblemKind::Missing,
                        format!("missing required field {shown} in this {coll} file"),
                        line.or_else(|| (!builtin).then(|| lines.top("extra")).flatten())
                            .unwrap_or(lines.start()),
                        &format!("add `{name}: …`{where_}"),
                    );
                }
                continue;
            }
            let Some(value) = value else { continue };
            if let Some(problem) = self.check_value(name, spec, value) {
                push(
                    ProblemKind::Invalid,
                    format!("field {shown} {problem}"),
                    line.unwrap_or(lines.start()),
                    "",
                );
            }
        }
        if self.collection.strict_fields {
            for key in map.keys().filter_map(Yaml::as_str) {
                if self.is_top(key) || self.collection.fields.contains_key(key) {
                    continue;
                }
                out.push(unknown(
                    key,
                    format!(
                        "unknown frontmatter key `{key}`: not a built-in key and not in the {coll} schema"
                    ),
                    lines.top(key).unwrap_or(lines.start()),
                    path,
                    self.collection,
                ));
            }
            for key in extra.keys().filter_map(Yaml::as_str) {
                if self.collection.fields.contains_key(key) && !self.is_top(key) {
                    continue;
                }
                out.push(unknown(
                    key,
                    format!("unknown field `extra.{key}`: not in the {coll} schema"),
                    lines.extra(key).unwrap_or(lines.start()),
                    path,
                    self.collection,
                ));
            }
        }
        out
    }

    /// Why `value` does not satisfy `spec` (phrased to follow "field `x` "),
    /// or `None` when it does.
    pub fn check_value(&self, name: &str, spec: &FieldSchema, value: &Yaml) -> Option<String> {
        let t = spec.field_type;
        let got = describe_value(value);
        let wrong = || {
            Some(format!(
                "has the wrong type: expected {}, got {got}",
                type_words(t)
            ))
        };
        match t {
            FieldType::List => {
                let Yaml::Sequence(items) = value else {
                    return wrong();
                };
                if let Some(problem) = self.check_len(spec, items.len(), "items") {
                    return Some(problem);
                }
                for (i, item) in items.iter().enumerate() {
                    let item_problem = match spec.items {
                        Some(FieldType::Enum) => check_enum(spec, item),
                        // Like the frontmatter parser (`tags: [2024, rust]`),
                        // numbers and bools are strings in a list of strings.
                        Some(FieldType::String | FieldType::Text)
                            if matches!(item, Yaml::Number(_) | Yaml::Bool(_)) =>
                        {
                            let text = match item {
                                Yaml::Number(n) => n.to_string(),
                                Yaml::Bool(b) => b.to_string(),
                                _ => String::new(),
                            };
                            self.check_pattern(name, &text)
                        }
                        Some(item_type) => check_scalar(item_type, item)
                            .or_else(|| item.as_str().and_then(|s| self.check_pattern(name, s))),
                        None => item.as_str().and_then(|s| self.check_pattern(name, s)),
                    };
                    if let Some(p) = item_problem {
                        return Some(format!("item {} {p}", i + 1));
                    }
                }
                None
            }
            FieldType::Object => match value {
                Yaml::Mapping(_) => None,
                _ => wrong(),
            },
            FieldType::Enum => check_enum(spec, value),
            FieldType::Number | FieldType::Integer => {
                if let Some(p) = check_scalar(t, value) {
                    return Some(p);
                }
                let n = value.as_f64()?;
                match (spec.min_f64(), spec.max_f64()) {
                    (Some(min), _) if n < min => Some(format!(
                        "is too small: {got}, at least {} allowed",
                        fmt_num(min)
                    )),
                    (_, Some(max)) if n > max => Some(format!(
                        "is too large: {got}, at most {} allowed",
                        fmt_num(max)
                    )),
                    _ => None,
                }
            }
            _ => {
                if let Some(p) = check_scalar(t, value) {
                    return Some(p);
                }
                let s = value.as_str()?;
                self.check_len(spec, s.chars().count(), "characters")
                    .or_else(|| self.check_pattern(name, s))
            }
        }
    }

    fn check_len(&self, spec: &FieldSchema, len: usize, unit: &str) -> Option<String> {
        let n = len as f64;
        match (spec.min_f64(), spec.max_f64()) {
            (Some(min), _) if n < min => Some(format!(
                "is too short: {len} {unit}, at least {} allowed",
                fmt_num(min)
            )),
            (_, Some(max)) if n > max => Some(format!(
                "is too long: {len} {unit}, at most {} allowed",
                fmt_num(max)
            )),
            _ => None,
        }
    }

    fn check_pattern(&self, name: &str, s: &str) -> Option<String> {
        let re = self.patterns.get(name)?;
        (!re.is_match(s))
            .then(|| format!("does not match the pattern `{}`: got \"{s}\"", re.as_str()))
    }
}

fn unknown(
    key: &str,
    message: String,
    line: usize,
    path: &Path,
    collection: &CollectionConfig,
) -> Problem {
    let declared: Vec<&str> = collection.fields.keys().map(String::as_str).collect();
    let hint = match crate::diagnostics::did_you_mean(key, declared.iter().copied()) {
        Some(h) => h,
        None => format!(
            "declare it with `[collections.fields.{key}]` in seite.toml, or remove it (strict_fields = true is set for {})",
            collection.name
        ),
    };
    Problem {
        field: key.to_string(),
        kind: ProblemKind::Unknown,
        diagnostic: Diagnostic::warning("frontmatter-schema-unknown", message)
            .with_file(path)
            .with_line(line)
            .with_hint(hint),
    }
}

fn is_empty_value(v: &Yaml) -> bool {
    match v {
        Yaml::Null => true,
        Yaml::String(s) => s.trim().is_empty(),
        Yaml::Sequence(s) => s.is_empty(),
        Yaml::Mapping(m) => m.is_empty(),
        _ => false,
    }
}

fn describe_value(v: &Yaml) -> String {
    match v {
        Yaml::Null => "nothing (null)".into(),
        Yaml::Bool(b) => format!("{b} (bool)"),
        Yaml::Number(n) => format!("{n} (number)"),
        Yaml::String(s) => {
            let short: String = s.chars().take(60).collect();
            let ellipsis = if s.chars().count() > 60 { "…" } else { "" };
            format!("\"{short}{ellipsis}\" (string)")
        }
        Yaml::Sequence(_) => "a list".into(),
        Yaml::Mapping(_) => "a mapping".into(),
        Yaml::Tagged(_) => "a tagged value".into(),
    }
}

fn check_enum(spec: &FieldSchema, value: &Yaml) -> Option<String> {
    let as_json = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
    if spec.allowed_values().any(|v| *v == as_json) {
        return None;
    }
    // `1` in YAML vs "1" in seite.toml (and back) should still match.
    let text = match &as_json {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if spec.allowed_values().any(|v| match v {
        serde_json::Value::String(s) => *s == text,
        other => {
            let other_text = other.to_string();
            other_text == text
        }
    }) {
        return None;
    }
    Some(format!(
        "must be one of {}, got {}",
        list_values(spec.allowed_values()),
        describe_value(value)
    ))
}

/// Type check of a scalar value (no bounds/pattern).
fn check_scalar(t: FieldType, value: &Yaml) -> Option<String> {
    let got = describe_value(value);
    let wrong = || {
        Some(format!(
            "has the wrong type: expected {}, got {got}",
            type_words(t)
        ))
    };
    match t {
        FieldType::String | FieldType::Text | FieldType::Image => match value {
            Yaml::String(_) => None,
            _ => wrong(),
        },
        FieldType::Url => match value.as_str() {
            Some(s) if is_url(s) => None,
            Some(_) => Some(format!(
                "is not a URL: expected an absolute http(s) URL or a site path starting with /, got {got}"
            )),
            None => wrong(),
        },
        FieldType::Number => match value {
            Yaml::Number(_) => None,
            _ => wrong(),
        },
        FieldType::Integer => match value {
            Yaml::Number(n) if n.is_i64() || n.is_u64() => None,
            _ => wrong(),
        },
        FieldType::Bool => match value {
            Yaml::Bool(_) => None,
            _ => wrong(),
        },
        FieldType::Date => match value.as_str() {
            Some(s) if chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").is_ok() => None,
            _ => Some(format!("is not a valid date: expected YYYY-MM-DD, got {got}")),
        },
        FieldType::Datetime => match value.as_str() {
            Some(s) if is_datetime(s.trim()) => None,
            _ => Some(format!(
                "is not a valid date and time: expected YYYY-MM-DDTHH:MM:SS (optionally with Z or +HH:MM), got {got}"
            )),
        },
        FieldType::Enum | FieldType::List | FieldType::Object => None,
    }
}

fn is_url(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    match rest {
        Some(host) => !host.is_empty() && !host.starts_with('/'),
        None => lower.starts_with('/') || lower.starts_with("mailto:"),
    }
}

fn is_datetime(s: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(s).is_ok()
        || [
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%dT%H:%M",
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
        ]
        .iter()
        .any(|f| chrono::NaiveDateTime::parse_from_str(s, f).is_ok())
}

/// Finds the file line of a top-level key or an `extra:` sub-key in a
/// frontmatter block (block-style YAML; flow mappings fall back to the
/// enclosing key's line).
struct KeyLines<'a> {
    lines: Vec<&'a str>,
    first_line: usize,
}

impl<'a> KeyLines<'a> {
    fn new(yaml: &'a str, first_line: usize) -> Self {
        Self {
            lines: yaml.lines().collect(),
            first_line,
        }
    }

    /// The opening `---` line (or line 1).
    fn start(&self) -> usize {
        self.first_line.saturating_sub(1).max(1)
    }

    fn top(&self, key: &str) -> Option<usize> {
        self.lines
            .iter()
            .position(|l| !l.starts_with([' ', '\t']) && key_of(l) == Some(key))
            .map(|i| self.first_line + i)
    }

    fn extra(&self, key: &str) -> Option<usize> {
        let start = self
            .lines
            .iter()
            .position(|l| !l.starts_with([' ', '\t']) && key_of(l) == Some("extra"))?;
        let mut indent: Option<usize> = None;
        for (i, line) in self.lines.iter().enumerate().skip(start + 1) {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let this = line.len() - trimmed.len();
            if this == 0 {
                break;
            }
            let level = *indent.get_or_insert(this);
            if this == level && key_of(trimmed) == Some(key) {
                return Some(self.first_line + i);
            }
        }
        None
    }
}

/// The key of a `key: value` line (plain or quoted key).
fn key_of(line: &str) -> Option<&str> {
    let line = line.trim_start();
    for quote in ['"', '\''] {
        if let Some(rest) = line.strip_prefix(quote) {
            let end = rest.find(quote)?;
            return rest[end + 1..]
                .trim_start()
                .starts_with(':')
                .then_some(&rest[..end]);
        }
    }
    let colon = line.find(':')?;
    let after = &line[colon + 1..];
    if !(after.is_empty() || after.starts_with([' ', '\t'])) {
        return None;
    }
    let key = line[..colon].trim_end();
    (!key.is_empty() && !key.starts_with(['-', '#'])).then_some(key)
}

/// Split diagnostics into (errors, warnings).
pub fn split(diagnostics: Vec<Diagnostic>) -> (Vec<Diagnostic>, Vec<Diagnostic>) {
    diagnostics.into_iter().partition(Diagnostic::is_error)
}

/// The collection a content file belongs to (longest matching directory).
pub fn collection_for<'c>(
    config: &'c SiteConfig,
    content_dir: &Path,
    file: &Path,
) -> Option<&'c CollectionConfig> {
    config
        .collections
        .iter()
        .filter(|c| file.starts_with(content_dir.join(&c.directory)))
        .max_by_key(|c| c.directory.len())
}

/// What a new file's frontmatter needs from the schema: defaults for absent
/// fields, typed empty placeholders for required fields without a default.
/// Modifies the serialized frontmatter mapping `map` in place and returns
/// the names of required fields that got a placeholder.
pub fn apply_defaults(
    config: &SiteConfig,
    collection: &CollectionConfig,
    map: &mut Mapping,
) -> Vec<String> {
    let top_level = top_level_keys(config);
    let mut placeholders = Vec::new();
    let mut extra = match map.get("extra") {
        Some(Yaml::Mapping(m)) => m.clone(),
        _ => Mapping::new(),
    };
    for (name, spec) in &collection.fields {
        let builtin = top_level.iter().any(|k| k == name);
        let target = if builtin { &mut *map } else { &mut extra };
        if target
            .get(name.as_str())
            .is_some_and(|v| !is_empty_value(v))
        {
            continue;
        }
        if name == "date" && collection.has_date {
            continue; // `seite new` writes today's date for dated collections
        }
        let value = match &spec.default {
            Some(default) => serde_yaml_ng::to_value(default).unwrap_or(Yaml::Null),
            None if spec.required => {
                placeholders.push(name.clone());
                match spec.field_type {
                    FieldType::List => Yaml::Sequence(Vec::new()),
                    FieldType::Bool => Yaml::Bool(false),
                    FieldType::Object => Yaml::Mapping(Mapping::new()),
                    FieldType::String | FieldType::Text | FieldType::Url | FieldType::Image => {
                        Yaml::String(String::new())
                    }
                    _ => Yaml::Null,
                }
            }
            None => continue,
        };
        target.insert(Yaml::String(name.clone()), value);
    }
    if !extra.is_empty() {
        map.insert(Yaml::String("extra".into()), Yaml::Mapping(extra));
    }
    placeholders
}

/// JSON description of a collection's schema for `seite templates`,
/// `seite context` and MCP (`location` says where the key goes).
pub fn describe_collection(
    config: &SiteConfig,
    collection: &CollectionConfig,
) -> serde_json::Value {
    let fields: Vec<serde_json::Value> = collection
        .fields
        .iter()
        .map(|(name, spec)| {
            let mut v = serde_json::to_value(spec).unwrap_or_else(|_| serde_json::json!({}));
            v["name"] = name.clone().into();
            v["location"] = if is_top_level(config, name) {
                "top-level".into()
            } else {
                format!("extra.{name}").into()
            };
            v["expected"] = spec.expectation().into();
            v
        })
        .collect();
    serde_json::json!({
        "collection": collection.name,
        "strict_fields": collection.strict_fields,
        "fields": fields,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection(toml_fields: &str) -> CollectionConfig {
        let src = format!(
            "name = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\ndefault_template = \"post.html\"\n{toml_fields}"
        );
        toml::from_str(&src).unwrap()
    }

    fn check(c: &CollectionConfig, yaml: &str) -> Vec<Problem> {
        Schema::compile(c).check_text(yaml, 2, Path::new("content/posts/a.md"))
    }

    #[test]
    fn test_required_type_enum_pattern_bounds() {
        let c = collection(
            r#"
[fields.description]
type = "string"
required = true
max = 10
description = "Short summary"
[fields.author]
type = "string"
required = true
[fields.status]
type = "enum"
enum = ["draft", "published"]
[fields.rating]
type = "integer"
min = 1
max = 5
[fields.code]
type = "string"
pattern = "^[A-Z]{3}$"
"#,
        );
        let yaml = "title: Hi\ndescription: this is far too long\nstatus: nope\nextra:\n  rating: 9\n  code: abc\n";
        // `status` is a custom field written at the top level: validated there.
        let problems = check(&c, yaml);
        let msgs: Vec<String> = problems.iter().map(|p| p.diagnostic.to_string()).collect();
        assert_eq!(problems.len(), 5, "{msgs:#?}");
        let by = |f: &str| problems.iter().find(|p| p.field == f).unwrap();
        let d = &by("description").diagnostic;
        assert_eq!(d.line, Some(3));
        assert!(
            d.message.contains("too long: 20 characters, at most 10"),
            "{}",
            d.message
        );
        assert!(d.hint.as_deref().unwrap().starts_with("Short summary"));
        let a = by("author");
        assert_eq!(a.kind, ProblemKind::Missing);
        assert_eq!(a.diagnostic.line, Some(5), "points at extra:");
        let s = by("status");
        assert_eq!(s.kind, ProblemKind::Invalid);
        assert_eq!(s.diagnostic.line, Some(4));
        assert!(
            s.diagnostic.message.starts_with("field `status` "),
            "{}",
            s.diagnostic.message
        );
        let r = &by("rating").diagnostic;
        assert_eq!(r.line, Some(6));
        assert!(r.message.contains("too large"), "{}", r.message);
        let code = &by("code").diagnostic;
        assert_eq!(code.line, Some(7));
        assert!(code.message.contains("does not match the pattern"));
    }

    #[test]
    fn test_custom_fields_at_top_level_or_under_extra() {
        let c = collection(
            r#"
[fields.author]
type = "string"
required = true
[fields.rating]
type = "integer"
max = 5
"#,
        );
        // Either spelling satisfies `required` and is validated in place.
        assert!(check(&c, "title: x\nauthor: Ada\n").is_empty());
        assert!(check(&c, "title: x\nextra:\n  author: Ada\n").is_empty());
        let p = check(&c, "title: x\nauthor: Ada\nrating: 9\n");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].kind, ProblemKind::Invalid);
        assert_eq!(p[0].diagnostic.line, Some(4));
        // Both places at once: one error naming both lines.
        let p = check(&c, "title: x\nauthor: Ada\nextra:\n  author: Bob\n");
        assert_eq!(p.len(), 1, "{p:#?}");
        assert_eq!(p[0].kind, ProblemKind::Duplicate);
        assert_eq!(p[0].diagnostic.line, Some(3));
        assert!(
            p[0].diagnostic.message.contains("line 3")
                && p[0].diagnostic.message.contains("line 5"),
            "{}",
            p[0].diagnostic.message
        );
        // Missing everywhere: the message names both homes.
        let p = check(&c, "title: x\n");
        assert_eq!(p[0].kind, ProblemKind::Missing);
        assert!(p[0]
            .diagnostic
            .hint
            .as_deref()
            .unwrap()
            .contains("top level or under `extra:`"));
    }

    #[test]
    fn test_taxonomy_names_are_top_level_keys() {
        let mut config: SiteConfig = toml::from_str(
            "collections = []\n[[taxonomies]]\nname = \"categories\"\n[site]\ntitle = \"t\"\n",
        )
        .unwrap();
        config.collections = vec![collection(
            "strict_fields = true\n[fields.author]\ntype = \"string\"\n",
        )];
        assert!(is_top_level(&config, "categories"));
        assert_eq!(
            custom_field_names(&config, &config.collections[0]),
            ["author"]
        );
        let schema = Schema::for_site(&config, &config.collections[0]);
        let p = schema.check_text("title: x\ncategories: [a]\n", 2, Path::new("a.md"));
        assert!(p.is_empty(), "{p:#?}");
        // Without the taxonomy, `categories` is an unknown key.
        config.taxonomies.clear();
        let schema = Schema::for_site(&config, &config.collections[0]);
        let p = schema.check_text("title: x\ncategories: [a]\n", 2, Path::new("a.md"));
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].kind, ProblemKind::Unknown);
    }

    #[test]
    fn test_page_extra_merges_top_level_custom_fields() {
        let fm: crate::content::Frontmatter =
            serde_yaml_ng::from_str("title: x\nauthor: Ada\nmood: ok\nextra:\n  rating: 4\n")
                .unwrap();
        let extra = page_extra(&fm, &["author".to_string(), "rating".to_string()]);
        assert_eq!(extra.len(), 2, "undeclared `mood` stays out: {extra:?}");
        assert_eq!(extra["author"], serde_yaml_ng::Value::from("Ada"));
        assert_eq!(page_extra(&fm, &[]), fm.extra);
    }

    #[test]
    fn test_types_and_lists() {
        let c = collection(
            r#"
[fields.tags]
type = "list"
items = "enum"
enum = ["new", "fix"]
max = 2
[fields.when]
type = "date"
[fields.at]
type = "datetime"
[fields.link]
type = "url"
[fields.flag]
type = "bool"
[fields.score]
type = "number"
[fields.meta]
type = "object"
"#,
        );
        assert!(check(
            &c,
            "title: x\ntags: [new]\nextra:\n  when: 2026-01-02\n  at: 2026-01-02T10:00:00Z\n  link: https://a.b\n  flag: true\n  score: 1.5\n  meta: {a: 1}\n"
        )
        .is_empty());
        let p = check(
            &c,
            "title: x\ntags: [new, bogus]\nextra:\n  when: 2026-13-02\n  at: yesterday\n  link: not a url\n  flag: \"yes\"\n  score: \"1\"\n  meta: [1]\n",
        );
        assert_eq!(p.len(), 7, "{p:#?}");
        assert!(p[0]
            .diagnostic
            .message
            .contains("item 2 must be one of `new`, `fix`"));
    }

    #[test]
    fn test_strict_fields_and_builtins_allowed() {
        let c = collection("strict_fields = true\n[fields.author]\ntype = \"string\"\n");
        let p = check(
            &c,
            "title: x\ndate: 2026-01-01\nauthr: y\nextra:\n  author: me\n  mood: ok\n",
        );
        assert_eq!(p.len(), 2, "{p:#?}");
        assert!(p
            .iter()
            .all(|p| p.kind == ProblemKind::Unknown && !p.diagnostic.is_error()));
        assert_eq!(p[0].diagnostic.line, Some(4));
        assert_eq!(
            p[0].diagnostic.hint.as_deref(),
            Some("did you mean `author`?")
        );
        assert_eq!(p[1].diagnostic.line, Some(7));
    }

    #[test]
    fn test_date_from_filename_satisfies_required_date() {
        let c = collection("[fields.date]\ntype = \"date\"\nrequired = true\n");
        let s = Schema::compile(&c);
        assert!(s
            .check_text("title: x", 2, Path::new("content/posts/2026-01-01-a.md"))
            .is_empty());
        assert_eq!(
            s.check_text("title: x", 2, Path::new("content/posts/a.md"))
                .len(),
            1
        );
    }

    #[test]
    fn test_validate_config_rejects_mistakes() {
        let bad = [
            "[fields.s]\ntype = \"enum\"\n",
            "[fields.s]\ntype = \"string\"\nenum = [\"a\"]\n",
            "[fields.s]\ntype = \"string\"\nitems = \"string\"\n",
            "[fields.s]\ntype = \"bool\"\nmax = 1\n",
            "[fields.s]\ntype = \"string\"\nmin = 5\nmax = 1\n",
            "[fields.s]\ntype = \"string\"\npattern = \"(\"\n",
            "[fields.weight]\ntype = \"string\"\n",
            "[fields.s]\ntype = \"integer\"\ndefault = \"x\"\n",
            "[fields.s]\ntype = \"list\"\nitems = \"object\"\n",
        ];
        for fields in bad {
            let src = format!(
                "[site]\ntitle = \"t\"\n[[collections]]\nname = \"posts\"\nlabel = \"P\"\ndirectory = \"posts\"\ndefault_template = \"post.html\"\n[collections.{}", fields.trim_start_matches('[')
            );
            let config: SiteConfig = toml::from_str(&src).unwrap();
            let err = validate_config(&config).unwrap_err();
            assert!(err.0.contains("[collections.fields."), "{fields}: {err:?}");
            assert!(config_line(&src, &err.1).is_some(), "{fields}");
        }
    }

    #[test]
    fn test_apply_defaults_and_placeholders() {
        let c = collection(
            "[fields.author]\ntype = \"string\"\ndefault = \"Jane\"\n[fields.summary]\ntype = \"string\"\nrequired = true\n[fields.description]\ntype = \"string\"\nrequired = true\n[fields.topics]\ntype = \"list\"\nrequired = true\n",
        );
        let mut map: Mapping = serde_yaml_ng::from_str("title: x\n").unwrap();
        let config: SiteConfig =
            toml::from_str("collections = []\n[site]\ntitle = \"t\"\n").unwrap();
        let placeholders = apply_defaults(&config, &c, &mut map);
        assert_eq!(placeholders, vec!["summary", "description", "topics"]);
        let text = serde_yaml_ng::to_string(&map).unwrap();
        assert!(
            text.contains("extra:\n  author: Jane\n  summary: ''\n  topics: []"),
            "{text}"
        );
        assert!(text.contains("description: ''"), "{text}");
    }

    #[test]
    fn test_key_of_and_expectation() {
        assert_eq!(key_of("title: x"), Some("title"));
        assert_eq!(key_of("\"a b\": x"), Some("a b"));
        assert_eq!(key_of("- item"), None);
        assert_eq!(key_of("url: https://x"), Some("url"));
        assert_eq!(key_of("https://x"), None);
        let spec = FieldSchema {
            min: Some(1.into()),
            max: Some(3.into()),
            ..FieldSchema::new(FieldType::List)
        };
        assert_eq!(spec.expectation(), "a list, 1 items to 3 items");
    }

    #[test]
    fn test_preset_fields_are_never_required() {
        for p in ["posts", "docs", "pages", "changelog", "roadmap", "trust"] {
            let f = preset_fields(p);
            assert!(!f.is_empty());
            assert!(f.values().all(|s| !s.required), "{p}");
        }
        assert!(preset_fields("custom").is_empty());
    }
}
