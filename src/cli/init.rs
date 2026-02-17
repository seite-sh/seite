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
    fs::create_dir_all(root.join(".claude"))?;

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

    // Write Claude Code settings (.claude/settings.json)
    fs::write(
        root.join(".claude/settings.json"),
        generate_claude_settings(),
    )?;

    // Write CLAUDE.md with site-specific context
    fs::write(
        root.join("CLAUDE.md"),
        generate_claude_md(&title, &description, &collections),
    )?;

    human::success(&format!("Created new site in '{name}'"));
    human::info("Next steps:");
    println!("  cd {name}");
    println!("  page build");
    println!("  page serve");

    Ok(())
}

/// Generate .claude/settings.json with pre-approved tools for the page workflow.
fn generate_claude_settings() -> String {
    r#"{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "permissions": {
    "allow": [
      "Read",
      "Write(content/**)",
      "Write(templates/**)",
      "Write(static/**)",
      "Edit(content/**)",
      "Edit(templates/**)",
      "Bash(page build:*)",
      "Bash(page build)",
      "Bash(page new:*)",
      "Bash(page serve:*)",
      "Bash(page theme:*)",
      "Glob",
      "Grep",
      "WebSearch"
    ],
    "deny": [
      "Read(.env)",
      "Read(.env.*)"
    ]
  }
}
"#
    .to_string()
}

/// Generate a CLAUDE.md tailored to the site's collections and structure.
fn generate_claude_md(
    title: &str,
    description: &str,
    collections: &[CollectionConfig],
) -> String {
    let mut md = String::with_capacity(2048);

    // Header
    md.push_str(&format!("# {title}\n\n"));
    if !description.is_empty() {
        md.push_str(&format!("{description}\n\n"));
    }
    md.push_str("This is a static site built with the `page` CLI tool.\n\n");

    // Quick commands
    md.push_str("## Quick Commands\n\n");
    md.push_str("```bash\n");
    md.push_str("page build              # Build the site\n");
    md.push_str("page build --drafts     # Build including draft content\n");
    md.push_str("page serve              # Start dev server with live reload\n");
    for c in collections {
        md.push_str(&format!(
            "page new {} \"Title\"     # Create new {}\n",
            singularize(&c.name),
            singularize(&c.name),
        ));
    }
    md.push_str("page theme list         # List available themes\n");
    md.push_str("page theme apply <name> # Apply a bundled theme\n");
    md.push_str("page agent              # Start an AI agent session\n");
    md.push_str("```\n\n");

    // Project structure
    md.push_str("## Project Structure\n\n");
    md.push_str("```\n");
    for c in collections {
        md.push_str(&format!("content/{}/    # {} content (markdown + YAML frontmatter)\n", c.directory, c.label));
    }
    md.push_str("templates/       # Tera (Jinja2-compatible) HTML templates\n");
    md.push_str("static/          # Static assets (copied as-is to dist/)\n");
    md.push_str("dist/            # Build output (generated, do not edit)\n");
    md.push_str("page.toml        # Site configuration\n");
    md.push_str("```\n\n");

    // Collections
    md.push_str("## Collections\n\n");
    for c in collections {
        md.push_str(&format!("### {}\n", c.label));
        md.push_str(&format!("- Directory: `content/{}/`\n", c.directory));
        md.push_str(&format!("- URL prefix: `{}`\n", c.url_prefix));
        md.push_str(&format!("- Template: `{}`\n", c.default_template));
        if c.has_date {
            md.push_str("- Date-based: yes (filename format: `YYYY-MM-DD-slug.md`)\n");
        } else {
            md.push_str("- Date-based: no (filename format: `slug.md`)\n");
        }
        if c.nested {
            md.push_str("- Supports nested directories (e.g., `section/slug.md`)\n");
        }
        md.push('\n');
    }

    // Content format
    md.push_str("## Content Format\n\n");
    md.push_str("Content files are markdown with YAML frontmatter:\n\n");
    md.push_str("```yaml\n");
    md.push_str("---\n");
    md.push_str("title: \"Post Title\"\n");
    if collections.iter().any(|c| c.has_date) {
        md.push_str("date: 2025-01-15        # required for dated collections\n");
    }
    md.push_str("description: \"Optional\"  # optional summary\n");
    md.push_str("tags:                     # optional\n");
    md.push_str("  - tag1\n");
    md.push_str("  - tag2\n");
    md.push_str("draft: true              # optional, hides from default build\n");
    md.push_str("---\n\n");
    md.push_str("Markdown content here.\n");
    md.push_str("```\n\n");

    // Key conventions
    md.push_str("## Key Conventions\n\n");
    md.push_str("- Run `page build` after creating or editing content to regenerate the site\n");
    md.push_str("- Set `draft: true` in frontmatter to exclude content from the default build\n");
    md.push_str("- URLs are clean (no extension): `/posts/hello-world`\n");
    md.push_str("- Templates use Tera syntax and extend `base.html`\n");
    md.push_str("- Each content file produces both HTML and markdown output\n");
    md.push_str("- The site generates `llms.txt` and `llms-full.txt` for LLM discoverability\n");

    md
}

/// Convert a plural collection name to singular for display.
fn singularize(name: &str) -> &str {
    match name {
        "posts" => "post",
        "docs" => "doc",
        "pages" => "page",
        _ => name,
    }
}
