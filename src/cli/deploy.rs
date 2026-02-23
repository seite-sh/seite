use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

    /// Override base_url for this deploy (e.g., `https://example.com`)
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

    /// Skip auto-commit and push (overrides deploy.auto_commit)
    #[arg(long)]
    pub no_commit: bool,
}

pub fn run(args: &DeployArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;

    // Check for workspace context
    if let Some(ws_root) = crate::workspace::find_workspace_root(&cwd) {
        let ws_config =
            crate::workspace::WorkspaceConfig::load(&ws_root.join("seite-workspace.toml"))?;

        let opts = crate::workspace::deploy::WorkspaceDeployOptions {
            site_filter: site_filter.map(String::from),
            build: args.build,
            dry_run: args.dry_run,
            preview: args.preview,
            base_url: args.base_url.clone(),
            verify: args.verify,
            skip_checks: args.skip_checks,
            no_commit: args.no_commit,
        };

        return crate::workspace::deploy::deploy_workspace(&ws_config, &ws_root, &opts);
    }

    // Standalone mode
    if site_filter.is_some() {
        human::warning("--site flag ignored (not in a workspace)");
    }

    let config_path = PathBuf::from("seite.toml");
    let mut config = SiteConfig::load(&config_path)?;
    let mut paths = config.resolve_paths(&cwd);

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

    // --- Pre-flight checks with interactive recovery ---
    if !args.skip_checks {
        let checks = deploy::preflight(&config, &paths, &target_str);
        let all_passed = deploy::print_preflight(&checks);
        if !all_passed {
            // base_url warning is non-fatal if --base-url override is provided
            let only_base_url_failed = checks.iter().all(|c| c.passed || c.name == "Base URL");
            if only_base_url_failed && args.base_url.is_some() {
                human::info("base_url check overridden via --base-url flag");
            } else {
                // Try to interactively fix each failed check
                let unresolved =
                    run_interactive_recovery(&checks, &config, &paths, &target_str, &config_path)?;

                if !unresolved.is_empty() {
                    // Some checks still failing — check if it's only base_url
                    let only_base_url = unresolved.iter().all(|name| name == "Base URL");
                    if only_base_url {
                        human::warning(
                            "Deploying with localhost base_url. Use --base-url to override.",
                        );
                        human::info("Continuing anyway...");
                    } else {
                        println!();
                        human::error("Some pre-flight checks could not be resolved:");
                        for name in &unresolved {
                            human::info(&format!("  - {name}"));
                        }
                        let cont = dialoguer::Confirm::new()
                            .with_prompt("Continue deploying anyway?")
                            .default(false)
                            .interact()
                            .unwrap_or(false);
                        if !cont {
                            return Err(PageError::Deploy(
                                "pre-flight checks failed — fix the issues above before deploying"
                                    .into(),
                            )
                            .into());
                        }
                    }
                }

                // Reload config in case it was updated (e.g., base_url fix, project creation)
                if let Ok(reloaded) = SiteConfig::load(&config_path) {
                    paths = reloaded.resolve_paths(&std::env::current_dir()?);
                    config = reloaded;
                }
            }
        }
    }

    // --- Auto-commit and push ---
    let mut preview = args.preview;
    let should_auto_commit = config.deploy.auto_commit && !args.no_commit;
    if should_auto_commit {
        match deploy::auto_commit_and_push(&paths) {
            Ok(result) => {
                if result.committed {
                    human::success(&format!("Committed and pushed to {}", result.branch));
                } else {
                    human::info(&format!(
                        "No uncommitted changes, pushed to {}",
                        result.branch
                    ));
                }
                // Auto-enable preview when on a non-main branch
                if !result.is_main && !preview {
                    human::info(&format!(
                        "On branch '{}' — deploying as preview",
                        result.branch
                    ));
                    preview = true;
                }
            }
            Err(e) => {
                human::warning(&format!("Auto-commit skipped: {e}"));
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
            human::info(if preview {
                "Deploying preview to Cloudflare Pages..."
            } else {
                "Deploying to Cloudflare Pages..."
            });
            let project = resolve_cloudflare_project(&config, &paths)?;
            deploy::deploy_cloudflare(&paths, &project, preview)?
        }
        "netlify" => {
            human::info(if preview {
                "Deploying preview to Netlify..."
            } else {
                "Deploying to Netlify..."
            });
            deploy::deploy_netlify(&paths, config.deploy.project.as_deref(), preview)?
        }
        other => {
            return Err(PageError::Deploy(format!(
                "unknown deploy target: '{other}'. Valid targets: github-pages, cloudflare, netlify"
            ))
            .into());
        }
    };

    if preview {
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
    if args.verify || !preview {
        // Auto-verify on production deploys, skip on preview unless --verify
        if let Some(ref url) = deploy_url {
            if !preview || args.verify {
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
    config_path: &Path,
) -> anyhow::Result<()> {
    let target = match target_str {
        "github-pages" => DeployTarget::GithubPages,
        "cloudflare" => DeployTarget::Cloudflare,
        "netlify" => DeployTarget::Netlify,
        other => {
            return Err(PageError::Deploy(format!("unknown target: {other}")).into());
        }
    };

    // Strip protocol for the domain field (store just the domain)
    let clean_domain = domain
        .strip_prefix("https://")
        .or_else(|| domain.strip_prefix("http://"))
        .unwrap_or(domain)
        .trim_end_matches('/');

    let setup = deploy::domain_setup_instructions(clean_domain, &target, config);
    deploy::print_domain_setup(&setup);

    // Update seite.toml with base_url and deploy.domain
    let new_base_url = if domain.starts_with("http") {
        domain.to_string()
    } else {
        format!("https://{domain}")
    };

    let mut updates = HashMap::new();
    updates.insert("base_url".to_string(), new_base_url.clone());
    updates.insert("domain".to_string(), clean_domain.to_string());
    deploy::update_deploy_config(config_path, &updates)?;
    human::success(&format!(
        "Updated base_url to '{new_base_url}' and deploy.domain to '{clean_domain}' in seite.toml"
    ));

    // Offer to attach the domain to the platform
    match target {
        DeployTarget::Cloudflare => {
            if let Some(ref project) = config.deploy.project {
                let attach = dialoguer::Confirm::new()
                    .with_prompt(format!(
                        "Attach '{clean_domain}' to Cloudflare Pages project '{project}'?"
                    ))
                    .default(true)
                    .interact()
                    .unwrap_or(false);
                if attach {
                    match deploy::cloudflare_attach_domain(project, clean_domain) {
                        Ok(true) => human::success(&format!(
                            "Domain '{clean_domain}' attached to project '{project}'"
                        )),
                        Ok(false) => human::warning(
                            "Could not attach domain — add it manually in the Cloudflare dashboard",
                        ),
                        Err(e) => human::warning(&format!(
                            "API call failed: {e} — add the domain manually"
                        )),
                    }
                }
            } else {
                human::info(
                    "Set deploy.project in seite.toml to enable automatic domain attachment",
                );
            }
        }
        DeployTarget::Netlify => {
            let paths = config.resolve_paths(&std::env::current_dir()?);
            let attach = dialoguer::Confirm::new()
                .with_prompt(format!("Add '{clean_domain}' to Netlify site?"))
                .default(true)
                .interact()
                .unwrap_or(false);
            if attach {
                match deploy::netlify_add_domain(&paths, clean_domain) {
                    Ok(true) => {
                        human::success(&format!("Domain '{clean_domain}' added to Netlify site"))
                    }
                    Ok(false) => {
                        human::warning("Could not add domain — run `netlify domains:add` manually")
                    }
                    Err(e) => human::warning(&format!("Failed: {e}")),
                }
            }
        }
        DeployTarget::GithubPages => {
            human::info("CNAME file will be auto-generated during deploy");
        }
    }

    Ok(())
}

fn run_setup(
    target_str: &str,
    config: &SiteConfig,
    paths: &crate::config::ResolvedPaths,
    config_path: &Path,
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

    // Update seite.toml
    deploy::update_deploy_config(config_path, &config_updates)?;
    human::success("Updated seite.toml with deploy configuration");

    // Offer contact form setup if not already configured
    if config.contact.is_none() {
        println!();
        let add_contact = dialoguer::Confirm::new()
            .with_prompt("Would you like to add a contact form?")
            .default(false)
            .interact()?;
        if add_contact {
            let setup_args = crate::cli::contact::SetupArgs {
                provider: None,
                endpoint: None,
                region: None,
                redirect: None,
                subject: None,
            };
            let contact = crate::cli::contact::prompt_contact_config(&setup_args, config)?;

            // Write [contact] section to seite.toml
            let contents = std::fs::read_to_string(config_path)?;
            let mut doc: toml::Table = contents
                .parse()
                .map_err(|e: toml::de::Error| anyhow::anyhow!("failed to parse seite.toml: {e}"))?;
            let contact_value = toml::Value::try_from(&contact)?;
            doc.insert("contact".to_string(), contact_value);
            let new_contents = toml::to_string_pretty(&doc)?;
            std::fs::write(config_path, new_contents)?;
            human::success("Added [contact] section to seite.toml");
        }
    }

    println!();
    human::info("Setup complete. Next steps:");
    human::info("  1. Set your production URL:  seite deploy --domain example.com");
    human::info("  2. Deploy:                   seite deploy");

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
                .unwrap_or("(not configured — set deploy.project in seite.toml)");
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
                .unwrap_or("(auto-detect or set deploy.project in seite.toml)");
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
// Interactive recovery
// ---------------------------------------------------------------------------

/// For each failed pre-flight check, offer the user an auto-fix or show manual instructions.
/// Returns a list of check names that are still unresolved after the recovery loop.
fn run_interactive_recovery(
    checks: &[deploy::PreflightCheck],
    config: &SiteConfig,
    paths: &crate::config::ResolvedPaths,
    target: &str,
    config_path: &std::path::Path,
) -> anyhow::Result<Vec<String>> {
    let mut unresolved = Vec::new();

    println!();
    for check in checks {
        if check.passed {
            continue;
        }

        let fix = deploy::try_fix_check(check, paths, target);
        match fix {
            Some(fix_action) if !fix_action.prompt.is_empty() => {
                // We have an auto-fix available — ask the user
                let do_fix = dialoguer::Confirm::new()
                    .with_prompt(&fix_action.prompt)
                    .default(true)
                    .interact()
                    .unwrap_or(false);

                if do_fix {
                    match deploy::execute_fix(&check.name, paths, config, config_path) {
                        Ok(true) => {
                            // Verify the fix worked
                            let recheck = deploy::recheck(&check.name, config, paths, target);
                            if recheck.passed {
                                println!(
                                    "  {} {}: {}",
                                    console::style("✓").green(),
                                    recheck.name,
                                    recheck.message
                                );
                            } else {
                                println!(
                                    "  {} {}: {}",
                                    console::style("✗").red(),
                                    recheck.name,
                                    recheck.message
                                );
                                unresolved.push(check.name.clone());
                            }
                        }
                        Ok(false) => {
                            human::warning(&format!("Could not fix: {}", check.name));
                            print_manual_instructions(&fix_action.manual_instructions);
                            unresolved.push(check.name.clone());
                        }
                        Err(e) => {
                            human::error(&format!("Fix failed: {e}"));
                            print_manual_instructions(&fix_action.manual_instructions);
                            unresolved.push(check.name.clone());
                        }
                    }
                } else {
                    // User declined — show manual instructions
                    print_manual_instructions(&fix_action.manual_instructions);
                    unresolved.push(check.name.clone());
                }
            }
            Some(fix_action) => {
                // No auto-fix prompt (empty prompt = can't auto-fix but has instructions)
                print_manual_instructions(&fix_action.manual_instructions);
                unresolved.push(check.name.clone());
            }
            None => {
                // No fix available at all
                unresolved.push(check.name.clone());
            }
        }
    }

    Ok(unresolved)
}

fn print_manual_instructions(instructions: &[String]) {
    for instruction in instructions {
        human::info(&format!("  {instruction}"));
    }
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
                "no project name configured. Set deploy.project in seite.toml, \
                 or run `seite deploy --setup` to configure"
                    .into(),
            )
            .into()
        }),
    }
}
