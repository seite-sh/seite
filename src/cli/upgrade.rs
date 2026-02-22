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
}

impl UpgradeAction {
    fn describe(&self) -> Vec<String> {
        match self {
            UpgradeAction::Create { description, .. } => {
                vec![description.clone()]
            }
            UpgradeAction::MergeJson { additions, .. } => additions.clone(),
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
        human::success(&format!(
            "Project is up to date (page {}).",
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
        }
    }

    // Stamp the new version
    let existing_meta = meta::load(&root);
    let new_meta = meta::PageMeta::stamp_current_version(existing_meta.as_ref());
    meta::write(&root, &new_meta)?;

    println!();
    human::success(&format!(
        "Project upgraded to page {}",
        meta::format_version(binary_ver)
    ));

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
