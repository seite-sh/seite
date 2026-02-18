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
    /// Deploy target override (github-pages, cloudflare, netlify)
    #[arg(short, long)]
    pub target: Option<String>,

    /// Build before deploying
    #[arg(long, default_value = "true")]
    pub build: bool,

    /// Show what would be done without actually deploying
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: &DeployArgs) -> anyhow::Result<()> {
    let config = SiteConfig::load(&PathBuf::from("page.toml"))?;
    let paths = config.resolve_paths(&std::env::current_dir()?);

    if args.build && !args.dry_run {
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
            DeployTarget::Netlify => "netlify",
        });

    if args.dry_run {
        human::info(&format!("Dry run: would deploy to {target_str}"));
        match target_str {
            "github-pages" => {
                let repo_url = config.deploy.repo.as_deref().unwrap_or("(auto-detect from git remote)");
                human::info(&format!("  Repository: {repo_url}"));
                human::info(&format!("  Output dir: {}", paths.output.display()));
                human::info("  Branch: gh-pages (force push)");
            }
            "cloudflare" => {
                let detected = deploy::detect_cloudflare_project(&paths);
                let project = config.deploy.project.as_deref()
                    .or(detected.as_deref())
                    .unwrap_or("(not configured — set deploy.project in page.toml)");
                human::info(&format!("  Project: {project}"));
                human::info(&format!("  Output dir: {}", paths.output.display()));
            }
            "netlify" => {
                let site_id = config.deploy.project.as_deref()
                    .unwrap_or("(auto-detect or set deploy.project in page.toml)");
                human::info(&format!("  Site ID/name: {site_id}"));
                human::info(&format!("  Output dir: {}", paths.output.display()));
            }
            other => {
                return Err(PageError::Deploy(format!(
                    "unknown deploy target: '{other}'. Valid targets: github-pages, cloudflare, netlify"
                )).into());
            }
        }
        human::success("Dry run complete (no changes made)");
        return Ok(());
    }

    match target_str {
        "github-pages" => {
            human::info("Deploying to GitHub Pages...");
            deploy::deploy_github_pages(&paths, config.deploy.repo.as_deref())?;
        }
        "cloudflare" => {
            human::info("Deploying to Cloudflare Pages...");
            let project = match config.deploy.project.as_deref() {
                Some(p) => p.to_string(),
                None => {
                    // Try auto-detecting project name from wrangler.toml or directory name
                    deploy::detect_cloudflare_project(&paths)
                        .ok_or_else(|| PageError::Deploy(
                            "no project name configured. Set deploy.project in page.toml, \
                             or add a wrangler.toml with a 'name' field, \
                             or pass --target cloudflare with deploy.project set".into(),
                        ))?
                }
            };
            deploy::deploy_cloudflare(&paths, &project)?;
        }
        "netlify" => {
            human::info("Deploying to Netlify...");
            deploy::deploy_netlify(&paths, config.deploy.project.as_deref())?;
        }
        other => {
            return Err(PageError::Deploy(format!(
                "unknown deploy target: '{other}'. Valid targets: github-pages, cloudflare, netlify"
            )).into());
        }
    }

    human::success("Deployed successfully");
    Ok(())
}
