//! `seite import <wordpress|ghost|jekyll|hugo> <source>`: move an existing
//! site into a seite site in one command, leaving a report an agent can
//! work through.
//!
//! # Design note
//!
//! - **Parse → plan → localize images → write.** Each source module
//!   (`wordpress`, `ghost`, `jekyll`, `hugo`) only turns its input into
//!   `Item`s (Markdown body, mapped frontmatter, old URLs) plus skipped
//!   entries and warnings. Everything else is shared here: target collection,
//!   slugs and filenames, aliases, cross-references, images, frontmatter,
//!   schema validation, writing, and the report.
//! - **Nothing is created silently.** The target collections must exist
//!   (`seite collection add` otherwise); existing content files are skipped
//!   unless `--force`; `--dry-run` writes nothing (no report file either).
//! - **Drafts.** Without `--drafts`, drafts / pending / private / scheduled /
//!   members-only items are skipped and listed; with it they are imported
//!   with `draft: true`. Trashed items, auto-drafts and revisions never are.
//! - **Deterministic.** Items are processed sorted by (collection, date,
//!   source id); slug collisions get `-2`, `-3`, … in that order; image
//!   names derive from the source name; the report has no timestamps and
//!   only site-relative paths. Import twice into fresh sites → identical
//!   trees.
//! - **The build must pass.** Constructs seite would read as a shortcode
//!   (Hugo's `{{< x >}}`, literal `{{<` in HTML) are converted or
//!   neutralized; everything left for a human or agent (unknown shortcodes,
//!   Liquid includes, galleries) is inert text plus a located warning.
//! - All writes go through `crate::safe_fs`. Images are downloaded only from
//!   the export's own host or `--allow-host` (see `images`), local copies stay
//!   inside the source directory.

mod convert;
mod ghost;
mod hugo;
mod images;
mod jekyll;
mod report;
mod wordpress;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use chrono::NaiveDate;
use clap::Args;
use serde::Serialize;

use crate::config::{self, CollectionConfig, SiteConfig};
use crate::content::{self, schema, Frontmatter};

/// Largest export file read (WXR XML, Ghost JSON).
pub(crate) const MAX_EXPORT_BYTES: u64 = 200 * 1024 * 1024;

/// Where the import report is written (gitignored with `.seite/cache/`).
pub const REPORT_PATH: &str = ".seite/cache/import-report.md";

#[derive(Args)]
pub struct ImportArgs {
    /// What to import from
    #[arg(value_enum)]
    pub from: SourceKind,

    /// WordPress: the WXR export (.xml). Ghost: the JSON export.
    /// Jekyll/Hugo: the site directory.
    pub source: PathBuf,

    /// Collection for posts (default: posts)
    #[arg(long, value_name = "NAME")]
    pub collection: Option<String>,

    /// Collection for pages (default: pages)
    #[arg(long, value_name = "NAME")]
    pub pages_collection: Option<String>,

    /// Report what would be imported; write nothing
    #[arg(long)]
    pub dry_run: bool,

    /// Overwrite existing content files (default: skip and report them)
    #[arg(long)]
    pub force: bool,

    /// Do not download or copy images (references keep their old URLs)
    #[arg(long)]
    pub no_images: bool,

    /// Also download images from this host (`host` or `host:port`; repeatable).
    /// The export's own site host is always allowed.
    #[arg(long = "allow-host", value_name = "HOST")]
    pub allow_host: Vec<String>,

    /// Import drafts, pending, private, scheduled and members-only items
    /// (as `draft: true`). Trashed items and revisions are never imported.
    #[arg(long)]
    pub drafts: bool,

    /// The old site's URL, when the export doesn't name it (Ghost exports
    /// write `__GHOST_URL__`): used for images, links and old-URL aliases
    #[arg(long, value_name = "URL")]
    pub site_url: Option<String>,

    /// Total image download budget in MB (each file is capped at 20 MB)
    #[arg(long, value_name = "MB", default_value_t = 500)]
    pub max_download_mb: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Wordpress,
    Ghost,
    Jekyll,
    Hugo,
}

impl SourceKind {
    fn label(self) -> &'static str {
        match self {
            SourceKind::Wordpress => "WordPress",
            SourceKind::Ghost => "Ghost",
            SourceKind::Jekyll => "Jekyll",
            SourceKind::Hugo => "Hugo",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Post,
    Page,
}

impl Kind {
    fn plural(self) -> &'static str {
        match self {
            Kind::Post => "posts",
            Kind::Page => "pages",
        }
    }
}

/// A link to another imported item, resolved once every item has a URL
/// (Jekyll `{% post_url %}` / `{% link %}`, Hugo `ref` / `relref`).
#[derive(Debug, Clone)]
pub(crate) struct RefLink {
    /// Placeholder in the body.
    pub token: String,
    /// One of the target's [`Item::keys`].
    pub key: String,
    /// Source text, put back (inert) when the target isn't found.
    pub original: String,
    pub line: usize,
}

/// One post or page read from the source.
#[derive(Debug, Clone, Default)]
pub(crate) struct Item {
    /// Stable identifier for the report (`wp:42`, `ghost:<id>`, a path).
    pub source_id: String,
    /// Where it came from (`export.xml:120`, `_posts/2020-01-01-x.md`).
    pub source_ref: String,
    pub kind: Option<Kind>,
    /// A specific collection (Hugo section matching a seite collection).
    pub collection_hint: Option<String>,
    pub title: String,
    pub date: Option<NaiveDate>,
    pub updated: Option<NaiveDate>,
    /// Slug candidate (sanitized later).
    pub slug: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub author: Option<String>,
    /// Why this is a draft (`draft`, `pending`, `private`, …); `None` = published.
    pub draft: Option<String>,
    /// Featured image reference as written in the source.
    pub image: Option<String>,
    /// Old URL paths (`/2020/01/02/hello.html`).
    pub aliases: Vec<String>,
    pub series: Option<String>,
    pub expires: Option<NaiveDate>,
    pub weight: Option<i32>,
    /// Markdown body.
    pub body: String,
    /// Directories relative image paths resolve against (local sources).
    pub base_dirs: Vec<PathBuf>,
    /// Names other items use to link here.
    pub keys: Vec<String>,
    pub refs: Vec<RefLink>,
}

/// Something read from the source and deliberately not imported.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Skipped {
    pub source_id: String,
    pub source: String,
    pub kind: Option<Kind>,
    pub title: String,
    pub reason: String,
}

/// A note for whoever finishes the import.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Warning {
    /// `file:line` in the source.
    pub source: String,
    pub code: &'static str,
    pub message: String,
    /// The written content file, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Source id of the item concerned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
}

impl Warning {
    pub(crate) fn new(
        source: impl Into<String>,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            code,
            message: message.into(),
            path: None,
            item: None,
        }
    }

    pub(crate) fn for_item(
        item: &Item,
        line: Option<usize>,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        let source = match line {
            Some(l) => format!("{}:{l}", file_part(&item.source_ref)),
            None => item.source_ref.clone(),
        };
        Self {
            item: Some(item.source_id.clone()),
            ..Self::new(source, code, message)
        }
    }
}

/// `file` of a `file:line` source reference.
fn file_part(source_ref: &str) -> &str {
    match source_ref.rsplit_once(':') {
        Some((file, line)) if line.chars().all(|c| c.is_ascii_digit()) && !line.is_empty() => file,
        _ => source_ref,
    }
}

/// What a source module hands back.
#[derive(Default)]
pub(crate) struct Parsed {
    pub items: Vec<Item>,
    pub skipped: Vec<Skipped>,
    pub warnings: Vec<Warning>,
    /// The old site's URL (images, aliases, absolute links).
    pub site_url: Option<url::Url>,
    /// Local source: directories root-relative image paths resolve against.
    pub local_roots: Vec<PathBuf>,
    /// Local source: the directory copies must stay inside.
    pub confine_root: Option<PathBuf>,
}

/// What the parsers may read from the command line and the site.
pub(crate) struct ParseCtx<'a> {
    pub config: &'a SiteConfig,
    pub site_url: Option<url::Url>,
    /// How the source path is shown in the report.
    pub display: String,
}

/// Outcome of one item.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ItemRecord {
    pub source_id: String,
    pub source: String,
    pub kind: Option<Kind>,
    pub title: String,
    /// `imported`, `would-import`, `skipped`, `failed`.
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub draft: bool,
}

/// Counts per kind.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Counts {
    pub imported: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// The whole outcome (JSON `data`, the human summary and the report file).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Report {
    pub source: SourceKind,
    pub source_path: String,
    pub dry_run: bool,
    pub summary: BTreeMap<&'static str, Counts>,
    pub images: images::Stats,
    pub items: Vec<ItemRecord>,
    pub warnings: Vec<Warning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
}

pub fn run(args: &ImportArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let root = crate::mcp::find_project_root(&cwd).ok_or_else(|| {
        anyhow::anyhow!(
            "not inside a seite site (no seite.toml in {} or any parent directory). \
             Create one with `seite init <name>`, then run `seite import` inside it",
            cwd.display()
        )
    })?;
    let config = SiteConfig::load(&root.join("seite.toml"))?;
    let source = cwd.join(&args.source);
    if !source.exists() {
        bail!("{} not found", args.source.display());
    }
    let site_url = match &args.site_url {
        Some(u) => Some(
            url::Url::parse(u)
                .ok()
                .filter(|u| matches!(u.scheme(), "http" | "https"))
                .ok_or_else(|| anyhow::anyhow!("--site-url '{u}' is not an http(s) URL"))?,
        ),
        None => None,
    };
    let display = display_path(&cwd, &args.source);
    let ctx = ParseCtx {
        config: &config,
        site_url,
        display: display.clone(),
    };
    let parsed = match args.from {
        SourceKind::Wordpress => wordpress::parse(&source, &ctx)?,
        SourceKind::Ghost => ghost::parse(&source, &ctx)?,
        SourceKind::Jekyll => jekyll::parse(&source, &ctx)?,
        SourceKind::Hugo => hugo::parse(&source, &ctx)?,
    };

    let report = import(&root, &config, args, parsed, display)?;
    report::print_human(&report, args.from);
    crate::output::json::set_data(serde_json::to_value(&report)?);
    Ok(())
}

/// How the source is named in warnings and the report: relative to the
/// working directory when inside it, else just its name (the report never
/// holds absolute paths, so imports from different places compare equal).
fn display_path(cwd: &Path, arg: &Path) -> String {
    let abs = cwd.join(arg);
    match abs.strip_prefix(cwd) {
        Ok(p) if !p.as_os_str().is_empty() => p.display().to_string().replace('\\', "/"),
        _ => abs
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| arg.display().to_string()),
    }
}

/// An item with everything decided.
struct Plan<'c> {
    item: Item,
    collection: &'c CollectionConfig,
    slug: String,
    abs_path: PathBuf,
    rel_path: String,
    url: String,
}

fn import(
    root: &Path,
    config: &SiteConfig,
    args: &ImportArgs,
    parsed: Parsed,
    source_path: String,
) -> anyhow::Result<Report> {
    let paths = config.resolve_paths(root);
    let Parsed {
        items,
        skipped,
        mut warnings,
        site_url,
        local_roots,
        confine_root,
    } = parsed;

    let mut records: Vec<ItemRecord> = skipped
        .into_iter()
        .map(|s| ItemRecord {
            source_id: s.source_id,
            source: s.source,
            kind: s.kind,
            title: s.title,
            status: "skipped",
            reason: Some(s.reason),
            collection: None,
            path: None,
            url: None,
            aliases: Vec::new(),
            draft: false,
        })
        .collect();

    // Drafts.
    let mut wanted = Vec::new();
    for item in items {
        match &item.draft {
            Some(why) if !args.drafts => records.push(ItemRecord {
                source_id: item.source_id.clone(),
                source: item.source_ref.clone(),
                kind: item.kind,
                title: item.title.clone(),
                status: "skipped",
                reason: Some(format!(
                    "{why} (pass --drafts to import it as `draft: true`)"
                )),
                collection: None,
                path: None,
                url: None,
                aliases: Vec::new(),
                draft: true,
            }),
            _ => wanted.push(item),
        }
    }

    // Target collections: must exist.
    let mut missing: Vec<String> = Vec::new();
    let mut targeted: Vec<(&CollectionConfig, Item)> = Vec::new();
    for item in wanted {
        let kind = item.kind.unwrap_or(Kind::Post);
        let name = match kind {
            Kind::Post => args
                .collection
                .clone()
                .or_else(|| item.collection_hint.clone())
                .unwrap_or_else(|| "posts".to_string()),
            Kind::Page => args
                .pages_collection
                .clone()
                .or_else(|| item.collection_hint.clone())
                .unwrap_or_else(|| "pages".to_string()),
        };
        match config::find_collection(&name, &config.collections) {
            Some(c) => targeted.push((c, item)),
            None => {
                if !missing.contains(&name) {
                    missing.push(name);
                }
            }
        }
    }
    if !missing.is_empty() {
        let available: Vec<&str> = config.collections.iter().map(|c| c.name.as_str()).collect();
        bail!(
            "the import needs the collection{} {} but this site has only: {}. \
             Add {} with `seite collection add {}` (or choose another with --collection / --pages-collection)",
            if missing.len() == 1 { "" } else { "s" },
            missing.iter().map(|m| format!("'{m}'")).collect::<Vec<_>>().join(", "),
            if available.is_empty() { "(none)".to_string() } else { available.join(", ") },
            if missing.len() == 1 { "it" } else { "them" },
            missing[0]
        );
    }

    targeted.sort_by(|(ca, a), (cb, b)| {
        (ca.name.as_str(), a.date, a.source_id.as_str()).cmp(&(
            cb.name.as_str(),
            b.date,
            b.source_id.as_str(),
        ))
    });

    // Slugs, filenames, URLs. A slug is taken when another imported item in
    // the collection has it, or when an existing file (other than the one
    // this item would write) already has that URL.
    let today = content::schedule::today(config);
    let mut used: HashMap<&str, HashSet<String>> = HashMap::new();
    let mut existing: HashMap<&str, HashMap<String, PathBuf>> = HashMap::new();
    let mut plans: Vec<Plan> = Vec::new();
    for (collection, mut item) in targeted {
        if collection.has_date && item.date.is_none() {
            item.date = Some(today);
            warnings.push(Warning::for_item(
                &item,
                None,
                "no-date",
                format!("no date in the source; dated with today ({today})"),
            ));
        }
        let base = sanitize_slug(&item.slug)
            .or_else(|| sanitize_slug(&item.title))
            .unwrap_or_else(|| "untitled".to_string());
        let site_urls = existing
            .entry(collection.name.as_str())
            .or_insert_with(|| existing_urls(config, &paths.content, collection));
        let taken = used.entry(collection.name.as_str()).or_default();
        let mut n = 1;
        let (slug, abs_path, url, clash) = loop {
            let slug = if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            };
            n += 1;
            let filename = match (collection.has_date, item.date) {
                (true, Some(d)) => format!("{}-{slug}.md", d.format("%Y-%m-%d")),
                _ => format!("{slug}.md"),
            };
            let abs_path = paths.content.join(&collection.directory).join(&filename);
            let fm = Frontmatter {
                title: item.title.clone(),
                date: item.date.filter(|_| collection.has_date),
                ..Default::default()
            };
            let url = crate::build::resolve_item_location(
                config,
                collection,
                &abs_path,
                Path::new(&filename),
                &fm,
            )
            .url;
            if taken.contains(&slug) {
                continue;
            }
            match site_urls.get(&url) {
                Some(other) if *other != abs_path => continue,
                _ => {}
            }
            let clash = if n == 2 {
                None
            } else if taken.contains(&base) {
                Some("another imported item".to_string())
            } else {
                Some("existing content".to_string())
            };
            break (slug, abs_path, url, clash);
        };
        if let Some(by) = clash {
            warnings.push(Warning::for_item(
                &item,
                None,
                "slug-collision",
                format!(
                    "slug '{base}' is already used by {by} in '{}'; this one became '{slug}'",
                    collection.name
                ),
            ));
        }
        taken.insert(slug.clone());
        let rel_path = abs_path
            .strip_prefix(root)
            .unwrap_or(&abs_path)
            .to_string_lossy()
            .replace('\\', "/");
        plans.push(Plan {
            item,
            collection,
            slug,
            abs_path,
            rel_path,
            url,
        });
    }

    // Cross-references between imported items.
    let mut by_key: HashMap<String, String> = HashMap::new();
    for plan in &plans {
        for key in &plan.item.keys {
            by_key
                .entry(key.clone())
                .or_insert_with(|| plan.url.clone());
        }
    }
    for plan in &mut plans {
        let refs = std::mem::take(&mut plan.item.refs);
        for r in refs {
            let replacement = match by_key.get(&r.key) {
                Some(url) => url.clone(),
                None => {
                    warnings.push(Warning::for_item(
                        &plan.item,
                        Some(r.line),
                        "unresolved-link",
                        format!(
                            "`{}` points at '{}', which was not imported; left as text",
                            r.original, r.key
                        ),
                    ));
                    format!("`{}`", r.original.replace('`', "'"))
                }
            };
            plan.item.body = plan.item.body.replace(&r.token, &replacement);
        }
    }

    let has_categories = config.taxonomies.iter().any(|t| t.name == "categories");
    let mut categories_merged = 0usize;
    let mut authors_dropped: BTreeMap<String, usize> = BTreeMap::new();

    let mut image_ctx = images::Images::new(images::Options {
        site_root: root.to_path_buf(),
        static_dir: paths.static_dir.clone(),
        site_url: site_url.clone(),
        explicit_site_url: args
            .site_url
            .as_deref()
            .and_then(|u| url::Url::parse(u).ok()),
        allow_hosts: args.allow_host.clone(),
        local_roots,
        confine_root,
        enabled: !args.no_images,
        dry_run: args.dry_run,
        force: args.force,
        max_total: args.max_download_mb.saturating_mul(1024 * 1024),
    })?;

    let mut summary: BTreeMap<&'static str, Counts> = BTreeMap::new();
    for kind in [Kind::Post, Kind::Page] {
        summary.insert(kind.plural(), Counts::default());
    }
    for record in &records {
        let kind = record.kind.unwrap_or(Kind::Post).plural();
        summary.entry(kind).or_default().skipped += 1;
    }

    for plan in plans {
        let Plan {
            mut item,
            collection,
            slug,
            abs_path,
            rel_path,
            url,
        } = plan;
        let kind_key = item.kind.unwrap_or(Kind::Post).plural();
        let aliases = clean_aliases(&item.aliases, &url);
        let mut record = ItemRecord {
            source_id: item.source_id.clone(),
            source: item.source_ref.clone(),
            kind: item.kind,
            title: item.title.clone(),
            status: if args.dry_run {
                "would-import"
            } else {
                "imported"
            },
            reason: None,
            collection: Some(collection.name.clone()),
            path: Some(rel_path.clone()),
            url: Some(url.clone()),
            aliases: aliases.clone(),
            draft: item.draft.is_some(),
        };
        if abs_path.exists() && !args.force {
            record.status = "skipped";
            record.reason = Some(format!(
                "{rel_path} already exists (pass --force to overwrite it)"
            ));
            summary.entry(kind_key).or_default().skipped += 1;
            records.push(record);
            continue;
        }

        let first_warning = warnings.len();
        // Images: body references and the featured image.
        let base_dirs = item.base_dirs.clone();
        let item_images = images::ItemImages {
            slug: &slug,
            item_id: &item.source_id,
            source_ref: &item.source_ref,
            base_dirs: &base_dirs,
        };
        item.body = image_ctx.localize_body(&item.body, &item_images, &mut warnings);
        if let Some(image) = item.image.take() {
            item.image = Some(
                image_ctx
                    .localize(&image, &item_images, true, &mut warnings)
                    .unwrap_or(image),
            );
        }

        // Frontmatter.
        let mut tags = item.tags.clone();
        let mut categories = Vec::new();
        if has_categories {
            categories = item.categories.clone();
        } else if !item.categories.is_empty() {
            categories_merged += 1;
            for c in &item.categories {
                if !tags.iter().any(|t| t.eq_ignore_ascii_case(c)) {
                    tags.push(c.clone());
                }
            }
        }
        let author_ok = !collection.strict_fields || collection.fields.contains_key("author");
        let author = match &item.author {
            Some(a) if !author_ok => {
                *authors_dropped.entry(collection.name.clone()).or_default() += 1;
                let _ = a;
                None
            }
            other => other.clone(),
        };
        let yaml = frontmatter_yaml(&FrontmatterParts {
            item: &item,
            collection,
            tags: &tags,
            categories: &categories,
            author: author.as_deref(),
            aliases: &aliases,
        })?;
        let field_schema = schema::Schema::for_site(config, collection);
        for problem in field_schema.check_text(&yaml, 2, Path::new(&rel_path)) {
            let d = problem.diagnostic;
            let mut w = Warning::for_item(
                &item,
                None,
                "schema",
                match &d.hint {
                    Some(h) => format!("{} ({}; hint: {h})", d.message, d.code),
                    None => format!("{} ({})", d.message, d.code),
                },
            );
            w.path = Some(match d.line {
                Some(l) => format!("{rel_path}:{l}"),
                None => rel_path.clone(),
            });
            warnings.push(w);
        }

        let body = convert::tidy_markdown(&item.body);
        let file = if body.is_empty() {
            format!("---\n{yaml}---\n")
        } else {
            format!("---\n{yaml}---\n\n{body}\n")
        };
        for w in &mut warnings[first_warning..] {
            if w.path.is_none() {
                w.path = Some(rel_path.clone());
            }
        }
        if !args.dry_run {
            if let Err(e) = crate::safe_fs::write(root, &abs_path, file) {
                record.status = "failed";
                record.reason = Some(format!("cannot write {rel_path}: {e:#}"));
                summary.entry(kind_key).or_default().failed += 1;
                records.push(record);
                continue;
            }
        }
        summary.entry(kind_key).or_default().imported += 1;
        records.push(record);
    }

    if categories_merged > 0 {
        warnings.push(Warning::new(
            source_path.clone(),
            "categories-as-tags",
            format!(
                "{categories_merged} item(s) had categories; they were added to `tags` because seite.toml has no \
                 `[[taxonomies]] name = \"categories\"` (add it and re-import with --force to keep them separate)"
            ),
        ));
    }
    for (collection, n) in authors_dropped {
        warnings.push(Warning::new(
            source_path.clone(),
            "author-dropped",
            format!(
                "{n} author name(s) not written: collection '{collection}' has `strict_fields = true` and no \
                 `author` field (add `[collections.fields.author]` and re-import with --force)"
            ),
        ));
    }

    // Source warnings point at the file to fix too.
    let written: HashMap<&str, &str> = records
        .iter()
        .filter(|r| matches!(r.status, "imported" | "would-import"))
        .filter_map(|r| Some((r.source_id.as_str(), r.path.as_deref()?)))
        .collect();
    for w in &mut warnings {
        if w.path.is_none() {
            if let Some(path) = w.item.as_deref().and_then(|id| written.get(id)) {
                w.path = Some(path.to_string());
            }
        }
    }

    records.sort_by(|a, b| (a.kind, &a.source_id).cmp(&(b.kind, &b.source_id)));
    warnings.sort();
    warnings.dedup();

    let mut report = Report {
        source: args.from,
        source_path,
        dry_run: args.dry_run,
        summary,
        images: image_ctx.stats(),
        items: records,
        warnings,
        report: None,
    };
    if !args.dry_run {
        let md = report::markdown(&report);
        crate::safe_fs::write(root, &root.join(REPORT_PATH), md)
            .with_context(|| format!("cannot write {REPORT_PATH}"))?;
        report.report = Some(REPORT_PATH.to_string());
    }
    Ok(report)
}

/// URL → file of the collection's existing content (as the build resolves it).
fn existing_urls(
    config: &SiteConfig,
    content_dir: &Path,
    collection: &CollectionConfig,
) -> HashMap<String, PathBuf> {
    let dir = content_dir.join(&collection.directory);
    let mut out = HashMap::new();
    for entry in walkdir::WalkDir::new(&dir)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if path.file_stem().and_then(|s| s.to_str()) == Some("index")
            && path.parent() == Some(dir.as_path())
        {
            continue; // the collection's index page
        }
        let Ok((fm, _)) = content::parse_content_file(path) else {
            continue;
        };
        let rel = path.strip_prefix(&dir).unwrap_or(path);
        let url = crate::build::resolve_item_location(config, collection, path, rel, &fm).url;
        out.insert(url, path.to_path_buf());
    }
    out
}

/// Inputs of [`frontmatter_yaml`].
struct FrontmatterParts<'a> {
    item: &'a Item,
    collection: &'a CollectionConfig,
    tags: &'a [String],
    categories: &'a [String],
    author: Option<&'a str>,
    aliases: &'a [String],
}

/// The frontmatter YAML (without `---` lines), keys in a fixed order.
fn frontmatter_yaml(p: &FrontmatterParts) -> anyhow::Result<String> {
    use serde_yaml_ng::{Mapping, Value};
    let s = |v: &str| Value::String(v.to_string());
    let list = |v: &[String]| Value::Sequence(v.iter().map(|x| s(x)).collect());
    let item = p.item;
    let mut map = Mapping::new();
    map.insert(s("title"), s(&item.title));
    if p.collection.has_date {
        if let Some(d) = item.date {
            map.insert(s("date"), s(&d.format("%Y-%m-%d").to_string()));
        }
    }
    if let Some(u) = item
        .updated
        .filter(|u| Some(*u) > item.date || !p.collection.has_date)
    {
        map.insert(s("updated"), s(&u.format("%Y-%m-%d").to_string()));
    }
    if let Some(d) = item.description.as_deref().filter(|d| !d.trim().is_empty()) {
        map.insert(s("description"), s(d.trim()));
    }
    if let Some(i) = item.image.as_deref().filter(|i| !i.trim().is_empty()) {
        map.insert(s("image"), s(i));
    }
    if !p.tags.is_empty() {
        map.insert(s("tags"), list(p.tags));
    }
    if !p.categories.is_empty() {
        map.insert(s("categories"), list(p.categories));
    }
    if let Some(series) = &item.series {
        map.insert(s("series"), s(series));
    }
    if let Some(w) = item.weight {
        map.insert(s("weight"), Value::Number(w.into()));
    }
    if item.draft.is_some() {
        map.insert(s("draft"), Value::Bool(true));
    }
    if let Some(e) = item.expires {
        map.insert(s("expires"), s(&e.format("%Y-%m-%d").to_string()));
    }
    if !p.aliases.is_empty() {
        map.insert(s("aliases"), list(p.aliases));
    }
    if let Some(a) = p.author {
        let mut extra = Mapping::new();
        extra.insert(s("author"), s(a));
        map.insert(s("extra"), Value::Mapping(extra));
    }
    Ok(serde_yaml_ng::to_string(&map)?)
}

/// A filename slug: percent-decoded, transliterated, lowercase, `-`-joined.
pub(crate) fn sanitize_slug(raw: &str) -> Option<String> {
    let decoded = urlencoding::decode(raw.trim())
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| raw.to_string());
    let slug = content::slug_from_title(&decoded);
    // `validate_slug` rules: starts with a letter or digit, [a-z0-9_-] only.
    (!slug.is_empty() && content::create::validate_slug(&slug).is_ok()).then_some(slug)
}

/// Old URLs worth a redirect: root-relative paths without `..`, query or
/// fragment, `index.html` / `.html` removed (the redirect page is written
/// as `<alias>.html`), not equal to the new URL, deduplicated in order.
pub(crate) fn clean_aliases(aliases: &[String], new_url: &str) -> Vec<String> {
    let key = |p: &str| p.trim_matches('/').to_ascii_lowercase();
    let new_key = key(new_url);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for alias in aliases {
        let Some(path) = normalize_alias(alias) else {
            continue;
        };
        let k = key(&path);
        if k.is_empty() || k == new_key || !seen.insert(k) {
            continue;
        }
        out.push(path);
    }
    out
}

fn normalize_alias(alias: &str) -> Option<String> {
    let alias = alias.trim();
    let path = match url::Url::parse(alias) {
        Ok(u) if matches!(u.scheme(), "http" | "https") => u.path().to_string(),
        Ok(_) => return None,
        Err(_) => alias.split(['?', '#']).next().unwrap_or("").to_string(),
    };
    let decoded = urlencoding::decode(&path)
        .map(|c| c.into_owned())
        .unwrap_or(path);
    let mut p = format!("/{}", decoded.trim_start_matches('/'));
    for suffix in ["/index.html", "/index.htm", ".html", ".htm"] {
        if let Some(stripped) = p.strip_suffix(suffix) {
            p = if stripped.is_empty() {
                "/".to_string()
            } else {
                stripped.to_string()
            };
            break;
        }
    }
    if p.contains("..")
        || p.contains('\\')
        || p.chars().any(char::is_control)
        || p.contains(['"', '<', '>', ' '])
    {
        return None;
    }
    Some(p)
}

/// Split comma/space separated term strings and lists into clean terms.
pub(crate) fn terms_from_json(
    value: Option<&serde_json::Value>,
    split_spaces: bool,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: &str| {
        let s = s.trim();
        if !s.is_empty() && !out.iter().any(|x: &String| x.eq_ignore_ascii_case(s)) {
            out.push(s.to_string());
        }
    };
    match value {
        Some(serde_json::Value::Array(items)) => {
            for v in items {
                match v {
                    serde_json::Value::String(s) => push(s),
                    serde_json::Value::Number(n) => push(&n.to_string()),
                    _ => {}
                }
            }
        }
        Some(serde_json::Value::String(s)) => {
            if split_spaces {
                s.split([' ', ',']).for_each(&mut push);
            } else {
                s.split(',').for_each(&mut push);
            }
        }
        Some(serde_json::Value::Number(n)) => push(&n.to_string()),
        _ => {}
    }
    out
}

/// A string-ish frontmatter value.
pub(crate) fn json_str(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Split a Markdown/HTML file into (frontmatter value, body, body's first
/// line). Supports YAML `---`, TOML `+++` and JSON `{` frontmatter.
pub(crate) fn split_frontmatter(
    text: &str,
) -> Result<Option<(serde_json::Value, &str, usize)>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let delimited = |delim: &str| -> Option<(&str, &str, usize)> {
        let rest = text.strip_prefix(delim)?;
        let rest = rest
            .strip_prefix("\r\n")
            .or_else(|| rest.strip_prefix('\n'))?;
        let mut offset = 0;
        for line in rest.split_inclusive('\n') {
            if line.trim_end() == delim {
                let fm = &rest[..offset];
                let body = &rest[offset + line.len()..];
                let body_line = convert::line_of(text, text.len() - body.len());
                return Some((fm, body, body_line));
            }
            offset += line.len();
        }
        None
    };
    if text.starts_with("---") {
        let Some((fm, body, line)) = delimited("---") else {
            return Err("frontmatter starts with `---` but has no closing `---`".into());
        };
        let value: serde_json::Value = if fm.trim().is_empty() {
            serde_json::Value::Object(Default::default())
        } else {
            serde_yaml_ng::from_str(fm).map_err(|e| format!("invalid YAML frontmatter: {e}"))?
        };
        return Ok(Some((value, body, line)));
    }
    if text.starts_with("+++") {
        let Some((fm, body, line)) = delimited("+++") else {
            return Err("frontmatter starts with `+++` but has no closing `+++`".into());
        };
        let value: toml::Value =
            toml::from_str(fm).map_err(|e| format!("invalid TOML frontmatter: {e}"))?;
        return Ok(Some((toml_to_json(value), body, line)));
    }
    if text.starts_with('{') {
        let mut stream = serde_json::Deserializer::from_str(text).into_iter::<serde_json::Value>();
        return match stream.next() {
            Some(Ok(value)) => {
                let end = stream.byte_offset();
                let body = text[end..].trim_start_matches(['\r', '\n']);
                let line = convert::line_of(text, text.len() - body.len());
                Ok(Some((value, body, line)))
            }
            Some(Err(e)) => Err(format!("invalid JSON frontmatter: {e}")),
            None => Ok(None),
        };
    }
    Ok(None)
}

/// TOML → JSON, dates as their string form.
pub(crate) fn toml_to_json(value: toml::Value) -> serde_json::Value {
    use serde_json::Value as J;
    match value {
        toml::Value::String(s) => J::String(s),
        toml::Value::Integer(i) => J::Number(i.into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(f).map_or(J::Null, J::Number),
        toml::Value::Boolean(b) => J::Bool(b),
        toml::Value::Datetime(d) => J::String(d.to_string()),
        toml::Value::Array(a) => J::Array(a.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => {
            J::Object(t.into_iter().map(|(k, v)| (k, toml_to_json(v))).collect())
        }
    }
}

/// Read an export file, refusing files over [`MAX_EXPORT_BYTES`].
pub(crate) fn read_export(path: &Path, display: &str) -> anyhow::Result<String> {
    let meta = std::fs::metadata(path).with_context(|| format!("cannot read {display}"))?;
    if !meta.is_file() {
        bail!("{display} is not a file");
    }
    if meta.len() > MAX_EXPORT_BYTES {
        bail!(
            "{display} is {} MB; exports over {} MB are not supported (split the export)",
            meta.len() / (1024 * 1024),
            MAX_EXPORT_BYTES / (1024 * 1024)
        );
    }
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {display}"))?;
    let text = String::from_utf8(bytes).map_err(|e| {
        let offset = e.utf8_error().valid_up_to();
        let line = e.as_bytes()[..offset]
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            + 1;
        anyhow::anyhow!("{display}:{line}: not valid UTF-8")
    })?;
    Ok(text
        .strip_prefix('\u{feff}')
        .map(str::to_string)
        .unwrap_or(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_aliases() {
        let aliases = vec![
            "https://old.example/2020/01/02/hello.html".to_string(),
            "/posts/hello/".to_string(),
            "/2020/01/02/hello/".to_string(),
            "/../etc".to_string(),
            "/about/index.html".to_string(),
            "/?p=12".to_string(),
        ];
        assert_eq!(
            clean_aliases(&aliases, "/posts/hello"),
            vec!["/2020/01/02/hello".to_string(), "/about".to_string()]
        );
    }

    #[test]
    fn test_sanitize_slug() {
        assert_eq!(
            sanitize_slug("Hello World!").as_deref(),
            Some("hello-world")
        );
        assert_eq!(
            sanitize_slug("%e6%97%a5%e6%9c%ac").as_deref(),
            Some("ri-ben")
        );
        assert_eq!(sanitize_slug("!!!"), None);
    }

    #[test]
    fn test_split_frontmatter_formats() {
        let (v, body, line) =
            split_frontmatter("+++\ntitle = \"T\"\ndate = 2020-01-02T10:00:00+01:00\n+++\nBody\n")
                .unwrap()
                .unwrap();
        assert_eq!(v["title"], "T");
        assert_eq!(v["date"], "2020-01-02T10:00:00+01:00");
        assert_eq!((body, line), ("Body\n", 5));
        let (v, body, _) = split_frontmatter("---\ntitle: Y\n---\nB").unwrap().unwrap();
        assert_eq!((v["title"].as_str(), body), (Some("Y"), "B"));
        let (v, body, _) = split_frontmatter("{\"title\": \"J\"}\n\nB")
            .unwrap()
            .unwrap();
        assert_eq!((v["title"].as_str(), body), (Some("J"), "B"));
        assert!(split_frontmatter("---\ntitle: [\n---\n").is_err());
        assert!(split_frontmatter("no frontmatter").unwrap().is_none());
    }

    #[test]
    fn test_terms_from_json() {
        let v = serde_json::json!("rust web, Rust");
        assert_eq!(terms_from_json(Some(&v), true), vec!["rust", "web"]);
        let v = serde_json::json!(["A b", 2]);
        assert_eq!(terms_from_json(Some(&v), true), vec!["A b", "2"]);
    }
}
