use std::collections::HashMap;
use std::path::Path;

use crate::build::{self, BuildOptions};
use crate::config::DeployTarget;
use crate::deploy;
use crate::error::PageError;
use crate::output::human;
use crate::output::CommandOutput;

use super::{load_site_in_workspace, WorkspaceConfig};

pub struct WorkspaceDeployOptions {
    pub site_filter: Option<String>,
    pub build: bool,
    pub dry_run: bool,
    pub preview: bool,
    pub base_url: Option<String>,
    pub verify: bool,
    pub skip_checks: bool,
    pub no_commit: bool,
}

/// Deploy all (or filtered) sites in a workspace.
pub fn deploy_workspace(
    ws_config: &WorkspaceConfig,
    ws_root: &Path,
    opts: &WorkspaceDeployOptions,
) -> anyhow::Result<()> {
    let sites = ws_config
        .sites_to_operate(opts.site_filter.as_deref())
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Cross-site domain conflict detection
    check_domain_conflicts(ws_config, ws_root, &sites);

    let total = sites.len();

    for (i, ws_site) in sites.iter().enumerate() {
        human::header(&format!(
            "[{}/{}] Deploying site '{}'",
            i + 1,
            total,
            ws_site.name
        ));

        let (config, paths) = load_site_in_workspace(ws_root, ws_site)?;

        let target_str = match &config.deploy.target {
            DeployTarget::GithubPages => "github-pages",
            DeployTarget::Cloudflare => "cloudflare",
            DeployTarget::Netlify => "netlify",
        };

        // --- Dry run ---
        if opts.dry_run {
            human::info(&format!("  Would deploy to {target_str}"));
            human::info(&format!("  Output dir: {}", paths.output.display()));
            human::info(&format!("  Base URL: {}", config.site.base_url));
            continue;
        }

        // --- Pre-flight checks ---
        if !opts.skip_checks {
            let checks = deploy::preflight(&config, &paths, target_str);
            let all_passed = deploy::print_preflight(&checks);
            if !all_passed {
                let only_base_url_failed = checks.iter().all(|c| c.passed || c.name == "Base URL");
                if only_base_url_failed {
                    human::warning("Deploying with current base_url");
                } else {
                    human::error(&format!(
                        "Pre-flight checks failed for site '{}'. Use --skip-checks to override.",
                        ws_site.name
                    ));
                    continue;
                }
            }
        }

        // --- Build ---
        if opts.build {
            human::info(&format!("Building site '{}'...", ws_site.name));
            let build_config = if opts.base_url.is_some() {
                let mut c = config.clone();
                if let Some(ref url) = opts.base_url {
                    c.site.base_url = url.clone();
                }
                c
            } else {
                config.clone()
            };
            let build_opts = BuildOptions {
                include_drafts: false,
            };
            let result = build::build_site(&build_config, &paths, &build_opts)?;
            human::success(&result.stats.human_display());
        }

        // --- Deploy ---
        let deploy_url = match target_str {
            "github-pages" => {
                human::info("Deploying to GitHub Pages...");
                deploy::deploy_github_pages(&config, &paths, config.deploy.repo.as_deref())?;
                Some(config.site.base_url.clone())
            }
            "cloudflare" => {
                let project = config.deploy.project.as_deref().ok_or_else(|| {
                    PageError::Deploy(format!(
                        "site '{}': no deploy.project configured in seite.toml",
                        ws_site.name
                    ))
                })?;
                human::info("Deploying to Cloudflare Pages...");
                deploy::deploy_cloudflare(&paths, project, opts.preview)?
            }
            "netlify" => {
                human::info("Deploying to Netlify...");
                deploy::deploy_netlify(&paths, config.deploy.project.as_deref(), opts.preview)?
            }
            other => {
                human::error(&format!(
                    "Unknown deploy target '{other}' for site '{}'",
                    ws_site.name
                ));
                continue;
            }
        };

        if let Some(ref url) = deploy_url {
            human::success(&format!("Site '{}' deployed: {url}", ws_site.name));
        } else {
            human::success(&format!("Site '{}' deployed", ws_site.name));
        }

        // --- Post-deploy verification ---
        if opts.verify {
            if let Some(ref url) = deploy_url {
                human::info("Verifying deployment...");
                let results = deploy::verify_deployment(url);
                deploy::print_verification(&results);
            }
        }
    }

    if opts.dry_run {
        human::success("Dry run complete (no changes made)");
    } else {
        human::header("Workspace deploy complete");
    }

    Ok(())
}

/// Collect all effective domains across workspace sites and warn on conflicts.
/// Each site contributes its base_url domain plus any subdomain collection domains.
fn check_domain_conflicts(
    _ws_config: &WorkspaceConfig,
    ws_root: &Path,
    sites: &[&super::WorkspaceSite],
) {
    // Map: effective domain → vec of (site_name, description)
    let mut domain_map: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for ws_site in sites {
        let Ok((config, _paths)) = load_site_in_workspace(ws_root, ws_site) else {
            continue;
        };

        // Extract the host from base_url
        if let Some(domain) = extract_host(&config.site.base_url) {
            domain_map
                .entry(domain.clone())
                .or_default()
                .push((ws_site.name.clone(), format!("base_url ({domain})")));
        }

        // Subdomain collection domains
        for c in config.subdomain_collections() {
            if let Some(ref subdomain) = c.subdomain {
                let sub_url = config.subdomain_base_url(subdomain);
                if let Some(domain) = extract_host(&sub_url) {
                    domain_map.entry(domain.clone()).or_default().push((
                        ws_site.name.clone(),
                        format!("subdomain '{subdomain}' ({domain})"),
                    ));
                }
            }
        }
    }

    // Report conflicts (same domain claimed by different sites)
    for (domain, sources) in &domain_map {
        if sources.len() <= 1 {
            continue;
        }
        // Check if all sources are from the same site (not a real conflict)
        let site_names: std::collections::HashSet<&str> =
            sources.iter().map(|(name, _)| name.as_str()).collect();
        if site_names.len() <= 1 {
            continue;
        }

        human::warning(&format!("Domain conflict detected for '{domain}':"));
        for (site_name, desc) in sources {
            human::info(&format!("  - site '{site_name}': {desc}"));
        }
    }
}

/// Extract the host (domain + optional port) from a URL string.
fn extract_host(url: &str) -> Option<String> {
    // Strip scheme
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    // Take everything before the first '/'
    let host = after_scheme.split('/').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}
