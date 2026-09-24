use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::config::{CollectionConfig, SiteConfig};
use crate::output::human;

#[derive(Args)]
pub struct CollectionArgs {
    #[command(subcommand)]
    pub command: CollectionCommand,
}

#[derive(Subcommand)]
pub enum CollectionCommand {
    /// Add a collection to the current site
    Add(AddArgs),
    /// List collections in the current site
    List,
}

#[derive(Args)]
pub struct AddArgs {
    /// Collection preset name (posts, docs, pages, changelog, roadmap, trust)
    pub name: String,
}

pub fn run(args: &CollectionArgs) -> anyhow::Result<()> {
    match &args.command {
        CollectionCommand::Add(add_args) => run_add(add_args),
        CollectionCommand::List => run_list(),
    }
}

/// Collection presets accepted by `seite collection add`.
pub const PRESET_NAMES: &[&str] = &["posts", "docs", "pages", "changelog", "roadmap", "trust"];

/// A collection added by [`add_preset_collection`].
pub struct AddedCollection {
    pub collection: CollectionConfig,
    /// The collection's content directory (created if missing).
    pub content_dir: PathBuf,
}

fn run_add(args: &AddArgs) -> anyhow::Result<()> {
    let added = add_preset_collection(&std::env::current_dir()?, &args.name)?;
    let content_dir = added.content_dir;
    let preset = added.collection;

    human::success(&format!("Added '{}' collection to seite.toml", args.name));
    crate::output::json::set_data(serde_json::json!({
        "added": args.name,
        "content_dir": content_dir.display().to_string(),
        "collection": preset,
    }));
    human::info(&format!("Content directory: {}", content_dir.display()));
    human::info(&format!(
        "Create content with: seite new {} \"My Title\"",
        args.name
    ));

    Ok(())
}

/// Add the preset collection `name` to `root/seite.toml` and create its
/// content directory. Shared by `seite collection add` and the MCP server.
pub fn add_preset_collection(root: &Path, name: &str) -> anyhow::Result<AddedCollection> {
    let config_path = root.join("seite.toml");
    let site_config = SiteConfig::load(&config_path)?;

    // Check if collection already exists
    if site_config.collections.iter().any(|c| c.name == name) {
        anyhow::bail!("collection '{}' already exists in seite.toml", name);
    }

    // Resolve preset
    let preset = CollectionConfig::from_preset(name).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown collection preset '{}'. Available: {}",
            name,
            PRESET_NAMES.join(", ")
        )
    })?;

    // Create content directory
    let paths = site_config.resolve_paths(root);
    let content_dir = paths.content.join(&preset.directory);
    fs::create_dir_all(&content_dir)?;

    // Append collection to seite.toml using toml table manipulation
    let contents = fs::read_to_string(&config_path)?;
    let mut doc: toml::Table = contents
        .parse()
        .map_err(|e: toml::de::Error| anyhow::anyhow!("failed to parse seite.toml: {}", e))?;

    // Get or create the collections array
    let collections = doc
        .entry("collections")
        .or_insert_with(|| toml::Value::Array(Vec::new()));

    if let toml::Value::Array(arr) = collections {
        let collection_value = toml::Value::try_from(&preset)?;
        arr.push(collection_value);
    }

    let new_contents = toml::to_string_pretty(&doc)?;
    fs::write(&config_path, new_contents)?;

    Ok(AddedCollection {
        collection: preset,
        content_dir,
    })
}

fn run_list() -> anyhow::Result<()> {
    let config_path = PathBuf::from("seite.toml");
    let site_config = SiteConfig::load(&config_path)?;
    crate::output::json::set_data(serde_json::json!({
        "collections": site_config.collections,
    }));

    if site_config.collections.is_empty() {
        human::info("No collections configured.");
        return Ok(());
    }

    crate::human_println!(
        "{:<12} {:<12} {:<6} {:<6} {:<8} {:<8} URL PREFIX",
        "NAME",
        "DIRECTORY",
        "DATED",
        "RSS",
        "LISTED",
        "NESTED"
    );
    crate::human_println!("{}", "-".repeat(70));

    for c in &site_config.collections {
        crate::human_println!(
            "{:<12} {:<12} {:<6} {:<6} {:<8} {:<8} {}",
            c.name,
            c.directory,
            if c.has_date { "yes" } else { "no" },
            if c.has_rss { "yes" } else { "no" },
            if c.listed { "yes" } else { "no" },
            if c.nested { "yes" } else { "no" },
            if c.url_prefix.is_empty() {
                "(none)"
            } else {
                &c.url_prefix
            },
        );
    }

    Ok(())
}
