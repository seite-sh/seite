use std::collections::HashMap;
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

    /// Deploy to a preview/staging URL instead of production
    #[arg(long)]
    pub preview: bool,

    /// Override base_url for this deploy (e.g., https://example.com)
    #[arg(long)]
    pub base_url: Option<String>,

    /// Run guided deploy setup: create project, configure CI, set domain
    #[arg(long)]
    pub setup: bool,

    /// Set up a custom domain for deployment
    #[arg(long)]
    pub domain: Option<String>,

    /// Verify deployment after it completes
    #[arg(long)]
    pub verify: bool,

    /// Skip pre-flight checks
    #[arg(long)]
    pub skip_checks: bool,
}

pub fn run(args: &DeployArgs) -> anyhow::Result<()> {
    let config_path = PathBuf::from("page.toml");
    let config = SiteConfig::load(&config_path)?;
    let paths = config.resolve_paths(&std::env::current_dir()?);

    let target_str = resolve_target_str(args, &config);

    // --- Domain setup mode ---
    if let Some(ref domain) = args.domain {
        return run_domain_setup(domain, &target_str, &config, &config_path);
    }

    // --- Setup mode ---
    if args.setup {
        return run_setup(&target_str, &config, &paths, &config_path);
    }

    // --- Dry run ---
    if args.dry_run {
        return run_dry_run(&target_str, &config, &paths, args);
    }

    // --- Pre-flight checks ---
    if !args.skip_checks {
        let checks = deploy::preflight(&config, &paths, &target_str);
        let all_passed = deploy::print_preflight(&checks);
        if !all_passed {
            // base_url warning is non-fatal if --base-url override is provided
            let only_base_url_failed = checks.iter().all(|c| c.passed || c.name == "Base URL");
            if only_base_url_failed && args.base_url.is_some() {
                human::info("base_url check overridden via --base-url flag");
            } else if only_base_url_failed {
                human::warning("Deploying with localhost base_url. Use --base-url to override or update page.toml.");
                human::info("Continuing anyway...");
            } else {
                return Err(PageError::Deploy(
                    "pre-flight checks failed — fix the issues above before deploying".into(),
                )
                .into());
            }
        }
    }

    // --- Resolve base_url for this deploy ---
    let deploy_base_url = deploy::resolve_deploy_base_url(&config, args.base_url.as_deref());

    // --- Build ---
    if args.build {
        human::info("Building site...");
        // If base_url override is specified, we need to temporarily update the config
        let build_config = if args.base_url.is_some() {
            let mut c = config.clone();
            c.site.base_url = deploy_base_url.clone();
            c
        } else {
            config.clone()
        };
        let opts = BuildOptions {
            include_drafts: false,
        };
        let result = build::build_site(&build_config, &paths, &opts)?;
        human::success(&result.stats.human_display());
    }

    // --- Deploy ---
    let deploy_url = match target_str.as_str() {
        "github-pages" => {
            human::info("Deploying to GitHub Pages...");
            deploy::deploy_github_pages(&config, &paths, config.deploy.repo.as_deref())?;
            // Infer the deploy URL from base_url or repo
            Some(deploy_base_url.clone())
        }
        "cloudflare" => {
            human::info(if args.preview {
                "Deploying preview to Cloudflare Pages..."
            } else {
                "Deploying to Cloudflare Pages..."
            });
            let project = resolve_cloudflare_project(&config, &paths)?;
            deploy::deploy_cloudflare(&paths, &project, args.preview)?
        }
        "netlify" => {
            human::info(if args.preview {
                "Deploying preview to Netlify..."
            } else {
                "Deploying to Netlify..."
            });
            deploy::deploy_netlify(&paths, config.deploy.project.as_deref(), args.preview)?
        }
        other => {
            return Err(PageError::Deploy(format!(
                "unknown deploy target: '{other}'. Valid targets: github-pages, cloudflare, netlify"
            ))
            .into());
        }
    };

    if args.preview {
        if let Some(ref url) = deploy_url {
            human::success(&format!("Preview deployed: {url}"));
        } else {
            human::success("Preview deployed (check CLI output above for URL)");
        }
    } else {
        human::success("Deployed successfully");
        if let Some(ref url) = deploy_url {
            human::info(&format!("Live at: {url}"));
        }
    }

    // --- Post-deploy verification ---
    if args.verify || (!args.preview && !args.verify) {
        // Auto-verify on production deploys, skip on preview unless --verify
        if let Some(ref url) = deploy_url {
            if !args.preview || args.verify {
                human::info("Verifying deployment...");
                let results = deploy::verify_deployment(url);
                deploy::print_verification(&results);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Subcommand handlers
// ---------------------------------------------------------------------------

fn run_domain_setup(
    domain: &str,
    target_str: &str,
    config: &SiteConfig,
    config_path: &PathBuf,
) -> anyhow::Result<()> {
    let target = match target_str {
        "github-pages" => DeployTarget::GithubPages,
        "cloudflare" => DeployTarget::Cloudflare,
        "netlify" => DeployTarget::Netlify,
        other => {
            return Err(PageError::Deploy(format!("unknown target: {other}")).into());
        }
    };

    let setup = deploy::domain_setup_instructions(domain, &target, config);
    deploy::print_domain_setup(&setup);

    // Update page.toml with the new base_url
    let new_base_url = if domain.starts_with("http") {
        domain.to_string()
    } else {
        format!("https://{domain}")
    };

    let mut updates = HashMap::new();
    updates.insert("base_url".to_string(), new_base_url.clone());
    deploy::update_deploy_config(config_path, &updates)?;
    human::success(&format!("Updated base_url to '{new_base_url}' in page.toml"));

    Ok(())
}

fn run_setup(
    target_str: &str,
    config: &SiteConfig,
    paths: &crate::config::ResolvedPaths,
    config_path: &PathBuf,
) -> anyhow::Result<()> {
    human::header(&format!("Setting up deployment for {target_str}"));

    let mut config_updates = HashMap::new();
    config_updates.insert("target".to_string(), target_str.to_string());

    match target_str {
        "github-pages" => {
            deploy::deploy_init_github_pages(paths)?;

            // Generate workflow
            let workflow_dir = paths.root.join(".github/workflows");
            std::fs::create_dir_all(&workflow_dir)?;
            let workflow = deploy::generate_github_actions_workflow(config);
            std::fs::write(workflow_dir.join("deploy.yml"), &workflow)?;
            human::success("Created .github/workflows/deploy.yml");
        }
        "cloudflare" => {
            let project = deploy::deploy_init_cloudflare(paths)?;
            config_updates.insert("project".to_string(), project.clone());

            // Generate workflow
            let workflow_dir = paths.root.join(".github/workflows");
            std::fs::create_dir_all(&workflow_dir)?;
            let workflow = deploy::generate_cloudflare_workflow(config);
            std::fs::write(workflow_dir.join("deploy.yml"), &workflow)?;
            human::success("Created .github/workflows/deploy.yml");

            human::info("Set these GitHub secrets for CI:");
            human::info("  CLOUDFLARE_API_TOKEN  — create at https://dash.cloudflare.com/profile/api-tokens");
            human::info("  CLOUDFLARE_ACCOUNT_ID — found in your Cloudflare dashboard");
        }
        "netlify" => {
            let site_name = deploy::deploy_init_netlify(paths)?;
            config_updates.insert("project".to_string(), site_name);

            // Generate netlify.toml
            let netlify_config = deploy::generate_netlify_config(config);
            std::fs::write(paths.root.join("netlify.toml"), &netlify_config)?;
            human::success("Created netlify.toml");

            // Also generate GitHub Actions workflow as an alternative
            let workflow_dir = paths.root.join(".github/workflows");
            std::fs::create_dir_all(&workflow_dir)?;
            let workflow = deploy::generate_netlify_workflow(config);
            std::fs::write(workflow_dir.join("deploy.yml"), &workflow)?;
            human::success("Created .github/workflows/deploy.yml");

            human::info("Set these GitHub secrets for CI:");
            human::info("  NETLIFY_AUTH_TOKEN — create at https://app.netlify.com/user/applications#personal-access-tokens");
            human::info("  NETLIFY_SITE_ID    — found in your site settings");
        }
        other => {
            return Err(PageError::Deploy(format!(
                "unknown deploy target: '{other}'. Valid targets: github-pages, cloudflare, netlify"
            ))
            .into());
        }
    }

    // Update page.toml
    deploy::update_deploy_config(config_path, &config_updates)?;
    human::success("Updated page.toml with deploy configuration");

    println!();
    human::info("Setup complete. Next steps:");
    human::info("  1. Set your production URL:  page deploy --domain example.com");
    human::info("  2. Deploy:                   page deploy");

    Ok(())
}

fn run_dry_run(
    target_str: &str,
    config: &SiteConfig,
    paths: &crate::config::ResolvedPaths,
    args: &DeployArgs,
) -> anyhow::Result<()> {
    human::info(&format!("Dry run: would deploy to {target_str}"));

    // Run pre-flight checks even in dry-run
    let checks = deploy::preflight(config, paths, target_str);
    deploy::print_preflight(&checks);

    match target_str {
        "github-pages" => {
            let repo_url = config
                .deploy
                .repo
                .as_deref()
                .unwrap_or("(auto-detect from git remote)");
            human::info(&format!("  Repository: {repo_url}"));
            human::info(&format!("  Output dir: {}", paths.output.display()));
            human::info("  Branch: gh-pages (force push)");
            human::info("  Files: .nojekyll (auto-generated)");
            if let Some(domain) = deploy::extract_custom_domain(&config.site.base_url) {
                if !domain.ends_with(".github.io")
                    && !domain.contains("localhost")
                    && !domain.contains("127.0.0.1")
                {
                    human::info(&format!("  CNAME: {domain} (auto-generated)"));
                }
            }
        }
        "cloudflare" => {
            let detected = deploy::detect_cloudflare_project(paths);
            let project = config
                .deploy
                .project
                .as_deref()
                .or(detected.as_deref())
                .unwrap_or("(not configured — set deploy.project in page.toml)");
            human::info(&format!("  Project: {project}"));
            human::info(&format!("  Output dir: {}", paths.output.display()));
            if args.preview {
                human::info("  Mode: preview (non-production)");
            }
        }
        "netlify" => {
            let site_id = config
                .deploy
                .project
                .as_deref()
                .unwrap_or("(auto-detect or set deploy.project in page.toml)");
            human::info(&format!("  Site ID/name: {site_id}"));
            human::info(&format!("  Output dir: {}", paths.output.display()));
            if args.preview {
                human::info("  Mode: preview (draft deploy)");
            } else {
                human::info("  Mode: production");
            }
        }
        other => {
            return Err(PageError::Deploy(format!(
                "unknown deploy target: '{other}'. Valid targets: github-pages, cloudflare, netlify"
            ))
            .into());
        }
    }

    if let Some(ref base_url) = args.base_url {
        human::info(&format!("  Base URL override: {base_url}"));
    }

    human::success("Dry run complete (no changes made)");
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_target_str(args: &DeployArgs, config: &SiteConfig) -> String {
    args.target
        .clone()
        .unwrap_or_else(|| match &config.deploy.target {
            DeployTarget::GithubPages => "github-pages".to_string(),
            DeployTarget::Cloudflare => "cloudflare".to_string(),
            DeployTarget::Netlify => "netlify".to_string(),
        })
}

fn resolve_cloudflare_project(
    config: &SiteConfig,
    paths: &crate::config::ResolvedPaths,
) -> anyhow::Result<String> {
    match config.deploy.project.as_deref() {
        Some(p) => Ok(p.to_string()),
        None => deploy::detect_cloudflare_project(paths).ok_or_else(|| {
            PageError::Deploy(
                "no project name configured. Set deploy.project in page.toml, \
                 or run `page deploy --setup` to configure"
                    .into(),
            )
            .into()
        }),
    }
}
