use std::path::PathBuf;

use clap::Args;

use crate::build;
use crate::config::{self, SiteConfig};
use crate::content::{
    self,
    create::{create_content_file, NewContent},
};
use crate::output::human::{self, suggest_match};

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
            let available: Vec<&str> = site_config
                .collections
                .iter()
                .map(|c| c.name.as_str())
                .collect();
            let hint = suggest_match(&args.collection, &available);
            anyhow::anyhow!(
                "unknown collection '{}'. Available: {}{}",
                args.collection,
                available.join(", "),
                hint
            )
        })?;

    let tags_vec: Vec<String> = args
        .tags
        .as_ref()
        .map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let spec = NewContent {
        title: &args.title,
        tags: tags_vec,
        draft: args.draft,
        lang: args.lang.as_deref(),
        body: "Write your content here.",
        ..Default::default()
    };
    let created = create_content_file(
        &site_config,
        &paths.content,
        collection,
        &spec,
        crate::content::schedule::today(&site_config),
    )?;

    human::success(&format!("Created {}", created.path.display()));
    // Required schema fields without a default were written empty: say which
    // ones to fill in (`seite check` reports each at its line until then).
    let cwd = std::env::current_dir()?;
    let schema_problems: Vec<_> = created
        .schema_problems
        .iter()
        .map(|d| d.clone().relative_to(&cwd))
        .collect();
    if !created.placeholders.is_empty() {
        human::warning(&format!(
            "fill in the required field{} {} (the {} schema in seite.toml)",
            if created.placeholders.len() == 1 {
                ""
            } else {
                "s"
            },
            created
                .placeholders
                .iter()
                .map(|f| format!("`{f}`"))
                .collect::<Vec<_>>()
                .join(", "),
            collection.name
        ));
    }
    for d in &schema_problems {
        crate::human_println!("{d}");
    }
    crate::human_println!(
        "  {} edit this file (a running `seite serve` reloads on save)",
        console::style("→").dim()
    );

    // Report the URL exactly as the build will generate it (the default
    // language is unprefixed, translations get `/{lang}`, `slug:`/date
    // handling), using the build's own resolver so the two can't disagree.
    let collection_dir = paths.content.join(&collection.directory);
    let rel_to_collection = created
        .path
        .strip_prefix(&collection_dir)
        .unwrap_or(&created.path);
    let (fm, _) = content::parse_content_file(&created.path)?;
    let url = build::resolve_item_location(
        &site_config,
        collection,
        &created.path,
        rel_to_collection,
        &fm,
    )
    .url;
    crate::output::json::set_data(serde_json::json!({
        "path": created.path.display().to_string(),
        "collection": collection.name,
        "slug": created.slug,
        "url": url,
        "draft": args.draft,
        "lang": spec.lang,
        "fill_in": created.placeholders,
        "schema_problems": schema_problems,
    }));

    Ok(())
}
