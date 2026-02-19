//! `page upgrade` — bring project configuration up to date with the current binary.
//!
//! When a user upgrades the `page` binary, their existing project may lack new
//! config entries (e.g., MCP server settings, new permission rules). This command
//! detects what's outdated and applies additive, non-destructive upgrades.
//!
//! Each upgrade step is gated to the version that introduced it, so running
//! `page upgrade` on an already-current project is a fast no-op.

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
            label: "Project metadata (.page/config.json)",
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
    ]
}

pub fn run(args: &UpgradeArgs) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;

    // Verify this is a page project
    if !root.join("page.toml").exists() {
        anyhow::bail!(
            "No page.toml found in current directory. Run this command from a page project root."
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
        human::info("Run `page upgrade` to apply these changes.");
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

/// Ensure `.page/config.json` exists.
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
        description: ".page/config.json (project metadata)".into(),
    }]
}

/// Ensure `.claude/settings.json` has the `mcpServers.page` block.
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
                    "Bash(page build:*)",
                    "Bash(page build)",
                    "Bash(page new:*)",
                    "Bash(page serve:*)",
                    "Bash(page theme:*)",
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

    // File exists — check if mcpServers.page is already there
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![], // malformed JSON, don't touch it
    };

    // Check if mcpServers.page already exists
    if settings
        .pointer("/mcpServers/page")
        .is_some()
    {
        return vec![];
    }

    // Merge: add mcpServers block
    let mcp_block = crate::cli::init::mcp_server_block();

    let mut additions = Vec::new();

    if let Some(existing_mcp) = settings.get_mut("mcpServers") {
        // mcpServers exists but no "page" key — add it
        if let Some(obj) = existing_mcp.as_object_mut() {
            obj.insert(
                "page".to_string(),
                mcp_block.get("page").cloned().unwrap_or_default(),
            );
            additions.push("Added mcpServers.page to .claude/settings.json".into());
        }
    } else {
        // No mcpServers at all — add the whole block
        if let Some(obj) = settings.as_object_mut() {
            obj.insert("mcpServers".to_string(), mcp_block);
            additions.push("Added mcpServers.page to .claude/settings.json".into());
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

**Available tools:** `page_build`, `page_create_content`, `page_search`,
`page_apply_theme`, `page_lookup_docs`

**Available resources:** `page://docs/*` (page documentation),
`page://content/*` (site content), `page://themes/*` (themes),
`page://config` (site configuration)

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
