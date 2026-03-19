//! `seite upgrade` — bring project configuration up to date with the current binary.
//!
//! When a user upgrades the `page` binary, their existing project may lack new
//! config entries (e.g., MCP server settings, new permission rules). This command
//! detects what's outdated and applies additive, non-destructive upgrades.
//!
//! Each upgrade step is gated to the version that introduced it, so running
//! `seite upgrade` on an already-current project is a fast no-op.

use std::fs;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::meta;
use crate::output::human;

#[derive(Args)]
pub struct UpgradeArgs {
    /// Apply all upgrades without confirmation
    #[arg(long)]
    pub force: bool,

    /// Check for needed upgrades without applying them (exits with code 1 if outdated)
    #[arg(long)]
    pub check: bool,
}

/// A single upgrade action to present and optionally apply.
enum UpgradeAction {
    Create {
        path: PathBuf,
        content: String,
        description: String,
    },
    MergeJson {
        path: PathBuf,
        merged: serde_json::Value,
        additions: Vec<String>,
    },
    Append {
        path: PathBuf,
        content: String,
        description: String,
    },
    /// Insert a `key = value` line into a `[section]` of a TOML file if not already present.
    InjectToml {
        path: PathBuf,
        section: String,
        key: String,
        value: String,
        description: String,
    },
}

impl UpgradeAction {
    fn describe(&self) -> Vec<String> {
        match self {
            UpgradeAction::Create { description, .. } => {
                vec![description.clone()]
            }
            UpgradeAction::MergeJson { additions, .. } => additions.clone(),
            UpgradeAction::InjectToml { description, .. } => {
                vec![description.clone()]
            }
            UpgradeAction::Append { description, .. } => {
                vec![description.clone()]
            }
        }
    }
}

/// A version-gated upgrade step.
#[allow(dead_code)]
struct UpgradeStep {
    /// The version that introduced this upgrade.
    introduced_in: (u64, u64, u64),
    /// Human-readable description (for documentation and future `--verbose` output).
    label: &'static str,
    /// The function that computes the upgrade action(s), if any.
    check: fn(root: &Path) -> Vec<UpgradeAction>,
}

/// All upgrade steps, ordered by version. New steps go at the bottom.
const fn upgrade_steps() -> &'static [UpgradeStep] {
    &[
        UpgradeStep {
            introduced_in: (0, 1, 0),
            label: "Project metadata (.seite/config.json)",
            check: check_page_meta,
        },
        UpgradeStep {
            introduced_in: (0, 1, 0),
            label: "MCP server for AI tools",
            check: check_mcp_server,
        },
        UpgradeStep {
            introduced_in: (0, 1, 0),
            label: "CLAUDE.md MCP documentation",
            check: check_claude_md_mcp,
        },
        UpgradeStep {
            introduced_in: (0, 1, 4),
            label: "Landing page builder skill (/landing-page)",
            check: check_landing_page_skill,
        },
        UpgradeStep {
            introduced_in: (0, 1, 5),
            label: "Theme builder skill (/theme-builder)",
            check: check_theme_builder_skill,
        },
        UpgradeStep {
            introduced_in: (0, 1, 6),
            label: "Fix deploy workflows (use shell installer instead of cargo install)",
            check: check_deploy_workflows,
        },
        UpgradeStep {
            introduced_in: (0, 1, 9),
            label: "Pin seite version in deploy workflows",
            check: check_deploy_version_pinning,
        },
        UpgradeStep {
            introduced_in: (0, 2, 0),
            label: "Contact form support",
            check: check_contact_form_docs,
        },
        UpgradeStep {
            introduced_in: (0, 2, 1),
            label: "Public directory for root-level files",
            check: check_public_dir,
        },
        UpgradeStep {
            introduced_in: (0, 2, 4),
            label: "Brand identity builder skill (/brand-identity)",
            check: check_brand_identity_skill,
        },
        UpgradeStep {
            introduced_in: (0, 4, 0),
            label: "Subdomain deploy support",
            check: check_subdomain_deploy_docs,
        },
        UpgradeStep {
            introduced_in: (0, 4, 0),
            label: ".gitignore dist-subdomains/ entry",
            check: check_gitignore_dist_subdomains,
        },
        UpgradeStep {
            introduced_in: (0, 5, 0),
            label: "Path-scoped .claude/rules/ context files",
            check: check_claude_rules,
        },
        UpgradeStep {
            introduced_in: (0, 7, 0),
            label: "Enable CSS/JS minification by default",
            check: check_minify_default,
        },
        UpgradeStep {
            introduced_in: (0, 8, 0),
            label: "Atom feed + redirect aliases documentation",
            check: check_atom_aliases_docs,
        },
        UpgradeStep {
            introduced_in: (0, 8, 0),
            label: "Atom autodiscovery in custom templates",
            check: check_atom_autodiscovery_template,
        },
        UpgradeStep {
            introduced_in: (0, 11, 0),
            label: "Remove pinned VERSION from deploy workflows (use latest)",
            check: check_deploy_version_unpinning,
        },
        UpgradeStep {
            introduced_in: (0, 11, 0),
            label: "Subdomain collection deploy steps in CI workflow",
            check: check_subdomain_workflow,
        },
    ]
}

pub fn run(args: &UpgradeArgs) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;

    // Verify this is a page project
    if !root.join("seite.toml").exists() {
        anyhow::bail!(
            "No seite.toml found in current directory. Run this command from a seite project root."
        );
    }

    let project_ver = meta::project_version(&root);
    let binary_ver = meta::binary_version();

    // Collect all applicable actions
    let mut actions: Vec<UpgradeAction> = Vec::new();
    for step in upgrade_steps() {
        if step.introduced_in > project_ver {
            let step_actions = (step.check)(&root);
            actions.extend(step_actions);
        }
    }

    if actions.is_empty() {
        // No file changes needed, but still stamp the version if it's behind.
        // This handles the case where upgrade steps exist but their checks found
        // nothing to do (files already present), or where the binary was bumped
        // without adding new upgrade steps (e.g. 0.4.0 → 0.4.3 with only bug fixes).
        if project_ver < binary_ver {
            let existing_meta = meta::load(&root);
            let new_meta = meta::PageMeta::stamp_current_version(existing_meta.as_ref());
            meta::write(&root, &new_meta)?;
        }
        human::success(&format!(
            "Project is up to date (seite {}).",
            meta::format_version(binary_ver)
        ));
        return Ok(());
    }

    // Show what will change
    human::header(&format!(
        "Upgrading from {} → {}",
        if project_ver == (0, 0, 0) {
            "pre-tracking".to_string()
        } else {
            meta::format_version(project_ver)
        },
        meta::format_version(binary_ver)
    ));
    println!();

    for action in &actions {
        for line in action.describe() {
            human::info(&format!("  {line}"));
        }
    }
    println!();

    // --check mode: just report and exit
    if args.check {
        human::info("Run `seite upgrade` to apply these changes.");
        std::process::exit(1);
    }

    // Confirm unless --force
    if !args.force {
        let proceed = dialoguer::Confirm::new()
            .with_prompt("Apply these upgrades?")
            .default(true)
            .interact()?;
        if !proceed {
            human::info("Upgrade cancelled.");
            return Ok(());
        }
    }

    // Apply all actions
    for action in actions {
        match action {
            UpgradeAction::Create {
                path,
                content,
                description,
            } => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&path, &content)?;
                human::success(&format!("Created {description}"));
            }
            UpgradeAction::MergeJson {
                path,
                merged,
                additions,
            } => {
                let json = serde_json::to_string_pretty(&merged)?;
                fs::write(&path, format!("{json}\n"))?;
                for desc in &additions {
                    human::success(desc);
                }
            }
            UpgradeAction::Append {
                path,
                content,
                description,
            } => {
                let mut existing = fs::read_to_string(&path).unwrap_or_default();
                existing.push_str(&content);
                fs::write(&path, existing)?;
                human::success(&format!("Updated {description}"));
            }
            UpgradeAction::InjectToml {
                path,
                section,
                key,
                value,
                description,
            } => {
                let existing = fs::read_to_string(&path).unwrap_or_default();
                let key_prefix = format!("{key} =");
                let patched = if existing.contains(&key_prefix) {
                    // Key already set — leave as-is
                    existing
                } else {
                    let section_header = format!("[{section}]");
                    existing.replacen(
                        &section_header,
                        &format!("{section_header}\n{key} = {value}"),
                        1,
                    )
                };
                fs::write(&path, patched)?;
                human::success(&format!("Updated {description}"));
            }
        }
    }

    // Stamp the new version
    let existing_meta = meta::load(&root);
    let new_meta = meta::PageMeta::stamp_current_version(existing_meta.as_ref());
    meta::write(&root, &new_meta)?;

    println!();
    human::success(&format!(
        "Project upgraded to seite {}",
        meta::format_version(binary_ver)
    ));

    // Hint about trimming CLAUDE.md if rules files were created
    if root.join(".claude/rules").exists() {
        if let Ok(content) = fs::read_to_string(root.join("CLAUDE.md")) {
            if content.lines().count() > 300 {
                println!();
                human::info(
                    "Detailed context now lives in .claude/rules/ and loads automatically.",
                );
                human::info("You can trim your CLAUDE.md — the rules files have the details.");
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Upgrade step implementations
// ---------------------------------------------------------------------------

/// Ensure `.seite/config.json` exists.
fn check_page_meta(root: &Path) -> Vec<UpgradeAction> {
    let path = meta::meta_path(root);
    if path.exists() {
        return vec![];
    }

    let meta = meta::PageMeta {
        version: env!("CARGO_PKG_VERSION").to_string(),
        initialized_at: None, // existing project, don't fake an init time
    };
    let content = serde_json::to_string_pretty(&meta).unwrap_or_default();

    vec![UpgradeAction::Create {
        path,
        content,
        description: ".seite/config.json (project metadata)".into(),
    }]
}

/// Ensure `.claude/settings.json` has the `mcpServers.seite` block.
fn check_mcp_server(root: &Path) -> Vec<UpgradeAction> {
    let path = root.join(".claude/settings.json");

    if !path.exists() {
        // No Claude settings at all — create the full file
        let content = crate::cli::init::mcp_server_block();
        let full_settings = serde_json::json!({
            "$schema": "https://json.schemastore.org/claude-code-settings.json",
            "permissions": {
                "allow": [
                    "Read",
                    "Write(content/**)",
                    "Write(templates/**)",
                    "Write(static/**)",
                    "Write(data/**)",
                    "Edit(content/**)",
                    "Edit(templates/**)",
                    "Edit(data/**)",
                    "Bash(seite build:*)",
                    "Bash(seite build)",
                    "Bash(seite new:*)",
                    "Bash(seite serve:*)",
                    "Bash(seite theme:*)",
                    "Glob",
                    "Grep",
                    "WebSearch"
                ],
                "deny": [
                    "Read(.env)",
                    "Read(.env.*)"
                ]
            },
            "mcpServers": content,
        });
        let json = serde_json::to_string_pretty(&full_settings).unwrap_or_default();
        return vec![UpgradeAction::Create {
            path,
            content: format!("{json}\n"),
            description: ".claude/settings.json (with MCP server)".into(),
        }];
    }

    // File exists — check if mcpServers.seite is already there
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![], // malformed JSON, don't touch it
    };

    // Check if mcpServers.seite already exists
    if settings.pointer("/mcpServers/seite").is_some() {
        return vec![];
    }

    // Merge: add mcpServers block
    let mcp_block = crate::cli::init::mcp_server_block();

    let mut additions = Vec::new();

    if let Some(existing_mcp) = settings.get_mut("mcpServers") {
        // mcpServers exists but no "seite" key — add it
        if let Some(obj) = existing_mcp.as_object_mut() {
            obj.insert(
                "seite".to_string(),
                mcp_block.get("seite").cloned().unwrap_or_default(),
            );
            additions.push("Added mcpServers.seite to .claude/settings.json".into());
        }
    } else {
        // No mcpServers at all — add the whole block
        if let Some(obj) = settings.as_object_mut() {
            obj.insert("mcpServers".to_string(), mcp_block);
            additions.push("Added mcpServers.seite to .claude/settings.json".into());
        }
    }

    if additions.is_empty() {
        return vec![];
    }

    vec![UpgradeAction::MergeJson {
        path,
        merged: settings,
        additions,
    }]
}

/// Ensure `.claude/skills/landing-page/SKILL.md` exists and is up-to-date when
/// the project has a pages collection.
///
/// Skills embed a `# seite-skill-version: N` comment in their YAML frontmatter.
/// If the existing file has a lower version (or none), the upgrade replaces it
/// with the bundled version. This lets us ship improved prompts in new releases
/// without requiring the user to manually diff skill files.
///
/// Also handles migration from the old `homepage` skill name to `landing-page`.
fn check_landing_page_skill(root: &Path) -> Vec<UpgradeAction> {
    // Only relevant if the project has a pages collection
    let config_path = root.join("seite.toml");
    let has_pages = match fs::read_to_string(&config_path) {
        Ok(content) => content.contains("name = \"pages\""),
        Err(_) => false,
    };
    if !has_pages {
        return vec![];
    }

    let bundled = include_str!("../scaffold/skill-landing-page.md");
    let bundled_version = extract_skill_version(bundled);
    let skill_path = root.join(".claude/skills/landing-page/SKILL.md");

    // Check if the new landing-page skill already exists and is current
    if skill_path.exists() {
        let existing = fs::read_to_string(&skill_path).unwrap_or_default();
        let existing_version = extract_skill_version(&existing);
        if existing_version >= bundled_version {
            return vec![];
        }
        // Outdated — replace with newer version
        return vec![UpgradeAction::Create {
            path: skill_path,
            content: bundled.to_string(),
            description: format!(
                ".claude/skills/landing-page/SKILL.md (updated v{existing_version} → v{bundled_version})"
            ),
        }];
    }

    // Check for old homepage skill — version comparison still applies
    let old_skill_path = root.join(".claude/skills/homepage/SKILL.md");
    if old_skill_path.exists() {
        let existing = fs::read_to_string(&old_skill_path).unwrap_or_default();
        let existing_version = extract_skill_version(&existing);
        if existing_version >= bundled_version {
            return vec![];
        }
    }

    vec![UpgradeAction::Create {
        path: skill_path,
        content: bundled.to_string(),
        description: ".claude/skills/landing-page/SKILL.md (/landing-page command)".into(),
    }]
}

/// Ensure `.claude/skills/theme-builder/SKILL.md` exists and is up-to-date.
///
/// Unlike the landing-page skill, the theme builder is unconditional — every
/// site benefits from interactive theme creation.
fn check_theme_builder_skill(root: &Path) -> Vec<UpgradeAction> {
    let bundled = include_str!("../scaffold/skill-theme-builder.md");
    let bundled_version = extract_skill_version(bundled);
    let skill_path = root.join(".claude/skills/theme-builder/SKILL.md");

    if skill_path.exists() {
        let existing = fs::read_to_string(&skill_path).unwrap_or_default();
        let existing_version = extract_skill_version(&existing);
        if existing_version >= bundled_version {
            return vec![];
        }
        return vec![UpgradeAction::Create {
            path: skill_path,
            content: bundled.to_string(),
            description: format!(
                ".claude/skills/theme-builder/SKILL.md (updated v{existing_version} → v{bundled_version})"
            ),
        }];
    }

    vec![UpgradeAction::Create {
        path: skill_path,
        content: bundled.to_string(),
        description: ".claude/skills/theme-builder/SKILL.md (/theme-builder command)".into(),
    }]
}

/// Ensure `.claude/skills/brand-identity/SKILL.md` exists and is up-to-date.
///
/// Like the theme builder, this is unconditional — every site benefits from
/// brand identity creation (logo, color palette, favicon).
fn check_brand_identity_skill(root: &Path) -> Vec<UpgradeAction> {
    let bundled = include_str!("../scaffold/skill-brand-identity.md");
    let bundled_version = extract_skill_version(bundled);
    let skill_path = root.join(".claude/skills/brand-identity/SKILL.md");

    if skill_path.exists() {
        let existing = fs::read_to_string(&skill_path).unwrap_or_default();
        let existing_version = extract_skill_version(&existing);
        if existing_version >= bundled_version {
            return vec![];
        }
        return vec![UpgradeAction::Create {
            path: skill_path,
            content: bundled.to_string(),
            description: format!(
                ".claude/skills/brand-identity/SKILL.md (updated v{existing_version} → v{bundled_version})"
            ),
        }];
    }

    vec![UpgradeAction::Create {
        path: skill_path,
        content: bundled.to_string(),
        description: ".claude/skills/brand-identity/SKILL.md (/brand-identity command)".into(),
    }]
}

/// Extract the `# seite-skill-version: N` value from a SKILL.md file.
/// Returns 0 if not found.
fn extract_skill_version(content: &str) -> u32 {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# seite-skill-version:") {
            if let Ok(v) = rest.trim().parse::<u32>() {
                return v;
            }
        }
    }
    0
}

/// Fix deploy workflows that use `cargo install --path .` instead of the shell installer.
///
/// The old generated workflows assumed the seite Rust source code was in the user's
/// repo. This replaces them with workflows that download the pre-built binary.
fn check_deploy_workflows(root: &Path) -> Vec<UpgradeAction> {
    let mut actions = Vec::new();

    // Load site config to determine deploy target and regenerate correct workflow
    let config_path = root.join("seite.toml");
    let config = match crate::config::SiteConfig::load(&config_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Check .github/workflows/deploy.yml
    let workflow_path = root.join(".github/workflows/deploy.yml");
    if workflow_path.exists() {
        let content = fs::read_to_string(&workflow_path).unwrap_or_default();
        if content.contains("cargo install --path .") {
            let new_workflow = match &config.deploy.target {
                crate::config::DeployTarget::GithubPages => {
                    crate::deploy::generate_github_actions_workflow(&config)
                }
                crate::config::DeployTarget::Cloudflare => {
                    crate::deploy::generate_cloudflare_workflow(&config)
                }
                crate::config::DeployTarget::Netlify => {
                    crate::deploy::generate_netlify_workflow(&config)
                }
            };
            actions.push(UpgradeAction::Create {
                path: workflow_path,
                content: new_workflow,
                description:
                    ".github/workflows/deploy.yml (use shell installer instead of cargo install)"
                        .into(),
            });
        }
    }

    // Check netlify.toml
    let netlify_path = root.join("netlify.toml");
    if netlify_path.exists() {
        let content = fs::read_to_string(&netlify_path).unwrap_or_default();
        if content.contains("cargo install --path .") {
            let new_config = crate::deploy::generate_netlify_config(&config);
            actions.push(UpgradeAction::Create {
                path: netlify_path,
                content: new_config,
                description: "netlify.toml (use shell installer instead of cargo install)".into(),
            });
        }
    }

    actions
}

/// Previously pinned VERSION in deploy workflows. Now a no-op because we
/// switched to always installing latest (the install script resolves latest
/// via version.txt). Kept as a no-op to avoid breaking the upgrade step chain.
fn check_deploy_version_pinning(_root: &Path) -> Vec<UpgradeAction> {
    vec![]
}

/// Ensure CLAUDE.md has an MCP server section.
fn check_claude_md_mcp(root: &Path) -> Vec<UpgradeAction> {
    let path = root.join("CLAUDE.md");
    if !path.exists() {
        return vec![];
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Already has MCP section — nothing to do
    if content.contains("## MCP Server") {
        return vec![];
    }

    let section = r#"

## MCP Server

This project includes an MCP server that AI tools can connect to for structured
access to site content, documentation, themes, and build tools.

The server is configured in `.claude/settings.json` and starts automatically
when Claude Code opens this project. No API keys or setup required.

**Available tools:** `seite_build`, `seite_create_content`, `seite_search`,
`seite_apply_theme`, `seite_lookup_docs`

**Available resources:** `seite://docs/*` (page documentation),
`seite://content/*` (site content), `seite://themes` (themes),
`seite://config` (site configuration), `seite://mcp-config` (MCP settings)

The MCP server provides typed, structured access to your site — AI tools work
with page concepts (collections, content items, themes) rather than parsing
raw files.
"#;

    vec![UpgradeAction::Append {
        path,
        content: section.to_string(),
        description: "CLAUDE.md (added MCP Server section)".into(),
    }]
}

/// Ensure `public/` directory exists for root-level files.
fn check_public_dir(root: &Path) -> Vec<UpgradeAction> {
    let public_dir = root.join("public");
    if public_dir.exists() {
        return vec![];
    }

    vec![UpgradeAction::Create {
        path: public_dir.join(".gitkeep"),
        content: String::new(),
        description: "public/ directory for root-level files (favicon.ico, .well-known/, etc.)"
            .into(),
    }]
}

/// Ensure CLAUDE.md mentions contact form support.
fn check_contact_form_docs(root: &Path) -> Vec<UpgradeAction> {
    let path = root.join("CLAUDE.md");
    if !path.exists() {
        return vec![];
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    if content.contains("contact_form") || content.contains("## Contact Form") {
        return vec![];
    }

    let section = r#"

## Contact Forms

This project supports built-in contact forms via the `{{< contact_form() >}}` shortcode.

**Supported providers:** Formspree, Web3Forms, Netlify Forms, HubSpot, Typeform

**Setup:** Run `seite contact setup` to configure a contact form provider.
The shortcode renders a styled form matching the current theme.

**Configuration** in `seite.toml`:
```toml
[contact]
provider = "formspree"   # or web3forms, netlify, hubspot, typeform
endpoint = "your-form-id"
```
"#;

    vec![UpgradeAction::Append {
        path,
        content: section.to_string(),
        description: "CLAUDE.md (added Contact Forms section)".into(),
    }]
}

/// Ensure CLAUDE.md mentions subdomain deploy support.
fn check_subdomain_deploy_docs(root: &Path) -> Vec<UpgradeAction> {
    let path = root.join("CLAUDE.md");
    if !path.exists() {
        return vec![];
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    if content.contains("subdomain") || content.contains("dist-subdomains") {
        return vec![];
    }

    let section = r#"

## Subdomain Deploys

Set `subdomain = "docs"` on a collection in `seite.toml` to deploy it to `docs.{base_domain}`.

```toml
[[collections]]
name = "docs"
subdomain = "docs"
deploy_project = "my-site-docs"  # optional, auto-created by deploy --setup
```

**What happens:**
- Collection builds to `dist-subdomains/{name}/` with own sitemap, RSS, robots.txt, search index
- Cross-subdomain links are auto-rewritten to absolute URLs (e.g., `/docs/setup` → `https://docs.example.com/setup`)
- Dev server previews at `/{name}-preview/`
- `seite deploy --setup` auto-creates Cloudflare/Netlify projects for subdomain collections
- GitHub Pages does not support subdomain deploys — use Cloudflare Pages or Netlify
"#;

    vec![UpgradeAction::Append {
        path,
        content: section.to_string(),
        description: "CLAUDE.md (added Subdomain Deploys section)".into(),
    }]
}

/// Ensure .gitignore includes dist-subdomains/.
fn check_gitignore_dist_subdomains(root: &Path) -> Vec<UpgradeAction> {
    let path = root.join(".gitignore");
    if !path.exists() {
        return vec![];
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    if content.contains("dist-subdomains") {
        return vec![];
    }

    vec![UpgradeAction::Append {
        path,
        content: "\n/dist-subdomains\n".to_string(),
        description: ".gitignore (added dist-subdomains/)".into(),
    }]
}

/// Create `.claude/rules/*.md` files with path-scoped context for Claude.
fn check_claude_rules(root: &Path) -> Vec<UpgradeAction> {
    use crate::cli::init::rules_file;

    let rules_dir = root.join(".claude/rules");
    let mut actions = Vec::new();

    // Always-present rules
    let always_rules: &[(&str, &[&str], &str)] = &[
        (
            "seo-requirements.md",
            &["templates/**"],
            include_str!("../scaffold/seo-requirements.md"),
        ),
        (
            "templates.md",
            &["templates/**"],
            include_str!("../scaffold/templates.md"),
        ),
        (
            "i18n.md",
            &["content/**", "templates/**", "data/i18n/**"],
            include_str!("../scaffold/i18n.md"),
        ),
        (
            "data-files.md",
            &["data/**"],
            include_str!("../scaffold/data-files.md"),
        ),
        (
            "shortcodes.md",
            &["content/**", "templates/shortcodes/**"],
            include_str!("../scaffold/shortcodes.md"),
        ),
        (
            "config-reference.md",
            &["seite.toml"],
            include_str!("../scaffold/config-reference.md"),
        ),
        (
            "features.md",
            &["content/**", "templates/**"],
            include_str!("../scaffold/features.md"),
        ),
        (
            "design-prompts.md",
            &["templates/**"],
            include_str!("../scaffold/design-prompts.md"),
        ),
    ];

    for (filename, paths, content) in always_rules {
        let path = rules_dir.join(filename);
        if !path.exists() {
            actions.push(UpgradeAction::Create {
                path,
                content: rules_file(paths, content),
                description: format!(".claude/rules/{filename}"),
            });
        }
    }

    // Conditional: contact form (check seite.toml for [contact])
    let config_path = root.join("seite.toml");
    if let Ok(config_content) = fs::read_to_string(&config_path) {
        if config_content.contains("[contact]") {
            let path = rules_dir.join("contact-form.md");
            if !path.exists() {
                actions.push(UpgradeAction::Create {
                    path,
                    content: rules_file(
                        &["content/**", "templates/**", "seite.toml"],
                        include_str!("../scaffold/contact-form.md"),
                    ),
                    description: ".claude/rules/contact-form.md".into(),
                });
            }
        }

        // Conditional: trust center
        if config_content.contains("name = \"trust\"") {
            let path = rules_dir.join("trust-center.md");
            if !path.exists() {
                actions.push(UpgradeAction::Create {
                    path,
                    content: rules_file(
                        &["content/trust/**", "data/trust/**"],
                        include_str!("../scaffold/rules-trust-center.md"),
                    ),
                    description: ".claude/rules/trust-center.md".into(),
                });
            }
        }
    }

    actions
}

/// Document Atom feed and redirect aliases in CLAUDE.md.
/// These features were added in 0.8.0.
fn check_atom_aliases_docs(root: &Path) -> Vec<UpgradeAction> {
    let path = root.join("CLAUDE.md");
    if !path.exists() {
        return vec![];
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Skip if already documented
    if content.contains("atom.xml") || content.contains("Atom Feed") {
        return vec![];
    }

    let section = r#"

## Atom Feeds & Redirect Aliases

**Atom feeds** are now generated alongside RSS. Every collection with `has_rss = true` produces
both `feed.xml` (RSS 2.0) and `atom.xml` (Atom 1.0). Bundled themes include autodiscovery
`<link>` tags for both formats. No configuration changes needed.

**Redirect aliases** let you set `aliases: ["/old-path"]` in any page's frontmatter.
During build, seite generates:
- A lightweight HTML redirect file at each alias path (`<meta http-equiv="refresh">`)
- A `_redirects` file (Netlify/Cloudflare compatible) with 301 status codes

Example frontmatter:
```yaml
aliases:
  - /old-url
  - /legacy/path
```
"#;

    vec![UpgradeAction::Append {
        path,
        content: section.to_string(),
        description: "CLAUDE.md (added Atom Feeds & Redirect Aliases section)".into(),
    }]
}

/// Check for custom templates that may need Atom autodiscovery `<link>` tags.
/// Bundled themes are auto-upgraded, but custom `templates/base.html` files need
/// the user to add the Atom link manually.
fn check_atom_autodiscovery_template(root: &Path) -> Vec<UpgradeAction> {
    let base_html = root.join("templates/base.html");
    if !base_html.exists() {
        return vec![]; // Using bundled theme — auto-upgraded
    }

    let content = match fs::read_to_string(&base_html) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Skip if already has Atom autodiscovery
    if content.contains("application/atom+xml") {
        return vec![];
    }

    // Check if there's an RSS link we can suggest adding the Atom link next to
    let hint = if content.contains("application/rss+xml") {
        "Add an Atom autodiscovery link next to your existing RSS link"
    } else {
        "Add Atom feed autodiscovery to your <head>"
    };

    let snippet = r#"<link rel="alternate" type="application/atom+xml" title="{{ site.title }}" href="{{ lang_prefix }}/atom.xml">"#;

    vec![UpgradeAction::Append {
        path: root.join("CLAUDE.md"),
        content: format!(
            "\n\n### Custom Template: Atom Autodiscovery\n\n\
             Your custom `templates/base.html` does not include Atom feed autodiscovery.\n\
             {hint}:\n\n\
             ```html\n{snippet}\n```\n"
        ),
        description: "CLAUDE.md (Atom autodiscovery hint for custom template)".into(),
    }]
}

/// Remove hardcoded `VERSION=x.y.z` from deploy workflows so they always install
/// the latest seite release. The install script resolves latest via `version.txt`.
///
/// Previously, workflows pinned to the version that generated them, which meant
/// projects never got seite updates in CI without manually editing the workflow
/// or running `seite upgrade` (which itself required the new binary).
fn check_deploy_version_unpinning(root: &Path) -> Vec<UpgradeAction> {
    let mut actions = Vec::new();

    let config_path = root.join("seite.toml");
    let config = match crate::config::SiteConfig::load(&config_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Check .github/workflows/deploy.yml
    let workflow_path = root.join(".github/workflows/deploy.yml");
    if workflow_path.exists() {
        let content = fs::read_to_string(&workflow_path).unwrap_or_default();
        if content.contains("VERSION=") {
            let new_workflow = match &config.deploy.target {
                crate::config::DeployTarget::GithubPages => {
                    crate::deploy::generate_github_actions_workflow(&config)
                }
                crate::config::DeployTarget::Cloudflare => {
                    crate::deploy::generate_cloudflare_workflow(&config)
                }
                crate::config::DeployTarget::Netlify => {
                    crate::deploy::generate_netlify_workflow(&config)
                }
            };
            actions.push(UpgradeAction::Create {
                path: workflow_path,
                content: new_workflow,
                description:
                    ".github/workflows/deploy.yml (removed VERSION pin, now installs latest)"
                        .into(),
            });
        }
    }

    // Check netlify.toml
    let netlify_path = root.join("netlify.toml");
    if netlify_path.exists() {
        let content = fs::read_to_string(&netlify_path).unwrap_or_default();
        if content.contains("VERSION=") {
            let new_config = crate::deploy::generate_netlify_config(&config);
            actions.push(UpgradeAction::Create {
                path: netlify_path,
                content: new_config,
                description: "netlify.toml (removed VERSION pin, now installs latest)".into(),
            });
        }
    }

    actions
}

/// Regenerate the deploy workflow when subdomain collections have `deploy_project`
/// but the workflow doesn't include their deploy steps.
fn check_subdomain_workflow(root: &Path) -> Vec<UpgradeAction> {
    let config_path = root.join("seite.toml");
    let config = match crate::config::SiteConfig::load(&config_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Only relevant for Cloudflare/Netlify (GitHub Pages can't do multi-project)
    let is_github_pages = matches!(
        config.deploy.target,
        crate::config::DeployTarget::GithubPages
    );
    if is_github_pages {
        return vec![];
    }

    // Check if any subdomain collections have deploy_project configured
    let subdomain_deploy_projects: Vec<_> = config
        .subdomain_collections()
        .into_iter()
        .filter_map(|c| {
            c.deploy_project
                .as_deref()
                .map(|p| (c.name.clone(), p.to_string()))
        })
        .collect();

    if subdomain_deploy_projects.is_empty() {
        return vec![];
    }

    let workflow_path = root.join(".github/workflows/deploy.yml");
    if !workflow_path.exists() {
        return vec![];
    }

    let content = fs::read_to_string(&workflow_path).unwrap_or_default();

    // Check if all subdomain deploy projects are already referenced in the workflow
    let all_present = subdomain_deploy_projects
        .iter()
        .all(|(_, project)| content.contains(project));

    if all_present {
        return vec![];
    }

    let missing: Vec<_> = subdomain_deploy_projects
        .iter()
        .filter(|(_, project)| !content.contains(project.as_str()))
        .map(|(name, _)| name.as_str())
        .collect();

    let new_workflow = match &config.deploy.target {
        crate::config::DeployTarget::Cloudflare => {
            crate::deploy::generate_cloudflare_workflow(&config)
        }
        crate::config::DeployTarget::Netlify => crate::deploy::generate_netlify_workflow(&config),
        crate::config::DeployTarget::GithubPages => unreachable!(),
    };

    vec![UpgradeAction::Create {
        path: workflow_path,
        content: new_workflow,
        description: format!(
            ".github/workflows/deploy.yml (added deploy steps for subdomain collections: {})",
            missing.join(", ")
        ),
    }]
}

/// Add `minify = true` to `[build]` in seite.toml if not already set.
/// This became the default for new sites in 0.7.0.
fn check_minify_default(root: &Path) -> Vec<UpgradeAction> {
    let path = root.join("seite.toml");
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Only act if the file has a [build] section and minify isn't already configured
    if !content.contains("[build]") || content.contains("minify =") {
        return vec![];
    }

    vec![UpgradeAction::InjectToml {
        path,
        section: "build".to_string(),
        key: "minify".to_string(),
        value: "true".to_string(),
        description: "seite.toml (enabled minify = true in [build])".to_string(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_skill_version_found() {
        let content = "---\n# seite-skill-version: 3\n---\nContent here";
        assert_eq!(extract_skill_version(content), 3);
    }

    #[test]
    fn test_extract_skill_version_not_found() {
        let content = "---\nname: test\n---\nContent here";
        assert_eq!(extract_skill_version(content), 0);
    }

    #[test]
    fn test_extract_skill_version_invalid_number() {
        let content = "# seite-skill-version: abc";
        assert_eq!(extract_skill_version(content), 0);
    }

    #[test]
    fn test_extract_skill_version_with_spaces() {
        let content = "# seite-skill-version:   7  ";
        assert_eq!(extract_skill_version(content), 7);
    }

    #[test]
    fn test_upgrade_action_describe_create() {
        let action = UpgradeAction::Create {
            path: PathBuf::from("test.txt"),
            content: "content".into(),
            description: "test file".into(),
        };
        assert_eq!(action.describe(), vec!["test file"]);
    }

    #[test]
    fn test_upgrade_action_describe_merge_json() {
        let action = UpgradeAction::MergeJson {
            path: PathBuf::from("settings.json"),
            merged: serde_json::json!({}),
            additions: vec!["Added A".into(), "Added B".into()],
        };
        assert_eq!(action.describe(), vec!["Added A", "Added B"]);
    }

    #[test]
    fn test_upgrade_action_describe_append() {
        let action = UpgradeAction::Append {
            path: PathBuf::from("README.md"),
            content: "new section".into(),
            description: "README.md (added section)".into(),
        };
        assert_eq!(action.describe(), vec!["README.md (added section)"]);
    }

    #[test]
    fn test_check_page_meta_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let meta_path = meta::meta_path(tmp.path());
        fs::create_dir_all(meta_path.parent().unwrap()).unwrap();
        fs::write(&meta_path, "{}").unwrap();

        let actions = check_page_meta(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_page_meta_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_page_meta(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { description, .. } => {
                assert!(description.contains("config.json"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_claude_md_mcp_no_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_claude_md_mcp(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_claude_md_mcp_already_has_section() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("CLAUDE.md"),
            "# Project\n\n## MCP Server\nExists",
        )
        .unwrap();
        let actions = check_claude_md_mcp(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_claude_md_mcp_needs_section() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# Project\nSome content").unwrap();
        let actions = check_claude_md_mcp(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Append {
                description,
                content,
                ..
            } => {
                assert!(description.contains("MCP Server"));
                assert!(content.contains("MCP Server"));
            }
            _ => panic!("expected Append action"),
        }
    }

    #[test]
    fn test_check_public_dir_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("public")).unwrap();
        let actions = check_public_dir(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_public_dir_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_public_dir(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { description, .. } => {
                assert!(description.contains("public/"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_contact_form_docs_no_claude_md() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_contact_form_docs(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_contact_form_docs_already_has_section() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("CLAUDE.md"),
            "# Project\n\n## Contact Form\ncontact_form shortcode",
        )
        .unwrap();
        let actions = check_contact_form_docs(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_contact_form_docs_needs_section() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("CLAUDE.md"),
            "# My Project\nNothing about contact",
        )
        .unwrap();
        let actions = check_contact_form_docs(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Append { content, .. } => {
                assert!(content.contains("Contact Forms"));
                assert!(content.contains("contact_form"));
            }
            _ => panic!("expected Append action"),
        }
    }

    #[test]
    fn test_check_mcp_server_no_claude_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_mcp_server(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { description, .. } => {
                assert!(description.contains("settings.json"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_mcp_server_already_has_seite() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        let settings = serde_json::json!({
            "mcpServers": {
                "seite": {
                    "command": "seite",
                    "args": ["mcp"]
                }
            }
        });
        fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();
        let actions = check_mcp_server(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_mcp_server_has_mcp_but_no_seite() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        let settings = serde_json::json!({
            "mcpServers": {
                "other": { "command": "other" }
            }
        });
        fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();
        let actions = check_mcp_server(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::MergeJson { additions, .. } => {
                assert!(additions[0].contains("mcpServers.seite"));
            }
            _ => panic!("expected MergeJson action"),
        }
    }

    #[test]
    fn test_check_mcp_server_has_settings_no_mcp() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        let settings = serde_json::json!({
            "permissions": { "allow": ["Read"] }
        });
        fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();
        let actions = check_mcp_server(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::MergeJson { additions, .. } => {
                assert!(additions[0].contains("mcpServers.seite"));
            }
            _ => panic!("expected MergeJson action"),
        }
    }

    #[test]
    fn test_check_mcp_server_malformed_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("settings.json"), "not json").unwrap();
        let actions = check_mcp_server(tmp.path());
        assert!(actions.is_empty()); // malformed JSON, don't touch
    }

    #[test]
    fn test_check_landing_page_skill_no_pages_collection() {
        let tmp = tempfile::TempDir::new().unwrap();
        // seite.toml without pages collection
        fs::write(
            tmp.path().join("seite.toml"),
            "[site]\ntitle = \"Test\"\ndescription = \"\"\nbase_url = \"http://localhost\"\nlanguage = \"en\"\n\n[[collections]]\nname = \"posts\"\n",
        )
        .unwrap();
        let actions = check_landing_page_skill(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_landing_page_skill_with_pages() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            "[site]\ntitle = \"Test\"\n\n[[collections]]\nname = \"pages\"\n",
        )
        .unwrap();
        let actions = check_landing_page_skill(tmp.path());
        assert!(!actions.is_empty());
        match &actions[0] {
            UpgradeAction::Create { description, .. } => {
                assert!(description.contains("landing-page"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_theme_builder_skill_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_theme_builder_skill(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { description, .. } => {
                assert!(description.contains("theme-builder"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_theme_builder_skill_up_to_date() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skill_dir = tmp.path().join(".claude/skills/theme-builder");
        fs::create_dir_all(&skill_dir).unwrap();
        // Write the bundled version so it's considered up-to-date
        let bundled = include_str!("../scaffold/skill-theme-builder.md");
        fs::write(skill_dir.join("SKILL.md"), bundled).unwrap();
        let actions = check_theme_builder_skill(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_brand_identity_skill_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_brand_identity_skill(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { description, .. } => {
                assert!(description.contains("brand-identity"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_brand_identity_skill_up_to_date() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skill_dir = tmp.path().join(".claude/skills/brand-identity");
        fs::create_dir_all(&skill_dir).unwrap();
        let bundled = include_str!("../scaffold/skill-brand-identity.md");
        fs::write(skill_dir.join("SKILL.md"), bundled).unwrap();
        let actions = check_brand_identity_skill(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_brand_identity_skill_outdated() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skill_dir = tmp.path().join(".claude/skills/brand-identity");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\n# seite-skill-version: 0\n---\nOld content",
        )
        .unwrap();
        let actions = check_brand_identity_skill(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { description, .. } => {
                assert!(description.contains("updated v0"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_deploy_workflows_no_workflow() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            valid_seite_toml_with_deploy(),
        )
        .unwrap();
        let actions = check_deploy_workflows(tmp.path());
        assert!(actions.is_empty());
    }

    fn valid_seite_toml_with_deploy() -> &'static str {
        "[site]\ntitle = \"Test\"\ndescription = \"\"\nbase_url = \"http://localhost\"\nlanguage = \"en\"\nauthor = \"\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\nhas_rss = true\nlisted = true\nnested = false\nurl_prefix = \"/posts\"\ndefault_template = \"post.html\"\n\n[deploy]\ntarget = \"github-pages\"\n"
    }

    #[test]
    fn test_check_deploy_workflows_old_cargo_install() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            valid_seite_toml_with_deploy(),
        )
        .unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(
            wf_dir.join("deploy.yml"),
            "steps:\n  - run: cargo install --path .\n",
        )
        .unwrap();
        let actions = check_deploy_workflows(tmp.path());
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn test_check_deploy_version_pinning_is_noop() {
        // check_deploy_version_pinning is now a no-op (we no longer pin versions)
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_deploy_version_pinning(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_deploy_version_unpinning_removes_version() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            valid_seite_toml_with_deploy(),
        )
        .unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(
            wf_dir.join("deploy.yml"),
            "steps:\n  - run: VERSION=0.2.0 curl -fsSL https://seite.sh/install.sh | sh\n",
        )
        .unwrap();
        let actions = check_deploy_version_unpinning(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { content, .. } => {
                assert!(!content.contains("VERSION="));
                assert!(content.contains("curl -fsSL https://seite.sh/install.sh | sh"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_deploy_version_unpinning_already_unpinned() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            valid_seite_toml_with_deploy(),
        )
        .unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(
            wf_dir.join("deploy.yml"),
            "steps:\n  - run: curl -fsSL https://seite.sh/install.sh | sh\n",
        )
        .unwrap();
        let actions = check_deploy_version_unpinning(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_upgrade_steps_ordered_by_version() {
        let steps = upgrade_steps();
        for i in 1..steps.len() {
            assert!(
                steps[i].introduced_in >= steps[i - 1].introduced_in,
                "upgrade steps should be ordered by version: {:?} < {:?}",
                steps[i].introduced_in,
                steps[i - 1].introduced_in,
            );
        }
    }

    #[test]
    fn test_upgrade_steps_all_have_labels() {
        for step in upgrade_steps() {
            assert!(!step.label.is_empty(), "upgrade step should have a label");
        }
    }

    #[test]
    fn test_check_minify_default_injects_when_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            "[site]\ntitle = \"Test\"\n\n[build]\noutput_dir = \"dist\"\n",
        )
        .unwrap();
        let actions = check_minify_default(tmp.path());
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn test_check_minify_default_skips_when_already_set() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("seite.toml"), "[build]\nminify = true\n").unwrap();
        assert!(check_minify_default(tmp.path()).is_empty());

        // Also skip if explicitly false
        fs::write(tmp.path().join("seite.toml"), "[build]\nminify = false\n").unwrap();
        assert!(check_minify_default(tmp.path()).is_empty());
    }

    #[test]
    fn test_check_minify_default_skips_without_build_section() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("seite.toml"), "[site]\ntitle = \"Test\"\n").unwrap();
        assert!(check_minify_default(tmp.path()).is_empty());
    }

    #[test]
    fn test_check_atom_aliases_docs_no_claude_md() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_atom_aliases_docs(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_atom_aliases_docs_already_documented() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("CLAUDE.md"),
            "# Project\n\nSite generates atom.xml feeds.",
        )
        .unwrap();
        let actions = check_atom_aliases_docs(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_atom_aliases_docs_needs_section() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# My Project\nSome content").unwrap();
        let actions = check_atom_aliases_docs(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Append { content, .. } => {
                assert!(content.contains("Atom"));
                assert!(content.contains("aliases"));
            }
            _ => panic!("expected Append action"),
        }
    }

    #[test]
    fn test_check_atom_autodiscovery_no_custom_template() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_atom_autodiscovery_template(tmp.path());
        assert!(actions.is_empty()); // no templates/ dir → bundled theme
    }

    #[test]
    fn test_check_atom_autodiscovery_already_present() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl_dir = tmp.path().join("templates");
        fs::create_dir_all(&tpl_dir).unwrap();
        fs::write(
            tpl_dir.join("base.html"),
            r#"<link rel="alternate" type="application/atom+xml" title="Blog" href="/atom.xml">"#,
        )
        .unwrap();
        let actions = check_atom_autodiscovery_template(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_atom_autodiscovery_needs_hint() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl_dir = tmp.path().join("templates");
        fs::create_dir_all(&tpl_dir).unwrap();
        fs::write(
            tpl_dir.join("base.html"),
            r#"<link rel="alternate" type="application/rss+xml" title="Blog" href="/feed.xml">"#,
        )
        .unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# Project").unwrap();
        let actions = check_atom_autodiscovery_template(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Append { content, .. } => {
                assert!(content.contains("application/atom+xml"));
                assert!(content.contains("next to your existing RSS"));
            }
            _ => panic!("expected Append action"),
        }
    }

    #[test]
    fn test_check_atom_autodiscovery_no_rss_either() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl_dir = tmp.path().join("templates");
        fs::create_dir_all(&tpl_dir).unwrap();
        fs::write(tpl_dir.join("base.html"), "<html><head></head></html>").unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# Project").unwrap();
        let actions = check_atom_autodiscovery_template(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Append { content, .. } => {
                assert!(content.contains("Add Atom feed autodiscovery"));
            }
            _ => panic!("expected Append action"),
        }
    }

    fn seite_toml_with_subdomain_collection() -> &'static str {
        "[site]\ntitle = \"Test\"\ndescription = \"\"\nbase_url = \"https://example.com\"\nlanguage = \"en\"\nauthor = \"\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\nhas_rss = true\nlisted = true\nnested = false\nurl_prefix = \"/posts\"\ndefault_template = \"post.html\"\n\n[[collections]]\nname = \"docs\"\nlabel = \"Docs\"\ndirectory = \"docs\"\nhas_date = false\nhas_rss = true\nlisted = true\nnested = true\nurl_prefix = \"/docs\"\ndefault_template = \"doc.html\"\nsubdomain = \"docs\"\nsubdomain_base_url = \"https://docs.example.com\"\ndeploy_project = \"my-docs\"\n\n[deploy]\ntarget = \"cloudflare\"\nproject = \"my-site\"\n"
    }

    #[test]
    fn test_check_subdomain_workflow_no_workflow() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            seite_toml_with_subdomain_collection(),
        )
        .unwrap();
        let actions = check_subdomain_workflow(tmp.path());
        assert!(actions.is_empty()); // no workflow file to upgrade
    }

    #[test]
    fn test_check_subdomain_workflow_already_has_steps() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            seite_toml_with_subdomain_collection(),
        )
        .unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        // Workflow already references the deploy project
        fs::write(
            wf_dir.join("deploy.yml"),
            "steps:\n  - command: pages deploy dist --project-name my-site\n  - command: pages deploy dist-subdomains/docs --project-name my-docs\n",
        )
        .unwrap();
        let actions = check_subdomain_workflow(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_subdomain_workflow_missing_steps() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            seite_toml_with_subdomain_collection(),
        )
        .unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        // Workflow only has main site deploy, no subdomain step
        fs::write(
            wf_dir.join("deploy.yml"),
            "steps:\n  - command: pages deploy dist --project-name my-site\n",
        )
        .unwrap();
        let actions = check_subdomain_workflow(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create {
                description,
                content,
                ..
            } => {
                assert!(description.contains("docs"));
                assert!(content.contains("my-docs"));
                assert!(content.contains("dist-subdomains/docs"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_subdomain_workflow_github_pages_skipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        // GitHub Pages config with subdomain collection — should be skipped
        let toml = "[site]\ntitle = \"Test\"\ndescription = \"\"\nbase_url = \"https://example.com\"\nlanguage = \"en\"\nauthor = \"\"\n\n[[collections]]\nname = \"docs\"\nlabel = \"Docs\"\ndirectory = \"docs\"\nhas_date = false\nhas_rss = true\nlisted = true\nnested = true\nurl_prefix = \"/docs\"\ndefault_template = \"doc.html\"\nsubdomain = \"docs\"\ndeploy_project = \"my-docs\"\n\n[deploy]\ntarget = \"github-pages\"\n";
        fs::write(tmp.path().join("seite.toml"), toml).unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(wf_dir.join("deploy.yml"), "steps:\n").unwrap();
        let actions = check_subdomain_workflow(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_subdomain_workflow_no_deploy_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Subdomain collection without deploy_project — should not trigger upgrade
        let toml = "[site]\ntitle = \"Test\"\ndescription = \"\"\nbase_url = \"https://example.com\"\nlanguage = \"en\"\nauthor = \"\"\n\n[[collections]]\nname = \"docs\"\nlabel = \"Docs\"\ndirectory = \"docs\"\nhas_date = false\nhas_rss = true\nlisted = true\nnested = true\nurl_prefix = \"/docs\"\ndefault_template = \"doc.html\"\nsubdomain = \"docs\"\n\n[deploy]\ntarget = \"cloudflare\"\nproject = \"my-site\"\n";
        fs::write(tmp.path().join("seite.toml"), toml).unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(wf_dir.join("deploy.yml"), "steps:\n").unwrap();
        let actions = check_subdomain_workflow(tmp.path());
        assert!(actions.is_empty());
    }
}
