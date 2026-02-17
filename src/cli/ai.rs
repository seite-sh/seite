use std::path::PathBuf;

use clap::Args;

use crate::ai::{AiClient, Provider};
use crate::cli::auth;
use crate::config::{self, SiteConfig};
use crate::content::{self, Frontmatter};
use crate::credential;
use crate::error::PageError;
use crate::output::human;

#[derive(Args)]
pub struct AiArgs {
    /// The prompt describing what to generate
    pub prompt: String,

    /// LLM provider to use (overrides config default)
    #[arg(short, long)]
    pub provider: Option<String>,

    /// Model to use
    #[arg(short, long)]
    pub model: Option<String>,

    /// Output file path (default: auto-generated from prompt)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Type to generate: content type name (post, doc, page) or "template"
    #[arg(short, long, default_value = "post")]
    pub r#type: String,
}

pub fn run(args: &AiArgs) -> anyhow::Result<()> {
    let config = SiteConfig::load(&PathBuf::from("page.toml"))?;
    let paths = config.resolve_paths(&std::env::current_dir()?);

    let provider_name = args
        .provider
        .as_deref()
        .unwrap_or(&config.ai.default_provider);

    let api_key = match credential::get_key(provider_name) {
        Ok(key) => key,
        Err(_) => {
            human::info(&format!("No API key for {provider_name}. Let's set one up."));
            auth::login(provider_name)?;
            credential::get_key(provider_name)?
        }
    };

    let provider = match provider_name {
        "claude" => Provider::Claude,
        "openai" => Provider::OpenAI,
        other => return Err(PageError::Ai(format!("unknown provider: {other}")).into()),
    };

    let model = args.model.clone().unwrap_or_else(|| match &provider {
        Provider::Claude => "claude-sonnet-4-20250514".to_string(),
        Provider::OpenAI => "gpt-4o".to_string(),
    });

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let client = AiClient::new(provider, api_key, model);

    if args.r#type == "template" {
        spinner.set_message("Generating template...");
        let raw = client.generate_template(&args.prompt)?;
        spinner.finish_and_clear();

        let output_path = match &args.output {
            Some(p) => PathBuf::from(p),
            None => paths.templates.join("custom.html"),
        };

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&output_path, raw)?;
        human::success(&format!("Generated template: {}", output_path.display()));
    } else {
        spinner.set_message("Generating content...");
        let generated = client.generate(&args.prompt, &args.r#type)?;
        spinner.finish_and_clear();

        let slug = content::slug_from_title(&generated.title);

        // Resolve collection for the type
        let collection = config::find_collection(&args.r#type, &config.collections);

        let has_date = collection.map(|c| c.has_date).unwrap_or(false);
        let date = if has_date {
            Some(chrono::Local::now().date_naive())
        } else {
            None
        };

        let fm = Frontmatter {
            title: generated.title.clone(),
            date,
            draft: true,
            ..Default::default()
        };

        let output_path = match &args.output {
            Some(p) => PathBuf::from(p),
            None => {
                let dir_name = collection
                    .map(|c| c.directory.as_str())
                    .unwrap_or("posts");
                if has_date {
                    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
                    paths
                        .content
                        .join(dir_name)
                        .join(format!("{date_str}-{slug}.md"))
                } else {
                    paths.content.join(dir_name).join(format!("{slug}.md"))
                }
            }
        };

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file_content = format!(
            "{}\n\n{}\n",
            content::generate_frontmatter(&fm),
            generated.body
        );
        std::fs::write(&output_path, file_content)?;
        human::success(&format!("Generated: {}", output_path.display()));
    }

    Ok(())
}
