use std::fs;
use std::path::PathBuf;

use clap::Args;

use crate::config::{CollectionConfig, DeployTarget};
use crate::content;
use crate::output::human;
use crate::templates;

#[derive(Args)]
pub struct InitArgs {
    /// Name of the site / directory to create
    pub name: Option<String>,

    /// Site title
    #[arg(long)]
    pub title: Option<String>,

    /// Site description
    #[arg(long)]
    pub description: Option<String>,

    /// Deploy target (github-pages, cloudflare)
    #[arg(long)]
    pub deploy_target: Option<String>,

    /// Collections to include (comma-separated: posts,docs,pages)
    #[arg(long)]
    pub collections: Option<String>,
}

pub fn run(args: &InitArgs) -> anyhow::Result<()> {
    let name = match &args.name {
        Some(n) => n.clone(),
        None => dialoguer::Input::<String>::new()
            .with_prompt("Site name (directory)")
            .interact_text()?,
    };

    let title = match &args.title {
        Some(t) => t.clone(),
        None => dialoguer::Input::<String>::new()
            .with_prompt("Site title")
            .default(name.clone())
            .interact_text()?,
    };

    let description = match &args.description {
        Some(d) => d.clone(),
        None => dialoguer::Input::<String>::new()
            .with_prompt("Site description")
            .default(String::new())
            .allow_empty(true)
            .interact_text()?,
    };

    let deploy_target = match &args.deploy_target {
        Some(t) => t.clone(),
        None => {
            let options = ["github-pages", "cloudflare"];
            let selection = dialoguer::Select::new()
                .with_prompt("Deploy target")
                .items(&options)
                .default(0)
                .interact()?;
            options[selection].to_string()
        }
    };

    // Resolve collections
    let collections: Vec<CollectionConfig> = match &args.collections {
        Some(list) => list
            .split(',')
            .filter_map(|name| CollectionConfig::from_preset(name.trim()))
            .collect(),
        None => {
            let preset_names = ["posts", "docs", "pages"];
            let defaults = &[true, false, true]; // posts + pages on by default
            let selections = dialoguer::MultiSelect::new()
                .with_prompt("Collections to include")
                .items(&preset_names)
                .defaults(defaults)
                .interact()?;
            selections
                .into_iter()
                .filter_map(|i| CollectionConfig::from_preset(preset_names[i]))
                .collect()
        }
    };

    if collections.is_empty() {
        anyhow::bail!("at least one collection is required");
    }

    let root = PathBuf::from(&name);
    if root.exists() {
        anyhow::bail!("directory '{}' already exists", name);
    }

    // Create directory structure per collection
    for c in &collections {
        fs::create_dir_all(root.join("content").join(&c.directory))?;
    }
    fs::create_dir_all(root.join("templates"))?;
    fs::create_dir_all(root.join("static"))?;

    // Generate page.toml
    let target = match deploy_target.as_str() {
        "cloudflare" => DeployTarget::Cloudflare,
        _ => DeployTarget::GithubPages,
    };
    let config = crate::config::SiteConfig {
        site: crate::config::SiteSection {
            title: title.clone(),
            description: description.clone(),
            base_url: "http://localhost:3000".into(),
            language: "en".into(),
            author: String::new(),
        },
        collections: collections.clone(),
        build: Default::default(),
        deploy: crate::config::DeploySection {
            target,
            repo: None,
            project: None,
        },
        ai: Default::default(),
    };
    let toml_str = toml::to_string_pretty(&config)?;
    fs::write(root.join("page.toml"), toml_str)?;

    // Write default templates
    fs::write(root.join("templates/base.html"), templates::default_base())?;
    fs::write(root.join("templates/index.html"), templates::DEFAULT_INDEX)?;
    for c in &collections {
        let tmpl_name = &c.default_template;
        let content = match tmpl_name.as_str() {
            "post.html" => templates::DEFAULT_POST,
            "doc.html" => templates::DEFAULT_DOC,
            "page.html" => templates::DEFAULT_PAGE,
            _ => continue,
        };
        fs::write(root.join("templates").join(tmpl_name), content)?;
    }

    // Create sample hello-world post if posts collection is included
    if collections.iter().any(|c| c.name == "posts") {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let fm = content::Frontmatter {
            title: "Hello World".into(),
            date: Some(chrono::Local::now().date_naive()),
            description: Some("Welcome to your new site!".into()),
            tags: vec!["intro".into()],
            draft: false,
            ..Default::default()
        };
        let frontmatter_str = content::generate_frontmatter(&fm);
        let post_content = format!(
            "{frontmatter_str}\n\nWelcome to your new site built with **page**.\n\nEdit this post or create new ones with `page new post \"My Post\"`.\n"
        );
        fs::write(
            root.join(format!("content/posts/{today}-hello-world.md")),
            post_content,
        )?;
    }

    human::success(&format!("Created new site in '{name}'"));
    human::info("Next steps:");
    println!("  cd {name}");
    println!("  page build");
    println!("  page serve");

    Ok(())
}
