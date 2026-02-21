use std::fs;
use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::config::CollectionConfig;
use crate::output::human;
use crate::workspace::{self, WorkspaceConfig};

#[derive(Args)]
pub struct WorkspaceArgs {
    #[command(subcommand)]
    pub command: WorkspaceCommand,
}

#[derive(Subcommand)]
pub enum WorkspaceCommand {
    /// Initialize a new workspace
    Init(WorkspaceInitArgs),

    /// List sites in the workspace
    List,

    /// Add a new site to the workspace
    Add(WorkspaceAddArgs),

    /// Show workspace status
    Status,
}

#[derive(Args)]
pub struct WorkspaceInitArgs {
    /// Workspace name
    pub name: Option<String>,
}

#[derive(Args)]
pub struct WorkspaceAddArgs {
    /// Site name
    pub name: String,

    /// Path to the site directory (relative to workspace root)
    #[arg(long)]
    pub path: Option<String>,

    /// Site title
    #[arg(long)]
    pub title: Option<String>,

    /// Collections to include (comma-separated: posts,docs,pages)
    #[arg(long)]
    pub collections: Option<String>,
}

pub fn run(args: &WorkspaceArgs) -> anyhow::Result<()> {
    match &args.command {
        WorkspaceCommand::Init(init_args) => run_init(init_args),
        WorkspaceCommand::List => run_list(),
        WorkspaceCommand::Add(add_args) => run_add(add_args),
        WorkspaceCommand::Status => run_status(),
    }
}

fn run_init(args: &WorkspaceInitArgs) -> anyhow::Result<()> {
    let name = match &args.name {
        Some(n) => n.clone(),
        None => dialoguer::Input::<String>::new()
            .with_prompt("Workspace name")
            .interact_text()?,
    };

    let ws_file = PathBuf::from("seite-workspace.toml");
    if ws_file.exists() {
        anyhow::bail!("seite-workspace.toml already exists in this directory");
    }

    // Create workspace structure
    fs::create_dir_all("sites")?;
    fs::create_dir_all("data")?;
    fs::create_dir_all("static")?;
    fs::create_dir_all("templates")?;

    // Write workspace config
    let config_content = format!(
        r#"[workspace]
name = "{name}"

# Shared resources available to all sites
# shared_data = "data"
# shared_static = "static"
# shared_templates = "templates"

# Add sites below. Each site needs its own seite.toml.
# [[sites]]
# name = "blog"
# path = "sites/blog"

# Cross-site features (uncomment to enable)
# [cross_site]
# unified_sitemap = true
# cross_site_links = true
# unified_search = false
"#
    );
    fs::write(&ws_file, config_content)?;

    human::success(&format!("Initialized workspace '{name}'"));
    human::info("  Created seite-workspace.toml");
    human::info("  Created sites/, data/, static/, templates/ directories");
    human::info("");
    human::info("Next steps:");
    human::info("  1. Add a site: seite workspace add blog --collections posts,pages");
    human::info("  2. Or move existing sites into sites/");

    Ok(())
}

fn run_list() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let ws_root = workspace::find_workspace_root(&cwd)
        .ok_or_else(|| anyhow::anyhow!("not in a workspace (no seite-workspace.toml found)"))?;

    let ws_config = WorkspaceConfig::load(&ws_root.join("seite-workspace.toml"))?;

    human::header(&format!("Workspace: {}", ws_config.workspace.name));
    for site in &ws_config.sites {
        let site_root = ws_root.join(&site.path);
        let status = if site_root.join("seite.toml").exists() {
            console::style("ok").green().to_string()
        } else {
            console::style("missing seite.toml").red().to_string()
        };
        human::info(&format!(
            "  {} ({}) [{}]",
            console::style(&site.name).bold(),
            site.path,
            status
        ));
        if let Some(ref base_url) = site.base_url {
            human::info(&format!("    base_url: {base_url}"));
        }
    }

    Ok(())
}

fn run_add(args: &WorkspaceAddArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let ws_root = workspace::find_workspace_root(&cwd)
        .ok_or_else(|| anyhow::anyhow!("not in a workspace (no seite-workspace.toml found)"))?;

    let site_path = args
        .path
        .clone()
        .unwrap_or_else(|| format!("sites/{}", args.name));

    let site_dir = ws_root.join(&site_path);

    // Check if site already exists in workspace config
    let ws_config_path = ws_root.join("seite-workspace.toml");
    let contents = fs::read_to_string(&ws_config_path)?;
    if contents.contains(&format!("name = \"{}\"", args.name)) {
        anyhow::bail!("site '{}' already exists in the workspace", args.name);
    }

    // Create site directory structure
    let title = args.title.clone().unwrap_or_else(|| args.name.clone());
    let collections_str = args.collections.as_deref().unwrap_or("posts,pages");
    let collections: Vec<&str> = collections_str.split(',').map(|s| s.trim()).collect();

    fs::create_dir_all(&site_dir)?;

    // Create content directories
    for col in &collections {
        fs::create_dir_all(site_dir.join("content").join(col))?;
    }
    fs::create_dir_all(site_dir.join("templates"))?;
    fs::create_dir_all(site_dir.join("static"))?;

    // Generate seite.toml for the site
    let mut collections_toml = String::new();
    for col in &collections {
        let preset = match *col {
            "posts" | "post" => CollectionConfig::preset_posts(),
            "docs" | "doc" => CollectionConfig::preset_docs(),
            _ => CollectionConfig::preset_pages(),
        };
        collections_toml.push_str(&format!(
            r#"
[[collections]]
name = "{}"
label = "{}"
directory = "{}"
has_date = {}
has_rss = {}
listed = {}
nested = {}
url_prefix = "{}"
default_template = "{}"
"#,
            preset.name,
            preset.label,
            preset.directory,
            preset.has_date,
            preset.has_rss,
            preset.listed,
            preset.nested,
            preset.url_prefix,
            preset.default_template,
        ));
    }

    let page_toml = format!(
        r#"[site]
title = "{title}"
description = ""
base_url = "http://localhost:3000/{name}"
language = "en"
author = ""
{collections_toml}
[build]
output_dir = "dist"

[deploy]
target = "github-pages"
"#,
        name = args.name,
    );

    fs::write(site_dir.join("seite.toml"), page_toml)?;

    // Append site entry to workspace config
    let site_entry = format!(
        r#"
[[sites]]
name = "{}"
path = "{}"
"#,
        args.name, site_path,
    );
    let mut ws_contents = fs::read_to_string(&ws_config_path)?;
    ws_contents.push_str(&site_entry);
    fs::write(&ws_config_path, ws_contents)?;

    human::success(&format!("Added site '{}' at {}", args.name, site_path));
    human::info(&format!("  Collections: {collections_str}"));
    human::info(&format!("  Config: {}/seite.toml", site_path));

    Ok(())
}

fn run_status() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let ws_root = workspace::find_workspace_root(&cwd)
        .ok_or_else(|| anyhow::anyhow!("not in a workspace (no seite-workspace.toml found)"))?;

    let ws_config = WorkspaceConfig::load(&ws_root.join("seite-workspace.toml"))?;

    human::header(&format!("Workspace: {}", ws_config.workspace.name));
    human::info(&format!("  Root: {}", ws_root.display()));
    human::info(&format!("  Sites: {}", ws_config.sites.len()));

    if let Some(ref data) = ws_config.workspace.shared_data {
        human::info(&format!("  Shared data: {data}"));
    }
    if let Some(ref static_dir) = ws_config.workspace.shared_static {
        human::info(&format!("  Shared static: {static_dir}"));
    }
    if let Some(ref templates) = ws_config.workspace.shared_templates {
        human::info(&format!("  Shared templates: {templates}"));
    }

    println!();
    for site in &ws_config.sites {
        let site_root = ws_root.join(&site.path);
        let has_config = site_root.join("seite.toml").exists();
        let has_dist = site_root.join("dist").exists();

        let config_status = if has_config {
            console::style("ok").green()
        } else {
            console::style("missing").red()
        };
        let build_status = if has_dist {
            console::style("built").green()
        } else {
            console::style("not built").yellow()
        };

        human::info(&format!(
            "  {} — config: {}, build: {}",
            console::style(&site.name).bold(),
            config_status,
            build_status
        ));

        if has_config {
            if let Ok(config) = crate::config::SiteConfig::load(&site_root.join("seite.toml")) {
                human::info(&format!("    title: {}", config.site.title));
                human::info(&format!("    base_url: {}", config.site.base_url));
                let collections: Vec<&str> =
                    config.collections.iter().map(|c| c.name.as_str()).collect();
                human::info(&format!("    collections: {}", collections.join(", ")));
            }
        }
    }

    Ok(())
}
