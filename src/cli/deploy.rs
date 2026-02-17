use std::path::PathBuf;

use clap::Args;

use crate::build::{self, BuildOptions};
use crate::config::{DeployTarget, SiteConfig};
use crate::deploy;
use crate::error::PageError;
use crate::output::human;
use crate::output::CommandOutput;

#[derive(Args)]
pub struct DeployArgs {
    /// Deploy target override (github-pages, cloudflare)
    #[arg(short, long)]
    pub target: Option<String>,

    /// Build before deploying
    #[arg(long, default_value = "true")]
    pub build: bool,
}

pub fn run(args: &DeployArgs) -> anyhow::Result<()> {
    let config = SiteConfig::load(&PathBuf::from("page.toml"))?;
    let paths = config.resolve_paths(&std::env::current_dir()?);

    if args.build {
        human::info("Building site...");
        let opts = BuildOptions {
            include_drafts: false,
        };
        let result = build::build_site(&config, &paths, &opts)?;
        human::success(&result.stats.human_display());
    }

    let target_str = args
        .target
        .as_deref()
        .unwrap_or(match &config.deploy.target {
            DeployTarget::GithubPages => "github-pages",
            DeployTarget::Cloudflare => "cloudflare",
        });

    match target_str {
        "github-pages" => {
            human::info("Deploying to GitHub Pages...");
            deploy::deploy_github_pages(&paths, config.deploy.repo.as_deref())?;
        }
        "cloudflare" => {
            human::info("Deploying to Cloudflare Pages...");
            let project = config.deploy.project.as_deref().ok_or_else(|| {
                PageError::Deploy(
                    "no project name configured in page.toml [deploy] section".into(),
                )
            })?;
            deploy::deploy_cloudflare(&paths, project)?;
        }
        other => {
            return Err(PageError::Deploy(format!("unknown deploy target: {other}")).into());
        }
    }

    human::success("Deployed successfully");
    Ok(())
}
