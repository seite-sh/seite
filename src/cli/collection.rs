use std::fs;
use std::path::PathBuf;

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

fn run_add(args: &AddArgs) -> anyhow::Result<()> {
    let config_path = PathBuf::from("page.toml");
    let site_config = SiteConfig::load(&config_path)?;

    // Check if collection already exists
    if site_config.collections.iter().any(|c| c.name == args.name) {
        anyhow::bail!(
            "collection '{}' already exists in page.toml",
            args.name
        );
    }

    // Resolve preset
    let preset = CollectionConfig::from_preset(&args.name).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown collection preset '{}'. Available: posts, docs, pages, changelog, roadmap, trust",
            args.name
        )
    })?;

    // Create content directory
    let paths = site_config.resolve_paths(&std::env::current_dir()?);
    let content_dir = paths.content.join(&preset.directory);
    fs::create_dir_all(&content_dir)?;

    // Append collection to page.toml using toml table manipulation
    let contents = fs::read_to_string(&config_path)?;
    let mut doc: toml::Table = contents.parse().map_err(|e: toml::de::Error| {
        anyhow::anyhow!("failed to parse page.toml: {}", e)
    })?;

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

    human::success(&format!(
        "Added '{}' collection to page.toml",
        args.name
    ));
    human::info(&format!(
        "Content directory: {}",
        content_dir.display()
    ));
    human::info(&format!(
        "Create content with: page new {} \"My Title\"",
        args.name
    ));

    Ok(())
}

fn run_list() -> anyhow::Result<()> {
    let config_path = PathBuf::from("page.toml");
    let site_config = SiteConfig::load(&config_path)?;

    if site_config.collections.is_empty() {
        human::info("No collections configured.");
        return Ok(());
    }

    println!(
        "{:<12} {:<12} {:<6} {:<6} {:<8} {:<8} URL PREFIX",
        "NAME", "DIRECTORY", "DATED", "RSS", "LISTED", "NESTED"
    );
    println!("{}", "-".repeat(70));

    for c in &site_config.collections {
        println!(
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
