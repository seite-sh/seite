//! `seite cms setup <pages-cms|decap>`: configuration for a free, Git-based
//! CMS that edits the site's markdown through GitHub.
//!
//! - Pages CMS (<https://pagescms.org/docs/>): `.pages.yml` at the repository
//!   root. Spec used: configuration overview
//!   (<https://pagescms.org/docs/configuration/>), `media` `input`/`output`
//!   (<https://pagescms.org/docs/configuration/media/>), content entries
//!   `name`/`label`/`type`/`path`/`filename`/`format`/`subfolders`/`fields`
//!   (<https://pagescms.org/docs/configuration/content/>), filename tokens
//!   `{year}-{month}-{day}-{primary}`
//!   (<https://pagescms.org/docs/configuration/content/filename/>), field keys
//!   `name`/`label`/`type`/`required`/`description`/`list`/`options`
//!   (<https://pagescms.org/docs/configuration/content/fields/>), `select`
//!   `options.values`/`multiple`, `date` `options.format` (date-fns tokens) /
//!   `time`, `rich-text` `options.format: markdown`
//!   (<https://pagescms.org/docs/configuration/fields/>).
//! - Decap CMS (<https://decapcms.org/docs/>): `static/admin/index.html`
//!   (the pinned bundle with Subresource Integrity) and
//!   `static/admin/config.yml` (`backend: github`, `media_folder`,
//!   `public_folder`, folder collections with widgets). Decap fields are
//!   required by default, so optional fields get `required: false`.
//!
//! Fields come from the built-in frontmatter (title, date, description, tags,
//! image, updated, draft) plus the collection's `[collections.fields.*]`
//! schema; a schema field with a built-in name replaces the built-in one.
//! `object` fields have no nested schema in seite and are skipped (listed in
//! the output). The YAML is built as ordered `serde_yaml_ng` mappings, never
//! by string concatenation, so config values cannot break its structure.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use clap::{Args, Subcommand, ValueEnum};
use serde_yaml_ng::{Mapping, Value as Yaml};

use crate::config::{CollectionConfig, SiteConfig};
use crate::content::schema::{FieldSchema, FieldType};
use crate::diagnostics::Diagnostic;
use crate::output::human;

/// Pinned Decap CMS release (published 2026-09-22) and the SHA-384 of
/// `dist/decap-cms.js`, computed from the file served by jsDelivr (identical
/// on unpkg). Bump both together.
pub const DECAP_VERSION: &str = "3.16.3";
pub const DECAP_SRI: &str =
    "sha384-A/Gdn928CNLufmnGWGs9RM4Q9o8bSNLeJERqBFrBjwzL7FRpoLT3MPF8Tl0+Wd2p";

/// Repository placeholder written when `origin` is not a GitHub remote.
pub const REPO_PLACEHOLDER: &str = "OWNER/REPO";

pub const PAGES_CMS_FILE: &str = ".pages.yml";
pub const DECAP_CONFIG_FILE: &str = "admin/config.yml";
pub const DECAP_INDEX_FILE: &str = "admin/index.html";

#[derive(Args)]
pub struct CmsArgs {
    #[command(subcommand)]
    pub command: CmsCommand,
}

#[derive(Subcommand)]
pub enum CmsCommand {
    /// Write the configuration for a Git-based CMS (Pages CMS or Decap CMS)
    Setup(SetupArgs),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CmsKind {
    /// Pages CMS (pagescms.org): writes .pages.yml
    PagesCms,
    /// Decap CMS: writes static/admin/index.html and static/admin/config.yml
    Decap,
}

#[derive(Args)]
pub struct SetupArgs {
    /// Which CMS to configure
    #[arg(value_enum)]
    pub cms: CmsKind,
    /// Overwrite existing files (each is first backed up to `<file>.bak`)
    #[arg(long)]
    pub force: bool,
    /// Branch the CMS commits to (Decap; default: the current git branch, else main)
    #[arg(long)]
    pub branch: Option<String>,
}

pub fn run(args: &CmsArgs) -> anyhow::Result<()> {
    match &args.command {
        CmsCommand::Setup(setup) => run_setup(setup),
    }
}

fn run_setup(args: &SetupArgs) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let config = SiteConfig::load(&root.join("seite.toml"))?;
    if let Some(branch) = &args.branch {
        if !is_branch_name(branch) {
            bail!("--branch '{branch}' is not a valid branch name");
        }
    }

    let mut skipped = Vec::new();
    let mut notes = Vec::new();
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let mut repo_json = serde_json::Value::Null;
    let mut branch_json = serde_json::Value::Null;
    let static_dir = rel_dir(&config.build.static_dir);
    match args.cms {
        CmsKind::PagesCms => {
            files.push((
                PathBuf::from(PAGES_CMS_FILE),
                pages_cms_config(&config, &mut skipped),
            ));
        }
        CmsKind::Decap => {
            let repo = git_origin(&root).and_then(|url| github_repo_from_url(&url));
            if repo.is_none() {
                notes.push(format!(
                    "`origin` is not a GitHub remote: set backend.repo in {static_dir}/{DECAP_CONFIG_FILE} (now {REPO_PLACEHOLDER})"
                ));
            }
            let branch = args
                .branch
                .clone()
                .or_else(|| git_branch(&root))
                .unwrap_or_else(|| "main".to_string());
            let repo = repo.unwrap_or_else(|| REPO_PLACEHOLDER.to_string());
            files.push((
                PathBuf::from(&static_dir).join(DECAP_INDEX_FILE),
                decap_index_html(&config),
            ));
            files.push((
                PathBuf::from(&static_dir).join(DECAP_CONFIG_FILE),
                decap_config(&config, &repo, &branch, &mut skipped),
            ));
            repo_json = serde_json::json!(repo);
            branch_json = serde_json::json!(branch);
        }
    }

    let existing: Vec<&PathBuf> = files
        .iter()
        .map(|(p, _)| p)
        .filter(|p| std::fs::symlink_metadata(root.join(p)).is_ok())
        .collect();
    if !existing.is_empty() && !args.force {
        bail!(
            "{} already exist{}; rerun with --force to regenerate (the old file{} will be kept as .bak)",
            existing
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            if existing.len() == 1 { "s" } else { "" },
            if existing.len() == 1 { "" } else { "s" }
        );
    }

    // Every check before anything is written: a cloned repository may ship
    // a symlinked `.pages.yml` (or a symlinked `static/`), and backing it
    // up must never copy a file from outside the project into it.
    for (path, _) in &files {
        crate::safe_fs::guard(&root, &root.join(path))?;
    }
    let mut pending = Vec::new();
    for path in &existing {
        let abs = root.join(path);
        let meta = std::fs::symlink_metadata(&abs)
            .with_context(|| format!("failed to inspect {}", abs.display()))?;
        if !meta.file_type().is_file() {
            bail!(
                "refusing to replace {}: it is not a regular file (a symlink or directory)",
                path.display()
            );
        }
        let bak = PathBuf::from(format!("{}.bak", path.display()));
        crate::safe_fs::guard(&root, &root.join(&bak))?;
        let bytes =
            std::fs::read(&abs).with_context(|| format!("failed to read {}", abs.display()))?;
        pending.push((bak, bytes));
    }
    let mut backups = Vec::new();
    for (bak, bytes) in pending {
        crate::safe_fs::write(&root, &root.join(&bak), bytes)?;
        backups.push(bak.display().to_string().replace('\\', "/"));
    }
    for (path, content) in &files {
        crate::safe_fs::write(&root, &root.join(path), content)?;
    }

    let written: Vec<String> = files
        .iter()
        .map(|(p, _)| p.display().to_string().replace('\\', "/"))
        .collect();
    let next_steps = next_steps(args.cms, &config);

    for file in &written {
        human::success(&format!("Wrote {file}"));
    }
    for bak in &backups {
        human::info(&format!("Backed up the previous file to {bak}"));
    }
    for field in &skipped {
        human::warning(&format!(
            "Skipped `{field}`: object fields have no nested schema to edit"
        ));
    }
    for note in &notes {
        human::warning(note);
    }
    human::info("Next steps:");
    for (i, step) in next_steps.iter().enumerate() {
        crate::human_println!("  {}. {}", i + 1, step);
    }

    crate::output::json::set_data(serde_json::json!({
        "cms": match args.cms { CmsKind::PagesCms => "pages-cms", CmsKind::Decap => "decap" },
        "files": written,
        "backups": backups,
        "skipped_fields": skipped,
        "repo": repo_json,
        "branch": branch_json,
        "notes": notes,
        "next_steps": next_steps,
    }));
    Ok(())
}

fn next_steps(cms: CmsKind, config: &SiteConfig) -> Vec<String> {
    let base = config.site.base_url.trim_end_matches('/');
    match cms {
        CmsKind::PagesCms => vec![
            "Commit and push .pages.yml to GitHub".into(),
            "Open https://app.pagescms.org, sign in with GitHub and pick this repository (free; no server or secrets needed)".into(),
            "After changing [collections.fields.*], rerun `seite cms setup pages-cms --force`".into(),
        ],
        CmsKind::Decap => vec![
            "Decap's GitHub backend needs an OAuth proxy. Free options: on Netlify, add a GitHub OAuth app under Site configuration > Access & security > OAuth (no base_url needed); anywhere else, deploy a Cloudflare Worker such as sveltia-cms-auth (https://github.com/sveltia/sveltia-cms-auth) or decap-proxy (https://github.com/sterlingwes/decap-proxy) and set backend.base_url to its URL. Avoid Netlify Identity / Git Gateway for new sites (deprecated by Netlify in 2025)".into(),
            "Commit, push and deploy (`seite deploy`)".into(),
            format!("Open {base}/static/admin/ and log in with GitHub"),
            "After changing [collections.fields.*], rerun `seite cms setup decap --force`".into(),
        ],
    }
}

// ---------------------------------------------------------------------------
// Field model shared by both CMSs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    String,
    Text,
    Number { integer: bool },
    Boolean,
    Date,
    Datetime,
    Image,
    Select { values: Vec<String>, multiple: bool },
}

#[derive(Debug, Clone, PartialEq)]
struct CmsField {
    name: String,
    label: String,
    kind: Kind,
    required: bool,
    /// Repeated values (a YAML list).
    list: bool,
    description: Option<String>,
    default: Option<serde_json::Value>,
}

impl CmsField {
    fn new(name: &str, kind: Kind) -> Self {
        Self {
            name: name.to_string(),
            label: label_of(name),
            kind,
            required: false,
            list: false,
            description: None,
            default: None,
        }
    }
}

/// `series_order` → `Series order`.
fn label_of(name: &str) -> String {
    let words = name.replace(['_', '-'], " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn kind_of(field_type: FieldType, spec: &FieldSchema) -> Option<Kind> {
    let values = || -> Vec<String> {
        spec.allowed_values()
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect()
    };
    Some(match field_type {
        FieldType::String | FieldType::Url => Kind::String,
        FieldType::Text => Kind::Text,
        FieldType::Number => Kind::Number { integer: false },
        FieldType::Integer => Kind::Number { integer: true },
        FieldType::Bool => Kind::Boolean,
        FieldType::Date => Kind::Date,
        FieldType::Datetime => Kind::Datetime,
        FieldType::Image => Kind::Image,
        FieldType::Enum => Kind::Select {
            values: values(),
            multiple: false,
        },
        FieldType::List | FieldType::Object => return None,
    })
}

/// One schema field as a CMS field, or `None` for `object` (and lists of
/// lists/objects, which the schema rejects anyway).
fn schema_field(name: &str, spec: &FieldSchema) -> Option<CmsField> {
    let (kind, list) = match spec.field_type {
        FieldType::List => match spec.items {
            Some(FieldType::Enum) => (
                Kind::Select {
                    values: spec
                        .allowed_values()
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect(),
                    multiple: true,
                },
                false,
            ),
            Some(items) => (kind_of(items, spec)?, true),
            None => (Kind::String, true),
        },
        other => (kind_of(other, spec)?, false),
    };
    let mut field = CmsField::new(name, kind);
    field.list = list;
    field.required = spec.required;
    field.description = spec.description.clone();
    field.default = spec.default.clone();
    Some(field)
}

/// Built-in frontmatter fields, then the schema (same name replaces).
/// `skipped` collects `collection.field` names that cannot be edited.
fn collection_fields(collection: &CollectionConfig, skipped: &mut Vec<String>) -> Vec<CmsField> {
    let mut fields = Vec::new();
    let mut title = CmsField::new("title", Kind::String);
    title.required = true;
    fields.push(title);
    if collection.has_date {
        let mut date = CmsField::new("date", Kind::Date);
        date.required = true;
        fields.push(date);
    }
    let mut description = CmsField::new("description", Kind::Text);
    description.description = Some("Summary for search results and social cards".into());
    fields.push(description);
    let mut tags = CmsField::new("tags", Kind::String);
    tags.list = true;
    fields.push(tags);
    fields.push(CmsField::new("image", Kind::Image));
    if collection.has_date {
        fields.push(CmsField::new("updated", Kind::Date));
    }
    let mut draft = CmsField::new("draft", Kind::Boolean);
    draft.default = Some(serde_json::Value::Bool(false));
    fields.push(draft);

    for (name, spec) in &collection.fields {
        match schema_field(name, spec) {
            Some(field) => match fields.iter_mut().find(|f| f.name == *name) {
                Some(existing) => *existing = field,
                None => fields.push(field),
            },
            None => {
                fields.retain(|f| f.name != *name);
                skipped.push(format!("{}.{name}", collection.name));
            }
        }
    }
    fields
}

// ---------------------------------------------------------------------------
// YAML helpers
// ---------------------------------------------------------------------------

fn s(v: &str) -> Yaml {
    Yaml::String(v.to_string())
}

fn map(entries: Vec<(&str, Yaml)>) -> Yaml {
    let mut m = Mapping::new();
    for (k, v) in entries {
        m.insert(s(k), v);
    }
    Yaml::Mapping(m)
}

fn json_to_yaml(v: &serde_json::Value) -> Yaml {
    serde_yaml_ng::to_value(v).unwrap_or(Yaml::Null)
}

fn to_yaml_string(header: &str, value: &Yaml) -> String {
    let body = serde_yaml_ng::to_string(value).unwrap_or_default();
    format!("{header}{body}")
}

/// A config directory as a repository-relative path (`./content/` → `content`).
fn rel_dir(dir: &str) -> String {
    let d = dir.replace('\\', "/");
    let d = d.trim_start_matches("./").trim_matches('/');
    if d.is_empty() {
        ".".into()
    } else {
        d.to_string()
    }
}

fn join(a: &str, b: &str) -> String {
    let b = b.trim_matches('/');
    if a == "." || a.is_empty() {
        b.to_string()
    } else if b.is_empty() {
        a.to_string()
    } else {
        format!("{a}/{b}")
    }
}

/// Repository path of a collection's folder.
fn collection_path(config: &SiteConfig, collection: &CollectionConfig) -> String {
    join(&rel_dir(&config.build.content_dir), &collection.directory)
}

fn media_input(config: &SiteConfig) -> String {
    join(&rel_dir(&config.build.static_dir), "images")
}

fn media_output(config: &SiteConfig) -> String {
    format!("{}/static/images", config.base_path())
}

// ---------------------------------------------------------------------------
// Pages CMS
// ---------------------------------------------------------------------------

fn pages_field(field: &CmsField) -> Yaml {
    let mut m = Mapping::new();
    m.insert(s("name"), s(&field.name));
    m.insert(s("label"), s(&field.label));
    let (ty, options): (&str, Option<Yaml>) = match &field.kind {
        Kind::String => ("string", None),
        Kind::Text => ("text", None),
        Kind::Number { .. } => ("number", None),
        Kind::Boolean => ("boolean", None),
        Kind::Date => ("date", Some(map(vec![("format", s("yyyy-MM-dd"))]))),
        Kind::Datetime => (
            "date",
            Some(map(vec![
                ("time", Yaml::Bool(true)),
                ("format", s("yyyy-MM-dd'T'HH:mm:ss")),
            ])),
        ),
        Kind::Image => ("image", None),
        Kind::Select { values, multiple } => {
            let mut o = vec![(
                "values",
                Yaml::Sequence(values.iter().map(|v| s(v)).collect()),
            )];
            if *multiple {
                o.push(("multiple", Yaml::Bool(true)));
            }
            ("select", Some(map(o)))
        }
    };
    m.insert(s("type"), s(ty));
    if field.required {
        m.insert(s("required"), Yaml::Bool(true));
    }
    if field.list {
        m.insert(s("list"), Yaml::Bool(true));
    }
    if let Some(d) = &field.description {
        m.insert(s("description"), s(d));
    }
    if let Some(d) = &field.default {
        m.insert(s("default"), json_to_yaml(d));
    }
    if let Some(o) = options {
        m.insert(s("options"), o);
    }
    Yaml::Mapping(m)
}

fn pages_cms_config(config: &SiteConfig, skipped: &mut Vec<String>) -> String {
    let mut content = Vec::new();
    for collection in &config.collections {
        let mut fields: Vec<Yaml> = collection_fields(collection, skipped)
            .iter()
            .map(pages_field)
            .collect();
        fields.push(map(vec![
            ("name", s("body")),
            ("label", s("Body")),
            ("type", s("rich-text")),
            ("options", map(vec![("format", s("markdown"))])),
        ]));
        let filename = if collection.has_date {
            "{year}-{month}-{day}-{primary}.md"
        } else {
            "{primary}.md"
        };
        let mut entry = vec![
            ("name", s(&collection.name)),
            ("label", s(&collection.label)),
            ("type", s("collection")),
            ("path", s(&collection_path(config, collection))),
            ("format", s("yaml-frontmatter")),
            ("filename", s(filename)),
        ];
        if collection.nested {
            entry.push(("subfolders", Yaml::Bool(true)));
        }
        entry.push(("fields", Yaml::Sequence(fields)));
        content.push(map(entry));
    }
    let doc = map(vec![
        (
            "media",
            map(vec![
                ("input", s(&media_input(config))),
                ("output", s(&media_output(config))),
            ]),
        ),
        ("content", Yaml::Sequence(content)),
    ]);
    to_yaml_string(
        "# Pages CMS configuration (https://pagescms.org/docs/configuration/).\n\
         # Generated by `seite cms setup pages-cms` from seite.toml; rerun with\n\
         # --force after changing collections or [collections.fields.*].\n",
        &doc,
    )
}

// ---------------------------------------------------------------------------
// Decap CMS
// ---------------------------------------------------------------------------

fn decap_widget(kind: &Kind) -> Vec<(&'static str, Yaml)> {
    match kind {
        Kind::String => vec![("widget", s("string"))],
        Kind::Text => vec![("widget", s("text"))],
        Kind::Number { integer } => vec![
            ("widget", s("number")),
            ("value_type", s(if *integer { "int" } else { "float" })),
        ],
        Kind::Boolean => vec![("widget", s("boolean"))],
        // Date-only values: seite parses `YYYY-MM-DD`.
        Kind::Date => vec![
            ("widget", s("datetime")),
            ("format", s("YYYY-MM-DD")),
            ("date_format", s("YYYY-MM-DD")),
            ("time_format", Yaml::Bool(false)),
        ],
        Kind::Datetime => vec![
            ("widget", s("datetime")),
            ("format", s("YYYY-MM-DDTHH:mm:ss")),
        ],
        Kind::Image => vec![("widget", s("image"))],
        Kind::Select { values, multiple } => {
            let mut w = vec![
                ("widget", s("select")),
                (
                    "options",
                    Yaml::Sequence(values.iter().map(|v| s(v)).collect()),
                ),
            ];
            if *multiple {
                w.push(("multiple", Yaml::Bool(true)));
            }
            w
        }
    }
}

fn decap_field(field: &CmsField) -> Yaml {
    let mut m = Mapping::new();
    m.insert(s("label"), s(&field.label));
    m.insert(s("name"), s(&field.name));
    if field.list {
        m.insert(s("widget"), s("list"));
        if field.kind != Kind::String {
            let mut item = Mapping::new();
            item.insert(s("label"), s(&field.label));
            item.insert(s("name"), s("item"));
            for (k, v) in decap_widget(&field.kind) {
                item.insert(s(k), v);
            }
            m.insert(s("field"), Yaml::Mapping(item));
        }
    } else {
        for (k, v) in decap_widget(&field.kind) {
            m.insert(s(k), v);
        }
    }
    // Decap fields are required unless told otherwise.
    m.insert(s("required"), Yaml::Bool(field.required));
    if let Some(d) = &field.description {
        m.insert(s("hint"), s(d));
    }
    if let Some(d) = &field.default {
        m.insert(s("default"), json_to_yaml(d));
    }
    Yaml::Mapping(m)
}

fn decap_config(
    config: &SiteConfig,
    repo: &str,
    branch: &str,
    skipped: &mut Vec<String>,
) -> String {
    let mut collections = Vec::new();
    for collection in &config.collections {
        let mut fields: Vec<Yaml> = collection_fields(collection, skipped)
            .iter()
            .map(decap_field)
            .collect();
        fields.push(map(vec![
            ("label", s("Body")),
            ("name", s("body")),
            ("widget", s("markdown")),
            ("required", Yaml::Bool(false)),
        ]));
        let slug = if collection.has_date {
            "{{year}}-{{month}}-{{day}}-{{slug}}"
        } else {
            "{{slug}}"
        };
        collections.push(map(vec![
            ("name", s(&collection.name)),
            ("label", s(&collection.label)),
            ("folder", s(&collection_path(config, collection))),
            ("create", Yaml::Bool(true)),
            ("extension", s("md")),
            ("format", s("frontmatter")),
            ("slug", s(slug)),
            ("fields", Yaml::Sequence(fields)),
        ]));
    }
    let doc = map(vec![
        (
            "backend",
            map(vec![
                ("name", s("github")),
                ("repo", s(repo)),
                ("branch", s(branch)),
            ]),
        ),
        ("media_folder", s(&media_input(config))),
        ("public_folder", s(&media_output(config))),
        ("collections", Yaml::Sequence(collections)),
    ]);
    to_yaml_string(
        "# Decap CMS configuration (https://decapcms.org/docs/configuration-options/).\n\
         # Generated by `seite cms setup decap` from seite.toml; rerun with --force\n\
         # after changing collections or [collections.fields.*].\n\
         # The GitHub backend needs an OAuth proxy: add `base_url: https://<your proxy>`\n\
         # under `backend` (e.g. a sveltia-cms-auth Cloudflare Worker), or host on\n\
         # Netlify with its GitHub OAuth provider. See `seite docs cms`.\n",
        &doc,
    )
}

fn decap_index_html(config: &SiteConfig) -> String {
    let lang = crate::templates::escape_attr(&config.site.language);
    format!(
        "<!doctype html>\n\
<html lang=\"{lang}\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
  <meta name=\"robots\" content=\"noindex, nofollow\">\n\
  <title>Content Manager</title>\n\
</head>\n\
<body>\n\
  <!-- Decap CMS {DECAP_VERSION}, pinned with Subresource Integrity. Generated by `seite cms setup decap`. -->\n\
  <script src=\"https://cdn.jsdelivr.net/npm/decap-cms@{DECAP_VERSION}/dist/decap-cms.js\" integrity=\"{DECAP_SRI}\" crossorigin=\"anonymous\"></script>\n\
</body>\n\
</html>\n"
    )
}

// ---------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------

/// `git config --get remote.origin.url`, with hooks and fsmonitor disabled
/// (reading config runs neither, but a cloned repo should not get a chance).
fn git_origin(root: &Path) -> Option<String> {
    git_output(root, &["config", "--get", "remote.origin.url"])
}

fn git_branch(root: &Path) -> Option<String> {
    git_output(root, &["symbolic-ref", "--short", "HEAD"]).filter(|b| is_branch_name(b))
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// `owner/name` of a GitHub remote URL (https, `git@github.com:`,
/// `ssh://git@github.com/`), or `None` for anything else.
pub fn github_repo_from_url(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("git@github.com:"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("git://github.com/"))
        .or_else(|| {
            // https://user@github.com/owner/repo
            let after = url.strip_prefix("https://")?;
            let (userinfo, path) = after.split_once('@')?;
            (!userinfo.contains('/')).then_some(())?;
            path.strip_prefix("github.com/")
        })?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    crate::config::embeds::is_github_repo(rest).then(|| rest.to_string())
}

/// A conservative git branch name.
fn is_branch_name(b: &str) -> bool {
    !b.is_empty()
        && b.len() <= 200
        && !b.starts_with('-')
        && !b.contains("..")
        && b.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b'/'))
}

// ---------------------------------------------------------------------------
// `seite check`: stale CMS config
// ---------------------------------------------------------------------------

/// `cms-config-stale` warnings: a `.pages.yml` content path or a Decap
/// collection folder that no longer exists (a collection was renamed or
/// removed). Paths are relative to `root`.
pub fn stale_config_diagnostics(root: &Path, config: &SiteConfig) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let sources = [
        (
            PathBuf::from(PAGES_CMS_FILE),
            "content",
            "path",
            "pages-cms",
        ),
        (
            PathBuf::from(rel_dir(&config.build.static_dir)).join(DECAP_CONFIG_FILE),
            "collections",
            "folder",
            "decap",
        ),
    ];
    for (file, list_key, path_key, cms) in sources {
        let abs = root.join(&file);
        if crate::safe_fs::guard(root, &abs).is_err() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&abs) else {
            continue;
        };
        let Ok(doc) = serde_yaml_ng::from_str::<Yaml>(&text) else {
            continue;
        };
        let Some(entries) = doc.get(list_key).and_then(Yaml::as_sequence) else {
            continue;
        };
        for entry in entries {
            if cms == "pages-cms" && entry.get("type").and_then(Yaml::as_str) != Some("collection")
            {
                continue;
            }
            let Some(path) = entry.get(path_key).and_then(Yaml::as_str) else {
                continue;
            };
            let rel = path.trim_matches('/');
            if rel.is_empty() || rel.split('/').any(|p| p == "..") || root.join(rel).is_dir() {
                continue;
            }
            let file_display = file.display().to_string().replace('\\', "/");
            let mut d = Diagnostic::warning(
                "cms-config-stale",
                format!("{file_display} points the CMS at `{path}`, which does not exist"),
            )
            .with_file(&file)
            .with_hint(format!(
                "a collection was renamed or removed: rerun `seite cms setup {cms} --force` to regenerate it"
            ));
            if let Some(line) = text.lines().position(|l| {
                l.trim_start().starts_with(&format!("{path_key}:")) && l.contains(path)
            }) {
                d = d.with_line(line + 1);
            }
            out.push(d);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_schema() -> SiteConfig {
        toml::from_str(
            r#"
[site]
title = "T"
description = ""
base_url = "https://example.github.io/blog"
language = "en"
author = ""

[[collections]]
name = "posts"
label = "Posts"
directory = "posts"
has_date = true
has_rss = true
listed = true
url_prefix = "/posts"
nested = false
default_template = "post.html"

[collections.fields.description]
type = "string"
required = true
max = 160
description = "Summary"

[collections.fields.status]
type = "enum"
enum = ["draft", "review", "published"]

[collections.fields.topics]
type = "list"
items = "enum"
enum = ["rust", "web"]

[collections.fields.rating]
type = "integer"

[collections.fields.scores]
type = "list"
items = "number"

[collections.fields.published_at]
type = "datetime"

[collections.fields.meta]
type = "object"

[[collections]]
name = "docs"
label = "Docs"
directory = "docs"
has_date = false
has_rss = false
listed = true
url_prefix = "/docs"
nested = true
default_template = "doc.html"
"#,
        )
        .unwrap()
    }

    fn field<'a>(fields: &'a [Yaml], name: &str) -> &'a Yaml {
        fields
            .iter()
            .find(|f| f.get("name").and_then(Yaml::as_str) == Some(name))
            .unwrap_or_else(|| panic!("no field {name}"))
    }

    #[test]
    fn test_pages_cms_field_mapping() {
        let mut skipped = Vec::new();
        let text = pages_cms_config(&config_with_schema(), &mut skipped);
        assert_eq!(skipped, vec!["posts.meta"]);
        let doc: Yaml = serde_yaml_ng::from_str(&text).unwrap();
        assert_eq!(doc["media"]["input"].as_str(), Some("static/images"));
        assert_eq!(doc["media"]["output"].as_str(), Some("/blog/static/images"));
        let posts = &doc["content"][0];
        assert_eq!(posts["path"].as_str(), Some("content/posts"));
        assert_eq!(posts["type"].as_str(), Some("collection"));
        assert_eq!(posts["format"].as_str(), Some("yaml-frontmatter"));
        assert_eq!(
            posts["filename"].as_str(),
            Some("{year}-{month}-{day}-{primary}.md")
        );
        let fields = posts["fields"].as_sequence().unwrap();
        assert_eq!(field(fields, "title")["required"].as_bool(), Some(true));
        assert_eq!(
            field(fields, "date")["options"]["format"].as_str(),
            Some("yyyy-MM-dd")
        );
        // The schema's `description` replaces the built-in one, in place.
        let description = field(fields, "description");
        assert_eq!(description["type"].as_str(), Some("string"));
        assert_eq!(description["required"].as_bool(), Some(true));
        assert_eq!(description["description"].as_str(), Some("Summary"));
        assert_eq!(fields[2]["name"].as_str(), Some("description"));
        assert_eq!(field(fields, "tags")["list"].as_bool(), Some(true));
        let status = field(fields, "status");
        assert_eq!(status["type"].as_str(), Some("select"));
        assert_eq!(status["options"]["values"][2].as_str(), Some("published"));
        assert_eq!(
            field(fields, "topics")["options"]["multiple"].as_bool(),
            Some(true)
        );
        assert_eq!(field(fields, "rating")["type"].as_str(), Some("number"));
        let scores = field(fields, "scores");
        assert_eq!(
            (scores["type"].as_str(), scores["list"].as_bool()),
            (Some("number"), Some(true))
        );
        assert_eq!(
            field(fields, "published_at")["options"]["time"].as_bool(),
            Some(true)
        );
        assert!(fields.iter().all(|f| f["name"].as_str() != Some("meta")));
        assert_eq!(field(fields, "body")["type"].as_str(), Some("rich-text"));
        let docs = &doc["content"][1];
        assert_eq!(docs["filename"].as_str(), Some("{primary}.md"));
        assert_eq!(docs["subfolders"].as_bool(), Some(true));
        assert!(docs["fields"]
            .as_sequence()
            .unwrap()
            .iter()
            .all(|f| f["name"].as_str() != Some("date")));
    }

    #[test]
    fn test_decap_field_mapping() {
        let mut skipped = Vec::new();
        let text = decap_config(&config_with_schema(), "o/r", "main", &mut skipped);
        let doc: Yaml = serde_yaml_ng::from_str(&text).unwrap();
        assert_eq!(doc["backend"]["name"].as_str(), Some("github"));
        assert_eq!(doc["backend"]["repo"].as_str(), Some("o/r"));
        assert_eq!(doc["media_folder"].as_str(), Some("static/images"));
        assert_eq!(doc["public_folder"].as_str(), Some("/blog/static/images"));
        let posts = &doc["collections"][0];
        assert_eq!(posts["folder"].as_str(), Some("content/posts"));
        assert_eq!(posts["create"].as_bool(), Some(true));
        assert_eq!(
            posts["slug"].as_str(),
            Some("{{year}}-{{month}}-{{day}}-{{slug}}")
        );
        let fields = posts["fields"].as_sequence().unwrap();
        let date = field(fields, "date");
        assert_eq!(date["widget"].as_str(), Some("datetime"));
        assert_eq!(date["format"].as_str(), Some("YYYY-MM-DD"));
        assert_eq!(date["time_format"].as_bool(), Some(false));
        assert_eq!(field(fields, "image")["required"].as_bool(), Some(false));
        assert_eq!(field(fields, "title")["required"].as_bool(), Some(true));
        assert_eq!(field(fields, "tags")["widget"].as_str(), Some("list"));
        assert!(field(fields, "tags").get("field").is_none());
        assert_eq!(
            field(fields, "scores")["field"]["widget"].as_str(),
            Some("number")
        );
        assert_eq!(field(fields, "rating")["value_type"].as_str(), Some("int"));
        assert_eq!(field(fields, "topics")["multiple"].as_bool(), Some(true));
        assert_eq!(field(fields, "draft")["default"].as_bool(), Some(false));
        assert_eq!(field(fields, "body")["widget"].as_str(), Some("markdown"));
        assert_eq!(doc["collections"][1]["slug"].as_str(), Some("{{slug}}"));
    }

    #[test]
    fn test_hostile_values_stay_yaml_strings() {
        let mut config = config_with_schema();
        config.collections[0].label = "x\n  evil: true\n\"]".into();
        config.collections[0]
            .fields
            .get_mut("status")
            .unwrap()
            .enum_values = vec![serde_json::json!("a: b"), serde_json::json!("- c")];
        let mut skipped = Vec::new();
        let doc: Yaml =
            serde_yaml_ng::from_str(&decap_config(&config, "o/r", "main", &mut skipped)).unwrap();
        assert_eq!(
            doc["collections"][0]["label"].as_str(),
            Some("x\n  evil: true\n\"]")
        );
        assert!(doc["collections"][0].get("evil").is_none());
        let doc: Yaml = serde_yaml_ng::from_str(&pages_cms_config(&config, &mut skipped)).unwrap();
        let status = field(doc["content"][0]["fields"].as_sequence().unwrap(), "status");
        assert_eq!(status["options"]["values"][0].as_str(), Some("a: b"));
    }

    #[test]
    fn test_decap_index_html_is_pinned_with_sri() {
        let mut config = config_with_schema();
        config.site.language = "en\"><script>".into();
        let html = decap_index_html(&config);
        assert!(html.contains(&format!(
            "src=\"https://cdn.jsdelivr.net/npm/decap-cms@{DECAP_VERSION}/dist/decap-cms.js\" integrity=\"{DECAP_SRI}\" crossorigin=\"anonymous\""
        )));
        assert!(html.contains("<meta name=\"robots\" content=\"noindex, nofollow\">"));
        assert!(html.contains("lang=\"en&quot;&gt;&lt;script&gt;\""));
        assert!(DECAP_SRI.starts_with("sha384-"));
    }

    #[test]
    fn test_github_repo_from_url() {
        for url in [
            "https://github.com/owner/repo.git",
            "https://github.com/owner/repo",
            "https://github.com/owner/repo/",
            "git@github.com:owner/repo.git",
            "ssh://git@github.com/owner/repo.git",
            "https://token@github.com/owner/repo.git",
        ] {
            assert_eq!(
                github_repo_from_url(url).as_deref(),
                Some("owner/repo"),
                "{url}"
            );
        }
        for url in [
            "https://gitlab.com/owner/repo.git",
            "https://github.com.evil.com/owner/repo",
            "git@github.com:owner/repo/extra.git",
            "https://github.com/owner/\"><x",
            "/local/path",
        ] {
            assert_eq!(github_repo_from_url(url), None, "{url}");
        }
    }

    #[test]
    fn test_branch_names() {
        assert!(is_branch_name("main"));
        assert!(is_branch_name("feature/cms-1.2"));
        assert!(!is_branch_name("-x"));
        assert!(!is_branch_name("a..b"));
        assert!(!is_branch_name("a b"));
        assert!(!is_branch_name("\"x\""));
    }

    #[test]
    fn test_stale_config_diagnostics() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let config = config_with_schema();
        std::fs::create_dir_all(root.join("content/posts")).unwrap();
        let mut skipped = Vec::new();
        std::fs::write(
            root.join(".pages.yml"),
            pages_cms_config(&config, &mut skipped),
        )
        .unwrap();
        std::fs::create_dir_all(root.join("static/admin")).unwrap();
        std::fs::write(
            root.join("static/admin/config.yml"),
            decap_config(&config, "o/r", "main", &mut skipped),
        )
        .unwrap();
        let found = stale_config_diagnostics(root, &config);
        // content/docs is missing, in both files.
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().all(|d| d.code == "cms-config-stale"
            && d.message.contains("content/docs")
            && d.line.is_some()));
        std::fs::create_dir_all(root.join("content/docs")).unwrap();
        assert!(stale_config_diagnostics(root, &config).is_empty());
    }
}
