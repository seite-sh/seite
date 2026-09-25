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

use crate::cli::harness::{self, Agent, SiteFeatures};
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

    /// Change which coding agents the project is set up for (comma-separated:
    /// claude,codex,opencode,cursor, or all). Adds files for newly selected
    /// agents; files of deselected agents are left in place but no longer maintained.
    #[arg(long, value_name = "LIST")]
    pub agents: Option<String>,
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
    /// Replace a file's content with an edited version computed at check time
    /// (e.g. a comment-preserving TOML edit).
    Update {
        path: PathBuf,
        content: String,
        description: String,
    },
    /// Refresh (or append) a seite-owned, marker-delimited block in the
    /// project instructions. Recomputed when applied, after earlier actions
    /// (appends, the AGENTS.md migration) have run.
    SyncInstructionsBlock {
        root: PathBuf,
        block: InstructionsBlock,
        description: String,
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
    StampProjectVersion {
        description: String,
    },
    MigrateAgentInstructions {
        root: PathBuf,
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
            UpgradeAction::Update { description, .. }
            | UpgradeAction::SyncInstructionsBlock { description, .. } => {
                vec![description.clone()]
            }
            UpgradeAction::InjectToml { description, .. } => {
                vec![description.clone()]
            }
            UpgradeAction::Append { description, .. } => {
                vec![description.clone()]
            }
            UpgradeAction::StampProjectVersion { description } => {
                vec![description.clone()]
            }
            UpgradeAction::MigrateAgentInstructions { description, .. } => {
                vec![description.clone()]
            }
        }
    }

    fn targets_path(&self, target: &Path) -> bool {
        match self {
            UpgradeAction::Create { path, .. }
            | UpgradeAction::MergeJson { path, .. }
            | UpgradeAction::Update { path, .. }
            | UpgradeAction::Append { path, .. }
            | UpgradeAction::InjectToml { path, .. } => path == target,
            UpgradeAction::StampProjectVersion { .. }
            | UpgradeAction::MigrateAgentInstructions { .. }
            | UpgradeAction::SyncInstructionsBlock { .. } => false,
        }
    }

    /// The file this action replaces wholesale, if any. Two such actions on
    /// the same file would clobber each other, so only the first is kept.
    fn replaced_path(&self) -> Option<&Path> {
        match self {
            UpgradeAction::Create { path, .. }
            | UpgradeAction::MergeJson { path, .. }
            | UpgradeAction::Update { path, .. } => Some(path),
            _ => None,
        }
    }
}

/// Add `new` actions to `actions`, skipping any that would replace a file an
/// earlier action already replaces.
fn extend_dedup(actions: &mut Vec<UpgradeAction>, new: Vec<UpgradeAction>) {
    for action in new {
        let duplicate = action.replaced_path().is_some_and(|path| {
            actions
                .iter()
                .any(|existing| existing.replaced_path() == Some(path))
        });
        if !duplicate {
            actions.push(action);
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
            label: "Project instructions MCP documentation",
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
            introduced_in: (0, 12, 0),
            label: "Remove pinned VERSION from deploy workflows (use latest)",
            check: check_deploy_version_unpinning,
        },
        UpgradeStep {
            introduced_in: (0, 12, 0),
            label: "Subdomain collection deploy steps in CI workflow",
            check: check_subdomain_workflow,
        },
        UpgradeStep {
            introduced_in: (0, 16, 0),
            label: "Private collections + password access guidance (.claude/rules)",
            check: check_private_collections_rule,
        },
        UpgradeStep {
            // Replaces the original (0.1.0) step that wrote `mcpServers` into
            // .claude/settings.json, which Claude Code never reads. The check
            // is idempotent, so it is safe for every older project.
            introduced_in: (0, 20, 0),
            label: "MCP server for AI tools (.mcp.json + settings migration)",
            check: check_mcp_server,
        },
    ]
}

/// Return the canonical project instructions file, falling back to the legacy
/// Claude-specific file for projects that have not reached the migration step.
fn project_instructions_path(root: &Path) -> PathBuf {
    crate::cli::agent_instructions::canonical_path(root)
}

/// Install the `.claude/rules/private-collections.md` context file so the project's
/// agent knows about `private` collections, password groups, and subdomain hubs.
/// Created only if missing.
fn check_private_collections_rule(root: &Path) -> Vec<UpgradeAction> {
    harness::rule("private-collections")
        .and_then(|rule| missing_rule(root, ".claude/rules", "md", rule, harness::claude_rule))
        .into_iter()
        .collect()
}

pub fn run(args: &UpgradeArgs) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;

    // Verify this is a page project
    if !root.join("seite.toml").exists() {
        anyhow::bail!(
            "No seite.toml found in current directory. Run this command from a seite project root."
        );
    }

    // Historical upgrade steps can append to the project instructions before
    // the v0.19 migration runs. Reject symlinks and other non-file entries up
    // front so no action can write through a path outside the project.
    crate::cli::agent_instructions::validate_paths(&root)?;

    let project_ver = meta::project_version(&root);
    let binary_ver = meta::binary_version();

    // Which coding agents to maintain: --agents, else the stored selection,
    // else (projects from before the selection existed) every agent.
    let stored_agents = meta::load(&root)
        .and_then(|m| m.agents)
        .map(|ids| harness::from_ids(&ids));
    let agents = match &args.agents {
        Some(list) => harness::parse_agent_list(list)?,
        None => stored_agents.clone().unwrap_or_else(|| Agent::ALL.to_vec()),
    };
    let record_agents = stored_agents.as_ref() != Some(&agents);
    let previously = stored_agents.unwrap_or_else(|| Agent::ALL.to_vec());
    let deselected: Vec<Agent> = previously
        .iter()
        .copied()
        .filter(|a| !agents.contains(a))
        .collect();

    // Collect all applicable actions
    let mut actions: Vec<UpgradeAction> = Vec::new();
    for step in upgrade_steps() {
        if step.introduced_in > project_ver {
            let step_actions = (step.check)(&root);
            extend_dedup(&mut actions, step_actions);
        }
    }
    // Historical steps predate the agent selection; don't let them recreate
    // Claude Code files for a project that no longer uses Claude Code.
    if !agents.contains(&Agent::Claude) {
        actions.retain(|a| !a.replaced_path().is_some_and(|p| is_claude_owned(&root, p)));
    }

    // Decide whether a migration action is needed after seeing every older
    // action. Some historical steps can create CLAUDE.md in projects where
    // neither instruction file existed when checks began.
    if project_ver < (0, 19, 0) {
        let claude_path = root.join("CLAUDE.md");
        let older_action_creates_instructions = actions
            .iter()
            .any(|action| action.targets_path(&claude_path));
        if crate::cli::agent_instructions::needs_migration(&root)
            || older_action_creates_instructions
        {
            actions.push(UpgradeAction::MigrateAgentInstructions {
                root: root.clone(),
                description: "Canonical AGENTS.md project instructions".into(),
            });
        }
    }

    // Per-agent harness files. Not version-gated: the checks are idempotent
    // and depend on the agent selection, which can change at any version.
    extend_dedup(&mut actions, check_agent_harness(&root, &agents));
    // Turn-end `seite check` hooks, once per agent: merged into (or next to)
    // the actions above, and skipped for agents recorded as done so a hook
    // the user deleted stays deleted.
    let stored_hooks = meta::load(&root).and_then(|m| m.hooks_installed);
    let hooks_installed = add_stop_hooks(
        &root,
        &agents,
        stored_hooks.as_deref().unwrap_or_default(),
        &mut actions,
    );
    let record_hooks = stored_hooks.as_ref() != Some(&hooks_installed);

    let agent_ids = harness::to_ids(&agents);
    let deselected_ids = harness::to_ids(&deselected);
    let write_meta = || -> anyhow::Result<()> {
        let existing = meta::load(&root);
        let mut new_meta = meta::PageMeta::stamp_current_version(existing.as_ref());
        new_meta.agents = Some(agent_ids.clone());
        new_meta.hooks_installed = Some(hooks_installed.clone());
        meta::write(&root, &new_meta)?;
        Ok(())
    };

    if actions.is_empty() {
        // No file changes needed, but still stamp the version if it's behind.
        // This handles the case where upgrade steps exist but their checks found
        // nothing to do (files already present), or where the binary was bumped
        // without adding new upgrade steps (e.g. 0.4.0 → 0.4.3 with only bug fixes).
        // Likewise record a new or first-time agent (or hook) selection.
        if !args.check && (project_ver < binary_ver || record_agents || record_hooks) {
            write_meta()?;
        }
        report_deselected(&deselected);
        human::success(&format!(
            "Project is up to date (seite {}).",
            meta::format_version(binary_ver)
        ));
        crate::output::json::set_data(serde_json::json!({
            "up_to_date": true,
            "applied": false,
            "version": meta::format_version(binary_ver),
            "changes": [],
            "agents": agent_ids,
            "unmaintained_agents": deselected_ids,
        }));
        return Ok(());
    }

    // Show what will change
    let from_label = if project_ver == (0, 0, 0) {
        "pre-tracking".to_string()
    } else {
        meta::format_version(project_ver)
    };
    human::header(&format!(
        "Upgrading from {} → {}",
        from_label,
        meta::format_version(binary_ver)
    ));
    crate::human_println!();

    let changes: Vec<String> = actions.iter().flat_map(|a| a.describe()).collect();
    for line in &changes {
        human::info(&format!("  {line}"));
    }
    crate::human_println!();
    let upgrade_data = |applied: bool| {
        serde_json::json!({
            "up_to_date": applied,
            "applied": applied,
            "from": from_label,
            "to": meta::format_version(binary_ver),
            "changes": changes,
            "agents": agent_ids,
            "unmaintained_agents": deselected_ids,
        })
    };

    // --check mode: just report and fail (exit 1 = upgrade needed; useful for CI)
    if args.check {
        human::info("Run `seite upgrade` to apply these changes.");
        anyhow::bail!(
            "project needs upgrading: {} pending change{}",
            changes.len(),
            if changes.len() == 1 { "" } else { "s" }
        );
    }

    // Confirm unless --force
    if !args.force {
        let proceed = crate::cli::prompt::confirm("Apply these upgrades?", true)?;
        if !proceed {
            human::info("Upgrade cancelled.");
            crate::output::json::set_data(upgrade_data(false));
            return Ok(());
        }
    }

    // Apply all actions
    for action in actions {
        apply_action(action)?;
    }
    report_deselected(&deselected);

    // Stamp the new version (and the agent selection)
    write_meta()?;
    crate::output::json::set_data(upgrade_data(true));

    crate::human_println!();
    human::success(&format!(
        "Project upgraded to seite {}",
        meta::format_version(binary_ver)
    ));

    // Hint about trimming the canonical project instructions if rules files were created.
    if root.join(".claude/rules").exists() {
        let instructions_path = if root.join("AGENTS.md").exists() {
            root.join("AGENTS.md")
        } else {
            root.join("CLAUDE.md")
        };
        if let Ok(content) = fs::read_to_string(instructions_path) {
            if content.lines().count() > 300 {
                crate::human_println!();
                human::info(
                    "Detailed context now lives in .claude/rules/ and loads automatically.",
                );
                human::info("You can trim your AGENTS.md — the rules files have the details.");
            }
        }
    }

    Ok(())
}

/// Apply one upgrade action.
fn apply_action(action: UpgradeAction) -> anyhow::Result<()> {
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
        UpgradeAction::Update {
            path,
            content,
            description,
        } => {
            fs::write(&path, &content)?;
            human::success(&format!("Updated {description}"));
        }
        UpgradeAction::SyncInstructionsBlock {
            root,
            block,
            description,
        } => {
            let path = project_instructions_path(&root);
            if let Ok(existing) = fs::read_to_string(&path) {
                if let Some(updated) = block.apply(&existing) {
                    crate::cli::agent_instructions::write_atomic(&path, &updated)?;
                    human::success(&format!("Updated {description}"));
                }
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
        UpgradeAction::StampProjectVersion { .. } => {
            // The version marker is the upgrade's commit point and is
            // written only after every file migration succeeds.
        }
        UpgradeAction::MigrateAgentInstructions { root, description } => {
            if crate::cli::agent_instructions::migrate(&root)? {
                human::success(&format!("Updated {description}"));
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

    vec![UpgradeAction::StampProjectVersion {
        description: ".seite/config.json (project metadata)".into(),
    }]
}

/// Read a JSON file whose top level is an object. `None` if unreadable,
/// malformed, or not an object — callers leave such files untouched.
fn read_json_object(path: &Path) -> Option<serde_json::Map<String, serde_json::Value>> {
    let content = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&content).ok()? {
        serde_json::Value::Object(map) => Some(map),
        _ => None,
    }
}

fn pretty_json(value: &serde_json::Value) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(value).unwrap_or_default()
    )
}

/// Configure the seite MCP server the way Claude Code actually loads it.
///
/// Claude Code only reads project MCP servers from `.mcp.json`; the
/// `mcpServers` block older versions wrote into `.claude/settings.json` was
/// never loaded. This step:
/// - creates `.mcp.json`, or merges the `seite` server into an existing one
///   (other servers are preserved);
/// - moves any legacy `mcpServers` from settings.json into `.mcp.json`
///   (only when `.mcp.json` can hold them, so nothing is lost);
/// - pre-approves the server via `enabledMcpjsonServers` and adds the newer
///   permission rules (`mcp__seite`, `Edit(static/**)`, `Edit(seite.toml)`).
///
/// Malformed JSON files are left untouched. Running it again is a no-op.
fn check_mcp_server(root: &Path) -> Vec<UpgradeAction> {
    use crate::cli::harness::{claude_settings, mcp_server_block, CLAUDE_ALLOWED_TOOLS_UPGRADE};

    let settings_path = root.join(".claude/settings.json");
    let mcp_path = root.join(".mcp.json");
    let seite_server = mcp_server_block().get("seite").cloned().unwrap_or_default();

    let settings = if settings_path.exists() {
        read_json_object(&settings_path)
    } else {
        None
    };
    let legacy_servers: serde_json::Map<String, serde_json::Value> = settings
        .as_ref()
        .and_then(|s| s.get("mcpServers"))
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let mut actions = Vec::new();

    // --- .mcp.json (first, so legacy servers land there before settings.json
    // drops them) ---
    let mcp_holds_servers = if !mcp_path.exists() {
        let mut servers = legacy_servers.clone();
        servers
            .entry("seite".to_string())
            .or_insert_with(|| seite_server.clone());
        actions.push(UpgradeAction::Create {
            path: mcp_path,
            content: pretty_json(&serde_json::json!({ "mcpServers": servers })),
            description: ".mcp.json (seite MCP server for Claude Code)".into(),
        });
        true
    } else if let Some(mut mcp) = read_json_object(&mcp_path) {
        let servers = mcp
            .entry("mcpServers".to_string())
            .or_insert_with(|| serde_json::json!({}));
        match servers.as_object_mut() {
            Some(map) => {
                let mut additions = Vec::new();
                for (name, server) in &legacy_servers {
                    if !map.contains_key(name) {
                        map.insert(name.clone(), server.clone());
                        additions.push(format!(
                            "Moved MCP server '{name}' from .claude/settings.json to .mcp.json"
                        ));
                    }
                }
                if !map.contains_key("seite") {
                    map.insert("seite".to_string(), seite_server.clone());
                    additions.push("Added mcpServers.seite to .mcp.json".to_string());
                }
                if !additions.is_empty() {
                    actions.push(UpgradeAction::MergeJson {
                        path: mcp_path,
                        merged: serde_json::Value::Object(mcp),
                        additions,
                    });
                }
                true
            }
            None => false,
        }
    } else {
        false // malformed .mcp.json — don't touch it
    };

    // --- .claude/settings.json ---
    if !settings_path.exists() {
        actions.push(UpgradeAction::Create {
            path: settings_path,
            content: pretty_json(&claude_settings()),
            description: ".claude/settings.json (permissions + MCP server approval)".into(),
        });
    } else if let Some(mut settings) = settings {
        let mut additions = Vec::new();

        if mcp_holds_servers && settings.remove("mcpServers").is_some() {
            additions.push(
                "Removed mcpServers from .claude/settings.json (Claude Code only loads project MCP servers from .mcp.json)"
                    .to_string(),
            );
        }

        let enabled = settings
            .entry("enabledMcpjsonServers".to_string())
            .or_insert_with(|| serde_json::json!([]));
        if let Some(list) = enabled.as_array_mut() {
            if !list.iter().any(|v| v == "seite") {
                list.push(serde_json::json!("seite"));
                additions.push(
                    "Pre-approved the seite MCP server (enabledMcpjsonServers) in .claude/settings.json"
                        .to_string(),
                );
            }
        }

        let permissions = settings
            .entry("permissions".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if let Some(permissions) = permissions.as_object_mut() {
            let allow = permissions
                .entry("allow".to_string())
                .or_insert_with(|| serde_json::json!([]));
            if let Some(allow) = allow.as_array_mut() {
                for rule in CLAUDE_ALLOWED_TOOLS_UPGRADE {
                    if !allow.iter().any(|v| v == rule) {
                        allow.push(serde_json::json!(rule));
                        additions.push(format!("Allowed {rule} in .claude/settings.json"));
                    }
                }
            }
        }

        if !additions.is_empty() {
            actions.push(UpgradeAction::MergeJson {
                path: settings_path,
                merged: serde_json::Value::Object(settings),
                additions,
            });
        }
    }

    actions
}

/// Create or refresh `<dir>/<name>/SKILL.md` for a bundled skill.
///
/// Skills embed a `# seite-skill-version: N` comment in their YAML frontmatter.
/// If the existing file has a lower version (or none), the upgrade replaces it
/// with the bundled version. This lets us ship improved prompts in new releases
/// without requiring the user to manually diff skill files. A file at the same
/// or a higher version is left alone.
fn check_skill(root: &Path, dir: &str, skill: &harness::Skill) -> Option<UpgradeAction> {
    let content = skill.render(harness::skill_frontmatter_for(dir));
    check_versioned_file(
        root,
        format!("{dir}/{}/SKILL.md", skill.name),
        content,
        &format!("/{} command", skill.name),
    )
}

/// Create `rel`, or replace it when its `# seite-skill-version` is older than
/// the bundled `content`'s. Same-or-newer files are left alone.
fn check_versioned_file(
    root: &Path,
    rel: String,
    content: String,
    what: &str,
) -> Option<UpgradeAction> {
    let bundled_version = extract_skill_version(&content);
    let path = root.join(&rel);

    if path.exists() {
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let existing_version = extract_skill_version(&existing);
        if existing_version >= bundled_version {
            return None;
        }
        return Some(UpgradeAction::Create {
            path,
            content,
            description: format!("{rel} (updated v{existing_version} → v{bundled_version})"),
        });
    }

    Some(UpgradeAction::Create {
        path,
        content,
        description: format!("{rel} ({what})"),
    })
}

/// Slash-command wrappers (OpenCode's `.opencode/commands/seite.md`) for the
/// selected agents, created or refreshed by version like skills.
fn check_command_files(root: &Path, agents: &[Agent]) -> Vec<UpgradeAction> {
    harness::command_files(agents, SiteFeatures::detect(root))
        .into_iter()
        .filter_map(|(rel, content)| {
            check_versioned_file(root, rel, content, "slash command → skill")
        })
        .collect()
}

/// A bundled Claude Code skill by name, as an upgrade action.
fn check_claude_skill(root: &Path, name: &str) -> Vec<UpgradeAction> {
    harness::skill(name)
        .and_then(|skill| check_skill(root, harness::CLAUDE_SKILLS_DIR, skill))
        .into_iter()
        .collect()
}

/// Ensure `.claude/skills/landing-page/SKILL.md` exists and is up-to-date when
/// the project has a pages collection.
///
/// Also handles migration from the old `homepage` skill name to `landing-page`.
fn check_landing_page_skill(root: &Path) -> Vec<UpgradeAction> {
    // Only relevant if the project has a pages collection
    if !SiteFeatures::detect(root).pages {
        return vec![];
    }

    // An old homepage skill at the current version stands in for landing-page.
    let skill_path = root.join(".claude/skills/landing-page/SKILL.md");
    let old_skill_path = root.join(".claude/skills/homepage/SKILL.md");
    if !skill_path.exists() && old_skill_path.exists() {
        let bundled_version =
            extract_skill_version(harness::skill("landing-page").map_or("", |s| s.content));
        let existing = fs::read_to_string(&old_skill_path).unwrap_or_default();
        if extract_skill_version(&existing) >= bundled_version {
            return vec![];
        }
    }

    check_claude_skill(root, "landing-page")
}

/// Ensure `.claude/skills/theme-builder/SKILL.md` exists and is up-to-date.
///
/// Unlike the landing-page skill, the theme builder is unconditional — every
/// site benefits from interactive theme creation.
fn check_theme_builder_skill(root: &Path) -> Vec<UpgradeAction> {
    check_claude_skill(root, "theme-builder")
}

/// Ensure `.claude/skills/brand-identity/SKILL.md` exists and is up-to-date.
///
/// Like the theme builder, this is unconditional — every site benefits from
/// brand identity creation (logo, color palette, favicon).
fn check_brand_identity_skill(root: &Path) -> Vec<UpgradeAction> {
    check_claude_skill(root, "brand-identity")
}

/// Extract the `# seite-skill-version: N` value from a SKILL.md file.
/// Returns 0 if not found.
fn extract_skill_version(content: &str) -> u32 {
    harness::extract_version(content)
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

/// Ensure the project instructions have an MCP server section.
fn check_claude_md_mcp(root: &Path) -> Vec<UpgradeAction> {
    let path = project_instructions_path(root);
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

The server is declared in `.mcp.json` (and pre-approved in
`.claude/settings.json`). Claude Code starts it when it opens this project;
the first time, it may ask you to approve the project's MCP server.
No API keys required.

**Available tools:** `seite_build`, `seite_create_content`, `seite_get_page`,
`seite_update_frontmatter`, `seite_search`, `seite_content_stats`,
`seite_list_templates`, `seite_apply_theme`, `seite_create_collection`,
`seite_lookup_docs`

**Available resources:** `seite://docs/*` (seite documentation),
`seite://content/*` (site content), `seite://themes` (themes),
`seite://config` (site configuration), `seite://mcp-config` (MCP settings)

The MCP server provides typed, structured access to your site — AI tools work
with seite concepts (collections, content items, themes) rather than parsing
raw files.
"#;

    vec![UpgradeAction::Append {
        path,
        content: section.to_string(),
        description: "Project instructions (added MCP Server section)".into(),
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

/// Ensure the project instructions mention contact form support.
fn check_contact_form_docs(root: &Path) -> Vec<UpgradeAction> {
    let path = project_instructions_path(root);
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
        description: "Project instructions (added Contact Forms section)".into(),
    }]
}

/// Ensure the project instructions mention subdomain deploy support.
fn check_subdomain_deploy_docs(root: &Path) -> Vec<UpgradeAction> {
    let path = project_instructions_path(root);
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
        description: "Project instructions (added Subdomain Deploys section)".into(),
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

/// A `Create` action for a rules file that is missing from `dir`.
///
/// Rules files are created only when missing — never overwritten — so user
/// edits survive upgrades.
fn missing_rule(
    root: &Path,
    dir: &str,
    ext: &str,
    rule: &harness::Rule,
    render: fn(&harness::Rule) -> String,
) -> Option<UpgradeAction> {
    let rel = format!("{dir}/{}.{ext}", rule.name);
    let path = root.join(&rel);
    if path.exists() {
        return None;
    }
    Some(UpgradeAction::Create {
        path,
        content: render(rule),
        description: rel,
    })
}

/// Create missing rules files for the site's features in `dir`.
fn missing_rules(
    root: &Path,
    dir: &str,
    ext: &str,
    render: fn(&harness::Rule) -> String,
) -> Vec<UpgradeAction> {
    harness::rules(SiteFeatures::detect(root))
        .filter_map(|rule| missing_rule(root, dir, ext, rule, render))
        .collect()
}

/// Create `.claude/rules/*.md` files with path-scoped context for Claude.
fn check_claude_rules(root: &Path) -> Vec<UpgradeAction> {
    missing_rules(root, ".claude/rules", "md", harness::claude_rule)
}

/// Document Atom feed and redirect aliases in the project instructions.
/// These features were added in 0.8.0.
fn check_atom_aliases_docs(root: &Path) -> Vec<UpgradeAction> {
    let path = project_instructions_path(root);
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
        description: "Project instructions (added Atom Feeds & Redirect Aliases section)".into(),
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
        path: project_instructions_path(root),
        content: format!(
            "\n\n### Custom Template: Atom Autodiscovery\n\n\
             Your custom `templates/base.html` does not include Atom feed autodiscovery.\n\
             {hint}:\n\n\
             ```html\n{snippet}\n```\n"
        ),
        description: "Project instructions (Atom autodiscovery hint for custom template)".into(),
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
    let subdomain_collections: Vec<_> = config
        .subdomain_collections()
        .into_iter()
        .filter(|c| c.deploy_project.is_some())
        .collect();

    if subdomain_collections.is_empty() {
        return vec![];
    }

    let workflow_path = root.join(".github/workflows/deploy.yml");
    if !workflow_path.exists() {
        return vec![];
    }

    let content = fs::read_to_string(&workflow_path).unwrap_or_default();

    // Check if all subdomain collections have deploy steps by looking for
    // `dist-subdomains/{name}` which appears in both Cloudflare and Netlify workflows
    let all_present = subdomain_collections
        .iter()
        .all(|c| content.contains(&format!("dist-subdomains/{}", c.name)));

    if all_present {
        return vec![];
    }

    let missing: Vec<_> = subdomain_collections
        .iter()
        .filter(|c| !content.contains(&format!("dist-subdomains/{}", c.name)))
        .map(|c| c.name.as_str())
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

// ---------------------------------------------------------------------------
// Coding-agent harness files (Claude Code, Codex, OpenCode, Cursor)
// ---------------------------------------------------------------------------

/// Files only Claude Code reads (`.claude/…`, `.mcp.json`).
fn is_claude_owned(root: &Path, path: &Path) -> bool {
    path.starts_with(root.join(".claude")) || path == root.join(".mcp.json")
}

/// Tell the user that files of deselected agents stay but are unmaintained.
fn report_deselected(deselected: &[Agent]) {
    if deselected.is_empty() {
        return;
    }
    let names: Vec<&str> = deselected.iter().map(|a| a.label()).collect();
    human::info(&format!(
        "No longer maintaining files for {} — existing files were left in place; delete them if you don't need them.",
        names.join(", ")
    ));
}

/// A seite-owned block in the project instructions (AGENTS.md), delimited by
/// `<!-- seite:<id> -->` markers. Refreshed in place when the markers exist;
/// otherwise appended under `heading` unless `skip_if_contains` is present
/// (an older, equivalent section the user already has).
#[derive(Debug)]
struct InstructionsBlock {
    id: &'static str,
    body: String,
    heading: &'static str,
    skip_if_contains: Option<&'static str>,
}

impl InstructionsBlock {
    /// The updated instructions, or `None` when nothing changes.
    fn apply(&self, content: &str) -> Option<String> {
        if let Some(updated) = harness::replace_block(content, self.id, &self.body) {
            return (updated != content).then_some(updated);
        }
        if self
            .skip_if_contains
            .is_some_and(|marker| content.contains(marker))
        {
            return None;
        }
        let mut updated = content.trim_end().to_string();
        updated.push_str(&format!(
            "\n\n{}\n\n{}",
            self.heading,
            harness::wrap_block(self.id, &self.body)
        ));
        Some(updated)
    }
}

/// Keep AGENTS.md's per-agent MCP table and rules index in sync with the
/// agent selection.
fn check_instructions_blocks(root: &Path, agents: &[Agent]) -> Vec<UpgradeAction> {
    let path = project_instructions_path(root);
    let Ok(content) = fs::read_to_string(&path) else {
        return vec![];
    };
    let features = SiteFeatures::detect(root);
    let blocks = [
        (
            InstructionsBlock {
                id: harness::MCP_SETUP_BLOCK,
                body: harness::mcp_setup_table(agents),
                heading: "## MCP Setup per Agent",
                skip_if_contains: None,
            },
            "Project instructions (per-agent MCP setup table)",
        ),
        (
            InstructionsBlock {
                id: harness::RULES_INDEX_BLOCK,
                body: harness::rules_index(features, agents),
                heading: "## Context Rules",
                // Sites from 0.19+ already carry an equivalent index.
                skip_if_contains: Some("## Context Rules"),
            },
            "Project instructions (context rules index)",
        ),
    ];
    blocks
        .into_iter()
        .filter(|(block, _)| block.apply(&content).is_some())
        .map(
            |(block, description)| UpgradeAction::SyncInstructionsBlock {
                root: root.to_path_buf(),
                block,
                description: description.to_string(),
            },
        )
        .collect()
}

/// Create `rel` (an `mcpServers` JSON file) or add the seite server to it,
/// keeping every other server. Malformed files are left untouched.
fn check_mcp_servers_json(root: &Path, rel: &str, agent: &str) -> Vec<UpgradeAction> {
    let path = root.join(rel);
    if !path.exists() {
        return vec![UpgradeAction::Create {
            path,
            content: harness::pretty_json(&harness::mcp_json()),
            description: format!("{rel} (seite MCP server for {agent})"),
        }];
    }
    let Some(mut json) = read_json_object(&path) else {
        human::warning(&format!(
            "{rel} is not a JSON object; add the seite MCP server to it manually"
        ));
        return vec![];
    };
    let servers = json
        .entry("mcpServers".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let Some(servers) = servers.as_object_mut() else {
        return vec![];
    };
    if servers.contains_key(harness::MCP_SERVER) {
        return vec![];
    }
    servers.insert(harness::MCP_SERVER.to_string(), harness::mcp_server_entry());
    vec![UpgradeAction::MergeJson {
        path,
        merged: serde_json::Value::Object(json),
        additions: vec![format!("Added mcpServers.seite to {rel}")],
    }]
}

/// Create `.cursor/cli.json`, or add seite's allow rules to an existing one
/// (other rules, and any deny list, are kept).
fn check_cursor_cli_json(root: &Path) -> Vec<UpgradeAction> {
    let rel = ".cursor/cli.json";
    let path = root.join(rel);
    if !path.exists() {
        return vec![UpgradeAction::Create {
            path,
            content: harness::pretty_json(&harness::cursor_cli_json()),
            description: format!("{rel} (Cursor CLI permissions for seite)"),
        }];
    }
    let Some(mut json) = read_json_object(&path) else {
        human::warning(&format!(
            "{rel} is not a JSON object; add seite's Cursor permissions to it manually"
        ));
        return vec![];
    };
    let permissions = json
        .entry("permissions".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let Some(permissions) = permissions.as_object_mut() else {
        return vec![];
    };
    let allow = permissions
        .entry("allow".to_string())
        .or_insert_with(|| serde_json::json!([]));
    let Some(allow) = allow.as_array_mut() else {
        return vec![];
    };
    let mut additions = Vec::new();
    for rule in harness::CURSOR_ALLOW {
        if !allow.iter().any(|v| v == rule) {
            allow.push(serde_json::json!(rule));
            additions.push(format!("Added {rule} to {rel}"));
        }
    }
    if additions.is_empty() {
        return vec![];
    }
    vec![UpgradeAction::MergeJson {
        path,
        merged: serde_json::Value::Object(json),
        additions,
    }]
}

/// Create `opencode.json`, or merge into an existing one: add `mcp.seite` if
/// missing, and the permission block only when there's no `permission` key.
/// Every other key is kept.
fn check_opencode_json(root: &Path) -> Vec<UpgradeAction> {
    let path = root.join("opencode.json");
    if !path.exists() {
        if root.join("opencode.jsonc").exists() {
            human::warning(
                "opencode.jsonc exists; add the seite MCP server to it manually (see seite.sh/docs/mcp)",
            );
            return vec![];
        }
        return vec![UpgradeAction::Create {
            path,
            content: harness::pretty_json(&harness::opencode_json()),
            description: "opencode.json (seite MCP server + permissions for OpenCode)".into(),
        }];
    }
    let Some(mut json) = read_json_object(&path) else {
        human::warning(
            "opencode.json isn't plain JSON (comments?); add the seite MCP server to it manually",
        );
        return vec![];
    };
    let mut additions = Vec::new();
    let mcp = json
        .entry("mcp".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if let Some(mcp) = mcp.as_object_mut() {
        if !mcp.contains_key(harness::MCP_SERVER) {
            mcp.insert(
                harness::MCP_SERVER.to_string(),
                harness::opencode_mcp_entry(),
            );
            additions.push("Added mcp.seite to opencode.json".to_string());
        }
    }
    if !json.contains_key("permission") {
        json.insert("permission".to_string(), harness::opencode_permission());
        additions.push("Added seite permission defaults to opencode.json".to_string());
    }
    if additions.is_empty() {
        return vec![];
    }
    vec![UpgradeAction::MergeJson {
        path,
        merged: serde_json::Value::Object(json),
        additions,
    }]
}

/// Create `.codex/config.toml`, or add `[mcp_servers.seite]` to an existing
/// one with a comment-preserving edit.
fn check_codex_config(root: &Path) -> Vec<UpgradeAction> {
    let rel = ".codex/config.toml";
    let path = root.join(rel);
    let Ok(existing) = fs::read_to_string(&path) else {
        if path.exists() {
            return vec![];
        }
        return vec![UpgradeAction::Create {
            path,
            content: harness::codex_config_toml(),
            description: format!("{rel} (seite MCP server for Codex CLI)"),
        }];
    };
    match harness::merge_codex_config(&existing) {
        Ok(Some(content)) => vec![UpgradeAction::Update {
            path,
            content,
            description: format!("{rel} (added [mcp_servers.seite])"),
        }],
        Ok(None) => vec![],
        Err(_) => {
            human::warning(&format!(
                "{rel} is not valid TOML; add [mcp_servers.seite] to it manually"
            ));
            vec![]
        }
    }
}

/// Create the CLAUDE.md `@AGENTS.md` shim when Claude Code is selected for a
/// project that doesn't have one.
fn check_claude_shim(root: &Path) -> Vec<UpgradeAction> {
    let claude = root.join("CLAUDE.md");
    if claude.exists() || !root.join("AGENTS.md").exists() {
        return vec![];
    }
    vec![UpgradeAction::Create {
        path: claude,
        content: crate::cli::agent_instructions::CLAUDE_SHIM.to_string(),
        description: "CLAUDE.md (@AGENTS.md import for Claude Code)".into(),
    }]
}

/// Every harness file the selected agents need, merged into what exists.
fn check_agent_harness(root: &Path, agents: &[Agent]) -> Vec<UpgradeAction> {
    let features = SiteFeatures::detect(root);
    let mut actions = Vec::new();

    if agents.contains(&Agent::Claude) {
        actions.extend(check_mcp_server(root));
        actions.extend(check_claude_rules(root));
        actions.extend(check_landing_page_skill(root));
        actions.extend(check_theme_builder_skill(root));
        actions.extend(check_brand_identity_skill(root));
        actions.extend(check_claude_skill(root, "seite"));
        actions.extend(check_claude_shim(root));
    }
    if agents.contains(&Agent::Cursor) {
        actions.extend(check_mcp_servers_json(root, ".cursor/mcp.json", "Cursor"));
        actions.extend(check_cursor_cli_json(root));
        actions.extend(missing_rules(
            root,
            ".cursor/rules",
            "mdc",
            harness::cursor_rule,
        ));
    }
    if agents.contains(&Agent::Codex) {
        actions.extend(check_codex_config(root));
    }
    if agents.contains(&Agent::Opencode) {
        actions.extend(check_opencode_json(root));
    }
    actions.extend(check_command_files(root, agents));
    if harness::uses_shared_skills(agents) {
        actions.extend(
            harness::skills(features)
                .filter_map(|skill| check_skill(root, harness::SHARED_SKILLS_DIR, skill)),
        );
    }
    if harness::uses_shared_rules(agents) {
        actions.extend(missing_rules(
            root,
            ".agents/rules",
            "md",
            harness::claude_rule,
        ));
    }
    actions.extend(check_instructions_blocks(root, agents));
    actions
}

/// Add the turn-end `seite check` hook for each selected agent that isn't in
/// `installed` (the ids recorded in `.seite/config.json`). Returns the new
/// record: `installed` plus every agent whose hook is now in place (added
/// here or already present). Agents whose config can't be merged are left
/// out so a later upgrade retries them.
fn add_stop_hooks(
    root: &Path,
    agents: &[Agent],
    installed: &[String],
    actions: &mut Vec<UpgradeAction>,
) -> Vec<String> {
    let mut record = installed.to_vec();
    for &agent in agents {
        if installed.iter().any(|id| id == agent.id()) {
            continue;
        }
        if add_stop_hook(root, agent, actions) {
            record.push(agent.id().to_string());
        }
    }
    record
}

/// Queue `agent`'s stop hook: merged into a pending action on the same file
/// (e.g. the `.claude/settings.json` the MCP step creates), else created or
/// merged on disk. `false` when the file can't be merged safely.
fn add_stop_hook(root: &Path, agent: Agent, actions: &mut Vec<UpgradeAction>) -> bool {
    use crate::cli::harness_hooks::{hook_path, merge_hook, new_hook_file};

    let rel = hook_path(agent);
    let path = root.join(rel);
    let added = format!("Added the seite check stop hook to {rel}");
    let unmergeable = || {
        human::warning(&format!(
            "{rel} can't be merged automatically; add a {} stop hook running `{}` by hand",
            agent.label(),
            crate::cli::harness_hooks::hook_command(agent)
        ));
        false
    };

    if let Some(action) = actions
        .iter_mut()
        .find(|a| a.replaced_path() == Some(path.as_path()))
    {
        return match action {
            UpgradeAction::Create {
                content,
                description,
                ..
            } => {
                let Ok(serde_json::Value::Object(mut doc)) = serde_json::from_str(content) else {
                    return unmergeable();
                };
                match merge_hook(agent, &mut doc) {
                    Ok(changed) => {
                        if changed {
                            *content = pretty_json(&serde_json::Value::Object(doc));
                            description.push_str(" + seite check stop hook");
                        }
                        true
                    }
                    Err(_) => unmergeable(),
                }
            }
            UpgradeAction::MergeJson {
                merged, additions, ..
            } => match merged.as_object_mut().map(|doc| merge_hook(agent, doc)) {
                Some(Ok(changed)) => {
                    if changed {
                        additions.push(added);
                    }
                    true
                }
                _ => unmergeable(),
            },
            _ => unmergeable(),
        };
    }

    if !path.exists() {
        actions.push(UpgradeAction::Create {
            path,
            content: new_hook_file(agent),
            description: format!("{rel} (seite check stop hook for {})", agent.label()),
        });
        return true;
    }
    if agent == Agent::Opencode {
        // A plugin file already sits at our path; leave it alone.
        return true;
    }
    let Some(mut doc) = read_json_object(&path) else {
        return unmergeable();
    };
    match merge_hook(agent, &mut doc) {
        Ok(true) => {
            actions.push(UpgradeAction::MergeJson {
                path,
                merged: serde_json::Value::Object(doc),
                additions: vec![added],
            });
            true
        }
        Ok(false) => true,
        Err(_) => unmergeable(),
    }
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
            UpgradeAction::StampProjectVersion { description } => {
                assert!(description.contains("config.json"));
            }
            _ => panic!("expected deferred version stamp action"),
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
        assert_eq!(actions.len(), 2);
        match (&actions[0], &actions[1]) {
            (
                UpgradeAction::Create {
                    description: mcp_desc,
                    content: mcp_content,
                    ..
                },
                UpgradeAction::Create {
                    description,
                    content,
                    ..
                },
            ) => {
                assert!(mcp_desc.contains(".mcp.json"));
                let mcp: serde_json::Value = serde_json::from_str(mcp_content).unwrap();
                assert_eq!(mcp["mcpServers"]["seite"]["command"], "seite");
                assert!(description.contains("settings.json"));
                let settings: serde_json::Value = serde_json::from_str(content).unwrap();
                assert!(settings.get("mcpServers").is_none());
                assert_eq!(settings["enabledMcpjsonServers"][0], "seite");
            }
            _ => panic!("expected two Create actions"),
        }
    }

    /// Apply Create/MergeJson actions the way `run` does (test helper).
    fn apply_json_actions(actions: Vec<UpgradeAction>) {
        for action in actions {
            match action {
                UpgradeAction::Create { path, content, .. } => {
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, content).unwrap();
                }
                UpgradeAction::MergeJson { path, merged, .. } => {
                    fs::write(path, serde_json::to_string_pretty(&merged).unwrap()).unwrap();
                }
                _ => panic!("unexpected action"),
            }
        }
    }

    fn write_settings(root: &Path, settings: serde_json::Value) {
        let claude_dir = root.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();
    }

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn test_check_mcp_server_migrates_legacy_settings() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            serde_json::json!({
                "permissions": { "allow": ["Read", "Write(static/**)"], "deny": ["Read(.env)"] },
                "mcpServers": {
                    "seite": { "command": "seite", "args": ["mcp"] },
                    "other": { "command": "other" }
                }
            }),
        );
        let actions = check_mcp_server(tmp.path());
        let descriptions: Vec<String> = actions.iter().flat_map(|a| a.describe()).collect();
        assert!(descriptions.iter().any(|d| d.contains(".mcp.json")));
        assert!(descriptions
            .iter()
            .any(|d| d.contains("Removed mcpServers")));
        assert!(descriptions.iter().any(|d| d.contains("mcp__seite")));
        apply_json_actions(actions);

        let mcp = read_json(&tmp.path().join(".mcp.json"));
        assert_eq!(mcp["mcpServers"]["seite"]["args"][0], "mcp");
        assert_eq!(mcp["mcpServers"]["other"]["command"], "other");
        let settings = read_json(&tmp.path().join(".claude/settings.json"));
        assert!(settings.get("mcpServers").is_none());
        assert_eq!(
            settings["enabledMcpjsonServers"],
            serde_json::json!(["seite"])
        );
        let allow: Vec<&str> = settings["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        for rule in [
            "Read",
            "Write(static/**)",
            "mcp__seite",
            "Edit(static/**)",
            "Edit(seite.toml)",
        ] {
            assert!(allow.contains(&rule), "missing {rule}: {allow:?}");
        }
        assert_eq!(settings["permissions"]["deny"][0], "Read(.env)");

        // Idempotent: a second run has nothing to do.
        assert!(check_mcp_server(tmp.path()).is_empty());
    }

    #[test]
    fn test_check_mcp_server_merges_into_existing_mcp_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".mcp.json"),
            r#"{"mcpServers":{"mine":{"command":"mine"}}}"#,
        )
        .unwrap();
        write_settings(
            tmp.path(),
            serde_json::json!({ "permissions": { "allow": ["Read"] } }),
        );
        apply_json_actions(check_mcp_server(tmp.path()));
        let mcp = read_json(&tmp.path().join(".mcp.json"));
        assert_eq!(mcp["mcpServers"]["mine"]["command"], "mine");
        assert_eq!(mcp["mcpServers"]["seite"]["command"], "seite");
        assert!(check_mcp_server(tmp.path()).is_empty());
    }

    #[test]
    fn test_check_mcp_server_keeps_legacy_servers_if_mcp_json_malformed() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join(".mcp.json"), "not json").unwrap();
        write_settings(
            tmp.path(),
            serde_json::json!({ "mcpServers": { "other": { "command": "other" } } }),
        );
        apply_json_actions(check_mcp_server(tmp.path()));
        assert_eq!(
            fs::read_to_string(tmp.path().join(".mcp.json")).unwrap(),
            "not json"
        );
        let settings = read_json(&tmp.path().join(".claude/settings.json"));
        assert_eq!(settings["mcpServers"]["other"]["command"], "other");
    }

    #[test]
    fn test_check_mcp_server_fresh_init_is_noop() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".mcp.json"),
            serde_json::to_string_pretty(&crate::cli::harness::mcp_json()).unwrap(),
        )
        .unwrap();
        write_settings(tmp.path(), crate::cli::harness::claude_settings());
        assert!(check_mcp_server(tmp.path()).is_empty());
    }

    #[test]
    fn test_check_mcp_server_malformed_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".mcp.json"),
            serde_json::to_string_pretty(&crate::cli::harness::mcp_json()).unwrap(),
        )
        .unwrap();
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

    #[test]
    fn test_check_deploy_version_unpinning_netlify_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let toml = "[site]\ntitle = \"Test\"\ndescription = \"\"\nbase_url = \"https://example.com\"\nlanguage = \"en\"\nauthor = \"\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\nhas_rss = true\nlisted = true\nnested = false\nurl_prefix = \"/posts\"\ndefault_template = \"post.html\"\n\n[deploy]\ntarget = \"netlify\"\n";
        fs::write(tmp.path().join("seite.toml"), toml).unwrap();
        fs::write(
            tmp.path().join("netlify.toml"),
            "[build]\n  command = \"VERSION=0.10.0 curl -fsSL https://seite.sh/install.sh | sh && seite build\"\n",
        )
        .unwrap();
        let actions = check_deploy_version_unpinning(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { content, .. } => {
                assert!(!content.contains("VERSION="));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_deploy_version_unpinning_no_seite_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_deploy_version_unpinning(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_subdomain_workflow_no_seite_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_subdomain_workflow(tmp.path());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_subdomain_workflow_netlify_target() {
        let tmp = tempfile::TempDir::new().unwrap();
        let toml = "[site]\ntitle = \"Test\"\ndescription = \"\"\nbase_url = \"https://example.com\"\nlanguage = \"en\"\nauthor = \"\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\nhas_rss = true\nlisted = true\nnested = false\nurl_prefix = \"/posts\"\ndefault_template = \"post.html\"\n\n[[collections]]\nname = \"docs\"\nlabel = \"Docs\"\ndirectory = \"docs\"\nhas_date = false\nhas_rss = true\nlisted = true\nnested = true\nurl_prefix = \"/docs\"\ndefault_template = \"doc.html\"\nsubdomain = \"docs\"\nsubdomain_base_url = \"https://docs.example.com\"\ndeploy_project = \"my-docs\"\n\n[deploy]\ntarget = \"netlify\"\n";
        fs::write(tmp.path().join("seite.toml"), toml).unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(wf_dir.join("deploy.yml"), "steps:\n  - run: seite build\n").unwrap();
        let actions = check_subdomain_workflow(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { content, .. } => {
                assert!(content.contains("NETLIFY_SITE_ID_DOCS"));
                assert!(content.contains("dist-subdomains/docs"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_deploy_version_unpinning_both_workflow_and_netlify() {
        let tmp = tempfile::TempDir::new().unwrap();
        let toml = "[site]\ntitle = \"Test\"\ndescription = \"\"\nbase_url = \"https://example.com\"\nlanguage = \"en\"\nauthor = \"\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\nhas_rss = true\nlisted = true\nnested = false\nurl_prefix = \"/posts\"\ndefault_template = \"post.html\"\n\n[deploy]\ntarget = \"netlify\"\n";
        fs::write(tmp.path().join("seite.toml"), toml).unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(
            wf_dir.join("deploy.yml"),
            "run: VERSION=0.10.0 curl -fsSL https://seite.sh/install.sh | sh\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join("netlify.toml"),
            "command = \"VERSION=0.10.0 curl -fsSL https://seite.sh/install.sh | sh\"\n",
        )
        .unwrap();
        let actions = check_deploy_version_unpinning(tmp.path());
        assert_eq!(actions.len(), 2);
    }

    fn valid_seite_toml_cloudflare() -> &'static str {
        "[site]\ntitle = \"Test\"\ndescription = \"\"\nbase_url = \"http://localhost\"\nlanguage = \"en\"\nauthor = \"\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\nhas_rss = true\nlisted = true\nnested = false\nurl_prefix = \"/posts\"\ndefault_template = \"post.html\"\n\n[deploy]\ntarget = \"cloudflare\"\nproject = \"my-site\"\n"
    }

    #[test]
    fn test_check_deploy_version_unpinning_cloudflare() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("seite.toml"), valid_seite_toml_cloudflare()).unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(
            wf_dir.join("deploy.yml"),
            "run: VERSION=0.10.0 curl -fsSL https://seite.sh/install.sh | sh\n",
        )
        .unwrap();
        let actions = check_deploy_version_unpinning(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { content, .. } => {
                assert!(!content.contains("VERSION="));
                assert!(content.contains("Cloudflare"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_check_deploy_version_unpinning_github_pages() {
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
            "run: VERSION=0.10.0 curl -fsSL https://seite.sh/install.sh | sh\n",
        )
        .unwrap();
        let actions = check_deploy_version_unpinning(tmp.path());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UpgradeAction::Create { content, .. } => {
                assert!(!content.contains("VERSION="));
                assert!(content.contains("GitHub Pages"));
            }
            _ => panic!("expected Create action"),
        }
    }

    #[test]
    fn test_instructions_block_refreshes_appends_or_skips() {
        let block = InstructionsBlock {
            id: "demo",
            body: "new body\n".into(),
            heading: "## Demo",
            skip_if_contains: Some("## Legacy"),
        };
        // Markers present: replaced in place, idempotent.
        let doc = format!("# Site\n\n{}\ntail\n", harness::wrap_block("demo", "old"));
        let updated = block.apply(&doc).unwrap();
        assert!(updated.contains("new body") && !updated.contains("old"));
        assert!(updated.ends_with("\ntail\n"));
        assert!(block.apply(&updated).is_none());
        // No markers: appended under the heading.
        let appended = block.apply("# Site\n").unwrap();
        assert!(appended.starts_with("# Site\n\n## Demo\n\n<!-- seite:demo -->"));
        assert!(block.apply(&appended).is_none());
        // An equivalent legacy section suppresses the append.
        assert!(block.apply("# Site\n\n## Legacy\n").is_none());
    }

    #[test]
    fn test_check_opencode_json_leaves_unparseable_config_alone() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("opencode.json"),
            "{\n  // comment\n  \"model\": \"x\"\n}\n",
        )
        .unwrap();
        assert!(check_opencode_json(tmp.path()).is_empty());

        // opencode.jsonc is never shadowed by a new opencode.json
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("opencode.jsonc"), "{}").unwrap();
        assert!(check_opencode_json(tmp.path()).is_empty());
    }

    #[test]
    fn test_check_opencode_json_merges_and_is_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("opencode.json"), r#"{"theme":"dark"}"#).unwrap();
        apply_json_actions(check_opencode_json(tmp.path()));
        let json = read_json(&tmp.path().join("opencode.json"));
        assert_eq!(json["theme"], "dark");
        assert_eq!(json["mcp"]["seite"]["type"], "local");
        assert_eq!(json["permission"]["bash"]["*"], "ask");
        assert!(check_opencode_json(tmp.path()).is_empty());
    }

    #[test]
    fn test_check_codex_config_invalid_toml_untouched() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".codex")).unwrap();
        fs::write(tmp.path().join(".codex/config.toml"), "[broken").unwrap();
        assert!(check_codex_config(tmp.path()).is_empty());
    }

    #[test]
    fn test_check_cursor_cli_json_merges_allow_rules() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".cursor")).unwrap();
        fs::write(
            tmp.path().join(".cursor/cli.json"),
            r#"{"permissions":{"allow":["Shell(git)"],"deny":["Shell(rm)"]},"editor":{"vimMode":true}}"#,
        )
        .unwrap();
        apply_json_actions(check_cursor_cli_json(tmp.path()));
        let json = read_json(&tmp.path().join(".cursor/cli.json"));
        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow[0], "Shell(git)");
        assert!(allow.iter().any(|v| v == "Mcp(seite:*)"));
        assert_eq!(json["permissions"]["deny"][0], "Shell(rm)");
        assert_eq!(json["editor"]["vimMode"], true);
        assert!(check_cursor_cli_json(tmp.path()).is_empty());
    }

    #[test]
    fn test_check_cursor_cli_json_creates_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_cursor_cli_json(tmp.path());
        assert!(matches!(actions.as_slice(), [UpgradeAction::Create { .. }]));
    }

    #[test]
    fn test_check_mcp_servers_json_keeps_other_servers() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".cursor")).unwrap();
        fs::write(
            tmp.path().join(".cursor/mcp.json"),
            r#"{"mcpServers":{"other":{"url":"https://x"}}}"#,
        )
        .unwrap();
        apply_json_actions(check_mcp_servers_json(
            tmp.path(),
            ".cursor/mcp.json",
            "Cursor",
        ));
        let json = read_json(&tmp.path().join(".cursor/mcp.json"));
        assert_eq!(json["mcpServers"]["other"]["url"], "https://x");
        assert_eq!(json["mcpServers"]["seite"]["command"], "seite");
        assert!(check_mcp_servers_json(tmp.path(), ".cursor/mcp.json", "Cursor").is_empty());
    }

    #[test]
    fn test_extend_dedup_skips_second_write_to_same_file() {
        let path = PathBuf::from("/tmp/x.json");
        let make = || UpgradeAction::Create {
            path: path.clone(),
            content: String::new(),
            description: String::new(),
        };
        let mut actions = vec![make()];
        extend_dedup(&mut actions, vec![make()]);
        assert_eq!(actions.len(), 1);
    }

    fn stop_command(settings: &serde_json::Value) -> &str {
        settings["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap_or_default()
    }

    #[test]
    fn test_add_stop_hooks_merges_into_pending_settings_merge() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            serde_json::json!({ "permissions": { "allow": ["Read"] } }),
        );
        let mut actions = check_mcp_server(tmp.path());
        let before = actions.len();
        let record = add_stop_hooks(tmp.path(), &[Agent::Claude], &[], &mut actions);
        assert_eq!(record, vec!["claude".to_string()]);
        // Merged into the MCP step's pending settings.json action, not a second one.
        assert_eq!(actions.len(), before);
        apply_json_actions(actions);
        let settings = read_json(&tmp.path().join(".claude/settings.json"));
        assert_eq!(stop_command(&settings), "seite check --hook claude");
        let allow = settings["permissions"]["allow"].as_array().unwrap();
        assert!(allow.iter().any(|v| v == "mcp__seite"), "{settings}");
        assert_eq!(
            settings.as_object().unwrap().keys().next().unwrap(),
            "permissions"
        );
    }

    #[test]
    fn test_add_stop_hooks_merges_into_pending_settings_create() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut actions = check_mcp_server(tmp.path());
        add_stop_hooks(tmp.path(), &[Agent::Claude], &[], &mut actions);
        let described: Vec<String> = actions.iter().flat_map(|a| a.describe()).collect();
        assert!(
            described
                .iter()
                .any(|d| d.contains("seite check stop hook")),
            "{described:?}"
        );
        apply_json_actions(actions);
        let settings = read_json(&tmp.path().join(".claude/settings.json"));
        assert_eq!(stop_command(&settings), "seite check --hook claude");
        assert!(settings["enabledMcpjsonServers"].is_array());
    }

    #[test]
    fn test_add_stop_hooks_skips_recorded_and_unmergeable() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        // Recorded as installed (the user may have deleted it): nothing to do.
        let mut actions = Vec::new();
        let record = add_stop_hooks(root, &[Agent::Codex], &["codex".into()], &mut actions);
        assert!(actions.is_empty());
        assert_eq!(record, vec!["codex".to_string()]);

        // A config we can't extend is left alone and not recorded (retried later).
        fs::create_dir_all(root.join(".cursor")).unwrap();
        fs::write(root.join(".cursor/hooks.json"), r#"{"hooks": []}"#).unwrap();
        let record = add_stop_hooks(root, &[Agent::Cursor], &[], &mut actions);
        assert!(actions.is_empty());
        assert!(record.is_empty());

        // An existing hook file gets seite's hook appended, others kept.
        fs::write(
            root.join(".cursor/hooks.json"),
            r#"{"version": 1, "hooks": {"afterFileEdit": [{"command": "fmt"}]}}"#,
        )
        .unwrap();
        let record = add_stop_hooks(root, &[Agent::Cursor], &[], &mut actions);
        assert_eq!(record, vec!["cursor".to_string()]);
        apply_json_actions(actions);
        let hooks = read_json(&root.join(".cursor/hooks.json"));
        assert_eq!(hooks["hooks"]["afterFileEdit"][0]["command"], "fmt");
        assert_eq!(
            hooks["hooks"]["stop"][0]["command"],
            "seite check --hook cursor"
        );

        // Already present: recorded without a change.
        let mut actions = Vec::new();
        let record = add_stop_hooks(root, &[Agent::Cursor], &[], &mut actions);
        assert!(actions.is_empty());
        assert_eq!(record, vec!["cursor".to_string()]);
    }
}
