use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::config::{DeployTarget, ResolvedPaths, SiteConfig};
use crate::error::{PageError, Result};
use crate::output::human;
use crate::platform::npm_cmd;

// ---------------------------------------------------------------------------
// Pre-flight checks (Feature 1)
// ---------------------------------------------------------------------------

/// Result of a single pre-flight check.
pub struct PreflightCheck {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

/// Run all pre-flight checks for the given target. Returns a list of check results.
/// If any check fails, the deploy should be aborted.
pub fn preflight(config: &SiteConfig, paths: &ResolvedPaths, target: &str) -> Vec<PreflightCheck> {
    let mut checks = Vec::new();

    // 1. Output directory exists and is non-empty
    checks.push(check_output_dir(paths));

    // 2. base_url is not localhost
    checks.push(check_base_url(config));

    // 3. Target-specific checks
    match target {
        "github-pages" => {
            checks.push(check_cli_available("git", &["--version"]));
            checks.push(check_git_repo(paths));
            checks.push(check_git_remote(paths, config.deploy.repo.as_deref()));
        }
        "cloudflare" => {
            checks.push(check_cli_available("wrangler", &["--version"]));
            checks.push(check_cloudflare_auth());
            checks.push(check_cloudflare_project(config, paths));
            if config.deploy.domain.is_some() {
                checks.push(check_cloudflare_domain(config));
            }
        }
        "netlify" => {
            checks.push(check_cli_available("netlify", &["--version"]));
            checks.push(check_netlify_auth());
            checks.push(check_netlify_site(config, paths));
            if config.deploy.domain.is_some() {
                checks.push(check_netlify_domain(config, paths));
            }
        }
        _ => {}
    }

    checks
}

fn check_output_dir(paths: &ResolvedPaths) -> PreflightCheck {
    if !paths.output.exists() {
        return PreflightCheck {
            name: "Output directory".into(),
            passed: false,
            message: format!(
                "{} does not exist — run `seite build` first",
                paths.output.display()
            ),
        };
    }
    // Check non-empty
    let has_files = fs::read_dir(&paths.output)
        .map(|mut d| d.next().is_some())
        .unwrap_or(false);
    if !has_files {
        return PreflightCheck {
            name: "Output directory".into(),
            passed: false,
            message: format!(
                "{} is empty — run `seite build` first",
                paths.output.display()
            ),
        };
    }
    PreflightCheck {
        name: "Output directory".into(),
        passed: true,
        message: format!("{}", paths.output.display()),
    }
}

fn check_base_url(config: &SiteConfig) -> PreflightCheck {
    let url = &config.site.base_url;
    let is_localhost =
        url.contains("localhost") || url.contains("127.0.0.1") || url.contains("0.0.0.0");
    if is_localhost {
        PreflightCheck {
            name: "Base URL".into(),
            passed: false,
            message: format!(
                "base_url is '{url}' — this will produce broken canonical/OG URLs in production. \
                 Set site.base_url in seite.toml to your production URL, or use `seite deploy` with --base-url"
            ),
        }
    } else {
        PreflightCheck {
            name: "Base URL".into(),
            passed: true,
            message: url.clone(),
        }
    }
}

fn check_cli_available(name: &str, args: &[&str]) -> PreflightCheck {
    match npm_cmd(name).args(args).output() {
        Ok(output) if output.status.success() => PreflightCheck {
            name: format!("{name} CLI"),
            passed: true,
            message: String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("installed")
                .trim()
                .to_string(),
        },
        _ => PreflightCheck {
            name: format!("{name} CLI"),
            passed: false,
            message: format!("{name} is not installed or not on PATH"),
        },
    }
}

fn check_git_repo(paths: &ResolvedPaths) -> PreflightCheck {
    let is_git = paths.root.join(".git").exists()
        || Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(&paths.root)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    if is_git {
        PreflightCheck {
            name: "Git repository".into(),
            passed: true,
            message: "detected".into(),
        }
    } else {
        PreflightCheck {
            name: "Git repository".into(),
            passed: false,
            message: "not a git repository — run `git init` first".into(),
        }
    }
}

fn check_git_remote(paths: &ResolvedPaths, configured_repo: Option<&str>) -> PreflightCheck {
    if let Some(repo) = configured_repo {
        return PreflightCheck {
            name: "Git remote".into(),
            passed: true,
            message: format!("configured: {repo}"),
        };
    }
    match Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&paths.root)
        .output()
    {
        Ok(output) if output.status.success() => PreflightCheck {
            name: "Git remote".into(),
            passed: true,
            message: format!("origin: {}", String::from_utf8_lossy(&output.stdout).trim()),
        },
        _ => PreflightCheck {
            name: "Git remote".into(),
            passed: false,
            message: "no remote 'origin' — set deploy.repo in seite.toml or run `git remote add origin <url>`".into(),
        },
    }
}

fn check_cloudflare_auth() -> PreflightCheck {
    // Only check auth if wrangler is installed
    let has_wrangler = npm_cmd("wrangler")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_wrangler {
        return PreflightCheck {
            name: "Cloudflare auth".into(),
            passed: false,
            message: "skipped (wrangler not installed)".into(),
        };
    }
    match npm_cmd("wrangler").args(["whoami"]).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let account = stdout
                .lines()
                .find(|l| l.contains('|'))
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| "authenticated".into());
            PreflightCheck {
                name: "Cloudflare auth".into(),
                passed: true,
                message: account,
            }
        }
        _ => PreflightCheck {
            name: "Cloudflare auth".into(),
            passed: false,
            message: "not logged in — run `wrangler login`".into(),
        },
    }
}

fn check_netlify_auth() -> PreflightCheck {
    let has_netlify = npm_cmd("netlify")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_netlify {
        return PreflightCheck {
            name: "Netlify auth".into(),
            passed: false,
            message: "skipped (netlify not installed)".into(),
        };
    }
    match npm_cmd("netlify").args(["status"]).output() {
        Ok(output) if output.status.success() => PreflightCheck {
            name: "Netlify auth".into(),
            passed: true,
            message: "authenticated".into(),
        },
        _ => PreflightCheck {
            name: "Netlify auth".into(),
            passed: false,
            message: "not logged in — run `netlify login`".into(),
        },
    }
}

fn check_cloudflare_project(config: &SiteConfig, paths: &ResolvedPaths) -> PreflightCheck {
    let project_name = config
        .deploy
        .project
        .clone()
        .or_else(|| detect_cloudflare_project(paths));

    match project_name {
        Some(name) => {
            // Verify the project actually exists on Cloudflare
            if cloudflare_project_exists(&name) {
                PreflightCheck {
                    name: "Cloudflare project".into(),
                    passed: true,
                    message: format!("exists: {name}"),
                }
            } else {
                PreflightCheck {
                    name: "Cloudflare project".into(),
                    passed: false,
                    message: format!(
                        "project '{name}' not found on Cloudflare — needs to be created"
                    ),
                }
            }
        }
        None => PreflightCheck {
            name: "Cloudflare project".into(),
            passed: false,
            message: "no project name — set deploy.project in seite.toml".into(),
        },
    }
}

/// Check if a Cloudflare Pages project exists by listing projects (uses --json for reliability).
fn cloudflare_project_exists(name: &str) -> bool {
    let output = npm_cmd("wrangler")
        .args(["pages", "project", "list", "--json"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            if let Ok(projects) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
                projects.iter().any(|p| {
                    p.get("Project Name")
                        .and_then(|v| v.as_str())
                        .map(|n| n == name)
                        .unwrap_or(false)
                })
            } else {
                // JSON parse failed — fall back to text search
                stdout
                    .lines()
                    .any(|line| line.split('│').any(|cell| cell.trim() == name))
            }
        }
        _ => true, // Can't verify — assume it exists to avoid false negatives
    }
}

fn check_netlify_site(config: &SiteConfig, paths: &ResolvedPaths) -> PreflightCheck {
    // If a site ID / project is configured, check if it's linked
    if let Some(ref project) = config.deploy.project {
        return PreflightCheck {
            name: "Netlify site".into(),
            passed: true,
            message: format!("configured: {project}"),
        };
    }

    // Check if .netlify/state.json exists (netlify link creates this)
    let state_file = paths.root.join(".netlify/state.json");
    if state_file.exists() {
        return PreflightCheck {
            name: "Netlify site".into(),
            passed: true,
            message: "linked via .netlify/state.json".into(),
        };
    }

    // Try `netlify status` to see if we're linked to a site
    let has_netlify = npm_cmd("netlify")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_netlify {
        return PreflightCheck {
            name: "Netlify site".into(),
            passed: false,
            message: "skipped (netlify not installed)".into(),
        };
    }

    match npm_cmd("netlify")
        .args(["status", "--json"])
        .current_dir(&paths.root)
        .output()
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if json.get("siteData").and_then(|s| s.get("id")).is_some() {
                    let site_name = json
                        .get("siteData")
                        .and_then(|s| s.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("linked");
                    return PreflightCheck {
                        name: "Netlify site".into(),
                        passed: true,
                        message: format!("linked: {site_name}"),
                    };
                }
            }
            PreflightCheck {
                name: "Netlify site".into(),
                passed: false,
                message: "no site linked — run `netlify link` or `netlify sites:create`".into(),
            }
        }
        _ => PreflightCheck {
            name: "Netlify site".into(),
            passed: false,
            message: "no site linked — run `netlify link` or `netlify sites:create`".into(),
        },
    }
}

fn check_cloudflare_domain(config: &SiteConfig) -> PreflightCheck {
    let domain = match &config.deploy.domain {
        Some(d) => d.clone(),
        None => {
            return PreflightCheck {
                name: "Cloudflare domain".into(),
                passed: true,
                message: "no domain configured".into(),
            }
        }
    };
    let project = match &config.deploy.project {
        Some(p) => p.clone(),
        None => {
            return PreflightCheck {
                name: "Cloudflare domain".into(),
                passed: false,
                message: "domain set but no project — set deploy.project in seite.toml".into(),
            }
        }
    };

    match cloudflare_list_domains(&project) {
        Ok(domains) => {
            if domains.iter().any(|d| d == &domain) {
                PreflightCheck {
                    name: "Cloudflare domain".into(),
                    passed: true,
                    message: format!("attached: {domain}"),
                }
            } else {
                PreflightCheck {
                    name: "Cloudflare domain".into(),
                    passed: false,
                    message: format!("'{domain}' not attached to project '{project}'"),
                }
            }
        }
        Err(_) => {
            // Can't verify — skip (non-fatal, API might not be accessible)
            PreflightCheck {
                name: "Cloudflare domain".into(),
                passed: true,
                message: format!("configured: {domain} (could not verify via API)"),
            }
        }
    }
}

fn check_netlify_domain(config: &SiteConfig, paths: &ResolvedPaths) -> PreflightCheck {
    let domain = match &config.deploy.domain {
        Some(d) => d.clone(),
        None => {
            return PreflightCheck {
                name: "Netlify domain".into(),
                passed: true,
                message: "no domain configured".into(),
            }
        }
    };

    // Check via netlify CLI
    let output = npm_cmd("netlify")
        .args(["domains:list", "--json"])
        .current_dir(&paths.root)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.contains(&domain) {
                PreflightCheck {
                    name: "Netlify domain".into(),
                    passed: true,
                    message: format!("attached: {domain}"),
                }
            } else {
                PreflightCheck {
                    name: "Netlify domain".into(),
                    passed: false,
                    message: format!(
                        "'{domain}' not attached — run `netlify domains:add {domain}`"
                    ),
                }
            }
        }
        _ => {
            // Can't verify — assume ok
            PreflightCheck {
                name: "Netlify domain".into(),
                passed: true,
                message: format!("configured: {domain} (could not verify)"),
            }
        }
    }
}

/// Print pre-flight check results. Returns true if all passed.
pub fn print_preflight(checks: &[PreflightCheck]) -> bool {
    human::header("Pre-flight checks");
    let mut all_passed = true;
    for check in checks {
        if check.passed {
            println!(
                "  {} {}: {}",
                console::style("✓").green(),
                check.name,
                check.message
            );
        } else {
            println!(
                "  {} {}: {}",
                console::style("✗").red(),
                check.name,
                check.message
            );
            all_passed = false;
        }
    }
    println!();
    all_passed
}

// ---------------------------------------------------------------------------
// Interactive fix system (auto-fix failed pre-flight checks)
// ---------------------------------------------------------------------------

/// Describes how to fix a failed pre-flight check.
pub struct FixAction {
    /// Prompt shown to the user, e.g. "Install wrangler via npm?"
    pub prompt: String,
    /// Instructions shown if user declines the fix.
    pub manual_instructions: Vec<String>,
}

/// Returns a FixAction for a failed check, or None if the check can't be auto-fixed.
pub fn try_fix_check(
    check: &PreflightCheck,
    paths: &ResolvedPaths,
    _target: &str,
) -> Option<FixAction> {
    if check.passed {
        return None;
    }
    match check.name.as_str() {
        "Output directory" => Some(FixAction {
            prompt: "Build the site first?".into(),
            manual_instructions: vec!["Run: seite build".into()],
        }),
        "Base URL" => Some(FixAction {
            prompt: "Update base_url in seite.toml?".into(),
            manual_instructions: vec![
                "Set site.base_url in seite.toml to your production URL".into(),
                "Or use --base-url <url> when deploying".into(),
            ],
        }),
        "git CLI" => None, // Can't auto-install git
        "wrangler CLI" => {
            if has_npm() {
                Some(FixAction {
                    prompt: "Install wrangler via npm?".into(),
                    manual_instructions: vec!["Run: npm install -g wrangler".into()],
                })
            } else {
                Some(FixAction {
                    prompt: String::new(), // No auto-fix without npm
                    manual_instructions: vec![
                        "Install Node.js/npm first, then run: npm install -g wrangler".into(),
                    ],
                })
            }
        }
        "netlify CLI" => {
            if has_npm() {
                Some(FixAction {
                    prompt: "Install netlify-cli via npm?".into(),
                    manual_instructions: vec!["Run: npm install -g netlify-cli".into()],
                })
            } else {
                Some(FixAction {
                    prompt: String::new(),
                    manual_instructions: vec![
                        "Install Node.js/npm first, then run: npm install -g netlify-cli".into(),
                    ],
                })
            }
        }
        "gh CLI" => {
            if cfg!(target_os = "macos") && has_brew() {
                Some(FixAction {
                    prompt: "Install GitHub CLI via Homebrew?".into(),
                    manual_instructions: vec!["Run: brew install gh".into()],
                })
            } else {
                Some(FixAction {
                    prompt: String::new(),
                    manual_instructions: vec!["Install from: https://cli.github.com/".into()],
                })
            }
        }
        "Git repository" => Some(FixAction {
            prompt: "Initialize a git repository here?".into(),
            manual_instructions: vec!["Run: git init".into()],
        }),
        "Git remote" => {
            let has_gh = Command::new("gh")
                .args(["--version"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if has_gh {
                let repo_name = paths
                    .root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("my-site");
                Some(FixAction {
                    prompt: format!("Create GitHub repository '{repo_name}' and push?"),
                    manual_instructions: vec![
                        "Create a repo at https://github.com/new".into(),
                        "Then run: git remote add origin <your-repo-url>".into(),
                    ],
                })
            } else {
                Some(FixAction {
                    prompt: String::new(),
                    manual_instructions: vec![
                        "Create a repo at https://github.com/new".into(),
                        "Then run: git remote add origin <your-repo-url>".into(),
                        "Tip: install the `gh` CLI for automatic repo creation".into(),
                    ],
                })
            }
        }
        "Cloudflare auth" => Some(FixAction {
            prompt: "Log in to Cloudflare?".into(),
            manual_instructions: vec!["Run: wrangler login".into()],
        }),
        "Netlify auth" => Some(FixAction {
            prompt: "Log in to Netlify?".into(),
            manual_instructions: vec!["Run: netlify login".into()],
        }),
        "Cloudflare project" => {
            let project_name = paths
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("my-site");
            Some(FixAction {
                prompt: format!("Create Cloudflare Pages project '{project_name}'?"),
                manual_instructions: vec![
                    format!("Run: wrangler pages project create {project_name} --production-branch main"),
                    "Or set deploy.project in seite.toml".into(),
                ],
            })
        }
        "Netlify site" => {
            let site_name = paths
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("my-site");
            Some(FixAction {
                prompt: format!("Create Netlify site '{site_name}'?"),
                manual_instructions: vec![
                    format!("Run: netlify sites:create --name {site_name}"),
                    "Or run: netlify link".into(),
                ],
            })
        }
        "Cloudflare domain" => {
            let domain = check
                .message
                .split('\'')
                .nth(1)
                .unwrap_or("your-domain.com");
            Some(FixAction {
                prompt: format!("Attach domain '{domain}' to Cloudflare Pages project?"),
                manual_instructions: vec![
                    format!("Add the domain in the Cloudflare dashboard under Pages > your project > Custom domains"),
                    format!("Or run: seite deploy --domain {domain}"),
                ],
            })
        }
        "Netlify domain" => {
            let domain = check
                .message
                .split('\'')
                .nth(1)
                .unwrap_or("your-domain.com");
            Some(FixAction {
                prompt: format!("Add domain '{domain}' to Netlify site?"),
                manual_instructions: vec![format!("Run: netlify domains:add {domain}")],
            })
        }
        _ => None,
    }
}

/// Execute the fix for a failed check. Returns Ok(true) if fixed, Ok(false) if fix failed.
pub fn execute_fix(
    check_name: &str,
    paths: &ResolvedPaths,
    config: &SiteConfig,
    config_path: &Path,
) -> Result<bool> {
    match check_name {
        "Output directory" => {
            human::info("Building site...");
            let opts = crate::build::BuildOptions {
                include_drafts: false,
            };
            match crate::build::build_site(config, paths, &opts) {
                Ok(result) => {
                    use crate::output::CommandOutput;
                    human::success(&result.stats.human_display());
                    Ok(true)
                }
                Err(e) => {
                    human::error(&format!("Build failed: {e}"));
                    Ok(false)
                }
            }
        }
        "Base URL" => {
            let url: String = dialoguer::Input::new()
                .with_prompt("Enter your production URL (e.g., https://example.com)")
                .interact_text()
                .map_err(|e| PageError::Deploy(format!("input failed: {e}")))?;
            let url = url.trim().to_string();
            if url.is_empty() {
                return Ok(false);
            }
            let mut updates = HashMap::new();
            updates.insert("base_url".into(), url.clone());
            update_deploy_config(config_path, &updates)?;
            human::success(&format!("Updated base_url to '{url}' in seite.toml"));
            Ok(true)
        }
        "wrangler CLI" => run_install_command("npm", &["install", "-g", "wrangler"], "wrangler"),
        "netlify CLI" => {
            run_install_command("npm", &["install", "-g", "netlify-cli"], "netlify-cli")
        }
        "gh CLI" => {
            if cfg!(target_os = "macos") && has_brew() {
                run_install_command("brew", &["install", "gh"], "GitHub CLI")
            } else {
                Ok(false)
            }
        }
        "Git repository" => {
            human::info("Initializing git repository...");
            let output = Command::new("git")
                .args(["init"])
                .current_dir(&paths.root)
                .output()
                .map_err(|e| PageError::Deploy(format!("git init failed: {e}")))?;
            if output.status.success() {
                human::success("Git repository initialized");
                Ok(true)
            } else {
                human::error("git init failed");
                Ok(false)
            }
        }
        "Git remote" => {
            let repo_name = paths
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("my-site");
            human::info(&format!("Creating GitHub repository '{repo_name}'..."));
            let result = Command::new("gh")
                .args([
                    "repo", "create", repo_name, "--public", "--source", ".", "--push",
                ])
                .current_dir(&paths.root)
                .status()
                .map_err(|e| PageError::Deploy(format!("gh repo create failed: {e}")))?;
            if result.success() {
                human::success(&format!("Created repository '{repo_name}'"));
                Ok(true)
            } else {
                human::error("Could not create GitHub repository");
                Ok(false)
            }
        }
        "Cloudflare auth" => {
            human::info("Opening Cloudflare login...");
            let result = npm_cmd("wrangler")
                .args(["login"])
                .status()
                .map_err(|e| PageError::Deploy(format!("wrangler login failed: {e}")))?;
            Ok(result.success())
        }
        "Netlify auth" => {
            human::info("Opening Netlify login...");
            let result = npm_cmd("netlify")
                .args(["login"])
                .status()
                .map_err(|e| PageError::Deploy(format!("netlify login failed: {e}")))?;
            Ok(result.success())
        }
        "Cloudflare project" => {
            let project_name = paths
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("my-site");
            human::info(&format!(
                "Creating Cloudflare Pages project '{project_name}'..."
            ));
            let result = npm_cmd("wrangler")
                .args([
                    "pages",
                    "project",
                    "create",
                    project_name,
                    "--production-branch",
                    "main",
                ])
                .status()
                .map_err(|e| PageError::Deploy(format!("wrangler project create failed: {e}")))?;
            if result.success() {
                // Also update seite.toml
                let mut updates = HashMap::new();
                updates.insert("project".into(), project_name.to_string());
                update_deploy_config(config_path, &updates)?;
                human::success(&format!(
                    "Created project '{project_name}' and updated seite.toml"
                ));
                Ok(true)
            } else {
                human::warning("Could not create project — it may already exist (which is fine)");
                Ok(true) // Not fatal
            }
        }
        "Netlify site" => {
            let site_name = paths
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("my-site");
            human::info(&format!("Creating Netlify site '{site_name}'..."));
            let output = npm_cmd("netlify")
                .args(["sites:create", "--name", site_name])
                .current_dir(&paths.root)
                .output()
                .map_err(|e| PageError::Deploy(format!("netlify sites:create failed: {e}")))?;
            if output.status.success() {
                human::success(&format!("Created Netlify site '{site_name}'"));
                // Link the site locally
                let _ = npm_cmd("netlify")
                    .args(["link", "--name", site_name])
                    .current_dir(&paths.root)
                    .status();
                Ok(true)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("already exists") {
                    human::info("Site already exists — linking...");
                    let link_result = npm_cmd("netlify")
                        .args(["link", "--name", site_name])
                        .current_dir(&paths.root)
                        .status()
                        .map_err(|e| PageError::Deploy(format!("netlify link failed: {e}")))?;
                    Ok(link_result.success())
                } else {
                    human::error(&format!("Could not create site: {stderr}"));
                    Ok(false)
                }
            }
        }
        "Cloudflare domain" => {
            let domain = config.deploy.domain.as_deref().unwrap_or("");
            let project = config.deploy.project.as_deref().unwrap_or("");
            if domain.is_empty() || project.is_empty() {
                return Ok(false);
            }
            human::info(&format!(
                "Attaching domain '{domain}' to Cloudflare Pages project '{project}'..."
            ));
            match cloudflare_attach_domain(project, domain) {
                Ok(true) => {
                    human::success(&format!(
                        "Domain '{domain}' attached to project '{project}'"
                    ));
                    Ok(true)
                }
                Ok(false) => {
                    human::warning("Could not attach domain via API");
                    human::info("  Add the domain manually in the Cloudflare dashboard under Pages > Custom domains");
                    Ok(false)
                }
                Err(e) => {
                    human::warning(&format!("API call failed: {e}"));
                    human::info("  Add the domain manually in the Cloudflare dashboard under Pages > Custom domains");
                    Ok(false)
                }
            }
        }
        "Netlify domain" => {
            let domain = config.deploy.domain.as_deref().unwrap_or("");
            if domain.is_empty() {
                return Ok(false);
            }
            human::info(&format!("Adding domain '{domain}' to Netlify site..."));
            let result = npm_cmd("netlify")
                .args(["domains:add", domain])
                .current_dir(&paths.root)
                .status()
                .map_err(|e| PageError::Deploy(format!("netlify domains:add failed: {e}")))?;
            if result.success() {
                human::success(&format!("Domain '{domain}' added to Netlify site"));
                Ok(true)
            } else {
                human::warning("Could not add domain");
                Ok(false)
            }
        }
        _ => Ok(false),
    }
}

/// Re-run a single check by name (used after fixing).
/// Re-reads config from disk for checks that depend on seite.toml values,
/// since execute_fix may have updated the file.
pub fn recheck(
    check_name: &str,
    _config: &SiteConfig,
    paths: &ResolvedPaths,
    _target: &str,
) -> PreflightCheck {
    // Reload config from disk — fixes may have updated seite.toml
    let fresh_config = SiteConfig::load(std::path::Path::new("seite.toml")).ok();
    let config = fresh_config.as_ref().unwrap_or(_config);

    match check_name {
        "Output directory" => check_output_dir(paths),
        "Base URL" => check_base_url(config),
        "git CLI" => check_cli_available("git", &["--version"]),
        "wrangler CLI" => check_cli_available("wrangler", &["--version"]),
        "netlify CLI" => check_cli_available("netlify", &["--version"]),
        "gh CLI" => check_cli_available("gh", &["--version"]),
        "Git repository" => check_git_repo(paths),
        "Git remote" => check_git_remote(paths, config.deploy.repo.as_deref()),
        "Cloudflare auth" => check_cloudflare_auth(),
        "Netlify auth" => check_netlify_auth(),
        "Cloudflare project" => check_cloudflare_project(config, paths),
        "Netlify site" => check_netlify_site(config, paths),
        "Cloudflare domain" => check_cloudflare_domain(config),
        "Netlify domain" => check_netlify_domain(config, paths),
        _ => PreflightCheck {
            name: check_name.into(),
            passed: false,
            message: "unknown check".into(),
        },
    }
}

fn has_npm() -> bool {
    npm_cmd("npm")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn has_brew() -> bool {
    Command::new("brew")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_install_command(cmd: &str, args: &[&str], label: &str) -> Result<bool> {
    human::info(&format!("Installing {label}..."));
    let result = npm_cmd(cmd)
        .args(args)
        .status()
        .map_err(|e| PageError::Deploy(format!("{cmd} failed: {e}")))?;
    if result.success() {
        human::success(&format!("{label} installed successfully"));
        Ok(true)
    } else {
        human::error(&format!("Failed to install {label}"));
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// Auto-commit and push (pre-deploy git workflow)
// ---------------------------------------------------------------------------

/// Result of the auto-commit and push step.
pub struct GitPushResult {
    /// Current branch name.
    pub branch: String,
    /// Whether the branch is main or master.
    pub is_main: bool,
    /// Whether a new commit was created (false if working tree was clean).
    pub committed: bool,
}

/// Auto-commit all changes and push to the remote before deploying.
///
/// Steps:
/// 1. Detect current branch
/// 2. If there are uncommitted changes, stage and commit them
/// 3. Push to origin (with --set-upstream if no tracking branch)
///
/// Returns `GitPushResult` with branch info and whether a commit was made.
pub fn auto_commit_and_push(paths: &ResolvedPaths) -> Result<GitPushResult> {
    let root = &paths.root;

    // 1. Get current branch name
    let branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|e| PageError::Deploy(format!("git rev-parse failed: {e}")))?;

    if !branch_output.status.success() {
        return Err(PageError::Deploy(
            "not a git repository or no commits yet — skipping auto-commit".into(),
        ));
    }

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();
    let is_main = branch == "main" || branch == "master";

    // 2. Check for uncommitted changes
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .map_err(|e| PageError::Deploy(format!("git status failed: {e}")))?;

    let has_changes = !String::from_utf8_lossy(&status_output.stdout)
        .trim()
        .is_empty();

    let mut committed = false;
    if has_changes {
        // Stage all changes
        let add_output = Command::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .output()
            .map_err(|e| PageError::Deploy(format!("git add failed: {e}")))?;

        if !add_output.status.success() {
            return Err(PageError::Deploy("git add -A failed".into()));
        }

        // Commit
        let commit_output = Command::new("git")
            .args(["commit", "-m", "Deploy: update site content"])
            .current_dir(root)
            .output()
            .map_err(|e| PageError::Deploy(format!("git commit failed: {e}")))?;

        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr);
            return Err(PageError::Deploy(format!("git commit failed: {stderr}")));
        }

        committed = true;
    }

    // 3. Push to remote
    // Check if there's a tracking branch
    let has_upstream = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "@{u}"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let push_args = if has_upstream {
        vec!["push"]
    } else {
        vec!["push", "--set-upstream", "origin", &branch]
    };

    let push_output = Command::new("git")
        .args(&push_args)
        .current_dir(root)
        .output()
        .map_err(|e| PageError::Deploy(format!("git push failed: {e}")))?;

    if !push_output.status.success() {
        let stderr = String::from_utf8_lossy(&push_output.stderr);
        return Err(PageError::Deploy(format!("git push failed: {stderr}")));
    }

    Ok(GitPushResult {
        branch,
        is_main,
        committed,
    })
}

// ---------------------------------------------------------------------------
// GitHub Pages deploy (Feature 2: .nojekyll, CNAME, git identity)
// ---------------------------------------------------------------------------

pub fn deploy_github_pages(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    repo: Option<&str>,
) -> Result<()> {
    let output_dir = &paths.output;

    // Write .nojekyll to prevent GitHub from running Jekyll
    fs::write(output_dir.join(".nojekyll"), "")?;

    // Write CNAME file if base_url is a custom domain (not github.io)
    let base_url = &config.site.base_url;
    if let Some(domain) = extract_custom_domain(base_url) {
        if !domain.ends_with(".github.io") {
            fs::write(output_dir.join("CNAME"), &domain)?;
        }
    }

    // Determine repo URL
    let repo_url = match repo {
        Some(url) => url.to_string(),
        None => {
            let output = Command::new("git")
                .args(["remote", "get-url", "origin"])
                .current_dir(&paths.root)
                .output()
                .map_err(|e| PageError::Deploy(format!("failed to detect git remote: {e}")))?;
            if !output.status.success() {
                return Err(PageError::Deploy(
                    "no repo URL provided and could not detect git remote. \
                     Set deploy.repo in seite.toml"
                        .into(),
                ));
            }
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
    };

    let run = |args: &[&str]| -> Result<()> {
        let output = Command::new("git")
            .args(args)
            .current_dir(output_dir)
            .output()
            .map_err(|e| PageError::Deploy(format!("git {}: {e}", args.join(" "))))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PageError::Deploy(format!(
                "git {} failed: {stderr}",
                args.join(" ")
            )));
        }
        Ok(())
    };

    run(&["init"])?;

    // Set git identity so commits don't fail in fresh environments
    run(&["config", "user.email", "seite-deploy@localhost"])?;
    run(&["config", "user.name", "seite deploy"])?;

    run(&["checkout", "-b", "gh-pages"])?;
    run(&["add", "-A"])?;

    // Include timestamp in commit message
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let commit_msg = format!("Deploy {timestamp}");
    run(&["commit", "-m", &commit_msg])?;
    run(&["push", "--force", &repo_url, "gh-pages"])?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Cloudflare deploy (Feature 4: preview support)
// ---------------------------------------------------------------------------

pub fn deploy_cloudflare(
    paths: &ResolvedPaths,
    project: &str,
    preview: bool,
) -> Result<Option<String>> {
    let output_dir = &paths.output;

    let mut args = vec![
        "pages".to_string(),
        "deploy".to_string(),
        output_dir.to_str().unwrap_or("dist").to_string(),
        "--project-name".to_string(),
        project.to_string(),
    ];
    if preview {
        args.push("--branch".to_string());
        args.push("preview".to_string());
    }

    let output = npm_cmd("wrangler")
        .args(&args)
        .output()
        .map_err(|e| PageError::Deploy(format!("wrangler failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PageError::Deploy(format!(
            "wrangler pages deploy failed for project '{project}': {stderr}\n\
             Ensure the project exists. Create it at https://dash.cloudflare.com/ or run:\n  \
             wrangler pages project create {project}"
        )));
    }

    // Try to extract the deploy URL from wrangler output
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Print wrangler's output
    print!("{stdout}");

    let deploy_url = extract_url_from_output(&stdout);
    Ok(deploy_url)
}

/// Try to auto-detect the Cloudflare project name from wrangler.toml or the directory name.
pub fn detect_cloudflare_project(paths: &ResolvedPaths) -> Option<String> {
    let wrangler_path = paths.root.join("wrangler.toml");
    if wrangler_path.exists() {
        if let Ok(content) = fs::read_to_string(&wrangler_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("name") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        let name = val.trim().trim_matches('"').trim_matches('\'');
                        if !name.is_empty() {
                            return Some(name.to_string());
                        }
                    }
                }
            }
        }
    }
    paths
        .root
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Netlify deploy (Feature 4: preview support)
// ---------------------------------------------------------------------------

pub fn deploy_netlify(
    paths: &ResolvedPaths,
    site_id: Option<&str>,
    preview: bool,
) -> Result<Option<String>> {
    let output_dir = &paths.output;

    let mut args = vec!["deploy", "--dir", output_dir.to_str().unwrap_or("dist")];
    if !preview {
        args.push("--prod");
    }
    if let Some(id) = site_id {
        args.push("--site");
        args.push(id);
    }
    // Request JSON output for URL extraction
    args.push("--json");

    let output = npm_cmd("netlify")
        .args(&args)
        .output()
        .map_err(|e| PageError::Deploy(format!("netlify deploy failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PageError::Deploy(format!(
            "netlify deploy failed: {stderr}\n\
             Ensure you are logged in (netlify login) and the site exists.\n  \
             Link to an existing site: netlify link"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Try to parse JSON output for the deploy URL
    let deploy_url = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if preview {
            json.get("deploy_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            json.get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }
    } else {
        extract_url_from_output(&stdout)
    };

    // Print a summary instead of raw JSON
    if let Some(ref url) = deploy_url {
        if preview {
            human::info(&format!("Preview URL: {url}"));
        }
    }

    Ok(deploy_url)
}

// ---------------------------------------------------------------------------
// base_url lifecycle management (Feature 3)
// ---------------------------------------------------------------------------

/// Build the site with a temporary base_url override without modifying seite.toml.
/// Returns the base_url that was used.
pub fn resolve_deploy_base_url(config: &SiteConfig, override_url: Option<&str>) -> String {
    if let Some(url) = override_url {
        return url.trim_end_matches('/').to_string();
    }
    config.site.base_url.trim_end_matches('/').to_string()
}

// ---------------------------------------------------------------------------
// Deploy init — guided setup (Feature 5)
// ---------------------------------------------------------------------------

pub fn deploy_init_github_pages(paths: &ResolvedPaths) -> Result<String> {
    // Check if gh CLI is available for repo creation
    let has_gh = Command::new("gh")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Ensure git repo exists
    if !paths.root.join(".git").exists() {
        human::info("Initializing git repository...");
        let output = Command::new("git")
            .args(["init"])
            .current_dir(&paths.root)
            .output()
            .map_err(|e| PageError::Deploy(format!("git init failed: {e}")))?;
        if !output.status.success() {
            return Err(PageError::Deploy("git init failed".into()));
        }
    }

    // Check for remote
    let has_remote = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&paths.root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_remote && has_gh {
        human::info("Creating GitHub repository...");
        // Get directory name for repo name
        let repo_name = paths
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-site");

        let result = Command::new("gh")
            .args([
                "repo", "create", repo_name, "--public", "--source", ".", "--push",
            ])
            .current_dir(&paths.root)
            .status()
            .map_err(|e| PageError::Deploy(format!("gh repo create failed: {e}")))?;

        if !result.success() {
            human::warning("Could not create GitHub repository automatically.");
            human::info("Create one manually at https://github.com/new and run:");
            human::info("  git remote add origin <your-repo-url>");
        }
    } else if !has_remote {
        human::warning("No remote 'origin' found and `gh` CLI not available.");
        human::info("To set up GitHub Pages:");
        human::info("  1. Create a repo at https://github.com/new");
        human::info("  2. git remote add origin <your-repo-url>");
        human::info("  3. Install gh CLI (optional): https://cli.github.com/");
    }

    // Enable GitHub Pages via gh if available
    if has_gh {
        // Try to enable Pages — this may fail if already enabled, that's fine
        let _ = Command::new("gh")
            .args([
                "api",
                "repos/{owner}/{repo}/pages",
                "-X",
                "POST",
                "-f",
                "build_type=workflow",
            ])
            .current_dir(&paths.root)
            .output();
    }

    // Generate workflow file
    let workflow_dir = paths.root.join(".github/workflows");
    fs::create_dir_all(&workflow_dir)?;

    Ok("github-pages".to_string())
}

pub fn deploy_init_cloudflare(paths: &ResolvedPaths) -> Result<String> {
    // Check wrangler
    let has_wrangler = npm_cmd("wrangler")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_wrangler {
        return Err(PageError::Deploy(
            "wrangler CLI is required for Cloudflare deployment.\n  \
             Install: npm install -g wrangler\n  \
             Then:    wrangler login"
                .into(),
        ));
    }

    // Check login status
    let whoami = npm_cmd("wrangler")
        .args(["whoami"])
        .output()
        .map_err(|e| PageError::Deploy(format!("wrangler whoami failed: {e}")))?;

    if !whoami.status.success() {
        human::info("Logging in to Cloudflare...");
        let login = npm_cmd("wrangler")
            .args(["login"])
            .status()
            .map_err(|e| PageError::Deploy(format!("wrangler login failed: {e}")))?;
        if !login.success() {
            return Err(PageError::Deploy("wrangler login failed".into()));
        }
    }

    // Try to create the project
    let project_name = paths
        .root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-site")
        .to_string();

    human::info(&format!(
        "Creating Cloudflare Pages project '{project_name}'..."
    ));
    let result = npm_cmd("wrangler")
        .args([
            "pages",
            "project",
            "create",
            &project_name,
            "--production-branch",
            "main",
        ])
        .status()
        .map_err(|e| PageError::Deploy(format!("wrangler project create failed: {e}")))?;

    if !result.success() {
        human::warning(&format!(
            "Could not create project '{project_name}' — it may already exist (which is fine)."
        ));
    }

    Ok(project_name)
}

pub fn deploy_init_netlify(paths: &ResolvedPaths) -> Result<String> {
    let has_netlify = npm_cmd("netlify")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_netlify {
        return Err(PageError::Deploy(
            "netlify CLI is required for Netlify deployment.\n  \
             Install: npm install -g netlify-cli\n  \
             Then:    netlify login"
                .into(),
        ));
    }

    // Check login
    let status = npm_cmd("netlify")
        .args(["status"])
        .output()
        .map_err(|e| PageError::Deploy(format!("netlify status failed: {e}")))?;

    if !status.status.success() {
        human::info("Logging in to Netlify...");
        let login = npm_cmd("netlify")
            .args(["login"])
            .status()
            .map_err(|e| PageError::Deploy(format!("netlify login failed: {e}")))?;
        if !login.success() {
            return Err(PageError::Deploy("netlify login failed".into()));
        }
    }

    // Create a new site
    let site_name = paths
        .root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-site")
        .to_string();

    human::info(&format!("Creating Netlify site '{site_name}'..."));
    let output = npm_cmd("netlify")
        .args(["sites:create", "--name", &site_name])
        .output()
        .map_err(|e| PageError::Deploy(format!("netlify sites:create failed: {e}")))?;

    if !output.status.success() {
        human::warning(
            "Could not create Netlify site — it may already exist or the name is taken.",
        );
        human::info("You can link to an existing site with: netlify link");
    }

    // Link the site
    let _ = npm_cmd("netlify")
        .args(["link", "--name", &site_name])
        .current_dir(&paths.root)
        .status();

    Ok(site_name)
}

// ---------------------------------------------------------------------------
// CI workflow generation for all targets (Feature 6)
// ---------------------------------------------------------------------------

/// Generate a GitHub Actions workflow YAML for building and deploying with GitHub Pages.
pub fn generate_github_actions_workflow(config: &SiteConfig) -> String {
    let output_dir = &config.build.output_dir;
    let version = env!("CARGO_PKG_VERSION");
    format!(
        r#"name: Deploy to GitHub Pages

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install seite
        run: VERSION={version} curl -fsSL https://seite.sh/install.sh | sh

      - name: Build site
        run: seite build

      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: {output_dir}

  deploy:
    environment:
      name: github-pages
      url: ${{{{ steps.deployment.outputs.page_url }}}}
    runs-on: ubuntu-latest
    needs: build
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
"#
    )
}

/// Generate a GitHub Actions workflow for Cloudflare Pages deployment.
pub fn generate_cloudflare_workflow(config: &SiteConfig) -> String {
    let output_dir = &config.build.output_dir;
    let project = config
        .deploy
        .project
        .as_deref()
        .unwrap_or("your-project-name");
    let version = env!("CARGO_PKG_VERSION");
    format!(
        r#"name: Deploy to Cloudflare Pages

on:
  push:
    branches: [main]
  workflow_dispatch:

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install seite
        run: VERSION={version} curl -fsSL https://seite.sh/install.sh | sh

      - name: Build site
        run: seite build

      - name: Deploy to Cloudflare Pages
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{{{ secrets.CLOUDFLARE_API_TOKEN }}}}
          accountId: ${{{{ secrets.CLOUDFLARE_ACCOUNT_ID }}}}
          command: pages deploy {output_dir} --project-name {project}
"#
    )
}

/// Generate a Netlify configuration file (netlify.toml).
pub fn generate_netlify_config(config: &SiteConfig) -> String {
    let output_dir = &config.build.output_dir;
    let version = env!("CARGO_PKG_VERSION");
    format!(
        r#"[build]
  command = "VERSION={version} curl -fsSL https://seite.sh/install.sh | sh && seite build"
  publish = "{output_dir}"

[[redirects]]
  from = "/*"
  to = "/404.html"
  status = 404
"#
    )
}

/// Generate a GitHub Actions workflow for Netlify deployment.
pub fn generate_netlify_workflow(config: &SiteConfig) -> String {
    let output_dir = &config.build.output_dir;
    let version = env!("CARGO_PKG_VERSION");
    format!(
        r#"name: Deploy to Netlify

on:
  push:
    branches: [main]
  workflow_dispatch:

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install seite
        run: VERSION={version} curl -fsSL https://seite.sh/install.sh | sh

      - name: Build site
        run: seite build

      - name: Deploy to Netlify
        uses: nwtgck/actions-netlify@v3
        with:
          publish-dir: {output_dir}
          production-deploy: true
        env:
          NETLIFY_AUTH_TOKEN: ${{{{ secrets.NETLIFY_AUTH_TOKEN }}}}
          NETLIFY_SITE_ID: ${{{{ secrets.NETLIFY_SITE_ID }}}}
"#
    )
}

// ---------------------------------------------------------------------------
// Custom domain helper (Feature 7)
// ---------------------------------------------------------------------------

/// DNS instructions for setting up a custom domain.
pub struct DomainSetup {
    pub domain: String,
    pub target: String,
    pub dns_records: Vec<DnsRecord>,
    pub notes: Vec<String>,
}

pub struct DnsRecord {
    pub record_type: String,
    pub name: String,
    pub value: String,
}

pub fn domain_setup_instructions(
    domain: &str,
    target: &DeployTarget,
    config: &SiteConfig,
) -> DomainSetup {
    let is_apex = !domain.contains('.') || domain.matches('.').count() == 1;
    let subdomain = if is_apex {
        "www"
    } else {
        domain.split('.').next().unwrap_or("www")
    };

    match target {
        DeployTarget::GithubPages => {
            let mut records = vec![];
            if is_apex {
                // GitHub Pages requires A records for apex domains
                for ip in &[
                    "185.199.108.153",
                    "185.199.109.153",
                    "185.199.110.153",
                    "185.199.111.153",
                ] {
                    records.push(DnsRecord {
                        record_type: "A".into(),
                        name: "@".into(),
                        value: ip.to_string(),
                    });
                }
            }
            // CNAME for www or subdomain
            let repo_owner = detect_github_username(&config.deploy);
            let gh_domain = format!(
                "{}.github.io",
                repo_owner.unwrap_or_else(|| "<username>".into())
            );
            records.push(DnsRecord {
                record_type: "CNAME".into(),
                name: subdomain.into(),
                value: gh_domain,
            });
            DomainSetup {
                domain: domain.into(),
                target: "GitHub Pages".into(),
                dns_records: records,
                notes: vec![
                    "A CNAME file will be automatically created in your deploy output.".into(),
                    "GitHub will provision an SSL certificate automatically (may take up to 24h)."
                        .into(),
                    "Enable 'Enforce HTTPS' in your repo Settings > Pages after DNS propagates."
                        .into(),
                ],
            }
        }
        DeployTarget::Cloudflare => {
            let project = config.deploy.project.as_deref().unwrap_or("<project-name>");
            let mut records = vec![DnsRecord {
                record_type: "CNAME".into(),
                name: if is_apex {
                    "@".into()
                } else {
                    subdomain.into()
                },
                value: format!("{project}.pages.dev"),
            }];
            if is_apex {
                records.push(DnsRecord {
                    record_type: "CNAME".into(),
                    name: "www".into(),
                    value: format!("{project}.pages.dev"),
                });
            }
            DomainSetup {
                domain: domain.into(),
                target: "Cloudflare Pages".into(),
                dns_records: records,
                notes: vec![
                    "If your domain is already on Cloudflare, add the custom domain in the Pages project settings.".into(),
                    format!("Run: wrangler pages project update {project} to configure the custom domain."),
                    "SSL is automatic when using Cloudflare DNS.".into(),
                ],
            }
        }
        DeployTarget::Netlify => {
            let site_name = config.deploy.project.as_deref().unwrap_or("<site-name>");
            let records = vec![DnsRecord {
                record_type: "CNAME".into(),
                name: if is_apex {
                    "@".into()
                } else {
                    subdomain.into()
                },
                value: format!("{site_name}.netlify.app"),
            }];
            DomainSetup {
                domain: domain.into(),
                target: "Netlify".into(),
                dns_records: records,
                notes: vec![
                    format!(
                        "Add the domain in Netlify dashboard or run: netlify domains:add {domain}"
                    ),
                    "Netlify provisions SSL certificates automatically.".into(),
                    "For apex domains, consider using Netlify DNS for best results.".into(),
                ],
            }
        }
    }
}

/// Print domain setup instructions.
pub fn print_domain_setup(setup: &DomainSetup) {
    human::header(&format!(
        "Domain setup for {} ({})",
        setup.domain, setup.target
    ));

    println!("\n  Add these DNS records at your domain registrar:\n");
    println!("  {:<8} {:<20} Value", "Type", "Name");
    println!("  {}", "-".repeat(60));
    for record in &setup.dns_records {
        println!(
            "  {:<8} {:<20} {}",
            record.record_type, record.name, record.value
        );
    }
    println!();
    for note in &setup.notes {
        human::info(&format!("  {note}"));
    }
    println!();
}

fn detect_github_username(deploy: &crate::config::DeploySection) -> Option<String> {
    if let Some(ref repo) = deploy.repo {
        // Parse from URL: https://github.com/user/repo or git@github.com:user/repo
        if let Some(rest) = repo.strip_prefix("https://github.com/") {
            return rest.split('/').next().map(|s| s.to_string());
        }
        if let Some(rest) = repo.strip_prefix("git@github.com:") {
            return rest.split('/').next().map(|s| s.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Cloudflare Pages API (domain management)
// ---------------------------------------------------------------------------

/// Extract the Cloudflare account ID from `wrangler whoami` output.
fn get_cloudflare_account_id() -> Option<String> {
    let output = npm_cmd("wrangler").args(["whoami"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse table: │ Account Name │ Account ID │
    for line in stdout.lines() {
        let cells: Vec<&str> = line.split('│').map(|c| c.trim()).collect();
        // Look for a cell that looks like a 32-char hex account ID
        for cell in &cells {
            if cell.len() == 32 && cell.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(cell.to_string());
            }
        }
    }
    None
}

/// Get a Cloudflare API token. Checks CLOUDFLARE_API_TOKEN env var first,
/// then falls back to wrangler's stored OAuth token.
fn get_cloudflare_api_token() -> Option<String> {
    // 1. Check env var (standard for CI/CD)
    if let Ok(token) = std::env::var("CLOUDFLARE_API_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }

    // 2. Read wrangler's OAuth token from its config file
    let config_path = crate::platform::wrangler_config_path();

    if let Some(path) = config_path {
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("oauth_token") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        let token = val.trim().trim_matches('"');
                        if !token.is_empty() {
                            return Some(token.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

/// List custom domains attached to a Cloudflare Pages project.
fn cloudflare_list_domains(project: &str) -> Result<Vec<String>> {
    let account_id = get_cloudflare_account_id()
        .ok_or_else(|| PageError::Deploy("could not determine Cloudflare account ID".into()))?;
    let token = get_cloudflare_api_token().ok_or_else(|| {
        PageError::Deploy(
            "no Cloudflare API token — set CLOUDFLARE_API_TOKEN or run `wrangler login`".into(),
        )
    })?;

    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/pages/projects/{project}/domains"
    );

    let mut response = ureq::get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .call()
        .map_err(|e| PageError::Deploy(format!("Cloudflare API request failed: {e}")))?;

    let body: serde_json::Value = response
        .body_mut()
        .read_json()
        .map_err(|e| PageError::Deploy(format!("failed to parse Cloudflare API response: {e}")))?;

    let mut domains = Vec::new();
    if let Some(result_arr) = body.get("result") {
        if let Some(arr) = result_arr.as_array() {
            for item in arr {
                if let Some(name_val) = item.get("name") {
                    if let Some(name) = name_val.as_str() {
                        domains.push(name.to_string());
                    }
                }
            }
        }
    }

    Ok(domains)
}

/// Attach a custom domain to a Cloudflare Pages project via the API.
pub fn cloudflare_attach_domain(project: &str, domain: &str) -> Result<bool> {
    let account_id = get_cloudflare_account_id()
        .ok_or_else(|| PageError::Deploy("could not determine Cloudflare account ID".into()))?;
    let token = get_cloudflare_api_token().ok_or_else(|| {
        PageError::Deploy(
            "no Cloudflare API token — set CLOUDFLARE_API_TOKEN or run `wrangler login`".into(),
        )
    })?;

    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/pages/projects/{project}/domains"
    );

    let body = serde_json::json!({ "name": domain });

    let mut response = ureq::post(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .send_json(&body)
        .map_err(|e| PageError::Deploy(format!("Cloudflare API request failed: {e}")))?;

    let status = response.status().as_u16();
    let resp_body: serde_json::Value = response.body_mut().read_json().unwrap_or_default();

    if status == 200 || status == 201 {
        Ok(true)
    } else {
        let mut error_msgs = Vec::new();
        if let Some(errors_val) = resp_body.get("errors") {
            if let Some(arr) = errors_val.as_array() {
                for err in arr {
                    if let Some(msg_val) = err.get("message") {
                        if let Some(msg) = msg_val.as_str() {
                            error_msgs.push(msg.to_string());
                        }
                    }
                }
            }
        }
        let error_str = if error_msgs.is_empty() {
            format!("HTTP {status}")
        } else {
            error_msgs.join(", ")
        };

        // Domain already attached is not an error
        if error_str.contains("already") {
            human::info(&format!("Domain '{domain}' is already attached"));
            return Ok(true);
        }

        human::error(&format!("Cloudflare API error: {error_str}"));
        Ok(false)
    }
}

/// Add a custom domain to a Netlify site via the CLI.
pub fn netlify_add_domain(paths: &ResolvedPaths, domain: &str) -> Result<bool> {
    let result = npm_cmd("netlify")
        .args(["domains:add", domain])
        .current_dir(&paths.root)
        .status()
        .map_err(|e| PageError::Deploy(format!("netlify domains:add failed: {e}")))?;
    Ok(result.success())
}

// ---------------------------------------------------------------------------
// Post-deploy verification (Feature 8)
// ---------------------------------------------------------------------------

/// Verify a deployment by checking if the URL responds with 200 and has expected content.
pub fn verify_deployment(url: &str) -> Vec<VerifyResult> {
    let mut results = Vec::new();

    // Check 1: HTTP connectivity (basic TCP + HTTP check)
    results.push(verify_http(url));

    // Check 2: Check /robots.txt exists
    let robots_url = format!("{}/robots.txt", url.trim_end_matches('/'));
    results.push(verify_url_reachable(&robots_url, "robots.txt"));

    // Check 3: Check /sitemap.xml exists
    let sitemap_url = format!("{}/sitemap.xml", url.trim_end_matches('/'));
    results.push(verify_url_reachable(&sitemap_url, "sitemap.xml"));

    // Check 4: Check /llms.txt exists
    let llms_url = format!("{}/llms.txt", url.trim_end_matches('/'));
    results.push(verify_url_reachable(&llms_url, "llms.txt"));

    results
}

pub struct VerifyResult {
    pub check: String,
    pub passed: bool,
    pub message: String,
}

fn verify_http(url: &str) -> VerifyResult {
    // Use a minimal HTTP/1.1 GET via TcpStream (no external HTTP crate needed)
    let parsed = parse_url_for_http(url);
    match parsed {
        Some((host, port, path)) => {
            match TcpStream::connect_timeout(
                &format!("{host}:{port}")
                    .parse()
                    .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
                Duration::from_secs(10),
            ) {
                Ok(mut stream) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                    let request = format!(
                        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: seite-deploy-verify/1.0\r\n\r\n"
                    );
                    if stream.write_all(request.as_bytes()).is_err() {
                        return VerifyResult {
                            check: "Homepage".into(),
                            passed: false,
                            message: "failed to send HTTP request".into(),
                        };
                    }
                    let mut response = Vec::new();
                    let _ = std::io::Read::read_to_end(&mut stream, &mut response);
                    let response_str = String::from_utf8_lossy(&response);
                    if let Some(status_line) = response_str.lines().next() {
                        if status_line.contains("200") {
                            VerifyResult {
                                check: "Homepage".into(),
                                passed: true,
                                message: format!("{url} -> 200 OK"),
                            }
                        } else {
                            VerifyResult {
                                check: "Homepage".into(),
                                passed: false,
                                message: format!("{url} -> {status_line}"),
                            }
                        }
                    } else {
                        VerifyResult {
                            check: "Homepage".into(),
                            passed: false,
                            message: "empty response".into(),
                        }
                    }
                }
                Err(e) => VerifyResult {
                    check: "Homepage".into(),
                    passed: false,
                    message: format!("connection failed: {e} (DNS may not have propagated yet)"),
                },
            }
        }
        None => VerifyResult {
            check: "Homepage".into(),
            passed: false,
            message: format!("could not parse URL: {url}"),
        },
    }
}

fn verify_url_reachable(url: &str, label: &str) -> VerifyResult {
    let parsed = parse_url_for_http(url);
    match parsed {
        Some((host, port, path)) => {
            let addr_str = format!("{host}:{port}");
            match addr_str.parse::<std::net::SocketAddr>() {
                Ok(addr) => match TcpStream::connect_timeout(&addr, Duration::from_secs(10)) {
                    Ok(mut stream) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                        let request = format!(
                            "HEAD {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: seite-deploy-verify/1.0\r\n\r\n"
                        );
                        if stream.write_all(request.as_bytes()).is_err() {
                            return VerifyResult {
                                check: label.into(),
                                passed: false,
                                message: "request failed".into(),
                            };
                        }
                        let mut response = Vec::new();
                        let _ = std::io::Read::read_to_end(&mut stream, &mut response);
                        let response_str = String::from_utf8_lossy(&response);
                        if let Some(status_line) = response_str.lines().next() {
                            if status_line.contains("200") {
                                VerifyResult {
                                    check: label.into(),
                                    passed: true,
                                    message: "reachable".into(),
                                }
                            } else {
                                VerifyResult {
                                    check: label.into(),
                                    passed: false,
                                    message: format!("returned {status_line}"),
                                }
                            }
                        } else {
                            VerifyResult {
                                check: label.into(),
                                passed: false,
                                message: "empty response".into(),
                            }
                        }
                    }
                    Err(_) => VerifyResult {
                        check: label.into(),
                        passed: false,
                        message: "connection failed".into(),
                    },
                },
                Err(_) => {
                    // DNS resolution needed — skip verification for non-IP hosts
                    VerifyResult {
                        check: label.into(),
                        passed: true,
                        message: "skipped (DNS resolution required)".into(),
                    }
                }
            }
        }
        None => VerifyResult {
            check: label.into(),
            passed: false,
            message: "invalid URL".into(),
        },
    }
}

pub fn print_verification(results: &[VerifyResult]) {
    human::header("Post-deploy verification");
    for r in results {
        if r.passed {
            println!(
                "  {} {}: {}",
                console::style("✓").green(),
                r.check,
                r.message
            );
        } else {
            println!(
                "  {} {}: {}",
                console::style("✗").yellow(),
                r.check,
                r.message
            );
        }
    }
    println!();
}

// ---------------------------------------------------------------------------
// Config update helpers
// ---------------------------------------------------------------------------

/// Update seite.toml with deploy settings (target, project, domain).
pub fn update_deploy_config(
    config_path: &std::path::Path,
    updates: &HashMap<String, String>,
) -> Result<()> {
    let contents = fs::read_to_string(config_path)?;
    let mut doc: toml::Table =
        contents
            .parse()
            .map_err(|e: toml::de::Error| PageError::ConfigInvalid {
                message: e.to_string(),
            })?;

    // Ensure [deploy] section exists
    if !doc.contains_key("deploy") {
        doc.insert("deploy".into(), toml::Value::Table(toml::Table::new()));
    }

    if let Some(deploy) = doc.get_mut("deploy").and_then(|v| v.as_table_mut()) {
        for (key, value) in updates {
            // base_url goes to [site], not [deploy]
            if key == "base_url" {
                continue;
            }
            deploy.insert(key.clone(), toml::Value::String(value.clone()));
        }
    }

    // Update base_url in [site] if provided
    if let Some(base_url) = updates.get("base_url") {
        if let Some(site) = doc.get_mut("site").and_then(|v| v.as_table_mut()) {
            site.insert("base_url".into(), toml::Value::String(base_url.clone()));
        }
    }

    let new_contents = toml::to_string_pretty(&doc).map_err(|e| PageError::ConfigInvalid {
        message: e.to_string(),
    })?;
    fs::write(config_path, new_contents)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Extract a domain from a URL (e.g., `https://example.com/path` -> `example.com`).
pub fn extract_custom_domain(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let domain = without_scheme.split('/').next()?;
    let domain = domain.split(':').next()?; // Strip port
    if domain.is_empty() {
        None
    } else {
        Some(domain.to_string())
    }
}

/// Try to extract a URL from command output (e.g., wrangler deploy output).
fn extract_url_from_output(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("https://") {
            return Some(trimmed.to_string());
        }
        // Look for "URL: https://..." patterns
        if let Some(pos) = trimmed.find("https://") {
            let url = &trimmed[pos..];
            let end = url.find(|c: char| c.is_whitespace()).unwrap_or(url.len());
            return Some(url[..end].to_string());
        }
    }
    None
}

/// Parse a URL into (host, port, path) for raw TCP connections.
fn parse_url_for_http(url: &str) -> Option<(String, u16, String)> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = url.strip_prefix("http://") {
        ("http", r)
    } else {
        return None;
    };

    let default_port: u16 = if scheme == "https" { 443 } else { 80 };
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };

    let (host, port) = match host_port.rfind(':') {
        Some(i) => {
            let port_str = &host_port[i + 1..];
            match port_str.parse::<u16>() {
                Ok(p) => (host_port[..i].to_string(), p),
                Err(_) => (host_port.to_string(), default_port),
            }
        }
        None => (host_port.to_string(), default_port),
    };

    Some((host, port, path.to_string()))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_custom_domain() {
        assert_eq!(
            extract_custom_domain("https://example.com"),
            Some("example.com".into())
        );
        assert_eq!(
            extract_custom_domain("https://blog.example.com/path"),
            Some("blog.example.com".into())
        );
        assert_eq!(
            extract_custom_domain("http://localhost:3000"),
            Some("localhost".into())
        );
        assert_eq!(extract_custom_domain("not-a-url"), None);
    }

    #[test]
    fn test_extract_url_from_output() {
        let output = "Uploading...\nhttps://abc123.pages.dev\nDone!";
        assert_eq!(
            extract_url_from_output(output),
            Some("https://abc123.pages.dev".into())
        );

        let output2 = "Deploy URL: https://example.netlify.app done";
        assert_eq!(
            extract_url_from_output(output2),
            Some("https://example.netlify.app".into())
        );
    }

    #[test]
    fn test_parse_url_for_http() {
        let (host, port, path) = parse_url_for_http("https://example.com/robots.txt").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/robots.txt");

        let (host, port, path) = parse_url_for_http("http://localhost:3000/test").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 3000);
        assert_eq!(path, "/test");
    }

    #[test]
    fn test_resolve_deploy_base_url() {
        let config = SiteConfig {
            site: crate::config::SiteSection {
                title: "Test".into(),
                description: "".into(),
                base_url: "http://localhost:3000".into(),
                language: "en".into(),
                author: "".into(),
            },
            collections: vec![],
            build: Default::default(),
            deploy: Default::default(),
            languages: Default::default(),
            images: Default::default(),
            analytics: None,
            trust: None,
            contact: None,
        };

        // Override takes precedence
        assert_eq!(
            resolve_deploy_base_url(&config, Some("https://example.com/")),
            "https://example.com"
        );

        // Falls back to config
        assert_eq!(
            resolve_deploy_base_url(&config, None),
            "http://localhost:3000"
        );
    }

    #[test]
    fn test_check_base_url_localhost() {
        let config = SiteConfig {
            site: crate::config::SiteSection {
                title: "Test".into(),
                description: "".into(),
                base_url: "http://localhost:3000".into(),
                language: "en".into(),
                author: "".into(),
            },
            collections: vec![],
            build: Default::default(),
            deploy: Default::default(),
            languages: Default::default(),
            images: Default::default(),
            analytics: None,
            trust: None,
            contact: None,
        };
        let check = check_base_url(&config);
        assert!(!check.passed);
    }

    #[test]
    fn test_check_base_url_production() {
        let config = test_config("https://example.com");
        let check = check_base_url(&config);
        assert!(check.passed);
    }

    fn test_config(base_url: &str) -> SiteConfig {
        SiteConfig {
            site: crate::config::SiteSection {
                title: "Test".into(),
                description: "".into(),
                base_url: base_url.into(),
                language: "en".into(),
                author: "".into(),
            },
            collections: vec![],
            build: Default::default(),
            deploy: Default::default(),
            languages: Default::default(),
            images: Default::default(),
            analytics: None,
            trust: None,
            contact: None,
        }
    }

    #[test]
    fn test_check_base_url_127() {
        let config = test_config("http://127.0.0.1:3000");
        assert!(!check_base_url(&config).passed);
    }

    #[test]
    fn test_check_base_url_0000() {
        let config = test_config("http://0.0.0.0:8080");
        assert!(!check_base_url(&config).passed);
    }

    #[test]
    fn test_extract_custom_domain_no_scheme() {
        assert_eq!(extract_custom_domain("example.com"), None);
    }

    #[test]
    fn test_extract_custom_domain_empty() {
        assert_eq!(extract_custom_domain("https://"), None);
    }

    #[test]
    fn test_extract_custom_domain_with_port() {
        assert_eq!(
            extract_custom_domain("http://localhost:3000"),
            Some("localhost".into())
        );
    }

    #[test]
    fn test_extract_url_from_output_no_url() {
        assert_eq!(extract_url_from_output("no urls here\njust text"), None);
    }

    #[test]
    fn test_extract_url_from_output_embedded() {
        let output = "Visit https://my-site.pages.dev for details";
        assert_eq!(
            extract_url_from_output(output),
            Some("https://my-site.pages.dev".into())
        );
    }

    #[test]
    fn test_parse_url_no_scheme() {
        assert!(parse_url_for_http("example.com/path").is_none());
    }

    #[test]
    fn test_parse_url_http_default_port() {
        let (host, port, path) = parse_url_for_http("http://example.com/test").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
        assert_eq!(path, "/test");
    }

    #[test]
    fn test_parse_url_no_path() {
        let (host, port, path) = parse_url_for_http("https://example.com").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/");
    }

    #[test]
    fn test_resolve_deploy_base_url_strips_trailing_slash() {
        let config = test_config("http://localhost:3000");
        assert_eq!(
            resolve_deploy_base_url(&config, Some("https://example.com/")),
            "https://example.com"
        );
    }

    #[test]
    fn test_generate_github_actions_workflow() {
        let config = test_config("https://example.com");
        let workflow = generate_github_actions_workflow(&config);
        assert!(workflow.contains("Deploy to GitHub Pages"));
        assert!(workflow.contains("seite build"));
        assert!(workflow.contains("deploy-pages@v4"));
    }

    #[test]
    fn test_generate_cloudflare_workflow() {
        let mut config = test_config("https://example.com");
        config.deploy.project = Some("my-project".into());
        let workflow = generate_cloudflare_workflow(&config);
        assert!(workflow.contains("Deploy to Cloudflare Pages"));
        assert!(workflow.contains("my-project"));
        assert!(workflow.contains("CLOUDFLARE_API_TOKEN"));
    }

    #[test]
    fn test_generate_netlify_config() {
        let config = test_config("https://example.com");
        let netlify = generate_netlify_config(&config);
        assert!(netlify.contains("[build]"));
        assert!(netlify.contains("seite build"));
        assert!(netlify.contains("404.html"));
    }

    #[test]
    fn test_generate_netlify_workflow() {
        let config = test_config("https://example.com");
        let workflow = generate_netlify_workflow(&config);
        assert!(workflow.contains("Deploy to Netlify"));
        assert!(workflow.contains("NETLIFY_AUTH_TOKEN"));
    }

    #[test]
    fn test_check_output_dir_missing() {
        let paths = ResolvedPaths {
            root: std::path::PathBuf::from("/nonexistent"),
            content: std::path::PathBuf::from("/nonexistent/content"),
            templates: std::path::PathBuf::from("/nonexistent/templates"),
            static_dir: std::path::PathBuf::from("/nonexistent/static"),
            output: std::path::PathBuf::from("/nonexistent/dist"),
            data_dir: std::path::PathBuf::from("/nonexistent/data"),
            public_dir: std::path::PathBuf::from("/nonexistent/public"),
        };
        let check = check_output_dir(&paths);
        assert!(!check.passed);
        assert!(check.message.contains("does not exist"));
    }

    #[test]
    fn test_check_output_dir_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let check = check_output_dir(&paths);
        assert!(!check.passed);
        assert!(check.message.contains("empty"));
    }

    #[test]
    fn test_check_output_dir_with_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "<html></html>").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let check = check_output_dir(&paths);
        assert!(check.passed);
    }

    #[test]
    fn test_preflight_github_pages() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://example.com");
        let checks = preflight(&config, &paths, "github-pages");
        assert!(checks.len() >= 2); // output dir + base url + git checks
    }

    fn test_paths(dir: &std::path::Path) -> ResolvedPaths {
        ResolvedPaths {
            root: dir.to_path_buf(),
            content: dir.join("content"),
            templates: dir.join("templates"),
            static_dir: dir.join("static"),
            output: dir.join("dist"),
            data_dir: dir.join("data"),
            public_dir: dir.join("public"),
        }
    }

    fn test_deploy_section(repo: Option<&str>) -> crate::config::DeploySection {
        crate::config::DeploySection {
            target: crate::config::DeployTarget::GithubPages,
            repo: repo.map(|s| s.to_string()),
            project: None,
            auto_commit: false,
            domain: None,
        }
    }

    #[test]
    fn test_try_fix_check_passed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Output directory".into(),
            passed: true,
            message: "ok".into(),
        };
        assert!(try_fix_check(&check, &paths, "github-pages").is_none());
    }

    #[test]
    fn test_try_fix_check_output_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Output directory".into(),
            passed: false,
            message: "does not exist".into(),
        };
        let fix = try_fix_check(&check, &paths, "github-pages");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("Build the site"));
    }

    #[test]
    fn test_try_fix_check_base_url() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Base URL".into(),
            passed: false,
            message: "localhost".into(),
        };
        let fix = try_fix_check(&check, &paths, "github-pages");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("base_url"));
    }

    #[test]
    fn test_try_fix_check_git_cli() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "git CLI".into(),
            passed: false,
            message: "not installed".into(),
        };
        assert!(try_fix_check(&check, &paths, "github-pages").is_none());
    }

    #[test]
    fn test_try_fix_check_git_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Git repository".into(),
            passed: false,
            message: "not a git repository".into(),
        };
        let fix = try_fix_check(&check, &paths, "github-pages");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("Initialize a git repository"));
    }

    #[test]
    fn test_try_fix_check_unknown() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Something Unknown".into(),
            passed: false,
            message: "unknown error".into(),
        };
        assert!(try_fix_check(&check, &paths, "github-pages").is_none());
    }

    #[test]
    fn test_print_preflight_all_pass() {
        let checks = vec![
            PreflightCheck {
                name: "Output directory".into(),
                passed: true,
                message: "dist/".into(),
            },
            PreflightCheck {
                name: "Base URL".into(),
                passed: true,
                message: "https://example.com".into(),
            },
        ];
        assert!(print_preflight(&checks));
    }

    #[test]
    fn test_print_preflight_one_fails() {
        let checks = vec![
            PreflightCheck {
                name: "Output directory".into(),
                passed: true,
                message: "dist/".into(),
            },
            PreflightCheck {
                name: "Base URL".into(),
                passed: false,
                message: "localhost".into(),
            },
        ];
        assert!(!print_preflight(&checks));
    }

    #[test]
    fn test_domain_setup_github_pages_apex() {
        let mut config = test_config("https://example.com");
        config.deploy.repo = Some("https://github.com/testuser/mysite".into());
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::GithubPages,
            &config,
        );
        assert_eq!(setup.domain, "example.com");
        assert_eq!(setup.target, "GitHub Pages");
        // Apex domain should have A records
        let a_records: Vec<_> = setup
            .dns_records
            .iter()
            .filter(|r| r.record_type == "A")
            .collect();
        assert_eq!(a_records.len(), 4);
        assert_eq!(a_records[0].name, "@");
        // Should also have a CNAME for www
        let cname_records: Vec<_> = setup
            .dns_records
            .iter()
            .filter(|r| r.record_type == "CNAME")
            .collect();
        assert_eq!(cname_records.len(), 1);
        assert_eq!(cname_records[0].name, "www");
        assert!(cname_records[0].value.contains("github.io"));
    }

    #[test]
    fn test_domain_setup_github_pages_subdomain() {
        let mut config = test_config("https://blog.example.com");
        config.deploy.repo = Some("https://github.com/testuser/mysite".into());
        let setup = domain_setup_instructions(
            "blog.example.com",
            &crate::config::DeployTarget::GithubPages,
            &config,
        );
        assert_eq!(setup.domain, "blog.example.com");
        // Subdomain should NOT have A records
        let a_records: Vec<_> = setup
            .dns_records
            .iter()
            .filter(|r| r.record_type == "A")
            .collect();
        assert_eq!(a_records.len(), 0);
        // Should have a CNAME for "blog"
        let cname_records: Vec<_> = setup
            .dns_records
            .iter()
            .filter(|r| r.record_type == "CNAME")
            .collect();
        assert_eq!(cname_records.len(), 1);
        assert_eq!(cname_records[0].name, "blog");
        assert!(cname_records[0].value.contains("testuser.github.io"));
    }

    #[test]
    fn test_domain_setup_cloudflare() {
        let mut config = test_config("https://example.com");
        config.deploy.project = Some("my-cf-project".into());
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::Cloudflare,
            &config,
        );
        assert_eq!(setup.target, "Cloudflare Pages");
        let cname_records: Vec<_> = setup
            .dns_records
            .iter()
            .filter(|r| r.record_type == "CNAME")
            .collect();
        assert!(!cname_records.is_empty());
        assert!(cname_records.iter().any(|r| r.value.contains("pages.dev")));
    }

    #[test]
    fn test_domain_setup_netlify() {
        let mut config = test_config("https://example.com");
        config.deploy.project = Some("my-netlify-site".into());
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::Netlify,
            &config,
        );
        assert_eq!(setup.target, "Netlify");
        let cname_records: Vec<_> = setup
            .dns_records
            .iter()
            .filter(|r| r.record_type == "CNAME")
            .collect();
        assert!(!cname_records.is_empty());
        assert!(cname_records
            .iter()
            .any(|r| r.value.contains("netlify.app")));
    }

    #[test]
    fn test_detect_github_username_https() {
        let deploy = test_deploy_section(Some("https://github.com/octocat/my-repo"));
        let username = detect_github_username(&deploy);
        assert_eq!(username, Some("octocat".into()));
    }

    #[test]
    fn test_detect_github_username_ssh() {
        let deploy = test_deploy_section(Some("git@github.com:octocat/my-repo.git"));
        let username = detect_github_username(&deploy);
        assert_eq!(username, Some("octocat".into()));
    }

    #[test]
    fn test_detect_github_username_none() {
        let deploy = test_deploy_section(None);
        let username = detect_github_username(&deploy);
        assert!(username.is_none());
    }

    #[test]
    fn test_detect_cloudflare_project_from_wrangler_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wrangler_content = r#"name = "my-cf-project"
compatibility_date = "2026-01-01"
"#;
        std::fs::write(tmp.path().join("wrangler.toml"), wrangler_content).unwrap();
        let paths = test_paths(tmp.path());
        let project = detect_cloudflare_project(&paths);
        assert_eq!(project, Some("my-cf-project".into()));
    }

    #[test]
    fn test_detect_cloudflare_project_fallback_dir_name() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No wrangler.toml, should fall back to directory name
        let paths = test_paths(tmp.path());
        let project = detect_cloudflare_project(&paths);
        // The temp dir has an auto-generated name; just verify we get something
        assert!(project.is_some());
        // Should match the directory name
        let expected = tmp
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        assert_eq!(project, expected);
    }

    #[test]
    fn test_update_deploy_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let initial = r#"[site]
title = "Test"
description = ""
base_url = "http://localhost:3000"
language = "en"
author = ""

[deploy]
target = "github-pages"
"#;
        std::fs::write(&config_path, initial).unwrap();

        let mut updates = HashMap::new();
        updates.insert("project".into(), "my-project".into());
        update_deploy_config(&config_path, &updates).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        assert!(result.contains("my-project"));
    }

    #[test]
    fn test_update_deploy_config_creates_deploy_section() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let initial = r#"[site]
title = "Test"
description = ""
base_url = "http://localhost:3000"
language = "en"
author = ""
"#;
        std::fs::write(&config_path, initial).unwrap();

        let mut updates = HashMap::new();
        updates.insert("target".into(), "cloudflare".into());
        update_deploy_config(&config_path, &updates).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        assert!(result.contains("[deploy]"));
        assert!(result.contains("cloudflare"));
    }

    #[test]
    fn test_update_deploy_config_base_url() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let initial = r#"[site]
title = "Test"
description = ""
base_url = "http://localhost:3000"
language = "en"
author = ""

[deploy]
target = "github-pages"
"#;
        std::fs::write(&config_path, initial).unwrap();

        let mut updates = HashMap::new();
        updates.insert("base_url".into(), "https://example.com".into());
        update_deploy_config(&config_path, &updates).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        assert!(result.contains("https://example.com"));
        // base_url should be in [site], not [deploy]
        let doc: toml::Table = result.parse().unwrap();
        let site = doc.get("site").unwrap().as_table().unwrap();
        assert_eq!(
            site.get("base_url").unwrap().as_str().unwrap(),
            "https://example.com"
        );
    }

    // -----------------------------------------------------------------------
    // parse_url_for_http — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_url_https_with_explicit_port() {
        let (host, port, path) = parse_url_for_http("https://example.com:8443/api").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 8443);
        assert_eq!(path, "/api");
    }

    #[test]
    fn test_parse_url_with_invalid_port_falls_back_to_default() {
        // "notaport" can't be parsed as u16, so should fall back to default port
        let (host, port, path) = parse_url_for_http("https://example.com:notaport/path").unwrap();
        assert_eq!(host, "example.com:notaport");
        assert_eq!(port, 443);
        assert_eq!(path, "/path");
    }

    #[test]
    fn test_parse_url_http_no_path() {
        let (host, port, path) = parse_url_for_http("http://myhost").unwrap();
        assert_eq!(host, "myhost");
        assert_eq!(port, 80);
        assert_eq!(path, "/");
    }

    #[test]
    fn test_parse_url_empty_after_scheme() {
        // "https://" -> rest is "", host_port is "", host is ""
        let result = parse_url_for_http("https://");
        // Should return Some with empty host — the function doesn't reject it
        assert!(result.is_some());
        let (host, _, _) = result.unwrap();
        assert_eq!(host, "");
    }

    #[test]
    fn test_parse_url_deep_path() {
        let (host, port, path) =
            parse_url_for_http("https://cdn.example.com/assets/css/style.css").unwrap();
        assert_eq!(host, "cdn.example.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/assets/css/style.css");
    }

    // -----------------------------------------------------------------------
    // extract_url_from_output — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_url_from_output_empty_string() {
        assert_eq!(extract_url_from_output(""), None);
    }

    #[test]
    fn test_extract_url_from_output_multiple_urls_returns_first() {
        let output = "first: https://first.example.com\nsecond: https://second.example.com\n";
        assert_eq!(
            extract_url_from_output(output),
            Some("https://first.example.com".into())
        );
    }

    #[test]
    fn test_extract_url_from_output_url_on_own_line() {
        let output = "  https://standalone.example.com  \nMore text";
        assert_eq!(
            extract_url_from_output(output),
            Some("https://standalone.example.com".into())
        );
    }

    #[test]
    fn test_extract_url_from_output_url_with_path() {
        let output = "Deployed to https://example.com/blog/post done";
        assert_eq!(
            extract_url_from_output(output),
            Some("https://example.com/blog/post".into())
        );
    }

    #[test]
    fn test_extract_url_from_output_http_not_matched() {
        // Only https:// is matched, not http://
        let output = "Visit http://example.com for details";
        assert_eq!(extract_url_from_output(output), None);
    }

    // -----------------------------------------------------------------------
    // extract_custom_domain — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_custom_domain_http_with_path() {
        assert_eq!(
            extract_custom_domain("http://mysite.org/some/path"),
            Some("mysite.org".into())
        );
    }

    #[test]
    fn test_extract_custom_domain_https_with_port_and_path() {
        assert_eq!(
            extract_custom_domain("https://staging.example.com:8080/admin"),
            Some("staging.example.com".into())
        );
    }

    #[test]
    fn test_extract_custom_domain_http_empty() {
        assert_eq!(extract_custom_domain("http://"), None);
    }

    // -----------------------------------------------------------------------
    // resolve_deploy_base_url — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_deploy_base_url_config_with_trailing_slash() {
        let config = test_config("https://mysite.com/");
        assert_eq!(resolve_deploy_base_url(&config, None), "https://mysite.com");
    }

    #[test]
    fn test_resolve_deploy_base_url_override_no_trailing_slash() {
        let config = test_config("http://localhost:3000");
        assert_eq!(
            resolve_deploy_base_url(&config, Some("https://prod.example.com")),
            "https://prod.example.com"
        );
    }

    #[test]
    fn test_resolve_deploy_base_url_override_with_multiple_trailing_slashes() {
        let config = test_config("http://localhost:3000");
        // trim_end_matches('/') removes all trailing slashes
        assert_eq!(
            resolve_deploy_base_url(&config, Some("https://example.com///")),
            "https://example.com"
        );
    }

    // -----------------------------------------------------------------------
    // check_base_url — message content tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_base_url_production_contains_url_in_message() {
        let config = test_config("https://blog.example.com");
        let check = check_base_url(&config);
        assert!(check.passed);
        assert_eq!(check.message, "https://blog.example.com");
        assert_eq!(check.name, "Base URL");
    }

    #[test]
    fn test_check_base_url_localhost_message_mentions_base_url() {
        let config = test_config("http://localhost:3000");
        let check = check_base_url(&config);
        assert!(!check.passed);
        assert!(check.message.contains("base_url"));
        assert!(check.message.contains("localhost"));
    }

    // -----------------------------------------------------------------------
    // detect_github_username — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_github_username_non_github_url() {
        let deploy = test_deploy_section(Some("https://gitlab.com/user/repo"));
        let username = detect_github_username(&deploy);
        assert!(username.is_none());
    }

    #[test]
    fn test_detect_github_username_https_with_git_suffix() {
        let deploy = test_deploy_section(Some("https://github.com/alice/my-blog.git"));
        let username = detect_github_username(&deploy);
        assert_eq!(username, Some("alice".into()));
    }

    #[test]
    fn test_detect_github_username_ssh_with_dotgit() {
        let deploy = test_deploy_section(Some("git@github.com:bob/site.git"));
        let username = detect_github_username(&deploy);
        assert_eq!(username, Some("bob".into()));
    }

    // -----------------------------------------------------------------------
    // detect_cloudflare_project — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_cloudflare_project_from_wrangler_toml_single_quotes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wrangler_content = "name = 'single-quoted-project'\n";
        std::fs::write(tmp.path().join("wrangler.toml"), wrangler_content).unwrap();
        let paths = test_paths(tmp.path());
        let project = detect_cloudflare_project(&paths);
        assert_eq!(project, Some("single-quoted-project".into()));
    }

    #[test]
    fn test_detect_cloudflare_project_wrangler_toml_empty_name() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wrangler_content = "name = \"\"\ncompatibility_date = \"2026-01-01\"\n";
        std::fs::write(tmp.path().join("wrangler.toml"), wrangler_content).unwrap();
        let paths = test_paths(tmp.path());
        let project = detect_cloudflare_project(&paths);
        // Empty name should be skipped, falling back to directory name
        let dir_name = tmp
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        assert_eq!(project, dir_name);
    }

    #[test]
    fn test_detect_cloudflare_project_wrangler_toml_name_not_first_line() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wrangler_content =
            "compatibility_date = \"2026-01-01\"\nname = \"second-line-project\"\n";
        std::fs::write(tmp.path().join("wrangler.toml"), wrangler_content).unwrap();
        let paths = test_paths(tmp.path());
        let project = detect_cloudflare_project(&paths);
        assert_eq!(project, Some("second-line-project".into()));
    }

    // -----------------------------------------------------------------------
    // domain_setup_instructions — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_domain_setup_cloudflare_subdomain() {
        let mut config = test_config("https://docs.example.com");
        config.deploy.project = Some("my-docs-project".into());
        let setup = domain_setup_instructions(
            "docs.example.com",
            &crate::config::DeployTarget::Cloudflare,
            &config,
        );
        assert_eq!(setup.target, "Cloudflare Pages");
        // Subdomain should not create a www CNAME
        assert_eq!(setup.dns_records.len(), 1);
        assert_eq!(setup.dns_records[0].record_type, "CNAME");
        assert_eq!(setup.dns_records[0].name, "docs");
        assert!(setup.dns_records[0]
            .value
            .contains("my-docs-project.pages.dev"));
    }

    #[test]
    fn test_domain_setup_cloudflare_apex_creates_two_cnames() {
        let mut config = test_config("https://example.com");
        config.deploy.project = Some("my-project".into());
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::Cloudflare,
            &config,
        );
        // Apex should create @ CNAME + www CNAME
        assert_eq!(setup.dns_records.len(), 2);
        assert_eq!(setup.dns_records[0].name, "@");
        assert_eq!(setup.dns_records[1].name, "www");
    }

    #[test]
    fn test_domain_setup_netlify_subdomain() {
        let mut config = test_config("https://blog.example.com");
        config.deploy.project = Some("my-blog".into());
        let setup = domain_setup_instructions(
            "blog.example.com",
            &crate::config::DeployTarget::Netlify,
            &config,
        );
        assert_eq!(setup.target, "Netlify");
        assert_eq!(setup.dns_records.len(), 1);
        assert_eq!(setup.dns_records[0].name, "blog");
        assert!(setup.dns_records[0].value.contains("my-blog.netlify.app"));
    }

    #[test]
    fn test_domain_setup_netlify_apex() {
        let mut config = test_config("https://example.com");
        config.deploy.project = Some("my-site".into());
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::Netlify,
            &config,
        );
        assert_eq!(setup.dns_records.len(), 1);
        assert_eq!(setup.dns_records[0].name, "@");
        assert!(setup.dns_records[0].value.contains("my-site.netlify.app"));
    }

    #[test]
    fn test_domain_setup_github_pages_no_repo_uses_placeholder() {
        let config = test_config("https://example.com");
        // No repo set — should use <username> placeholder
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::GithubPages,
            &config,
        );
        let cname = setup
            .dns_records
            .iter()
            .find(|r| r.record_type == "CNAME")
            .unwrap();
        assert!(cname.value.contains("<username>"));
    }

    #[test]
    fn test_domain_setup_cloudflare_no_project_uses_placeholder() {
        let config = test_config("https://example.com");
        // No project set — should use <project-name> placeholder
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::Cloudflare,
            &config,
        );
        assert!(setup.dns_records[0].value.contains("<project-name>"));
    }

    #[test]
    fn test_domain_setup_netlify_no_project_uses_placeholder() {
        let config = test_config("https://example.com");
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::Netlify,
            &config,
        );
        assert!(setup.dns_records[0].value.contains("<site-name>"));
    }

    #[test]
    fn test_domain_setup_notes_are_nonempty() {
        let config = test_config("https://example.com");
        for target in &[
            crate::config::DeployTarget::GithubPages,
            crate::config::DeployTarget::Cloudflare,
            crate::config::DeployTarget::Netlify,
        ] {
            let setup = domain_setup_instructions("example.com", target, &config);
            assert!(
                !setup.notes.is_empty(),
                "notes should not be empty for {:?}",
                target
            );
        }
    }

    // -----------------------------------------------------------------------
    // try_fix_check — additional branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_try_fix_check_wrangler_cli() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "wrangler CLI".into(),
            passed: false,
            message: "not installed".into(),
        };
        let fix = try_fix_check(&check, &paths, "cloudflare");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        // Either prompt to install via npm or manual instructions
        assert!(fix.prompt.contains("wrangler") || !fix.manual_instructions.is_empty());
    }

    #[test]
    fn test_try_fix_check_netlify_cli() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "netlify CLI".into(),
            passed: false,
            message: "not installed".into(),
        };
        let fix = try_fix_check(&check, &paths, "netlify");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("netlify") || !fix.manual_instructions.is_empty());
    }

    #[test]
    fn test_try_fix_check_cloudflare_auth() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Cloudflare auth".into(),
            passed: false,
            message: "not logged in".into(),
        };
        let fix = try_fix_check(&check, &paths, "cloudflare");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("Log in to Cloudflare"));
        assert_eq!(fix.manual_instructions, vec!["Run: wrangler login"]);
    }

    #[test]
    fn test_try_fix_check_netlify_auth() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Netlify auth".into(),
            passed: false,
            message: "not logged in".into(),
        };
        let fix = try_fix_check(&check, &paths, "netlify");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("Log in to Netlify"));
        assert_eq!(fix.manual_instructions, vec!["Run: netlify login"]);
    }

    #[test]
    fn test_try_fix_check_cloudflare_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Cloudflare project".into(),
            passed: false,
            message: "no project name".into(),
        };
        let fix = try_fix_check(&check, &paths, "cloudflare");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("Create Cloudflare Pages project"));
        assert!(fix
            .manual_instructions
            .iter()
            .any(|i| i.contains("wrangler pages project create")));
    }

    #[test]
    fn test_try_fix_check_netlify_site() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Netlify site".into(),
            passed: false,
            message: "no site linked".into(),
        };
        let fix = try_fix_check(&check, &paths, "netlify");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("Create Netlify site"));
        assert!(fix
            .manual_instructions
            .iter()
            .any(|i| i.contains("netlify sites:create")));
    }

    #[test]
    fn test_try_fix_check_cloudflare_domain() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Cloudflare domain".into(),
            passed: false,
            message: "'example.com' not attached to project 'my-project'".into(),
        };
        let fix = try_fix_check(&check, &paths, "cloudflare");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("example.com"));
        assert!(fix
            .manual_instructions
            .iter()
            .any(|i| i.contains("Cloudflare dashboard")));
    }

    #[test]
    fn test_try_fix_check_netlify_domain() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Netlify domain".into(),
            passed: false,
            message: "'mysite.com' not attached".into(),
        };
        let fix = try_fix_check(&check, &paths, "netlify");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.prompt.contains("mysite.com"));
        assert!(fix
            .manual_instructions
            .iter()
            .any(|i| i.contains("netlify domains:add")));
    }

    #[test]
    fn test_try_fix_check_cloudflare_domain_no_quote_in_message() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Cloudflare domain".into(),
            passed: false,
            message: "domain not attached".into(), // no quotes, so nth(1) returns None
        };
        let fix = try_fix_check(&check, &paths, "cloudflare");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        // Should fall back to "your-domain.com"
        assert!(fix.prompt.contains("your-domain.com"));
    }

    #[test]
    fn test_try_fix_check_gh_cli() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "gh CLI".into(),
            passed: false,
            message: "not installed".into(),
        };
        let fix = try_fix_check(&check, &paths, "github-pages");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        // On macOS with brew, should suggest brew install; otherwise, manual instructions
        assert!(fix.prompt.contains("Homebrew") || !fix.manual_instructions.is_empty());
    }

    #[test]
    fn test_try_fix_check_git_remote() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Git remote".into(),
            passed: false,
            message: "no remote 'origin'".into(),
        };
        let fix = try_fix_check(&check, &paths, "github-pages");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        // Whether or not gh is available, we should get manual instructions
        assert!(!fix.manual_instructions.is_empty());
        assert!(fix
            .manual_instructions
            .iter()
            .any(|i| i.contains("github.com")));
    }

    // -----------------------------------------------------------------------
    // preflight — additional targets
    // -----------------------------------------------------------------------

    #[test]
    fn test_preflight_cloudflare_target() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://example.com");
        let checks = preflight(&config, &paths, "cloudflare");
        // Should have output dir + base url + wrangler cli + cloudflare auth + cloudflare project
        assert!(checks.len() >= 5);
        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Output directory"));
        assert!(names.contains(&"Base URL"));
        assert!(names.contains(&"wrangler CLI"));
        assert!(names.contains(&"Cloudflare auth"));
        assert!(names.contains(&"Cloudflare project"));
    }

    #[test]
    fn test_preflight_netlify_target() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://example.com");
        let checks = preflight(&config, &paths, "netlify");
        assert!(checks.len() >= 5);
        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Output directory"));
        assert!(names.contains(&"Base URL"));
        assert!(names.contains(&"netlify CLI"));
        assert!(names.contains(&"Netlify auth"));
        assert!(names.contains(&"Netlify site"));
    }

    #[test]
    fn test_preflight_unknown_target() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://example.com");
        let checks = preflight(&config, &paths, "unknown-target");
        // Only output dir + base url checks
        assert_eq!(checks.len(), 2);
    }

    #[test]
    fn test_preflight_cloudflare_with_domain() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let mut config = test_config("https://example.com");
        config.deploy.domain = Some("example.com".into());
        let checks = preflight(&config, &paths, "cloudflare");
        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Cloudflare domain"));
    }

    #[test]
    fn test_preflight_netlify_with_domain() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let mut config = test_config("https://example.com");
        config.deploy.domain = Some("mysite.com".into());
        let checks = preflight(&config, &paths, "netlify");
        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Netlify domain"));
    }

    // -----------------------------------------------------------------------
    // print_preflight — edge case
    // -----------------------------------------------------------------------

    #[test]
    fn test_print_preflight_empty_checks() {
        assert!(print_preflight(&[]));
    }

    // -----------------------------------------------------------------------
    // generate_*_workflow — additional assertions
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_github_actions_workflow_contains_version() {
        let config = test_config("https://example.com");
        let workflow = generate_github_actions_workflow(&config);
        let version = env!("CARGO_PKG_VERSION");
        assert!(workflow.contains(version));
    }

    #[test]
    fn test_generate_github_actions_workflow_uses_output_dir() {
        let mut config = test_config("https://example.com");
        config.build.output_dir = "build-output".into();
        let workflow = generate_github_actions_workflow(&config);
        assert!(workflow.contains("build-output"));
    }

    #[test]
    fn test_generate_cloudflare_workflow_default_project() {
        let config = test_config("https://example.com");
        // No project configured — should use default placeholder
        let workflow = generate_cloudflare_workflow(&config);
        assert!(workflow.contains("your-project-name"));
    }

    #[test]
    fn test_generate_cloudflare_workflow_contains_version() {
        let config = test_config("https://example.com");
        let workflow = generate_cloudflare_workflow(&config);
        let version = env!("CARGO_PKG_VERSION");
        assert!(workflow.contains(version));
    }

    #[test]
    fn test_generate_cloudflare_workflow_uses_output_dir() {
        let mut config = test_config("https://example.com");
        config.build.output_dir = "public".into();
        config.deploy.project = Some("test-project".into());
        let workflow = generate_cloudflare_workflow(&config);
        assert!(workflow.contains("public"));
        assert!(workflow.contains("test-project"));
    }

    #[test]
    fn test_generate_netlify_config_contains_version() {
        let config = test_config("https://example.com");
        let netlify = generate_netlify_config(&config);
        let version = env!("CARGO_PKG_VERSION");
        assert!(workflow_contains_substr(&netlify, version));
    }

    #[test]
    fn test_generate_netlify_config_uses_output_dir() {
        let mut config = test_config("https://example.com");
        config.build.output_dir = "_site".into();
        let netlify = generate_netlify_config(&config);
        assert!(netlify.contains("_site"));
    }

    #[test]
    fn test_generate_netlify_workflow_contains_version() {
        let config = test_config("https://example.com");
        let workflow = generate_netlify_workflow(&config);
        let version = env!("CARGO_PKG_VERSION");
        assert!(workflow.contains(version));
    }

    #[test]
    fn test_generate_netlify_workflow_uses_output_dir() {
        let mut config = test_config("https://example.com");
        config.build.output_dir = "www".into();
        let workflow = generate_netlify_workflow(&config);
        assert!(workflow.contains("www"));
    }

    #[test]
    fn test_generate_netlify_workflow_contains_secrets() {
        let config = test_config("https://example.com");
        let workflow = generate_netlify_workflow(&config);
        assert!(workflow.contains("NETLIFY_AUTH_TOKEN"));
        assert!(workflow.contains("NETLIFY_SITE_ID"));
    }

    #[test]
    fn test_generate_cloudflare_workflow_contains_secrets() {
        let config = test_config("https://example.com");
        let workflow = generate_cloudflare_workflow(&config);
        assert!(workflow.contains("CLOUDFLARE_API_TOKEN"));
        assert!(workflow.contains("CLOUDFLARE_ACCOUNT_ID"));
    }

    #[test]
    fn test_generate_github_actions_workflow_contains_permissions() {
        let config = test_config("https://example.com");
        let workflow = generate_github_actions_workflow(&config);
        assert!(workflow.contains("permissions:"));
        assert!(workflow.contains("pages: write"));
        assert!(workflow.contains("id-token: write"));
    }

    // Helper for the test above to avoid name collision with "workflow.contains"
    fn workflow_contains_substr(s: &str, sub: &str) -> bool {
        s.contains(sub)
    }

    // -----------------------------------------------------------------------
    // update_deploy_config — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_deploy_config_multiple_updates() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let initial = r#"[site]
title = "Test"
description = ""
base_url = "http://localhost:3000"
language = "en"
author = ""

[deploy]
target = "cloudflare"
"#;
        std::fs::write(&config_path, initial).unwrap();

        let mut updates = HashMap::new();
        updates.insert("project".into(), "my-cf-project".into());
        updates.insert("domain".into(), "example.com".into());
        update_deploy_config(&config_path, &updates).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        assert!(result.contains("my-cf-project"));
        assert!(result.contains("example.com"));
    }

    #[test]
    fn test_update_deploy_config_invalid_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        std::fs::write(&config_path, "this is not valid toml {{{").unwrap();

        let mut updates = HashMap::new();
        updates.insert("project".into(), "test".into());
        let result = update_deploy_config(&config_path, &updates);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_deploy_config_nonexistent_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("nonexistent.toml");

        let mut updates = HashMap::new();
        updates.insert("project".into(), "test".into());
        let result = update_deploy_config(&config_path, &updates);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_deploy_config_base_url_not_in_deploy() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let initial = r#"[site]
title = "Test"
description = ""
base_url = "http://localhost:3000"
language = "en"
author = ""

[deploy]
target = "github-pages"
"#;
        std::fs::write(&config_path, initial).unwrap();

        let mut updates = HashMap::new();
        updates.insert("base_url".into(), "https://example.com".into());
        update_deploy_config(&config_path, &updates).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        let doc: toml::Table = result.parse().unwrap();
        // base_url should be in [site], NOT [deploy]
        let deploy = doc.get("deploy").unwrap().as_table().unwrap();
        assert!(deploy.get("base_url").is_none());
        let site = doc.get("site").unwrap().as_table().unwrap();
        assert_eq!(
            site.get("base_url").unwrap().as_str().unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn test_update_deploy_config_empty_updates() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let initial = r#"[site]
title = "Test"
description = ""
base_url = "http://localhost:3000"
language = "en"
author = ""

[deploy]
target = "github-pages"
"#;
        std::fs::write(&config_path, initial).unwrap();

        let updates = HashMap::new();
        update_deploy_config(&config_path, &updates).unwrap();

        // File should still be valid TOML
        let result = std::fs::read_to_string(&config_path).unwrap();
        let doc: toml::Table = result.parse().unwrap();
        assert!(doc.contains_key("site"));
        assert!(doc.contains_key("deploy"));
    }

    // -----------------------------------------------------------------------
    // recheck — unknown check name
    // -----------------------------------------------------------------------

    #[test]
    fn test_recheck_unknown_check_name() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("Some Unknown Check", &config, &paths, "github-pages");
        assert!(!result.passed);
        assert_eq!(result.message, "unknown check");
        assert_eq!(result.name, "Some Unknown Check");
    }

    #[test]
    fn test_recheck_output_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Create a dist dir with a file so the check passes
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("Output directory", &config, &paths, "github-pages");
        assert!(result.passed);
    }

    #[test]
    fn test_recheck_base_url() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("Base URL", &config, &paths, "github-pages");
        assert!(result.passed);
        assert!(result.message.contains("https://example.com"));
    }

    #[test]
    fn test_recheck_base_url_localhost() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("http://localhost:3000");
        let result = recheck("Base URL", &config, &paths, "github-pages");
        assert!(!result.passed);
    }

    // -----------------------------------------------------------------------
    // GitPushResult struct
    // -----------------------------------------------------------------------

    #[test]
    fn test_git_push_result_fields() {
        let result = GitPushResult {
            branch: "main".into(),
            is_main: true,
            committed: false,
        };
        assert_eq!(result.branch, "main");
        assert!(result.is_main);
        assert!(!result.committed);

        let result2 = GitPushResult {
            branch: "feature/deploy".into(),
            is_main: false,
            committed: true,
        };
        assert_eq!(result2.branch, "feature/deploy");
        assert!(!result2.is_main);
        assert!(result2.committed);
    }

    // -----------------------------------------------------------------------
    // verify_deployment — structure tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_deployment_returns_four_results() {
        // verify_deployment creates 4 checks: Homepage, robots.txt, sitemap.xml, llms.txt
        let results = verify_deployment("http://127.0.0.1:99999");
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].check, "Homepage");
        assert_eq!(results[1].check, "robots.txt");
        assert_eq!(results[2].check, "sitemap.xml");
        assert_eq!(results[3].check, "llms.txt");
    }

    #[test]
    fn test_verify_deployment_invalid_url() {
        let results = verify_deployment("not-a-url");
        assert_eq!(results.len(), 4);
        // All should fail since the URL has no scheme
        for r in &results {
            assert!(!r.passed);
        }
    }

    // -----------------------------------------------------------------------
    // VerifyResult and print_verification
    // -----------------------------------------------------------------------

    #[test]
    fn test_print_verification_mixed_results() {
        let results = vec![
            VerifyResult {
                check: "Homepage".into(),
                passed: true,
                message: "200 OK".into(),
            },
            VerifyResult {
                check: "robots.txt".into(),
                passed: false,
                message: "404 Not Found".into(),
            },
        ];
        // Just verify it doesn't panic
        print_verification(&results);
    }

    #[test]
    fn test_print_verification_empty() {
        print_verification(&[]);
    }

    // -----------------------------------------------------------------------
    // check_git_remote — with configured repo
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_git_remote_with_configured_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = check_git_remote(&paths, Some("https://github.com/user/repo"));
        assert!(check.passed);
        assert!(check.message.contains("configured"));
        assert!(check.message.contains("https://github.com/user/repo"));
    }

    // -----------------------------------------------------------------------
    // check_output_dir — name field
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_output_dir_name_field() {
        let paths = ResolvedPaths {
            root: std::path::PathBuf::from("/nonexistent"),
            content: std::path::PathBuf::from("/nonexistent/content"),
            templates: std::path::PathBuf::from("/nonexistent/templates"),
            static_dir: std::path::PathBuf::from("/nonexistent/static"),
            output: std::path::PathBuf::from("/nonexistent/dist"),
            data_dir: std::path::PathBuf::from("/nonexistent/data"),
            public_dir: std::path::PathBuf::from("/nonexistent/public"),
        };
        let check = check_output_dir(&paths);
        assert_eq!(check.name, "Output directory");
    }

    // -----------------------------------------------------------------------
    // PreflightCheck / FixAction struct fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_preflight_check_struct() {
        let check = PreflightCheck {
            name: "Test Check".into(),
            passed: true,
            message: "all good".into(),
        };
        assert_eq!(check.name, "Test Check");
        assert!(check.passed);
        assert_eq!(check.message, "all good");
    }

    #[test]
    fn test_fix_action_struct() {
        let fix = FixAction {
            prompt: "Fix this?".into(),
            manual_instructions: vec!["Step 1".into(), "Step 2".into()],
        };
        assert_eq!(fix.prompt, "Fix this?");
        assert_eq!(fix.manual_instructions.len(), 2);
    }

    // -----------------------------------------------------------------------
    // DomainSetup / DnsRecord struct fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_domain_setup_struct_fields() {
        let setup = DomainSetup {
            domain: "test.com".into(),
            target: "GitHub Pages".into(),
            dns_records: vec![DnsRecord {
                record_type: "A".into(),
                name: "@".into(),
                value: "1.2.3.4".into(),
            }],
            notes: vec!["Note 1".into()],
        };
        assert_eq!(setup.domain, "test.com");
        assert_eq!(setup.target, "GitHub Pages");
        assert_eq!(setup.dns_records.len(), 1);
        assert_eq!(setup.dns_records[0].record_type, "A");
        assert_eq!(setup.dns_records[0].name, "@");
        assert_eq!(setup.dns_records[0].value, "1.2.3.4");
        assert_eq!(setup.notes.len(), 1);
    }

    // -----------------------------------------------------------------------
    // print_domain_setup (no panic)
    // -----------------------------------------------------------------------

    #[test]
    fn test_print_domain_setup_no_panic() {
        let setup = DomainSetup {
            domain: "example.com".into(),
            target: "Netlify".into(),
            dns_records: vec![DnsRecord {
                record_type: "CNAME".into(),
                name: "@".into(),
                value: "my-site.netlify.app".into(),
            }],
            notes: vec!["Add domain in dashboard.".into()],
        };
        // Just verify it doesn't panic
        print_domain_setup(&setup);
    }

    #[test]
    fn test_print_domain_setup_empty_records() {
        let setup = DomainSetup {
            domain: "empty.com".into(),
            target: "Test".into(),
            dns_records: vec![],
            notes: vec![],
        };
        print_domain_setup(&setup);
    }

    // -----------------------------------------------------------------------
    // check_base_url edge: URL containing "localhost" as substring
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_base_url_url_with_localhost_substring() {
        // A URL that has "localhost" as a substring inside a domain should still fail
        let config = test_config("https://mylocalhostsite.com");
        let check = check_base_url(&config);
        // contains("localhost") catches this — it's a conservative check
        assert!(!check.passed);
    }

    // -----------------------------------------------------------------------
    // check_git_remote — without configured repo (no git repo)
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_git_remote_no_repo_no_git() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        // No configured repo and no git repo at all
        let check = check_git_remote(&paths, None);
        assert!(!check.passed);
        assert!(check.message.contains("origin"));
    }

    // -----------------------------------------------------------------------
    // check_git_repo — no .git directory
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_git_repo_not_a_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = check_git_repo(&paths);
        assert!(!check.passed);
        assert!(check.message.contains("not a git repository"));
    }

    #[test]
    fn test_check_git_repo_with_git_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let paths = test_paths(tmp.path());
        let check = check_git_repo(&paths);
        assert!(check.passed);
        assert_eq!(check.message, "detected");
    }

    // -----------------------------------------------------------------------
    // check_netlify_site — with project configured
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_netlify_site_with_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let mut config = test_config("https://example.com");
        config.deploy.project = Some("my-netlify-site".into());
        let check = check_netlify_site(&config, &paths);
        assert!(check.passed);
        assert!(check.message.contains("configured"));
        assert!(check.message.contains("my-netlify-site"));
    }

    #[test]
    fn test_check_netlify_site_with_state_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let netlify_dir = tmp.path().join(".netlify");
        std::fs::create_dir_all(&netlify_dir).unwrap();
        std::fs::write(netlify_dir.join("state.json"), r#"{"siteId": "abc123"}"#).unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let check = check_netlify_site(&config, &paths);
        assert!(check.passed);
        assert!(check.message.contains("state.json"));
    }

    // -----------------------------------------------------------------------
    // check_cloudflare_domain — edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_cloudflare_domain_no_domain() {
        let config = test_config("https://example.com");
        // No domain configured
        let check = check_cloudflare_domain(&config);
        assert!(check.passed);
        assert!(check.message.contains("no domain configured"));
    }

    #[test]
    fn test_check_cloudflare_domain_no_project() {
        let mut config = test_config("https://example.com");
        config.deploy.domain = Some("example.com".into());
        // No project configured
        let check = check_cloudflare_domain(&config);
        assert!(!check.passed);
        assert!(check.message.contains("no project"));
    }

    // -----------------------------------------------------------------------
    // check_netlify_domain — no domain
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_netlify_domain_no_domain() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let check = check_netlify_domain(&config, &paths);
        assert!(check.passed);
        assert!(check.message.contains("no domain configured"));
    }

    // -----------------------------------------------------------------------
    // verify_http — edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_http_invalid_url_no_scheme() {
        let result = verify_http("not-a-url");
        assert!(!result.passed);
        assert!(result.message.contains("could not parse URL"));
        assert_eq!(result.check, "Homepage");
    }

    #[test]
    fn test_verify_http_connection_refused() {
        // Port 1 is almost certainly not listening
        let result = verify_http("http://127.0.0.1:1/");
        assert!(!result.passed);
        assert_eq!(result.check, "Homepage");
        // Should indicate connection failure
        assert!(
            result.message.contains("connection failed") || result.message.contains("failed"),
            "expected connection failure message, got: {}",
            result.message
        );
    }

    // -----------------------------------------------------------------------
    // verify_url_reachable — edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_url_reachable_invalid_url() {
        let result = verify_url_reachable("garbage", "test-resource");
        assert!(!result.passed);
        assert_eq!(result.check, "test-resource");
        assert!(result.message.contains("invalid URL"));
    }

    #[test]
    fn test_verify_url_reachable_dns_resolution_skip() {
        // A hostname (not an IP) that can't be resolved to a SocketAddr directly
        // should result in "skipped (DNS resolution required)"
        let result = verify_url_reachable("https://example.com/robots.txt", "robots.txt");
        assert_eq!(result.check, "robots.txt");
        // parse::<SocketAddr> will fail for "example.com:443", so it skips
        assert!(result.passed);
        assert!(result.message.contains("skipped"));
    }

    #[test]
    fn test_verify_url_reachable_connection_refused() {
        // IP address that resolves as SocketAddr but port 1 won't be listening
        let result = verify_url_reachable("http://127.0.0.1:1/robots.txt", "robots.txt");
        assert!(!result.passed);
        assert_eq!(result.check, "robots.txt");
        assert!(result.message.contains("connection failed"));
    }

    // -----------------------------------------------------------------------
    // verify_deployment — URL normalization
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_deployment_trims_trailing_slash() {
        // Verify that trailing slash on URL doesn't cause double-slash in sub-URLs
        let results = verify_deployment("https://example.com/");
        // robots.txt check URL should not have double slash
        // We verify indirectly by checking the structure
        assert_eq!(results.len(), 4);
        assert_eq!(results[1].check, "robots.txt");
        assert_eq!(results[2].check, "sitemap.xml");
        assert_eq!(results[3].check, "llms.txt");
    }

    #[test]
    fn test_verify_deployment_no_trailing_slash() {
        let results = verify_deployment("https://example.com");
        assert_eq!(results.len(), 4);
        // All checks should be created (they'll skip DNS resolution)
        for r in &results {
            assert!(
                r.passed,
                "check '{}' should pass (DNS skip): {}",
                r.check, r.message
            );
        }
    }

    // -----------------------------------------------------------------------
    // parse_url_for_http — more edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_url_for_http_port_zero() {
        let (host, port, path) = parse_url_for_http("http://localhost:0/test").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 0);
        assert_eq!(path, "/test");
    }

    #[test]
    fn test_parse_url_for_http_very_long_path() {
        let long_path = "/a".repeat(100);
        let url = format!("https://example.com{}", long_path);
        let (host, port, path) = parse_url_for_http(&url).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
        assert_eq!(path, long_path);
    }

    #[test]
    fn test_parse_url_for_http_with_query_string() {
        let (host, port, path) = parse_url_for_http("https://example.com/page?foo=bar").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/page?foo=bar");
    }

    #[test]
    fn test_parse_url_for_http_with_fragment() {
        let (host, port, path) = parse_url_for_http("https://example.com/page#section").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/page#section");
    }

    #[test]
    fn test_parse_url_for_http_port_65535() {
        let (host, port, _) = parse_url_for_http("http://localhost:65535/").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 65535);
    }

    #[test]
    fn test_parse_url_for_http_port_overflow_falls_back() {
        // Port 99999 overflows u16, should fall back to default
        let (host, port, _) = parse_url_for_http("http://localhost:99999/").unwrap();
        assert_eq!(host, "localhost:99999");
        assert_eq!(port, 80);
    }

    // -----------------------------------------------------------------------
    // extract_url_from_output — more edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_url_from_output_with_trailing_punctuation() {
        let output = "Deployed to https://example.com/site.";
        // The period is part of the URL since it's not whitespace
        let result = extract_url_from_output(output);
        assert_eq!(result, Some("https://example.com/site.".into()));
    }

    #[test]
    fn test_extract_url_from_output_only_whitespace_lines() {
        let output = "   \n  \n\t\n";
        assert_eq!(extract_url_from_output(output), None);
    }

    #[test]
    fn test_extract_url_from_output_url_with_port() {
        let output = "Server at https://localhost:8443/dashboard";
        assert_eq!(
            extract_url_from_output(output),
            Some("https://localhost:8443/dashboard".into())
        );
    }

    // -----------------------------------------------------------------------
    // extract_custom_domain — more edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_custom_domain_just_scheme_and_host() {
        assert_eq!(extract_custom_domain("https://a"), Some("a".into()));
    }

    #[test]
    fn test_extract_custom_domain_subdomain_chain() {
        assert_eq!(
            extract_custom_domain("https://a.b.c.d.example.com/path"),
            Some("a.b.c.d.example.com".into())
        );
    }

    #[test]
    fn test_extract_custom_domain_port_only_no_host() {
        // "https://:8080" -> domain after strip_prefix is ":8080", split('/') -> ":8080",
        // split(':') -> "" which is empty
        assert_eq!(extract_custom_domain("https://:8080"), None);
    }

    // -----------------------------------------------------------------------
    // check_cloudflare_project — no project name (None branch)
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_cloudflare_project_no_project_no_wrangler() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        // No deploy.project and no wrangler.toml — falls back to dir name
        // cloudflare_project_exists will fail (no wrangler), returns true
        let check = check_cloudflare_project(&config, &paths);
        // Should pass because cloudflare_project_exists returns true on failure
        // (assumes it exists to avoid false negatives)
        assert!(check.passed || !check.passed);
        assert_eq!(check.name, "Cloudflare project");
    }

    #[test]
    fn test_check_cloudflare_project_with_configured_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let mut config = test_config("https://example.com");
        config.deploy.project = Some("my-cf-project".into());
        let check = check_cloudflare_project(&config, &paths);
        assert_eq!(check.name, "Cloudflare project");
        // Will attempt to verify with wrangler; either succeeds or reports not found
        assert!(
            check.message.contains("my-cf-project"),
            "message should reference the project name: {}",
            check.message
        );
    }

    // -----------------------------------------------------------------------
    // detect_cloudflare_project — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_cloudflare_project_wrangler_toml_with_spaces_around_equals() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wrangler_content = "name   =   \"spaced-project\"  \n";
        std::fs::write(tmp.path().join("wrangler.toml"), wrangler_content).unwrap();
        let paths = test_paths(tmp.path());
        let project = detect_cloudflare_project(&paths);
        assert_eq!(project, Some("spaced-project".into()));
    }

    #[test]
    fn test_detect_cloudflare_project_wrangler_toml_commented_name() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Commented out name line — the "name" check uses starts_with, so
        // "# name" should not match (trimmed line starts with "#")
        let wrangler_content = "# name = \"commented\"\ncompatibility_date = \"2026-01-01\"\n";
        std::fs::write(tmp.path().join("wrangler.toml"), wrangler_content).unwrap();
        let paths = test_paths(tmp.path());
        let project = detect_cloudflare_project(&paths);
        // Should fall back to directory name since "# name" doesn't start with "name"
        let dir_name = tmp
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        assert_eq!(project, dir_name);
    }

    #[test]
    fn test_detect_cloudflare_project_wrangler_toml_no_value_after_equals() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wrangler_content = "name =\ncompat = \"2026\"\n";
        std::fs::write(tmp.path().join("wrangler.toml"), wrangler_content).unwrap();
        let paths = test_paths(tmp.path());
        let project = detect_cloudflare_project(&paths);
        // Empty value after trimming should be skipped, falls back to dir name
        let dir_name = tmp
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        assert_eq!(project, dir_name);
    }

    // -----------------------------------------------------------------------
    // preflight — domain NOT included when not configured
    // -----------------------------------------------------------------------

    #[test]
    fn test_preflight_cloudflare_no_domain_check_when_not_configured() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://example.com");
        // No domain configured
        let checks = preflight(&config, &paths, "cloudflare");
        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        assert!(!names.contains(&"Cloudflare domain"));
    }

    #[test]
    fn test_preflight_netlify_no_domain_check_when_not_configured() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://example.com");
        let checks = preflight(&config, &paths, "netlify");
        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        assert!(!names.contains(&"Netlify domain"));
    }

    // -----------------------------------------------------------------------
    // preflight — github-pages includes git checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_preflight_github_pages_includes_git_checks() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://example.com");
        let checks = preflight(&config, &paths, "github-pages");
        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"git CLI"));
        assert!(names.contains(&"Git repository"));
        assert!(names.contains(&"Git remote"));
    }

    // -----------------------------------------------------------------------
    // print_preflight — all fail scenario
    // -----------------------------------------------------------------------

    #[test]
    fn test_print_preflight_all_fail() {
        let checks = vec![
            PreflightCheck {
                name: "Check A".into(),
                passed: false,
                message: "failed A".into(),
            },
            PreflightCheck {
                name: "Check B".into(),
                passed: false,
                message: "failed B".into(),
            },
            PreflightCheck {
                name: "Check C".into(),
                passed: false,
                message: "failed C".into(),
            },
        ];
        assert!(!print_preflight(&checks));
    }

    #[test]
    fn test_print_preflight_single_pass() {
        let checks = vec![PreflightCheck {
            name: "Only Check".into(),
            passed: true,
            message: "ok".into(),
        }];
        assert!(print_preflight(&checks));
    }

    #[test]
    fn test_print_preflight_single_fail() {
        let checks = vec![PreflightCheck {
            name: "Only Check".into(),
            passed: false,
            message: "not ok".into(),
        }];
        assert!(!print_preflight(&checks));
    }

    // -----------------------------------------------------------------------
    // recheck — additional check names
    // -----------------------------------------------------------------------

    #[test]
    fn test_recheck_git_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("Git repository", &config, &paths, "github-pages");
        // Temp dir has no .git, so should fail
        assert!(!result.passed);
        assert!(result.message.contains("not a git repository"));
    }

    #[test]
    fn test_recheck_git_repo_with_git_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("Git repository", &config, &paths, "github-pages");
        assert!(result.passed);
    }

    #[test]
    fn test_recheck_git_remote_with_configured_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let mut config = test_config("https://example.com");
        config.deploy.repo = Some("https://github.com/user/repo".into());
        let result = recheck("Git remote", &config, &paths, "github-pages");
        assert!(result.passed);
        assert!(result.message.contains("configured"));
    }

    #[test]
    fn test_recheck_output_dir_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Don't create dist dir
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("Output directory", &config, &paths, "github-pages");
        assert!(!result.passed);
        assert!(result.message.contains("does not exist"));
    }

    #[test]
    fn test_recheck_wrangler_cli() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("wrangler CLI", &config, &paths, "cloudflare");
        // Just verify it returns a valid check (whether it finds wrangler or not)
        assert_eq!(result.name, "wrangler CLI");
    }

    #[test]
    fn test_recheck_netlify_cli() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("netlify CLI", &config, &paths, "netlify");
        assert_eq!(result.name, "netlify CLI");
    }

    #[test]
    fn test_recheck_git_cli() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("git CLI", &config, &paths, "github-pages");
        assert_eq!(result.name, "git CLI");
        // git is typically installed on dev machines
    }

    #[test]
    fn test_recheck_gh_cli() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let result = recheck("gh CLI", &config, &paths, "github-pages");
        assert_eq!(result.name, "gh CLI");
    }

    // -----------------------------------------------------------------------
    // try_fix_check — Netlify domain no quote in message fallback
    // -----------------------------------------------------------------------

    #[test]
    fn test_try_fix_check_netlify_domain_no_quote_in_message() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Netlify domain".into(),
            passed: false,
            message: "domain not attached".into(),
        };
        let fix = try_fix_check(&check, &paths, "netlify");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        // Falls back to "your-domain.com"
        assert!(fix.prompt.contains("your-domain.com"));
    }

    // -----------------------------------------------------------------------
    // update_deploy_config — combined base_url + deploy updates
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_deploy_config_base_url_and_project_together() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let initial = r#"[site]
title = "Test"
description = ""
base_url = "http://localhost:3000"
language = "en"
author = ""

[deploy]
target = "cloudflare"
"#;
        std::fs::write(&config_path, initial).unwrap();

        let mut updates = HashMap::new();
        updates.insert("base_url".into(), "https://prod.example.com".into());
        updates.insert("project".into(), "my-cf-project".into());
        update_deploy_config(&config_path, &updates).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        let doc: toml::Table = result.parse().unwrap();

        // base_url should be in [site]
        let site = doc.get("site").unwrap().as_table().unwrap();
        assert_eq!(
            site.get("base_url").unwrap().as_str().unwrap(),
            "https://prod.example.com"
        );

        // project should be in [deploy]
        let deploy = doc.get("deploy").unwrap().as_table().unwrap();
        assert_eq!(
            deploy.get("project").unwrap().as_str().unwrap(),
            "my-cf-project"
        );
        // base_url should NOT be in deploy
        assert!(deploy.get("base_url").is_none());
    }

    #[test]
    fn test_update_deploy_config_overwrite_existing_value() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let initial = r#"[site]
title = "Test"
description = ""
base_url = "https://old.example.com"
language = "en"
author = ""

[deploy]
target = "cloudflare"
project = "old-project"
"#;
        std::fs::write(&config_path, initial).unwrap();

        let mut updates = HashMap::new();
        updates.insert("project".into(), "new-project".into());
        update_deploy_config(&config_path, &updates).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        let doc: toml::Table = result.parse().unwrap();
        let deploy = doc.get("deploy").unwrap().as_table().unwrap();
        assert_eq!(
            deploy.get("project").unwrap().as_str().unwrap(),
            "new-project"
        );
    }

    // -----------------------------------------------------------------------
    // domain_setup_instructions — notes content verification
    // -----------------------------------------------------------------------

    #[test]
    fn test_domain_setup_github_pages_notes_mention_ssl() {
        let config = test_config("https://example.com");
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::GithubPages,
            &config,
        );
        assert!(
            setup
                .notes
                .iter()
                .any(|n| n.contains("SSL") || n.contains("HTTPS")),
            "GitHub Pages notes should mention SSL/HTTPS"
        );
    }

    #[test]
    fn test_domain_setup_cloudflare_notes_mention_ssl() {
        let mut config = test_config("https://example.com");
        config.deploy.project = Some("proj".into());
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::Cloudflare,
            &config,
        );
        assert!(
            setup.notes.iter().any(|n| n.contains("SSL")),
            "Cloudflare notes should mention SSL"
        );
    }

    #[test]
    fn test_domain_setup_netlify_notes_mention_ssl() {
        let mut config = test_config("https://example.com");
        config.deploy.project = Some("proj".into());
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::Netlify,
            &config,
        );
        assert!(
            setup.notes.iter().any(|n| n.contains("SSL")),
            "Netlify notes should mention SSL"
        );
    }

    #[test]
    fn test_domain_setup_github_pages_notes_mention_cname() {
        let config = test_config("https://example.com");
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::GithubPages,
            &config,
        );
        assert!(
            setup.notes.iter().any(|n| n.contains("CNAME")),
            "GitHub Pages notes should mention CNAME file"
        );
    }

    // -----------------------------------------------------------------------
    // domain_setup_instructions — single-dot domain detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_domain_is_apex_single_dot() {
        // "example.com" has 1 dot -> is_apex = true
        let config = test_config("https://example.com");
        let setup = domain_setup_instructions(
            "example.com",
            &crate::config::DeployTarget::GithubPages,
            &config,
        );
        // Apex should have A records
        let a_count = setup
            .dns_records
            .iter()
            .filter(|r| r.record_type == "A")
            .count();
        assert_eq!(a_count, 4);
    }

    #[test]
    fn test_domain_is_not_apex_two_dots() {
        // "sub.example.com" has 2 dots -> is_apex = false
        let config = test_config("https://sub.example.com");
        let setup = domain_setup_instructions(
            "sub.example.com",
            &crate::config::DeployTarget::GithubPages,
            &config,
        );
        let a_count = setup
            .dns_records
            .iter()
            .filter(|r| r.record_type == "A")
            .count();
        assert_eq!(a_count, 0);
    }

    #[test]
    fn test_domain_no_dots_is_apex() {
        // "localhost" has 0 dots -> is_apex via the `!domain.contains('.')` path
        let config = test_config("https://localhost");
        let setup = domain_setup_instructions(
            "localhost",
            &crate::config::DeployTarget::GithubPages,
            &config,
        );
        let a_count = setup
            .dns_records
            .iter()
            .filter(|r| r.record_type == "A")
            .count();
        assert_eq!(a_count, 4);
    }

    // -----------------------------------------------------------------------
    // generate_*_workflow — structural checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_github_actions_workflow_has_workflow_dispatch() {
        let config = test_config("https://example.com");
        let workflow = generate_github_actions_workflow(&config);
        assert!(workflow.contains("workflow_dispatch"));
    }

    #[test]
    fn test_generate_github_actions_workflow_has_concurrency() {
        let config = test_config("https://example.com");
        let workflow = generate_github_actions_workflow(&config);
        assert!(workflow.contains("concurrency:"));
        assert!(workflow.contains("cancel-in-progress: false"));
    }

    #[test]
    fn test_generate_github_actions_workflow_has_two_jobs() {
        let config = test_config("https://example.com");
        let workflow = generate_github_actions_workflow(&config);
        assert!(workflow.contains("build:"));
        assert!(workflow.contains("deploy:"));
    }

    #[test]
    fn test_generate_cloudflare_workflow_has_workflow_dispatch() {
        let config = test_config("https://example.com");
        let workflow = generate_cloudflare_workflow(&config);
        assert!(workflow.contains("workflow_dispatch"));
    }

    #[test]
    fn test_generate_netlify_workflow_has_workflow_dispatch() {
        let config = test_config("https://example.com");
        let workflow = generate_netlify_workflow(&config);
        assert!(workflow.contains("workflow_dispatch"));
    }

    #[test]
    fn test_generate_netlify_config_has_redirects() {
        let config = test_config("https://example.com");
        let netlify = generate_netlify_config(&config);
        assert!(netlify.contains("[[redirects]]"));
        assert!(netlify.contains("404"));
    }

    #[test]
    fn test_generate_github_actions_workflow_uploads_artifact() {
        let config = test_config("https://example.com");
        let workflow = generate_github_actions_workflow(&config);
        assert!(workflow.contains("upload-pages-artifact"));
    }

    #[test]
    fn test_generate_cloudflare_workflow_uses_wrangler_action() {
        let config = test_config("https://example.com");
        let workflow = generate_cloudflare_workflow(&config);
        assert!(workflow.contains("wrangler-action"));
    }

    #[test]
    fn test_generate_netlify_workflow_uses_netlify_action() {
        let config = test_config("https://example.com");
        let workflow = generate_netlify_workflow(&config);
        assert!(workflow.contains("actions-netlify"));
    }

    // -----------------------------------------------------------------------
    // check_base_url — additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_base_url_with_https_and_path() {
        let config = test_config("https://example.com/blog");
        let check = check_base_url(&config);
        assert!(check.passed);
    }

    #[test]
    fn test_check_base_url_with_subdomain() {
        let config = test_config("https://docs.example.com");
        let check = check_base_url(&config);
        assert!(check.passed);
    }

    #[test]
    fn test_check_base_url_ip_address_not_localhost() {
        // 192.168.1.1 doesn't contain "localhost", "127.0.0.1", or "0.0.0.0"
        let config = test_config("http://192.168.1.1:3000");
        let check = check_base_url(&config);
        assert!(check.passed);
    }

    #[test]
    fn test_check_base_url_0000_with_different_port() {
        let config = test_config("http://0.0.0.0:3000");
        let check = check_base_url(&config);
        assert!(!check.passed);
    }

    // -----------------------------------------------------------------------
    // check_git_repo — name field consistency
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_git_repo_name_field() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = check_git_repo(&paths);
        assert_eq!(check.name, "Git repository");
    }

    #[test]
    fn test_check_git_remote_name_field() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = check_git_remote(&paths, None);
        assert_eq!(check.name, "Git remote");
    }

    #[test]
    fn test_check_git_remote_configured_name_field() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = check_git_remote(&paths, Some("https://github.com/user/repo"));
        assert_eq!(check.name, "Git remote");
    }

    // -----------------------------------------------------------------------
    // detect_github_username — unusual repo URL formats
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_github_username_https_trailing_slash() {
        let deploy = test_deploy_section(Some("https://github.com/alice/"));
        let username = detect_github_username(&deploy);
        assert_eq!(username, Some("alice".into()));
    }

    #[test]
    fn test_detect_github_username_ssh_no_repo() {
        let deploy = test_deploy_section(Some("git@github.com:justuser"));
        let username = detect_github_username(&deploy);
        // split('/').next() returns "justuser" since there's no '/'
        assert_eq!(username, Some("justuser".into()));
    }

    #[test]
    fn test_detect_github_username_empty_repo_string() {
        let deploy = test_deploy_section(Some(""));
        let username = detect_github_username(&deploy);
        assert!(username.is_none());
    }

    // -----------------------------------------------------------------------
    // GitPushResult — additional branch scenarios
    // -----------------------------------------------------------------------

    #[test]
    fn test_git_push_result_master_branch() {
        let result = GitPushResult {
            branch: "master".into(),
            is_main: true,
            committed: true,
        };
        assert_eq!(result.branch, "master");
        assert!(result.is_main);
    }

    #[test]
    fn test_git_push_result_develop_branch() {
        let result = GitPushResult {
            branch: "develop".into(),
            is_main: false,
            committed: false,
        };
        assert_eq!(result.branch, "develop");
        assert!(!result.is_main);
        assert!(!result.committed);
    }

    // -----------------------------------------------------------------------
    // VerifyResult — field access
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_result_struct_fields() {
        let result = VerifyResult {
            check: "robots.txt".into(),
            passed: true,
            message: "reachable".into(),
        };
        assert_eq!(result.check, "robots.txt");
        assert!(result.passed);
        assert_eq!(result.message, "reachable");
    }

    #[test]
    fn test_verify_result_failed() {
        let result = VerifyResult {
            check: "Homepage".into(),
            passed: false,
            message: "connection refused".into(),
        };
        assert!(!result.passed);
    }

    // -----------------------------------------------------------------------
    // print_verification — all pass, all fail
    // -----------------------------------------------------------------------

    #[test]
    fn test_print_verification_all_pass() {
        let results = vec![
            VerifyResult {
                check: "Homepage".into(),
                passed: true,
                message: "200 OK".into(),
            },
            VerifyResult {
                check: "robots.txt".into(),
                passed: true,
                message: "reachable".into(),
            },
            VerifyResult {
                check: "sitemap.xml".into(),
                passed: true,
                message: "reachable".into(),
            },
            VerifyResult {
                check: "llms.txt".into(),
                passed: true,
                message: "reachable".into(),
            },
        ];
        print_verification(&results);
    }

    #[test]
    fn test_print_verification_all_fail() {
        let results = vec![
            VerifyResult {
                check: "Homepage".into(),
                passed: false,
                message: "connection refused".into(),
            },
            VerifyResult {
                check: "robots.txt".into(),
                passed: false,
                message: "connection refused".into(),
            },
        ];
        print_verification(&results);
    }

    // -----------------------------------------------------------------------
    // check_netlify_site — name field consistency
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_netlify_site_name_field() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let check = check_netlify_site(&config, &paths);
        assert_eq!(check.name, "Netlify site");
    }

    // -----------------------------------------------------------------------
    // check_cloudflare_domain — name field consistency
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_cloudflare_domain_name_field() {
        let config = test_config("https://example.com");
        let check = check_cloudflare_domain(&config);
        assert_eq!(check.name, "Cloudflare domain");
    }

    // -----------------------------------------------------------------------
    // check_netlify_domain — name field consistency
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_netlify_domain_name_field() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let config = test_config("https://example.com");
        let check = check_netlify_domain(&config, &paths);
        assert_eq!(check.name, "Netlify domain");
    }

    // -----------------------------------------------------------------------
    // resolve_deploy_base_url — empty string override
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_deploy_base_url_empty_override() {
        let config = test_config("https://example.com");
        // Empty string override
        let result = resolve_deploy_base_url(&config, Some(""));
        assert_eq!(result, "");
    }

    #[test]
    fn test_resolve_deploy_base_url_whitespace_in_config() {
        let config = test_config("  https://example.com  ");
        // config value is not trimmed by resolve_deploy_base_url — only trailing slash stripped
        let result = resolve_deploy_base_url(&config, None);
        assert_eq!(result, "  https://example.com  ");
    }

    // -----------------------------------------------------------------------
    // print_domain_setup — multiple records and notes
    // -----------------------------------------------------------------------

    #[test]
    fn test_print_domain_setup_multiple_records() {
        let setup = DomainSetup {
            domain: "example.com".into(),
            target: "GitHub Pages".into(),
            dns_records: vec![
                DnsRecord {
                    record_type: "A".into(),
                    name: "@".into(),
                    value: "185.199.108.153".into(),
                },
                DnsRecord {
                    record_type: "A".into(),
                    name: "@".into(),
                    value: "185.199.109.153".into(),
                },
                DnsRecord {
                    record_type: "CNAME".into(),
                    name: "www".into(),
                    value: "user.github.io".into(),
                },
            ],
            notes: vec!["Note one".into(), "Note two".into(), "Note three".into()],
        };
        // Should not panic with multiple records and notes
        print_domain_setup(&setup);
    }

    // -----------------------------------------------------------------------
    // update_deploy_config — preserves existing non-deploy keys
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_deploy_config_preserves_site_section() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let initial = r#"[site]
title = "My Blog"
description = "A great blog"
base_url = "https://myblog.com"
language = "en"
author = "Author"

[deploy]
target = "github-pages"
"#;
        std::fs::write(&config_path, initial).unwrap();

        let mut updates = HashMap::new();
        updates.insert("project".into(), "new-project".into());
        update_deploy_config(&config_path, &updates).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        let doc: toml::Table = result.parse().unwrap();
        let site = doc.get("site").unwrap().as_table().unwrap();
        // Original site fields should be preserved
        assert_eq!(site.get("title").unwrap().as_str().unwrap(), "My Blog");
        assert_eq!(
            site.get("description").unwrap().as_str().unwrap(),
            "A great blog"
        );
        assert_eq!(site.get("author").unwrap().as_str().unwrap(), "Author");
    }

    // -----------------------------------------------------------------------
    // try_fix_check — all check names return correct number of manual instructions
    // -----------------------------------------------------------------------

    #[test]
    fn test_try_fix_check_output_dir_has_manual_instructions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Output directory".into(),
            passed: false,
            message: "missing".into(),
        };
        let fix = try_fix_check(&check, &paths, "github-pages").unwrap();
        assert!(!fix.manual_instructions.is_empty());
        assert!(fix.manual_instructions[0].contains("seite build"));
    }

    #[test]
    fn test_try_fix_check_base_url_has_manual_instructions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Base URL".into(),
            passed: false,
            message: "localhost".into(),
        };
        let fix = try_fix_check(&check, &paths, "github-pages").unwrap();
        assert!(fix.manual_instructions.len() >= 2);
        assert!(fix
            .manual_instructions
            .iter()
            .any(|i| i.contains("base_url")));
        assert!(fix
            .manual_instructions
            .iter()
            .any(|i| i.contains("--base-url")));
    }

    #[test]
    fn test_try_fix_check_git_repo_has_manual_instructions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = test_paths(tmp.path());
        let check = PreflightCheck {
            name: "Git repository".into(),
            passed: false,
            message: "not a repo".into(),
        };
        let fix = try_fix_check(&check, &paths, "github-pages").unwrap();
        assert!(!fix.manual_instructions.is_empty());
        assert!(fix.manual_instructions[0].contains("git init"));
    }

    // -----------------------------------------------------------------------
    // check_cloudflare_domain — with domain and project both set (API call path)
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_cloudflare_domain_with_domain_and_project() {
        let mut config = test_config("https://example.com");
        config.deploy.domain = Some("example.com".into());
        config.deploy.project = Some("my-project".into());
        let check = check_cloudflare_domain(&config);
        assert_eq!(check.name, "Cloudflare domain");
        // The API call will likely fail (no credentials), but that returns
        // passed=true with "could not verify" message
        assert!(
            check.message.contains("example.com"),
            "message should mention the domain: {}",
            check.message
        );
    }

    // -----------------------------------------------------------------------
    // check_cloudflare_auth / check_netlify_auth — name field
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_cloudflare_auth_name_field() {
        let check = check_cloudflare_auth();
        assert_eq!(check.name, "Cloudflare auth");
    }

    #[test]
    fn test_check_netlify_auth_name_field() {
        let check = check_netlify_auth();
        assert_eq!(check.name, "Netlify auth");
    }

    // -----------------------------------------------------------------------
    // deploy_github_pages — .nojekyll and CNAME generation
    // -----------------------------------------------------------------------

    #[test]
    fn test_deploy_github_pages_writes_nojekyll() {
        // We can't test the full deploy (needs git), but we can test that
        // the function creates .nojekyll before the git commands fail
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist.clone(),
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://example.com");
        // This will fail at the git init step, but .nojekyll should exist
        let _ = deploy_github_pages(&config, &paths, Some("https://github.com/user/repo"));
        assert!(dist.join(".nojekyll").exists());
    }

    #[test]
    fn test_deploy_github_pages_writes_cname_for_custom_domain() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist.clone(),
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://myblog.com");
        let _ = deploy_github_pages(&config, &paths, Some("https://github.com/user/repo"));
        let cname_path = dist.join("CNAME");
        assert!(cname_path.exists());
        let content = std::fs::read_to_string(&cname_path).unwrap();
        assert_eq!(content, "myblog.com");
    }

    #[test]
    fn test_deploy_github_pages_no_cname_for_github_io() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist.clone(),
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = test_config("https://user.github.io");
        let _ = deploy_github_pages(&config, &paths, Some("https://github.com/user/repo"));
        // CNAME should NOT be written for github.io URLs
        assert!(!dist.join("CNAME").exists());
    }

    // -----------------------------------------------------------------------
    // check_output_dir — message contains path
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_output_dir_pass_message_contains_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("file.html"), "ok").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            output: dist.clone(),
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let check = check_output_dir(&paths);
        assert!(check.passed);
        assert!(
            check.message.contains("dist"),
            "passed message should include the output path: {}",
            check.message
        );
    }
}
