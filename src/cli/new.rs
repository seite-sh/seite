use std::fs;
use std::path::PathBuf;

use clap::Args;

use crate::config::{self, SiteConfig};
use crate::content::{self, Frontmatter};
use crate::output::human;

#[derive(Args)]
pub struct NewArgs {
    /// Collection name (e.g., post, doc, page)
    pub collection: String,

    /// Title of the content
    pub title: String,

    /// Tags (comma-separated)
    #[arg(short, long)]
    pub tags: Option<String>,

    /// Mark as draft
    #[arg(long)]
    pub draft: bool,

    /// Language code (e.g., es, fr). Appends language suffix to filename.
    /// Only needed for non-default language translations.
    #[arg(long)]
    pub lang: Option<String>,
}

pub fn run(args: &NewArgs) -> anyhow::Result<()> {
    let site_config = SiteConfig::load(&PathBuf::from("seite.toml"))?;
    let paths = site_config.resolve_paths(&std::env::current_dir()?);

    let collection = config::find_collection(&args.collection, &site_config.collections)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "unknown collection '{}'. Available: {}",
                args.collection,
                site_config
                    .collections
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

    let slug = content::slug_from_title(&args.title);
    let tags_vec: Vec<String> = args
        .tags
        .as_ref()
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let date = if collection.has_date {
        Some(chrono::Local::now().date_naive())
    } else {
        None
    };

    let fm = Frontmatter {
        title: args.title.clone(),
        date,
        tags: tags_vec,
        draft: args.draft,
        ..Default::default()
    };

    // Validate --lang if provided: must be a configured non-default language
    let lang_suffix = if let Some(ref lang) = args.lang {
        if *lang == site_config.site.language {
            // Default language doesn't need a suffix
            None
        } else if site_config.languages.contains_key(lang) {
            Some(lang.as_str())
        } else {
            anyhow::bail!(
                "unknown language '{}'. Configured languages: {}",
                lang,
                site_config.all_languages().join(", ")
            );
        }
    } else {
        None
    };

    let filename = if collection.has_date {
        let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
        if let Some(lang) = lang_suffix {
            format!("{date_str}-{slug}.{lang}.md")
        } else {
            format!("{date_str}-{slug}.md")
        }
    } else if let Some(lang) = lang_suffix {
        format!("{slug}.{lang}.md")
    } else {
        format!("{slug}.md")
    };

    let filepath = paths.content.join(&collection.directory).join(&filename);
    fs::create_dir_all(filepath.parent().unwrap())?;
    let file_content = format!(
        "{}\n\nWrite your content here.\n",
        content::generate_frontmatter(&fm)
    );
    fs::write(&filepath, file_content)?;
    human::success(&format!("Created {}", filepath.display()));

    Ok(())
}
