use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::config::SiteConfig;
use crate::output::human;
use crate::themes;

#[derive(Args)]
pub struct ThemeArgs {
    #[command(subcommand)]
    pub command: ThemeCommand,
}

#[derive(Subcommand)]
pub enum ThemeCommand {
    /// List available themes
    List,

    /// Apply a theme to the current site
    Apply {
        /// Theme name
        name: String,
    },
}

pub fn run(args: &ThemeArgs) -> anyhow::Result<()> {
    match &args.command {
        ThemeCommand::List => {
            human::header("Available themes");
            for theme in themes::all() {
                println!(
                    "  {} - {}",
                    console::style(theme.name).bold(),
                    theme.description
                );
            }
        }
        ThemeCommand::Apply { name } => {
            let theme = themes::by_name(name)
                .ok_or_else(|| anyhow::anyhow!(
                    "unknown theme '{}'. Run 'page theme list' to see available themes",
                    name
                ))?;

            // Ensure we're in a page project
            let _config = SiteConfig::load(&PathBuf::from("page.toml"))?;

            let template_dir = PathBuf::from("templates");
            std::fs::create_dir_all(&template_dir)?;
            std::fs::write(template_dir.join("base.html"), theme.base_html)?;

            human::success(&format!("Applied theme '{}'", name));
            human::info("Run 'page build' or the watcher will pick it up automatically.");
        }
    }
    Ok(())
}
