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
use crate::docs::ReleaseNote;
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
    /// claude,codex,opencode,cursor,gemini, or all). Adds files for newly selected
    /// agents; files of deselected agents are left in place but no longer maintained.
    #[arg(long, value_name = "LIST")]
    pub agents: Option<String>,

    /// Also add the seite MCP server to the selected agents' configs. Without
    /// it, existing MCP config is left exactly as it is.
    #[arg(long)]
    pub mcp: bool,

    /// After upgrading, hand the migration to a coding agent: collect the
    /// release notes since the project's recorded version into a brief.
    /// Inside an agent session the brief is printed (and put in `data.brief`
    /// with --json); from a terminal the agent is started with it. Cannot be
    /// combined with --check.
    #[arg(long)]
    pub agent: bool,

    /// With --agent: the coding-agent harness to start (claude, codex,
    /// opencode, cursor, gemini). Defaults to `SEITE_AGENT`, else claude.
    #[arg(long, value_name = "HARNESS", requires = "agent")]
    pub with: Option<String>,
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
    /// Delete a seite-owned file or directory that is still exactly as seite
    /// wrote it (e.g. a skill folded into `/seite`).
    Remove {
        path: PathBuf,
        description: String,
    },
    /// Remove 0.20's generated AGENTS.md sections for retired features
    /// (recomputed when applied, after earlier AGENTS.md edits).
    PruneLegacySections {
        root: PathBuf,
        description: String,
    },
    /// Write several files at once (the version-matched `.seite/docs/`).
    WriteFiles {
        files: Vec<(PathBuf, String)>,
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
            UpgradeAction::MigrateAgentInstructions { description, .. }
            | UpgradeAction::PruneLegacySections { description, .. }
            | UpgradeAction::Remove { description, .. }
            | UpgradeAction::WriteFiles { description, .. } => {
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
            UpgradeAction::Remove { path, .. } => path == target,
            UpgradeAction::StampProjectVersion { .. }
            | UpgradeAction::MigrateAgentInstructions { .. }
            | UpgradeAction::SyncInstructionsBlock { .. }
            | UpgradeAction::PruneLegacySections { .. }
            | UpgradeAction::WriteFiles { .. } => false,
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
    ]
}

/// What the deploy workflow needs for scheduled publishing.
enum WorkflowSchedule {
    /// Nothing to do: no workflow, or it already has a `schedule:` trigger.
    Done,
    /// Still exactly as seite generated it (without the cron): replace it.
    Generated(String),
    /// Edited by hand: tell the user how to add the cron.
    Modified,
}

fn workflow_schedule(root: &Path) -> WorkflowSchedule {
    let path = root.join(".github/workflows/deploy.yml");
    let Ok(existing) = fs::read_to_string(&path) else {
        return WorkflowSchedule::Done;
    };
    if existing.contains("schedule:") {
        return WorkflowSchedule::Done;
    }
    let Ok(config) = crate::config::SiteConfig::load(&root.join("seite.toml")) else {
        return WorkflowSchedule::Done;
    };
    // The daily rebuild only matters when scheduled publishing is on.
    if !config.build.schedule {
        return WorkflowSchedule::Done;
    }
    let current = match config.deploy.target {
        crate::config::DeployTarget::GithubPages => {
            crate::deploy::generate_github_actions_workflow(&config)
        }
        crate::config::DeployTarget::Cloudflare => {
            crate::deploy::generate_cloudflare_workflow(&config)
        }
        crate::config::DeployTarget::Netlify => crate::deploy::generate_netlify_workflow(&config),
    };
    let previous = current.replace(crate::deploy::SCHEDULE_TRIGGER, "");
    let normalize = |s: &str| s.replace("\r\n", "\n").trim_end().to_string();
    if normalize(&existing) == normalize(&previous) {
        WorkflowSchedule::Generated(current)
    } else {
        WorkflowSchedule::Modified
    }
}

/// Add the daily cron to a deploy workflow still exactly as seite generated
/// it, when `[build] schedule = true` (an edited workflow gets
/// [`workflow_schedule_notice`] instead). Not version-gated: idempotent, and
/// it applies whenever a site turns scheduling on.
fn check_workflow_schedule(root: &Path) -> Vec<UpgradeAction> {
    match workflow_schedule(root) {
        WorkflowSchedule::Generated(content) => vec![UpgradeAction::Update {
            path: root.join(".github/workflows/deploy.yml"),
            content,
            description: ".github/workflows/deploy.yml (daily cron rebuild for scheduled posts)"
                .into(),
        }],
        WorkflowSchedule::Done | WorkflowSchedule::Modified => vec![],
    }
}

/// One-time notice about scheduled publishing for sites with a dated
/// collection that have not set `[build] schedule` (upgrade never turns it
/// on: it would hide existing future-dated posts).
fn schedule_feature_notice(root: &Path) -> Option<Notice> {
    let text = fs::read_to_string(root.join("seite.toml")).ok()?;
    let config: crate::config::SiteConfig = toml::from_str(&text).ok()?;
    let has_dated = config.collections.iter().any(|c| c.has_date);
    let mentioned = text.lines().any(|l| {
        l.trim_start().starts_with("schedule ") || l.trim_start().starts_with("schedule=")
    });
    (has_dated && !mentioned).then(|| Notice {
        key: "schedule-feature".into(),
        text: format!("Scheduled publishing is available: add `schedule = true` under [build] in seite.toml to keep posts dated in the future out of the build until their date (and `expires:` posts after it). It is off for existing sites; `seite upgrade` then also adds a daily rebuild to an unedited deploy workflow. {TIMEZONE_ADVICE} See https://seite.sh/docs/configuration#scheduled-publishing"),
    })
}

/// One-time notice about full-text search for sites that have not set
/// `[build] search` (absent = "basic"; upgrade never switches it, so the
/// output of existing sites stays unchanged until they opt in).
fn search_feature_notice(root: &Path) -> Option<Notice> {
    let text = fs::read_to_string(root.join("seite.toml")).ok()?;
    let mentioned = text.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("search ") || l.starts_with("search=")
    });
    (!mentioned).then(|| Notice {
        key: "search-feature".into(),
        text: "Full-text search is available: add `search = \"full\"` under [build] in seite.toml to index page text (not just titles, descriptions and tags) into a sharded index under dist/search/ that the bundled themes load lazily; any static host serves it. Without the key, sites keep the basic search-index.json. See https://seite.sh/docs/configuration#search".into(),
    })
}

/// One-time notice about generated social cards for sites without an `[og]`
/// section (upgrade never adds it: it changes every page's `og:image`).
fn og_feature_notice(root: &Path) -> Option<Notice> {
    let text = fs::read_to_string(root.join("seite.toml")).ok()?;
    let config: crate::config::SiteConfig = toml::from_str(&text).ok()?;
    config.og.is_none().then(|| Notice {
        key: "og-feature".into(),
        text: "Social preview cards are available: add `[og]` with `generate = true` to seite.toml and every page without an `image:` gets a generated 1200x630 card (`dist/og/*.png`, set as og:image and twitter:image), in your brand colors from data/brand.yaml. Try one first with `seite og preview /`. See https://seite.sh/docs/configuration#social-cards".into(),
    })
}

/// Shared by the schedule notices and the `TIMEZONE_UNSET` directive.
const TIMEZONE_ADVICE: &str = "Also set `timezone = \"<IANA name>\"` (e.g. \"Europe/Berlin\") under [site], so \"today\" is the same on every machine and in CI.";

/// One-time notice for sites that already use scheduled publishing
/// (`[build] schedule = true`) without a `[site] timezone` (upgrade never
/// adds one: the right zone is the author's call).
fn timezone_notice(root: &Path) -> Option<Notice> {
    let text = fs::read_to_string(root.join("seite.toml")).ok()?;
    let config: crate::config::SiteConfig = toml::from_str(&text).ok()?;
    (config.build.schedule && config.site.timezone.is_none()).then(|| Notice {
        key: "timezone-unset".into(),
        text: format!("Scheduled publishing decides what is live by \"today\", which without [site] timezone is each machine's own clock (CI often runs in UTC). {TIMEZONE_ADVICE}"),
    })
}

/// One-time notice for an edited deploy workflow without a `schedule:`.
fn workflow_schedule_notice(root: &Path) -> Option<Notice> {
    matches!(workflow_schedule(root), WorkflowSchedule::Modified).then(|| Notice {
        key: "workflow-schedule".into(),
        text: format!(
            "Scheduled publishing: .github/workflows/deploy.yml was edited, so seite left it alone. \
             To publish future-dated posts on their date, add a daily trigger under `on:`:\n{}",
            crate::deploy::SCHEDULE_TRIGGER.trim_end()
        ),
    })
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

/// Upgrade the project in the current directory (or, from a workspace root,
/// every workspace site). See [`run_for_site`].
pub fn run(args: &UpgradeArgs) -> anyhow::Result<()> {
    run_for_site(args, None)
}

/// Upgrade the project in the current directory. In a workspace, `--site
/// <name>` (or running from the workspace root, which has no `seite.toml`)
/// upgrades the workspace's sites instead, behind a single confirmation.
pub fn run_for_site(args: &UpgradeArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    if !args.agent {
        return upgrade_sites(args, site_filter).map(|_| ());
    }
    if args.check {
        anyhow::bail!(
            "--agent can't be combined with --check: --check changes nothing, so there is nothing to migrate. Run `seite upgrade --agent` to upgrade and get the migration brief."
        );
    }
    // Fail on a bad harness name (--with or SEITE_AGENT) before anything is
    // changed, whenever an agent could be launched.
    let harness = if args.with.is_some() || will_launch() {
        Some(crate::cli::agent::resolve_harness(args.with.as_deref())?)
    } else {
        None
    };
    let outcome = upgrade_sites(args, site_filter)?;
    if !outcome.applied {
        attach_brief_to_json(None, "cancelled");
        return Ok(());
    }
    deliver_brief(&outcome, harness)
}

/// What an upgrade run did, for building the `--agent` migration brief.
struct Outcome {
    /// False when the user declined the confirmation.
    applied: bool,
    sites: Vec<SiteOutcome>,
}

struct SiteOutcome {
    /// Workspace site name (None for a standalone project).
    name: Option<String>,
    /// Version recorded before the upgrade ((0, 0, 0) = unknown).
    from: (u64, u64, u64),
    changes: Vec<String>,
    notices: Vec<String>,
}

impl SiteOutcome {
    fn new(name: Option<String>, plan: &UpgradePlan) -> Self {
        SiteOutcome {
            name,
            from: plan.project_ver,
            changes: plan.changes.clone(),
            notices: plan.notices.iter().map(|n| n.text.clone()).collect(),
        }
    }
}

fn upgrade_sites(args: &UpgradeArgs, site_filter: Option<&str>) -> anyhow::Result<Outcome> {
    let cwd = std::env::current_dir()?;
    if let Some(ws_root) = crate::workspace::find_workspace_root(&cwd) {
        if site_filter.is_some() || !cwd.join("seite.toml").exists() {
            let ws_config =
                crate::workspace::WorkspaceConfig::load(&ws_root.join("seite-workspace.toml"))?;
            return run_workspace(args, &ws_config, &ws_root, site_filter);
        }
    }
    if site_filter.is_some() {
        human::warning("--site flag ignored (not in a workspace)");
    }

    // Verify this is a page project
    if !cwd.join("seite.toml").exists() {
        anyhow::bail!(
            "No seite.toml found in current directory. Run this command from a seite project root or a workspace."
        );
    }

    let mut plan = plan_upgrade(&cwd, args)?;
    let binary_ver = meta::binary_version();

    if plan.actions.is_empty() {
        // No file changes needed, but still stamp the version if it's behind.
        // This handles the case where upgrade steps exist but their checks found
        // nothing to do (files already present), or where the binary was bumped
        // without adding new upgrade steps (e.g. 0.4.0 → 0.4.3 with only bug fixes).
        // Likewise record a new or first-time agent (or hook) selection.
        if !args.check {
            plan.finish_without_changes()?;
        }
        report_deselected(&plan.deselected);
        human::success(&format!(
            "Project is up to date (seite {}).",
            meta::format_version(binary_ver)
        ));
        crate::output::json::set_data(plan.data(false));
        return Ok(Outcome {
            applied: true,
            sites: vec![SiteOutcome::new(None, &plan)],
        });
    }

    // Show what will change
    human::header(&format!(
        "Upgrading from {} → {}",
        plan.source_label(),
        meta::format_version(binary_ver)
    ));
    crate::human_println!();
    for line in &plan.changes {
        human::info(&format!("  {line}"));
    }
    crate::human_println!();

    // --check mode: just report and fail (exit 1 = upgrade needed; useful for CI)
    if args.check {
        human::info("Run `seite upgrade` to apply these changes.");
        crate::output::json::set_data(plan.data(false));
        return Err(pending_error(plan.changes.len(), None));
    }

    // Confirm unless --force
    if !args.force {
        let proceed = crate::cli::prompt::confirm("Apply these upgrades?", true)?;
        if !proceed {
            human::info("Upgrade cancelled.");
            crate::output::json::set_data(plan.data(false));
            return Ok(Outcome {
                applied: false,
                sites: Vec::new(),
            });
        }
    }

    let site = SiteOutcome::new(None, &plan);
    plan.apply()?;
    crate::output::json::set_data(plan.data(true));

    crate::human_println!();
    human::success(&format!(
        "Project upgraded to seite {}",
        meta::format_version(binary_ver)
    ));
    hint_trim_instructions(&plan.root);
    Ok(Outcome {
        applied: true,
        sites: vec![site],
    })
}

/// Upgrade every (or the `site_filter`) site of a workspace: plan them all,
/// show the changes grouped by site, confirm once, then apply.
fn run_workspace(
    args: &UpgradeArgs,
    ws_config: &crate::workspace::WorkspaceConfig,
    ws_root: &Path,
    site_filter: Option<&str>,
) -> anyhow::Result<Outcome> {
    let binary_ver = meta::binary_version();
    let mut plans = Vec::new();
    for ws_site in ws_config.sites_to_operate(site_filter)? {
        let site_root = ws_root.join(&ws_site.path);
        if !site_root.join("seite.toml").exists() {
            anyhow::bail!(
                "site '{}' has no seite.toml at {}",
                ws_site.name,
                site_root.display()
            );
        }
        let plan = plan_upgrade(&site_root, args)
            .map_err(|e| e.context(format!("site '{}'", ws_site.name)))?;
        plans.push((ws_site.name.clone(), plan));
    }

    let workspace_data = |plans: &[(String, UpgradePlan)], up_to_date: bool, applied: bool| {
        let sites: Vec<serde_json::Value> = plans
            .iter()
            .map(|(name, plan)| {
                let mut data = plan.data(applied);
                data["site"] = serde_json::json!(name);
                data
            })
            .collect();
        serde_json::json!({
            "up_to_date": up_to_date,
            "applied": applied,
            "version": meta::format_version(binary_ver),
            "workspace": ws_config.workspace.name,
            "sites": sites,
        })
    };
    let pending: usize = plans.iter().map(|(_, p)| p.changes.len()).sum();
    let outcome = |applied: bool, plans: &[(String, UpgradePlan)]| Outcome {
        applied,
        sites: plans
            .iter()
            .map(|(name, plan)| SiteOutcome::new(Some(name.clone()), plan))
            .collect(),
    };

    if pending == 0 {
        for (_, plan) in &plans {
            if !args.check {
                plan.finish_without_changes()?;
            }
            report_deselected(&plan.deselected);
        }
        human::success(&format!(
            "All {} workspace site{} up to date (seite {}).",
            plans.len(),
            if plans.len() == 1 { " is" } else { "s are" },
            meta::format_version(binary_ver)
        ));
        crate::output::json::set_data(workspace_data(&plans, true, false));
        return Ok(outcome(true, &plans));
    }

    human::header(&format!(
        "Upgrading workspace '{}' → {}",
        ws_config.workspace.name,
        meta::format_version(binary_ver)
    ));
    for (name, plan) in &plans {
        crate::human_println!();
        if plan.changes.is_empty() {
            human::info(&format!("Site '{name}': up to date"));
            continue;
        }
        human::info(&format!("Site '{name}' (from {}):", plan.source_label()));
        for line in &plan.changes {
            human::info(&format!("  {line}"));
        }
    }
    crate::human_println!();

    if args.check {
        human::info("Run `seite upgrade` to apply these changes.");
        let outdated = plans.iter().filter(|(_, p)| !p.changes.is_empty()).count();
        crate::output::json::set_data(workspace_data(&plans, false, false));
        return Err(pending_error(pending, Some(outdated)));
    }

    if !args.force {
        let proceed = crate::cli::prompt::confirm("Apply these upgrades?", true)?;
        if !proceed {
            human::info("Upgrade cancelled.");
            crate::output::json::set_data(workspace_data(&plans, false, false));
            return Ok(outcome(false, &[]));
        }
    }

    let outcome = outcome(true, &plans);
    for (name, plan) in &mut plans {
        if plan.actions.is_empty() {
            plan.finish_without_changes()?;
        } else {
            plan.apply()
                .map_err(|e| e.context(format!("upgrading site '{name}'")))?;
        }
    }
    crate::output::json::set_data(workspace_data(&plans, true, true));

    crate::human_println!();
    human::success(&format!(
        "Workspace sites upgraded to seite {}",
        meta::format_version(binary_ver)
    ));
    for (_, plan) in &plans {
        hint_trim_instructions(&plan.root);
    }
    Ok(outcome)
}

// ---------------------------------------------------------------------------
// `seite upgrade --agent`: migration brief
// ---------------------------------------------------------------------------

/// Release notes that apply to an upgrade from `from` to `to`, oldest first.
/// A `from` of `(0, 0, 0)` means the project's version is unknown (missing or
/// unparseable): like the upgrade itself, it is treated as the oldest
/// possible, so every entry up to `to` is returned and the flag says so.
fn select_notes(
    entries: &[ReleaseNote],
    from: (u64, u64, u64),
    to: (u64, u64, u64),
) -> (Vec<&ReleaseNote>, bool) {
    let mut in_range: Vec<&ReleaseNote> = entries.iter().filter(|n| n.version <= to).collect();
    in_range.sort_by_key(|n| n.version);
    if from == (0, 0, 0) {
        return (in_range, true);
    }
    in_range.retain(|n| n.version > from);
    (in_range, false)
}

/// Most release-note text (bytes) included for a project of unknown version.
const UNKNOWN_NOTES_CAP: usize = 48 * 1024;

/// What the launched agent is told; the brief itself lives in a file, since
/// argv is a poor place for tens of KB of markdown (length limits, `cmd /C`
/// quoting on Windows).
const LAUNCH_PROMPT: &str = "Read .seite/upgrade-brief.md and follow it (seite upgrade migration).";

/// Where the brief is written, relative to the directory `seite` ran in.
const BRIEF_FILE: &str = ".seite/upgrade-brief.md";

/// Push markdown headings in `body` down by `by` levels (outside code
/// fences) so an entry's own `##` sections nest under the brief's headings.
fn demote_headings(body: &str, by: usize) -> String {
    let mut in_fence = false;
    let mut out = Vec::new();
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if !in_fence && (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
            // Markdown has six heading levels.
            out.push(format!(
                "{}{}",
                "#".repeat((hashes + by).min(6)),
                &line[hashes..]
            ));
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// One site's part of the brief; `h` is the heading prefix for its sections
/// (`##`, or `###` under a workspace site heading).
fn render_site_brief(
    site: &SiteOutcome,
    entries: &[ReleaseNote],
    to: (u64, u64, u64),
    h: &str,
) -> String {
    let (notes, unknown) = select_notes(entries, site.from, to);
    let from_label = if unknown {
        "an unknown version".to_string()
    } else {
        format!("v{}", meta::format_version(site.from))
    };
    let mut out = format!(
        "seite upgraded this project from {from_label} to v{}.\n\n",
        meta::format_version(to)
    );

    out.push_str(&format!("{h} Applied automatically\n\n"));
    if site.changes.is_empty() {
        out.push_str("Nothing: the project files were already current.\n");
    } else {
        for change in &site.changes {
            out.push_str(&format!("- {change}\n"));
        }
    }

    if !site.notices.is_empty() {
        out.push_str(&format!("\n{h} Notices from the upgrade\n\n"));
        for notice in &site.notices {
            out.push_str(&format!("- {}\n", notice.trim()));
        }
    }

    out.push_str(&format!("\n{h} Release notes\n\n"));
    if notes.is_empty() {
        out.push_str("No release notes apply: this binary has none newer than the project.\n");
    } else {
        let mut notes = notes;
        if unknown {
            // Keep the newest entries that fit the size cap.
            let mut size = 0;
            let mut keep_from = notes.len();
            for (i, note) in notes.iter().enumerate().rev() {
                size += note.body.len();
                if size > UNKNOWN_NOTES_CAP && keep_from < notes.len() {
                    break;
                }
                keep_from = i;
            }
            let omitted = keep_from > 0;
            notes.drain(..keep_from);
            out.push_str(
                "The project's recorded seite version is missing or unreadable, so every release note is included",
            );
            out.push_str(if omitted {
                ", except older ones omitted for size (see `seite docs --topic releases`).\n\n"
            } else {
                ".\n\n"
            });
        }
        // Oldest first, so the agent reads them in the order they happened.
        for note in notes {
            out.push_str(&format!("{h}# {}\n\n", note.title));
            out.push_str(&demote_headings(&note.body, h.len()));
            out.push_str("\n\n");
        }
    }
    out.trim_end().to_string() + "\n"
}

/// The migration brief for an upgrade, or `None` when nothing changed and no
/// release notes apply (already up to date).
fn build_brief(
    sites: &[SiteOutcome],
    entries: &[ReleaseNote],
    to: (u64, u64, u64),
) -> Option<String> {
    let nothing_to_do = sites
        .iter()
        .all(|s| s.changes.is_empty() && select_notes(entries, s.from, to).0.is_empty());
    if sites.is_empty() || nothing_to_do {
        return None;
    }

    let mut brief = String::from("# seite upgrade: migration brief\n\n");
    if sites.len() == 1 && sites[0].name.is_none() {
        brief.push_str(&render_site_brief(&sites[0], entries, to, "##"));
    } else {
        for site in sites {
            let name = site.name.as_deref().unwrap_or("site");
            brief.push_str(&format!("## Site `{name}`\n\n"));
            brief.push_str(&render_site_brief(site, entries, to, "###"));
            brief.push('\n');
        }
    }
    brief.push_str(
        "\n## What to do\n\n\
         1. Read the release notes above. For every manual migration step or breaking change they describe, apply it to this site's content, templates, and config. Skip steps that don't apply to this site.\n\
         2. Keep the site's own customizations: edit, don't overwrite, files the site has changed (templates, `seite.toml`, content).\n\
         3. Run `seite check` and then `seite build`. Fix what they report (`seite explain <code>` describes a code) and re-run until both are clean.\n\
         4. Report what you changed, and anything you left for the user to decide.\n",
    );
    if sites.len() > 1 {
        brief.push_str(
            "\nThis is a workspace: apply each step to the site it belongs to, and run the checks per site with `--site <name>`.\n",
        );
    }
    Some(brief)
}

/// How the brief reaches the agent.
#[derive(Debug, PartialEq, Eq)]
enum Delivery {
    /// Print it (for the agent already running this command, or a script).
    Print,
    /// Start a coding agent seeded with it.
    Launch,
}

/// Inside an agent session, in `--json` mode, or without a terminal, the
/// brief is printed; only an interactive terminal outside any agent session
/// gets a launched agent.
fn choose_delivery(in_session: bool, json: bool, interactive: bool) -> Delivery {
    if in_session || json || !interactive {
        Delivery::Print
    } else {
        Delivery::Launch
    }
}

/// Record the brief (or its absence) in the command's `--json` payload.
fn attach_brief_to_json(brief: Option<&str>, delivery: &str) {
    let mut data = crate::output::json::take_data().unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = data.as_object_mut() {
        obj.insert("brief".into(), serde_json::json!(brief));
        obj.insert("agent_delivery".into(), serde_json::json!(delivery));
    }
    crate::output::json::set_data(data);
}

fn print_brief(brief: &str) {
    crate::human_println!();
    crate::human_println!("{}", brief.trim_end());
}

/// Hand the finished upgrade's migration brief to the agent.
fn deliver_brief(
    outcome: &Outcome,
    harness: Option<crate::cli::agent::Harness>,
) -> anyhow::Result<()> {
    let entries = crate::docs::release_notes();
    let Some(brief) = build_brief(&outcome.sites, &entries, meta::binary_version()) else {
        human::success(
            "Nothing to migrate: the project was already up to date and no release notes apply.",
        );
        attach_brief_to_json(None, "none");
        return Ok(());
    };

    let brief_path = write_brief_file(&brief);
    if !will_launch() {
        attach_brief_to_json(Some(&brief), "printed");
        if let Some(path) = &brief_path {
            if let Some(obj) = crate::output::json::take_data().map(|mut d| {
                d["brief_file"] = serde_json::json!(path);
                d
            }) {
                crate::output::json::set_data(obj);
            }
        }
        print_brief(&brief);
        if let Some(path) = &brief_path {
            human::info(&format!("The same brief is saved in {path}."));
        }
        return Ok(());
    }

    // Launch with the same resolution and argv shape as `seite agent` (never
    // an auto-approve flag, not even with --yes).
    let harness = match harness {
        Some(h) => h,
        None => crate::cli::agent::resolve_harness(None)?,
    };
    let Some(path) = brief_path else {
        human::warning("Could not save the brief to .seite/upgrade-brief.md; printing it instead.");
        print_brief(&brief);
        return Ok(());
    };
    let problem = match crate::cli::agent::install_hint_if_missing(harness) {
        Some(problem) => Some(problem),
        None => crate::cli::agent::launch_interactive(harness, LAUNCH_PROMPT)
            .err()
            .map(|e| format!("{e:#}")),
    };
    if let Some(problem) = problem {
        human::warning(&problem);
        human::info(&format!(
            "The migration brief is saved in {path}. Give it to your coding agent: \"{LAUNCH_PROMPT}\""
        ));
    }
    Ok(())
}

/// Whether an interactive agent would be started: only from a terminal,
/// outside an agent session, and not in `--json` mode.
fn will_launch() -> bool {
    use std::io::IsTerminal;
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    choose_delivery(
        crate::cli::agent::in_agent_session(),
        crate::output::is_json(),
        interactive,
    ) == Delivery::Launch
}

/// Save the brief to `.seite/upgrade-brief.md` in the current directory
/// (gitignored). Returns the relative path, or `None` if it can't be written.
fn write_brief_file(brief: &str) -> Option<String> {
    let root = std::env::current_dir().ok()?;
    crate::safe_fs::write(&root, &root.join(BRIEF_FILE), brief).ok()?;
    ignore_brief_file(&root);
    Some(BRIEF_FILE.to_string())
}

/// A site's `.gitignore` lists the brief, but a workspace root has no
/// seite-written `.gitignore`: there, add `.seite/.gitignore` (only if
/// missing) so the brief and the dev-server lock never show up in git.
fn ignore_brief_file(root: &Path) {
    let listed = std::fs::read_to_string(root.join(".gitignore"))
        .map(|s| s.lines().any(|l| l.trim() == "/.seite/upgrade-brief.md"))
        .unwrap_or(false);
    let nested = root.join(".seite").join(".gitignore");
    if !listed && !nested.exists() {
        let _ = crate::safe_fs::write(root, &nested, "upgrade-brief.md\ndev.json\n");
    }
}

/// The `--check` failure: the number of pending changes (and outdated sites).
fn pending_error(changes: usize, sites: Option<usize>) -> anyhow::Error {
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let in_sites = sites
        .map(|n| format!(" in {n} site{}", plural(n)))
        .unwrap_or_default();
    anyhow::anyhow!(
        "project needs upgrading: {changes} pending change{}{in_sites}",
        plural(changes)
    )
}

/// Everything `seite upgrade` would do to one project, computed without
/// writing anything.
struct UpgradePlan {
    root: PathBuf,
    project_ver: (u64, u64, u64),
    actions: Vec<UpgradeAction>,
    changes: Vec<String>,
    agents: Vec<Agent>,
    deselected: Vec<Agent>,
    hooks_installed: Vec<String>,
    /// The agent or hook selection differs from what `.seite/config.json` records.
    record_selection: bool,
    /// One-time notices not shown yet (printed when applying, never with
    /// `--check`).
    notices: Vec<Notice>,
}

impl UpgradePlan {
    fn source_label(&self) -> String {
        if self.project_ver == (0, 0, 0) {
            "pre-tracking".to_string()
        } else {
            meta::format_version(self.project_ver)
        }
    }

    /// The `--json` payload for this project.
    fn data(&self, applied: bool) -> serde_json::Value {
        let binary_ver = meta::format_version(meta::binary_version());
        let agents = harness::to_ids(&self.agents);
        let unmaintained = harness::to_ids(&self.deselected);
        if self.changes.is_empty() {
            serde_json::json!({
                "up_to_date": true,
                "applied": false,
                "version": binary_ver,
                "changes": [],
                "agents": agents,
                "unmaintained_agents": unmaintained,
            })
        } else {
            serde_json::json!({
                "up_to_date": applied,
                "applied": applied,
                "from": self.source_label(),
                "to": binary_ver,
                "changes": self.changes,
                "agents": agents,
                "unmaintained_agents": unmaintained,
            })
        }
    }

    /// Stamp the current version and the agent/hook selection.
    fn write_meta(&self) -> anyhow::Result<()> {
        let existing = meta::load(&self.root);
        let mut new_meta = meta::PageMeta::stamp_current_version(existing.as_ref());
        new_meta.agents = Some(harness::to_ids(&self.agents));
        new_meta.hooks_installed = Some(self.hooks_installed.clone());
        if !self.notices.is_empty() {
            let mut shown = new_meta.notices_shown.take().unwrap_or_default();
            shown.extend(self.notices.iter().map(|n| n.key.clone()));
            new_meta.notices_shown = Some(shown);
        }
        meta::write(&self.root, &new_meta)?;
        Ok(())
    }

    fn show_notices(&self) {
        for notice in &self.notices {
            human::info(&notice.text);
        }
    }

    /// With no file changes, still stamp a stale version or a new selection
    /// (and show + record any one-time notices).
    fn finish_without_changes(&self) -> anyhow::Result<()> {
        self.show_notices();
        if self.project_ver < meta::binary_version()
            || self.record_selection
            || !self.notices.is_empty()
        {
            self.write_meta()?;
        }
        Ok(())
    }

    /// Apply every action, then stamp the new version and selection.
    fn apply(&mut self) -> anyhow::Result<()> {
        for action in std::mem::take(&mut self.actions) {
            apply_action(&self.root, action)?;
        }
        report_deselected(&self.deselected);
        self.show_notices();
        self.write_meta()
    }
}

/// Collect every upgrade action the project at `root` needs.
fn plan_upgrade(root: &Path, args: &UpgradeArgs) -> anyhow::Result<UpgradePlan> {
    // Historical upgrade steps can append to the project instructions before
    // the v0.19 migration runs. Reject symlinks and other non-file entries up
    // front so no action can write through a path outside the project.
    crate::cli::agent_instructions::validate_paths(root)?;

    let project_ver = meta::project_version(root);

    // Which coding agents to maintain: --agents, else the stored selection,
    // else (projects from before the selection existed) every agent.
    let stored_agents = meta::load(root)
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
            let step_actions = (step.check)(root);
            extend_dedup(&mut actions, step_actions);
        }
    }
    // Historical steps predate the agent selection; don't let them recreate
    // Claude Code files for a project that no longer uses Claude Code.
    if !agents.contains(&Agent::Claude) {
        actions.retain(|a| !a.replaced_path().is_some_and(|p| is_claude_owned(root, p)));
    }

    // Decide whether a migration action is needed after seeing every older
    // action. Some historical steps can create CLAUDE.md in projects where
    // neither instruction file existed when checks began.
    if project_ver < (0, 19, 0) {
        let claude_path = root.join("CLAUDE.md");
        let older_action_creates_instructions = actions
            .iter()
            .any(|action| action.targets_path(&claude_path));
        if crate::cli::agent_instructions::needs_migration(root)
            || older_action_creates_instructions
        {
            actions.push(UpgradeAction::MigrateAgentInstructions {
                root: root.to_path_buf(),
                description: "Canonical AGENTS.md project instructions".into(),
            });
        }
    }

    // Daily cron for scheduled publishing (only with `[build] schedule`).
    extend_dedup(&mut actions, check_workflow_schedule(root));

    // Per-agent harness files. Not version-gated: the checks are idempotent
    // and depend on the agent selection, which can change at any version.
    extend_dedup(&mut actions, check_agent_harness(root, &agents, args.mcp));
    // Turn-end `seite check` hooks, once per agent: merged into (or next to)
    // the actions above, and skipped for agents recorded as done so a hook
    // the user deleted stays deleted.
    let stored_hooks = meta::load(root).and_then(|m| m.hooks_installed);
    let hooks_installed = add_stop_hooks(
        root,
        &agents,
        stored_hooks.as_deref().unwrap_or_default(),
        &mut actions,
    );
    let record_hooks = stored_hooks.as_ref() != Some(&hooks_installed);

    // Retired 0.20 skills and AGENTS.md sections: removed when untouched,
    // otherwise kept with a notice shown once.
    let (legacy_actions, mut notices) = check_legacy_skills(root);
    actions.extend(legacy_actions);
    let (section_actions, section_notices) = check_legacy_sections(root);
    actions.extend(section_actions);
    notices.extend(section_notices);
    // Default templates `seite init` copied: refreshed when untouched.
    let (template_actions, template_notices) = check_default_templates(root);
    actions.extend(template_actions);
    notices.extend(template_notices);
    // Bundled themes from before the seite/* partials: replaced when untouched.
    let (theme_actions, theme_notices) = check_bundled_theme_base(root);
    extend_dedup(&mut actions, theme_actions);
    notices.extend(theme_notices);
    notices.extend(workflow_schedule_notice(root));
    notices.extend(schedule_feature_notice(root));
    notices.extend(search_feature_notice(root));
    notices.extend(og_feature_notice(root));
    notices.extend(timezone_notice(root));
    let shown = meta::load(root)
        .and_then(|m| m.notices_shown)
        .unwrap_or_default();
    notices.retain(|n| !shown.contains(&n.key));

    let changes = actions.iter().flat_map(|a| a.describe()).collect();
    Ok(UpgradePlan {
        root: root.to_path_buf(),
        project_ver,
        actions,
        changes,
        agents,
        deselected,
        hooks_installed,
        record_selection: record_agents || record_hooks,
        notices,
    })
}

/// Hint about trimming the canonical project instructions if rules files were created.
fn hint_trim_instructions(root: &Path) {
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
}

/// Apply one upgrade action. Every write and delete goes through
/// [`crate::safe_fs`], so a symlink committed into the project can't redirect
/// it outside `root`.
fn apply_action(root: &Path, action: UpgradeAction) -> anyhow::Result<()> {
    use crate::safe_fs;
    match action {
        UpgradeAction::Create {
            path,
            content,
            description,
        } => {
            safe_fs::write(root, &path, &content)?;
            human::success(&format!("Created {description}"));
        }
        UpgradeAction::MergeJson {
            path,
            merged,
            additions,
        } => {
            let json = serde_json::to_string_pretty(&merged)?;
            safe_fs::write(root, &path, format!("{json}\n"))?;
            for desc in &additions {
                human::success(desc);
            }
        }
        UpgradeAction::Update {
            path,
            content,
            description,
        } => {
            safe_fs::write(root, &path, &content)?;
            human::success(&format!("Updated {description}"));
        }
        UpgradeAction::SyncInstructionsBlock {
            root: site_root,
            block,
            description,
        } => {
            let path = project_instructions_path(&site_root);
            safe_fs::guard(root, &path)?;
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
            safe_fs::guard(root, &path)?;
            let mut existing = fs::read_to_string(&path).unwrap_or_default();
            existing.push_str(&content);
            safe_fs::write(root, &path, existing)?;
            human::success(&format!("Updated {description}"));
        }
        UpgradeAction::InjectToml {
            path,
            section,
            key,
            value,
            description,
        } => {
            safe_fs::guard(root, &path)?;
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
            safe_fs::write(root, &path, patched)?;
            human::success(&format!("Updated {description}"));
        }
        UpgradeAction::StampProjectVersion { .. } => {
            // The version marker is the upgrade's commit point and is
            // written only after every file migration succeeds.
        }
        UpgradeAction::MigrateAgentInstructions {
            root: site_root,
            description,
        } => {
            safe_fs::guard(root, &site_root.join("AGENTS.md"))?;
            safe_fs::guard(root, &site_root.join("CLAUDE.md"))?;
            if crate::cli::agent_instructions::migrate(&site_root)? {
                human::success(&format!("Updated {description}"));
            }
        }
        UpgradeAction::PruneLegacySections {
            root: site_root,
            description,
        } => {
            let path = project_instructions_path(&site_root);
            safe_fs::guard(root, &path)?;
            if let Ok(existing) = fs::read_to_string(&path) {
                if let (Some(updated), _) = prune_legacy_sections(&existing) {
                    crate::cli::agent_instructions::write_atomic(&path, &updated)?;
                    human::success(&format!("Updated {description}"));
                }
            }
        }
        UpgradeAction::Remove { path, description } => {
            safe_fs::remove(root, &path)?;
            human::success(&format!("Removed {description}"));
        }
        UpgradeAction::WriteFiles { files, description } => {
            for (path, content) in files {
                safe_fs::write(root, &path, content)?;
            }
            human::success(&format!("Updated {description}"));
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

/// Keep `.claude/settings.json` (and, with `mcp`, `.mcp.json`) current.
///
/// Always: create the settings file when missing, and add the allow rules
/// introduced after the original template ([`CLAUDE_ALLOWED_TOOLS_UPGRADE`]).
///
/// With `mcp` (`seite upgrade --mcp`), also configure the seite MCP server
/// the way Claude Code loads it: create `.mcp.json` or merge the `seite`
/// server into it (other servers are kept), move any legacy `mcpServers`
/// from settings.json into `.mcp.json` (only when `.mcp.json` can hold them),
/// pre-approve it via `enabledMcpjsonServers`, and allow `mcp__seite`.
/// Without `mcp`, existing MCP config — including a legacy `mcpServers`
/// block — is left exactly as it is.
///
/// Malformed JSON files are left untouched. Running it again is a no-op.
///
/// [`CLAUDE_ALLOWED_TOOLS_UPGRADE`]: harness::CLAUDE_ALLOWED_TOOLS_UPGRADE
fn check_claude_config(root: &Path, mcp: bool) -> Vec<UpgradeAction> {
    use crate::cli::harness::{
        claude_settings, mcp_server_block, CLAUDE_ALLOWED_TOOLS_UPGRADE, CLAUDE_MCP_ALLOW,
    };

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
    let mcp_holds_servers = if !mcp {
        false
    } else if !mcp_path.exists() {
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
    } else if let Some(mut mcp_json) = read_json_object(&mcp_path) {
        let servers = mcp_json
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
                        merged: serde_json::Value::Object(mcp_json),
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
            content: pretty_json(&claude_settings(mcp)),
            description: ".claude/settings.json (permissions for Claude Code)".into(),
        });
    } else if let Some(mut settings) = settings {
        let mut additions = Vec::new();

        if mcp {
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
        }

        let permissions = settings
            .entry("permissions".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if let Some(permissions) = permissions.as_object_mut() {
            let allow = permissions
                .entry("allow".to_string())
                .or_insert_with(|| serde_json::json!([]));
            if let Some(allow) = allow.as_array_mut() {
                if replace_rule(
                    allow,
                    harness::CLAUDE_LEGACY_THEME_ALLOW,
                    harness::CLAUDE_THEME_ALLOW,
                ) {
                    additions.push(format!(
                        "Narrowed {} to `seite theme apply|list` in .claude/settings.json",
                        harness::CLAUDE_LEGACY_THEME_ALLOW
                    ));
                }
                let mcp_rule = mcp.then_some(CLAUDE_MCP_ALLOW);
                for rule in CLAUDE_ALLOWED_TOOLS_UPGRADE.iter().copied().chain(mcp_rule) {
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

/// Create `.claude/launch.json` (Claude desktop app preview server), or add
/// the `seite` configuration to an existing one.
fn check_launch_json(root: &Path) -> Vec<UpgradeAction> {
    let rel = harness::LAUNCH_JSON_PATH;
    let path = root.join(rel);
    if !path.exists() {
        return vec![UpgradeAction::Create {
            path,
            content: pretty_json(&harness::launch_json()),
            description: format!("{rel} (dev server for the Claude desktop app's preview)"),
        }];
    }
    let Some(mut doc) = read_json_object(&path) else {
        human::warning(&format!(
            "{rel} is not a JSON object; add the seite dev server to it manually"
        ));
        return vec![];
    };
    match harness::merge_launch_json(&mut doc) {
        Ok(true) => vec![UpgradeAction::MergeJson {
            path,
            merged: serde_json::Value::Object(doc),
            additions: vec![format!("Added the seite dev server to {rel}")],
        }],
        _ => vec![],
    }
}

/// Create or refresh a bundled skill in skills directory `dir`: SKILL.md and
/// its `reference/*.md` files.
///
/// SKILL.md embeds a `# seite-skill-version: N` comment in its YAML
/// frontmatter. If the existing file has a lower version (or none), the
/// upgrade replaces it — and every reference file — with the bundled version.
/// This lets us ship improved prompts in new releases without requiring the
/// user to manually diff skill files. A SKILL.md at the same or a higher
/// version is left alone; only missing reference files are added then.
fn check_skill(root: &Path, dir: &str, skill: &harness::Skill) -> Vec<UpgradeAction> {
    let mut files = harness::skill_files(dir, skill).into_iter();
    let Some((skill_md, content)) = files.next() else {
        return vec![];
    };
    let main = check_versioned_file(root, skill_md, content, &format!("/{} skill", skill.name));
    let replace_all = main.is_some();
    let mut actions: Vec<UpgradeAction> = main.into_iter().collect();
    let references: Vec<(PathBuf, String)> = files
        .map(|(rel, content)| (root.join(rel), content))
        .filter(|(path, _)| replace_all || !path.exists())
        .collect();
    if !references.is_empty() {
        actions.push(UpgradeAction::WriteFiles {
            description: format!(
                "{dir}/{}/reference/ ({} file{})",
                skill.name,
                references.len(),
                if references.len() == 1 { "" } else { "s" }
            ),
            files: references,
        });
    }
    actions
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

/// Slash-command wrappers (OpenCode's `.opencode/commands/seite.md`, Gemini
/// CLI's `.gemini/commands/seite.toml`) for the selected agents, created or
/// refreshed by version like skills.
fn check_command_files(root: &Path, agents: &[Agent]) -> Vec<UpgradeAction> {
    harness::command_files(agents, SiteFeatures::detect(root))
        .into_iter()
        .filter_map(|(rel, content)| {
            check_versioned_file(root, rel, content, "slash command → skill")
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Skills and rules folded into `/seite` (0.21)
// ---------------------------------------------------------------------------

/// SHA-256 of the body (everything after the YAML frontmatter, CRLF→LF,
/// trimmed) of every version seite ever bundled of the skills and the rule
/// that 0.21 folded into the `seite` skill. Comparing bodies rather than
/// whole files makes one hash cover every agent's rendering (Claude and
/// portable skill frontmatter, Claude `paths:` and Cursor MDC rule
/// frontmatter). Edge case: a copy whose user only edited the frontmatter
/// counts as unmodified.
///
/// Recompute from history with `git log --follow -- src/scaffold/<file>` and
/// `git show <commit>:src/scaffold/<file>` (files removed in 0.21):
const LEGACY_BODY_HASHES: &[(&str, &[&str])] = &[
    (
        // skill-theme-builder.md @ 253aba7, b5e3876, 80f0df5
        "theme-builder",
        &[
            "4c3f8ce70b70ed1b95b7767d379135babe3ba259cc5f23ac5cca8314c38d070d",
            "d2ece8146129ded7b44f71629a9739f3a508c7479642b18c88d1349641e8b16b",
            "9a36807cc0ea01fa6fc59d6383cc826e78e407e80ee2af132f7a4ec7c4c0f88d",
        ],
    ),
    (
        // skill-brand-identity.md @ 3baa808
        "brand-identity",
        &["e6675d9e01b2af4742b08182fed94309f0d9e9506685a5d627a92a195578ffbe"],
    ),
    (
        // skill-landing-page.md @ b40e871, b5e3876, 257718f
        "landing-page",
        &[
            "fc9960ae709c23e531e3eb158cd88bdfeeb783f2f72cd8f35914c067cfa6c062",
            "979779305a30c237b0d76afb7a2ba8ed9812fa2f31b01dbd36d0b8b18229275d",
            "464be45ca2bffbce98929b8e1339bd76fb1c38fe262d5f0903154f396e086cf0",
        ],
    ),
    (
        // design-prompts.md @ bf32552, f2235fa
        "design-prompts",
        &[
            "9db95f6ed5675195fff0dd93477aa466b22f886fa38f84a60558a86b389f7463",
            "3a2197dc5d93ef2e89bb90a2c8f0e12269810c3c6f8aff35f256de91dd30ccb7",
        ],
    ),
];

/// Hex SHA-256 of `content`'s body (see [`LEGACY_BODY_HASHES`]).
fn legacy_body_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let text = content.replace("\r\n", "\n");
    let body = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.find("\n---\n").map(|i| &rest[i + 5..]))
        .unwrap_or(&text);
    Sha256::digest(body.trim().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether `content` is an unmodified bundled version of `name`.
fn is_unmodified_legacy(name: &str, content: &str) -> bool {
    let hash = legacy_body_hash(content);
    LEGACY_BODY_HASHES
        .iter()
        .filter(|(n, _)| *n == name)
        .any(|(_, hashes)| hashes.contains(&hash.as_str()))
}

/// Remove the `theme-builder`, `brand-identity`, and `landing-page` skills
/// and the `design-prompts` rule seite wrote before 0.21 — only copies still
/// exactly as seite shipped them. Modified copies stay, with a notice that
/// `/seite` now covers them.
fn check_legacy_skills(root: &Path) -> (Vec<UpgradeAction>, Vec<Notice>) {
    let mut actions = Vec::new();
    let mut kept = Vec::new();
    for dir in [harness::CLAUDE_SKILLS_DIR, harness::SHARED_SKILLS_DIR] {
        for name in ["theme-builder", "brand-identity", "landing-page"] {
            let rel = format!("{dir}/{name}");
            let skill_dir = root.join(&rel);
            let Ok(content) = fs::read_to_string(skill_dir.join("SKILL.md")) else {
                continue;
            };
            let only_skill_md = fs::read_dir(&skill_dir)
                .map(|entries| entries.count() == 1)
                .unwrap_or(false);
            if only_skill_md && is_unmodified_legacy(name, &content) {
                actions.push(UpgradeAction::Remove {
                    path: skill_dir,
                    description: format!("{rel}/ (now part of /seite)"),
                });
            } else {
                kept.push(rel);
            }
        }
    }
    for (dir, ext) in [
        (".claude/rules", "md"),
        (".cursor/rules", "mdc"),
        (".agents/rules", "md"),
    ] {
        let rel = format!("{dir}/design-prompts.{ext}");
        let path = root.join(&rel);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if is_unmodified_legacy("design-prompts", &content) {
            actions.push(UpgradeAction::Remove {
                path,
                description: format!("{rel} (replaced by /seite theme)"),
            });
        } else {
            kept.push(rel);
        }
    }
    let notices = kept
        .iter()
        .map(|rel| Notice {
            key: format!("kept-legacy:{rel}"),
            text: format!(
                "Kept {rel} (edited since seite wrote it); `/seite theme`, `/seite brand`, and `/seite landing` now cover it, so delete it when you no longer need it."
            ),
        })
        .collect();
    (actions, notices)
}

// ---------------------------------------------------------------------------
// Default templates copied by `seite init`
// ---------------------------------------------------------------------------

/// One template `seite init` copies into `templates/` whose default has
/// changed since it was first shipped.
struct DefaultTemplate {
    /// File name under the template dir.
    file: &'static str,
    /// The current default, written over an untouched older copy.
    current: &'static str,
    /// SHA-256 (as [`template_hash`]) of every version `seite init` has
    /// written, oldest first; the **last entry is the current default**.
    hashes: &'static [&'static str],
    /// What the refresh brings, for the upgrade plan.
    what: &'static str,
    /// Present in an edited copy that already has the change (no notice).
    has_change: &'static [&'static str],
    /// Notice for an edited copy without the change.
    edited: &'static str,
}

/// Defaults `seite init` copies whose content changed after release. Hashes
/// are recomputed from the history of `src/templates/mod.rs` (`log -S "pub
/// const DEFAULT_X"`, then `show <commit>:src/templates/mod.rs`, hash the raw
/// string body). **Editing one of these defaults? Append its new hash** (the
/// test `current_default_templates_are_in_hash_table` prints it), so sites
/// with the previous version keep upgrading.
const DEFAULT_TEMPLATES: &[DefaultTemplate] = &[
    DefaultTemplate {
        file: "post.html",
        current: crate::templates::DEFAULT_POST,
        // 60e1456, 89da9cd, 42dbe4f, f7a5557..0dc5dbc (0.20.x), current.
        hashes: &[
            "c6600c4e5b34ff95d5733b948b25f083e2a899a5aa61bd85e5c00d1a0cd3aee3",
            "14b31eb7e680a51665780ceb8b3541bf441d086b414e84c27722c8685eb02485",
            "ca09ea84916e726a2fbad0d31204b2cca79d3ecf87e20ca6be1875f8e6bd6469",
            "63ee8cde622a4ae2494533cf5c4df2ea48994297538c8d71a8b65aa34d2029ca",
            "bc879538cd2597b916d7f1a38117c8febb1ad1a52583ff2b0857cbceeaf83da6",
        ],
        what: "categories, series box, related posts",
        has_change: &["page.related", "page.series"],
        edited: "To show categories, series and related posts, add the snippet from the Templates docs (`seite docs templates`, section \"Taxonomy, term and series templates\").",
    },
    DefaultTemplate {
        file: "index.html",
        current: crate::templates::DEFAULT_INDEX,
        // a92563e, 6928a7d, d308b69, 4ab86e8, a58a8d9, 375fe9d,
        // df8b337..0.20.x, 0af5137 (unreleased), current.
        hashes: &[
            "202ef49b4d78e6427110c35e0e7e73188416f2d21a8743204ff8a3cd69a1ae63",
            "910b2a35769987f35ca153c1163f992028b26b20a9280b0926c40f501a2107b6",
            "cd737feb740999e01ff56113e5f196169503b10b5cb69eaa7e10584e235b94c3",
            "bff860a2901a162b2d85cc40af59075e610316051b131bd758f5491ca60b241d",
            "8c2604f638f0b32d85070fbfa6b70b4a2fc76611e676e8affe8fd2b4c9db97b2",
            "641003bf73cefda9ed490a1056ca71d9bf61bb579573a9c146fe674d4e4dfaa9",
            "c9071fb9307f0daa1e3619889c819d4ad1d008edc9e8877ea8a373018b661c87",
            "921c4be7b2f134740dbde81fa556555fe84a95878014d8a876b08bbcdabde1de",
            "45978b6a0bf926a3f5162b77ef54e9b2cf347f8c274f71823341ebbb03753efa",
        ],
        what: "distinct titles for collection index pages, an <h1> with the blank starter",
        has_change: &["_home_url"],
        edited: "Collection index pages it renders all get the site title; to give each its own, add `{% elif page and page.collection and page.title and page.url != lang_prefix ~ \"/\" %}{{ page.title }} — {{ site.title }}` before the `{% else %}` of its `{% block title %}`.",
    },
    DefaultTemplate {
        file: "trust-index.html",
        current: crate::templates::DEFAULT_TRUST_INDEX,
        // 7dc79bd, 097fe4b, be1a2d1..0.20.x, current.
        hashes: &[
            "b23eae820d06f2aa30a9a7b4f0ff9f3d073c41789dde5ab8208520f8cf4be7fe",
            "12cf04e61e8a6100d429864eef6a84f92575b073fb8833f344ce3ed19864e9e4",
            "c0a2ce48051ba3ad59d1714d9b72434f959f7dbaa5a94ff0e3dc6859d5222952",
            "e31e5656f1777750dbbb6fd0750875a3a387ef72c4b15f543ee761655fa51724",
        ],
        what: "descriptive \"Learn more\" link names",
        has_change: &["cert-card__link\" aria-label"],
        edited: "Its \"Learn more\" links all have the same name; add `aria-label=\"{{ t.learn_more | escape_attr }}: {{ cert.name | i18n(lang=lang) | escape_attr }}\"` to the `cert-card__link` anchor.",
    },
];

fn template_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(content.replace("\r\n", "\n").trim().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Replace each template in [`DEFAULT_TEMPLATES`] with the current default
/// when it is exactly a default an older `seite init` wrote. An edited copy
/// without the change is kept, with a one-time notice saying what to add.
fn check_default_templates(root: &Path) -> (Vec<UpgradeAction>, Vec<Notice>) {
    let template_dir = crate::config::SiteConfig::load(&root.join("seite.toml"))
        .map(|c| c.build.template_dir)
        .unwrap_or_else(|_| "templates".to_string());
    let mut actions = Vec::new();
    let mut notices = Vec::new();
    for t in DEFAULT_TEMPLATES {
        let rel = format!("{template_dir}/{}", t.file);
        let path = root.join(&rel);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let hash = template_hash(&content);
        if hash == template_hash(t.current) {
            continue;
        }
        if t.hashes.contains(&hash.as_str()) {
            actions.push(UpgradeAction::Update {
                path,
                content: t.current.to_string(),
                description: format!("{rel} ({})", t.what),
            });
        } else if !t.has_change.iter().any(|m| content.contains(m)) {
            notices.push(Notice {
                key: format!("default-template:{rel}"),
                text: format!("Kept {rel} (edited since seite wrote it). {}", t.edited),
            });
        }
    }
    (actions, notices)
}

// ---------------------------------------------------------------------------
// Bundled themes written to templates/base.html
// ---------------------------------------------------------------------------

/// SHA-256 (CRLF→LF, trimmed, as [`template_hash`]) of every `base.html` a
/// bundled theme has ever had, per theme: every commit of
/// `src/themes/<name>.tera`, plus the inline `base_html: r#"..."#` strings in
/// `src/themes.rs` from before the `.tera` files existed. `seite init` and
/// `seite theme apply` write the theme verbatim, so an untouched copy hashes
/// to one of these. The last entry of each list is the current theme.
///
/// **Editing a bundled theme? Append its new hash to its list** (the test
/// `current_bundled_themes_are_in_legacy_table` prints it), so sites with the
/// previous version keep upgrading.
const LEGACY_THEME_HASHES: &[(&str, &[&str])] = &[
    (
        "default",
        &[
            "a90a8c533823dc9acffa73663a9e98fcf627c7fffd3686cbf42cc1908bb065a0",
            "220f6e5221148bdedf1987d1ff6adce279008811f1c03255610fcd9fb0e8c50b",
            "2ee2d6673a834af54151fcb500bb83917c21e743c762c40a29180d5f3e7b864d",
            "2981d61dd79e933c240c0495e70a74c7fc595b40c73ed9047682095b1a7e4d88",
            "5f2ed3af520097ad4b301af8849703adc10f1523eea94bd91d7e7059c1035f13",
            "a0ed7ddffab746c691d2fcb05e71c66bfcc192cfea45a2c2dd3e4d252e9a0810",
            "d4d473cc4d000d80751423331a6676d6c5274317794a38e4f611287e19474f11",
            "24f9f79fa8bfd4a488ff340509e2fd53dae7f036d63ad6aaed5bcd9d4726b785",
            "68532be5fbde48c04fc6ef870dbf75d18504524b9e0f3e1acabcfd25e85d093f",
            "4e26f9701dc7298c9281ace7d42b03ba8781b048bfc3fed7b748dea57d96732d",
            "0ce2c0bc85779d80943c552561c14337200b89cdefeacb165e2aae718540633e",
            "dd7f96210e92499a5cfd792a4f28afb9e9583dbcc16426d3b9f1b13cd94de4c2",
            "08668162d70762021941c19af606d446726016f358fb95e917c1095d42b39bd8",
            "ac8f801fb34a1306ff3818ce97e2abf54e39f743611ea339783a9919bb0d9ff8",
            "a7d37ccc2130902d6134f23dfca95af2ee6612cc40704746362d0423ba6951cd",
            "21593a5af2f1417dca44de174dce511c51b17b4ef027bf4d7b2922304fa1701b",
            "45482b37d30a18b2f58a7a3b4572d601ee5f2575999d9d880f55f9bdede90193",
            "f3e634c4ca05485ce7248172b53ff0e8069bd84f890419d4b31ab74a00a6f4e5",
            "9e6f8804c0d69f928c3855087aa7d9c930efda4fdd78516feabcb0af2bb7160f",
            "50dd3c9ec6045d35486fa87eae57d44aeda24d8ac8c2714c858ff0ffc97fe328",
            "77249a03e2ddecabff6182e8c6a8e6e66d99cf984800c92b997c1ad0948116eb",
            "5a40e887bc7e83c6cf3fa36e86e9714ba0ff7afc2926b8a0daad159f8afae1b8",
            "7698e00076d18442fb0d7b6177900cfdd2f3f4acf30c1867b2b07c3ee314e7f9",
            "d564410184a4ebd56c3569782c343c91d5ee36497a2b829821572d1705eef4c4",
            "06de5f742f090647bb3b072a3ae1b29bd8543ba57edc39a9150b71e340f2bd28",
            "50d702148735f7b8946e79b9631dc4a8b72e152274d12c220dd509c1ca42a7c5",
            "dd2ec66f4f544217097508197ef07840d4e080726b9ae094b34e7d4c2de5ad24",
            "8f53eea39d36f27d80588b2d13be68aa659b5c0e44d160369a241d7413fdf83d",
        ],
    ),
    (
        "docs",
        &[
            "d41e1df30d1ca84c8a498a23f0d5cd05db949cd09ffb335cfacca7ab6073012b",
            "2bb170623b03bf1bc56c03b219b743477ccbf54b20ed08157a02387fc3dc8cc6",
            "95756f966f43e9eb6903364247e318f74543a950814954bcb06428f1d247e501",
            "9d1d5377bd5d259a26f13af98f2e6baabcbb64c0dc2d7cc210651f7bc961877f",
            "2a1ae04c67ee06344b2caa29cbca47c38efc3c84c76e44d7cc68ebde1a58e790",
            "de6bc882e812792775c134202c210de492525262fb4b5ee668e02d6566bcd1d6",
            "4293fd38cb9f5d7ee3f6f01efed45d397d275f173ca3c7afac25feb3ef040d09",
            "5e0e6566fca6227d364dd0d27c42bac5856003a9539b9d691d834e83dff4a75c",
            "0c38ddd8e2f75a2a178d5ad3a6d79fb926abb7fe0e8b94a3fb5b49586b0709a2",
            "44982601ac62da39b0885be3844438bdf8c2ab84e66e3232b48c816e1cb6003f",
            "27aa68a0befb1bd9adbd2333cac682a7aed9951c754cff70b679158fa62ff211",
            "303cdeb2a16fd93dd2f1dac938f3f70a855c7b52641f01c88c7f90614b5bd098",
            "b294702ecf16dd848d0456d22b483bdad922680c184bcc76b28a245fdad7be60",
            "9aa6082367f31856f2aa0db0fd4aa8b600cc352010eb724598ab6ea95c4fab70",
            "b1d3db5fe1f268c18c23c5f2155899e5d18ec68049d2fb0993b71acce41df579",
            "a6e2157301e9fc1367c053437ebe76052a16fbf4acd5ff3cc2b0c24c7db083b2",
            "bcd90c27069901f39c2865ed5c2e713195bb6a5abba051bc005f0c0650a90d8e",
            "319f27442a4881e6615452e11ffb7748e95638d3cbafff569c18b67fdc6e9757",
            "ec6567fc2f864a43eedac2709c7fdbba0150bf21a8fbab55bc54cfc1c848b8e1",
            "ad70b101d9ad99796cc7e481ca60cc87e78ad7e31ab00f25ec05f3d8901d9f53",
            "63cb5cf07db152d6dbcc839f811c9813e6c3ae3e24a7d887d79c15579cb2732e",
            "67d2ab2e5cd888eaa9e7a3406046fb5ffd3e7002d062c815a5e5db9e60d15758",
            "13d21288125b4ce74dc2a8a1664b62f9a9ef38d04d995c815bbce851ce671cab",
            "aed48911c56bea2f750cf880427a442ebadd48bc7422aa34f95a12eed9b9e419",
            "b9d7d6b516236214d3dd5c5a95f85fa1e84d771ae13f5144e14d1fbd8efc09ef",
            "67d4c8039ed0538ca34020304249019e16fc1ce2d3a62719404814443afb4140",
            "23e5570a2dc970bc9fa7f9e96b132b20d3867156e66051a90074cc9e157d5ad9",
            "e9ec1aeb94ffb008f68ded0eb02a12f55ab495e78aed05cb768f27d01dacfb0d",
            "bac947d8408c456c35c5244c9c23d352ac9e195a074a3797de85acec0de22b9a",
        ],
    ),
    (
        "landing",
        &[
            "3ade55868c66e2adb90f5957de48509047a316397f0387e5e68605167376d12e",
            "7f1dbb2fb2ef9dc4f208a60a4ea5da379373c505f1ceb53dfc16a54908d29290",
            "948f00f969f826470594b937a8118391ee4ce506084569d6250d15cc4f79a07a",
            "61b494102d94e5264f483212ca045dbdd4dab48eaa471882fbb87f7fece1803a",
            "e78187030059098ced2093081419f1340cb2b5aa2738f99545e9b2148159b43d",
            "fdf3edd9113b2c9df03e456191241876f89669011890ca8ecee1d6d07f8772be",
            "f34d4793be84e9aa96e6256c012bd83161029980cb9365ac7e46d0ca1151aab9",
        ],
    ),
    (
        "blank",
        &[
            "5fba48b8862df4b95f7cfc2474192feae35e145591ef68ef1e6b7b1aee7c0ea3",
            "0e4ee7b20d4d052c0e347895da73ae4e1d3806678c84f828f554c5ea4459cfdf",
        ],
    ),
    (
        "minimal",
        &[
            "f771f18184b99ea5aed2a17ab7ae501e97384bb58bdc5da74915fa86b8a7d55f",
            "fad41995a6ca6904a802a824f755f2c3747e58ef0ddf6d6ffeabca33c89d94d4",
            "e7e42cb2e59ce07508bd545017c76bd6977facec3a8c1fd7bd6b6689087efce9",
            "5c3f4e92ae58630c0ccdf6ad49d11563371e0ff509eb5c437a28adbf3172e220",
            "4f743c9fb341375a4209230d1ccbd8fb3ca28e58f7e913ab71c482689c66fade",
            "187509fb298ff02b70187eb5d8362e032af5391a7d174652b5251016aea3b945",
            "d15624c062580064ad855a699a4b4252f3e0784ee46d1eb7cd02dc4b21d21f87",
            "adaf12de14f513f82424ab8b6bf04e57819c950c68e46c36c84fc1a2cdeb32b7",
            "2866ce3bb60a0b1da72369ae12d8e82aac54247acfb1266a4dd9abab450bfb8a",
            "6a84188c99cedbe50a11a76cd2d5ee4b612f00a958c7f2b38a116e49e962de33",
            "c97008ad96aa12b846e734c689b0cc1570adb427d6fc4c9eecfca670f53fdfc4",
            "9a44ffc8f3b56b3c475bacc63113ce8041459f71621f4a5a3f3e5e207bba4020",
            "f126d181320e89f3743409ccf96e0d82abe077a9ec766563554913c4a93835c4",
            "bf00b213729ff824f366658bdad8943f37391e176ce0d8a70896a9be5147d6c5",
            "adb1236fec8af037f1c4be6a6a1252f4fb27077725013ba946fb0a26947952d8",
            "672419d484d9f4de01c2bc03b5eafd00b929994aab649d567fbfc65c1a487670",
            "233b13882910785de60bab14d9345f18efbb419458b9769eae49f9f1a9b52847",
            "955b45edb57936443b084ab421d0cabcd08efb4f8e026106b6d1d1f2e9af6b59",
            "8b261156686bfa5b8e47e98d6cdd06fbb5ae5de42c3b444b60ec31e1d3e7058f",
            "4c8119305369be1cf6cf070e158c09333946a2297af1d62eb71ed802d1112acd",
            "07d614b221b52913f62951d803480810d258a18301c8033e97a5b723c067f29f",
            "07cde97fac333a3671d18e7ec8430600f839f91513f131a2fad730330a482f88",
            "a937357cef63fbbd3b41ab1e4a552242e117afe825c6ae3f417b74902231fa33",
            "a51d7dd48f7cd7730e40aa426616a9942bca36b09a6810ad765f72ddea03b128",
            "dfb02d635bbe791c96d1fa7e7a9b731a6462515daa177675f0fa88d69c816b5a",
            "b0e2eb4b08dccd20b979be4bc729163e22efd995253cb37d9523af14b0b04aed",
            "d64bb60b1c7e2cdc544ca5833072f6891e39625e2631352bd5eea8c1d76fc427",
            "3ab28d59da8c926b74d62ae5a27947df00d0346cb51cd3d2bb31784c03bfdff6",
        ],
    ),
    (
        "dark",
        &[
            "655f71f67ca3bb13d5b7d6942cc284dc2a966c27f934248a5c7a1b00c052e056",
            "0fa0300e04dfb721f9e8809edd095bab6534ded3754d300912873b4bbd1483bd",
            "1b7da9bce81b6fc827e3a48f032f0bb00bde6fbd3cefd3c0711b5485ab55dc0d",
            "9f173c4a3b71cd5000c7c212545aa1407231741129dc6ee456bcb8ae8c5e3b9d",
            "47bab6964eb828411cd91433b8e08ed16162d4f5777888e790d6be3f5fc44da5",
            "f670640d210e30c5c48242bdef2749aeb1e8f03c0dc2dc61e48b4ea2a28c7626",
            "1baa8dff66ed7f594d4b533b48ea68a78bff438560bb994a71e280a1d9a49620",
            "56dbb0ae5de4e12fb6e546828d542a50070542e8a3025aa734d719b61522fff6",
            "2ee3bd487b48c5a05ca6070ad4f23b966df234c23f085a5e617b3da60e6d6a64",
            "df530cfc93795c5f6da0f2907efeb90ac85414d8debf282badb27dd04bafb959",
            "a54905607456dc88af9bf4570160b54e736666db02f68a2723550188bc6b83ac",
            "f295c93cbad1d94cd098f9f7dde27c57b124099e892999e333eb0e8a3e88d723",
            "509789404ff3008cdd3707ba9cf9870f5b67ce2d5860b3f5443d60c32fd5afb1",
            "eebaae38759f40c164010d1132e21bdaedc6d12c96542c339a73d985a8304f11",
            "0ef9e71026651c06343d5e3bacd609087c84a0a948b340c1ef5257cf28a13be1",
            "d99b83f67c7ef21647a42ebaa4c60a7d8a83d8f972232129c99ea62f6dc687ae",
            "ed71e4973f74b68280a6065e084cd05a40e4d49cfa43b7789c1c42599b729331",
            "7f6081e501c669dd52fe61689b02b308db2e4f9253491634538862917c19d688",
            "a66c7a083a59e0cfe2eb361d7202876304eaaa921f8072d6699cd10abde56e56",
            "293acf6a31800771b8565aaf7bf361f1ad2d71b8903a6b51bd5b0b82080ad0ab",
            "a41d09bfc2d93da3c749776d2be65959930c09604b167a7462309f47918c5162",
            "e6bcfe0c91dad307fc2d3f75c21f0a776e3d381ee2bab6a7a075401d7c238c2c",
            "96f8ef92ab2dfc6dc70b29f6dfcacb12e484371a8ce605ebc38e6c30d58f6b3d",
            "9f7be6f321ebdc0bdcc2abea56264ae8f16ca78780b4835e0a3b3397392ca4fd",
            "ae097eabe405888b84265a523b63d306932657ed8b048f7f0a893d728e43e976",
            "520251acfb14348f471ec93d7cd1aa78cd64c841e341fd151cc1cd29a450e85d",
            "724794a6a2e401271dc7787fec722239be103c0bbc2acc82d11900b7fcd79471",
            "7b4e157e98a3d7b19721f88ac26792fdd2fa6e94490e18b73e1ea200ba3a8ea7",
        ],
    ),
    (
        "brutalist",
        &[
            "b62dd29a08e0865cf9cafc777589f1c3bc9eed1d09189f3e540e4611022641ff",
            "24230b0eb0e120eba25acffa96f4152a4ac8de626b51c79cca2597184bd8c7a9",
            "f156f357a672f384bcdfc75a364bd8b105801d443e0d23f1bdf191b6a450a267",
            "fe6dda96ca93e8c102de130c61cb95c58ea03abafc6c9b014bb60540c535b526",
            "4057962628763effaa059c379b6aca103e5fc9938c7073a891a80efd82c747b4",
            "a45900ffbb3f3cf8665c264834980b8020466b5e710c80e44c5442503ad88cd9",
            "cac41c78eeff66612a4595a525c5cc2febfb21dd15ed1d38086880557b66f8be",
            "e599d5a1b386b9acb53b56189d5caf4aa7fbd8ae9cfd50ee67f3130327aba941",
            "94d35e66e3e940e07180aba391cfe7eba8689cc2fe5c57f8ab5149a7346921b8",
            "739ff4d16c34783c002346bed659eaf1bf4d781ab7d2f0452268cf152a48316a",
            "53f5d69ecc71c098f7bf6427ee1ddb5efd3708d6723023495738d6f2da9d470c",
            "bbd33b9507f7c8e14a4dc239b2a5771c2f7b47961a6c164a4da4f3ce4add12f5",
            "89345a741013c466baebaa323b0535eb8feb0a5b3225eadaa6a9901fe638238e",
            "42d1e8a8155706426c40352899770af8431aa697bd3c22c77e75f642891893c7",
            "3791545b88c4aab1cec1c345e56b4777fefd1c1b4b2ff954f4f4cca107802932",
            "3c66e983ea90d1a0213fa3b45d1da37980ad801733c23eb6b06a3a7310cb3a7c",
            "9d48ecd6eeba015bbf9318776118461b8d237020cb5256016cc46dd87a5db93a",
            "610ce5a424388f176538fac1843280e2892de86ff198e6432166e90ad572a920",
            "9cbf90264e4f24676c77c84aabc5fb995dc5b359468de6e948f54ede7fff1a8f",
            "52c292e91c215eb66859a42e5cacbe1b0b0bfb8dbbf2f28441d0f486049ce659",
            "ffd435c6a19d9385d1af3bbc836ccb02357f76288e548e9d5fe9a31ffda1b394",
            "21475e7cf8badae085c279989e6d42061c53181c2db9e800c8901af508771869",
            "6661e028e79eba4a480239fc11566aedf485b42fb21291da801d85b1a84f09b9",
            "8467f522983a8eed778887a737688cb258bfbdbfc173d5a54995d51b4903abb7",
            "63fa19e04d563818fe23726d67498e19147b1f28d39531f19723cfc6bf0bd44a",
            "32ac5aa459691e572d4ba20bee595aff2580f6b6e47091a640d790a82ab2c79a",
        ],
    ),
    (
        "bento",
        &[
            "73042f8b7b7c052a589778f3d319ce2a224b98369ea7b6e1e4e1911e5c67ba25",
            "34117f6ed4efd0394b74dbdf4c12757b8edb2f94506591cae7529567d300b6eb",
            "d13fe9ed5a17b1a9341606e2b31f91d74f0f228041883d9871e2cd299094a561",
            "4426796a9744171fa20585aa4fdcadea984466d9fc5bbfba90e1b095b388e5df",
            "59fc7c2058dee677de487c566ecac4639bddeb94e4100d3865af1ad24741fdf9",
            "525f1f725db0155e8d395bf738d6cafed37a0de2b05edc5dfaf3cc08b9c160ef",
            "279935aecee7fd38bc7cca2ff2b2e389fd9e2f5f521617f857dae9abd3ca66ed",
            "8e786a837252267bad9867e8f188a42c229b2a273478ad9cc7248fe996d62a4f",
            "7fde4e0ba35b413767d045c2aab4cceb64e14f617aed9e05e5e5324792770591",
            "9eacbfc016582de7d40deaf20a03fb8f8c69909f05c14114b636da49dc67424f",
            "3d31b6259be8c7e153c14a41a6842f892711bf39339903080ac495b1fb53d963",
            "5db3d082634b68ad7d96917e9120724d88ee03529359938cfa36832f5c11bd56",
            "692999f613d73fa2a41e451b11f0ea47d61c8cd05a19d468e78061428154e835",
            "f0e7105d19f993829355c75887e226f2834d58c3314614af1df467a843223999",
            "06948ddb4d6eeb200a8f625bd2c24a2a9b1dbc59a92f570bdc73407f8ce6436f",
            "fa7aefc16ad95209a7b4a0171c04135312ce65b96c9fe6bca197f785033b5476",
            "e22c4f5a2d11660a7a9b8cfba37f941c698f25e33e042d999b0b57fe224cb25f",
            "c94d01e9e5b361b0459c69688247e17f313c82cea3a02af33f014382190cd152",
            "bd26a28c7dacf2cca52c58982c30c24ee76b4da7a574b57d61e8479c0e54dc8e",
            "49026a33710bcc6b4e718ea18c4e5b63b2b37166f5e8c50434a025d9650bc582",
            "4cb57a028c7a5681d1e4b88fb9b2c05cf296a16c6a8f57014e309b3f4ba2785f",
            "62f5defd8e549909f4f1a503fd4919b589ab0ee01dd863df76cda57abcc3b888",
            "ea4a43ed2f011b1ac4f1e79e89d0cee9a034fad492c9bf46d6d2586e3782168c",
            "18da3adfc0983acaa1de7e7951f990926aae2bce424c63e8384263d955468312",
            "22f6e094348547ef0e71c718d49fcc87a8ea96c8222aeb59ad1f6fd4924e55ec",
            "3aba02ba86b0e79f7d8de9f1af74763e2fa15e871471ed241ed62692d76fafe3",
        ],
    ),
    (
        "terminal",
        &[
            "cb21b14af919c7311d6049d9ab4d04449059b9bfaa010326b0120effa32f56e0",
            "d229ebbf585b2d98c94aae22fff256412d7d1b96e433f4424d0021215b993c88",
            "27fd3ac2dac8d76a3b5ee1457e23b5ffcbb27592ecf5133bcddf889559e1c923",
            "2177b18b92f4b4b385f84148765e35367e1b141fcd42aef5bd70453f2f91fc06",
            "297cde2ed99d71a88a17486541cd6f6bb9e2f44ca7fc7c138fede60ddc61ebf2",
            "b1d9cbc5ffcfc5d219b8fd863ef18c9fe1b8204568be69a5feca4929ebd970b7",
            "20f690a9805b2a6e990fc666217ec32bb14d0e9d6595de8d2e4721681684f243",
        ],
    ),
    (
        "magazine",
        &[
            "3573e9fbf51525608af6edb78f0458b069df8218917227d09bdb0fc6619d10f7",
            "8189b0593ee79613a05ca0cd4f1e172086a867860366e5015fe4bce7e2edd2f1",
            "ba02fe464a5ab7581f03cd50dce483be6dadba86c908e33c0d490c66c5760282",
            "83eb867c64c4809ef7c8c6a9e8f46baa15af19f7146fe8a67bfb71be72e9c7b9",
            "8b15b677c3d364dca85703408aa2d6ce4d0fcfdb51af82cd5e001008ca03ed17",
            "95eec46b34053a662e6a06d16d29f17b1abbb2cd7b0fa9ffc2480468abf718bb",
            "c10197ce9620c1a3004799517287ec618422c6c431d4fde972e33f472b3be1ba",
            "3b20c6ff0e0620b18f72c0f4f5c19ffd45da4606df4b9c75c1316355f927fcf0",
        ],
    ),
    (
        "academic",
        &[
            "c3870c4852c60003415be20a9b0a1dfad5bf20655906c00c81562dfbe2544559",
            "52786ced85652ddb4c1da123a1f2de721991a3e1c92da37fb940cc49acce2a81",
            "730ac39afa2004f01036987b72ec43927280cb1d52df2b07eeba9b69d18c2e5c",
            "b1a8c13ba9c5cb984a86da0e3078013a61513a5be0927a015f3cb45fadb353c2",
            "99e4bf5611d3f8209642bcdf4eae519e5a7929f881d22560b7eb8e08d588cb65",
            "9a692e0451c2e271b0578d210f5876673b73c68d9efd775410275cb3d2b7ef57",
            "04aed39b34e7f8c24a251bd3de9fa7312b969182a91770da0ce2c7f6885d5d31",
        ],
    ),
];

/// The bundled theme `content` (a `base.html`) is an unedited copy of, in
/// any released version.
pub(crate) fn legacy_bundled_theme(content: &str) -> Option<&'static str> {
    bundled_theme_for_hash(&template_hash(content))
}

/// The bundled theme a `base.html` is an unedited copy of (any version).
fn bundled_theme_for_hash(hash: &str) -> Option<&'static str> {
    LEGACY_THEME_HASHES
        .iter()
        .find(|(_, hashes)| hashes.contains(&hash))
        .map(|(name, _)| *name)
}

/// Replace `templates/base.html` with the current version of its bundled
/// theme when it is an unedited copy of any earlier version, so the site
/// moves to the theme that includes seite's partials (`seite/head.html`,
/// ...) and gets SEO/GEO fixes automatically from then on. An edited copy
/// that does not include the partials is kept, with a one-time notice
/// showing how to adopt them.
fn check_bundled_theme_base(root: &Path) -> (Vec<UpgradeAction>, Vec<Notice>) {
    let template_dir = crate::config::SiteConfig::load(&root.join("seite.toml"))
        .map(|c| c.build.template_dir)
        .unwrap_or_else(|_| "templates".to_string());
    let rel = format!("{template_dir}/base.html");
    let path = root.join(&rel);
    let Ok(content) = fs::read_to_string(&path) else {
        return (Vec::new(), Vec::new());
    };
    let hash = template_hash(&content);
    let current = crate::themes::all()
        .into_iter()
        .find(|t| template_hash(t.base_html) == hash);
    if current.is_some() {
        return (Vec::new(), Vec::new());
    }
    if let Some(theme) = bundled_theme_for_hash(&hash).and_then(crate::themes::by_name) {
        return (
            vec![UpgradeAction::Update {
                path,
                content: theme.base_html.to_string(),
                description: format!(
                    "{rel} ({} theme, now with seite's SEO/search partials)",
                    theme.name
                ),
            }],
            Vec::new(),
        );
    }
    if content.contains("seite/head.html") {
        return (Vec::new(), Vec::new());
    }
    (
        Vec::new(),
        vec![Notice {
            key: format!("theme-partials:{rel}"),
            text: format!(
                "Kept {rel} (edited since seite wrote it). It has its own copy of the SEO head, so it gets no automatic SEO/GEO updates. To adopt seite's partials, replace the SEO tags after <title>, the search box and the search script with:\n{}\nSee `seite docs templates` (section \"seite partials\").",
                crate::templates::SEITE_PARTIALS_SNIPPET
            ),
        }],
    )
}

/// A message `seite upgrade` shows once per project (recorded in
/// `.seite/config.json` `notices_shown`), never under `--check`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Notice {
    key: String,
    text: String,
}

/// AGENTS.md sections seite 0.20 generated for features 0.21 removed (the
/// `/theme-builder`, `/brand-identity` and `/landing-page` skills, the dev
/// server REPL), exactly as 0.20.x wrote them (`src/scaffold/legacy/`), with
/// whether each is a skill section (replaced by the `/seite` pointer).
const LEGACY_SECTIONS: &[(&str, bool)] = &[
    (include_str!("../scaffold/legacy/repl.md"), false),
    (
        include_str!("../scaffold/legacy/landing-page-builder.md"),
        true,
    ),
    (include_str!("../scaffold/legacy/theme-builder.md"), true),
    (include_str!("../scaffold/legacy/brand-identity.md"), true),
];

/// The line that replaces the removed skill sections.
const SEITE_DESIGN_POINTER: &str = "Design workflows: `/seite theme`, `/seite brand`, `/seite landing` (Codex: `$seite theme|brand|landing`) replace the old `/theme-builder`, `/brand-identity`, and `/landing-page` skills.";

fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The line range `[start, end)` of the section headed by `heading` (a
/// markdown heading line): up to the next heading of the same or a higher
/// level, outside code fences.
fn section_range(lines: &[&str], heading: &str) -> Option<(usize, usize)> {
    let level = heading.chars().take_while(|c| *c == '#').count();
    let start = lines.iter().position(|l| l.trim_end() == heading)?;
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if !in_fence && hashes > 0 && hashes <= level && line[hashes..].starts_with(' ') {
            return Some((start, i));
        }
    }
    Some((start, lines.len()))
}

/// Remove [`LEGACY_SECTIONS`] still exactly as seite wrote them (whitespace
/// aside) from project instructions, putting [`SEITE_DESIGN_POINTER`] where
/// the first skill section was (unless `/seite brand` is already mentioned).
/// Returns the new content (`None` when nothing is removed) and the headings
/// of legacy sections the user edited (kept).
fn prune_legacy_sections(content: &str) -> (Option<String>, Vec<String>) {
    let mut text = content.to_string();
    let mut edited = Vec::new();
    let mut changed = false;
    let mut pointer_needed = !content.contains("/seite brand");
    for (legacy, is_skill) in LEGACY_SECTIONS {
        let heading = legacy.lines().next().unwrap_or_default().trim_end();
        let lines: Vec<&str> = text.lines().collect();
        let Some((start, end)) = section_range(&lines, heading) else {
            continue;
        };
        if normalize_ws(&lines[start..end].join("\n")) != normalize_ws(legacy) {
            edited.push(heading.trim_start_matches('#').trim().to_string());
            continue;
        }
        let mut out: Vec<&str> = lines[..start].to_vec();
        if *is_skill && pointer_needed {
            out.push(SEITE_DESIGN_POINTER);
            out.push("");
            pointer_needed = false;
        }
        out.extend_from_slice(&lines[end..]);
        let trailing_newline = text.ends_with('\n');
        text = out.join("\n");
        if trailing_newline {
            text.push('\n');
        }
        changed = true;
    }
    (changed.then_some(text), edited)
}

/// Remove 0.20's legacy AGENTS.md sections (see [`prune_legacy_sections`]);
/// edited ones are kept with a one-time notice.
fn check_legacy_sections(root: &Path) -> (Vec<UpgradeAction>, Vec<Notice>) {
    let path = project_instructions_path(root);
    let Ok(content) = fs::read_to_string(&path) else {
        return (vec![], vec![]);
    };
    let (pruned, edited) = prune_legacy_sections(&content);
    let notices = edited
        .into_iter()
        .map(|heading| Notice {
            key: format!("kept-legacy-section:{heading}"),
            text: format!(
                "Kept the \"{heading}\" section in AGENTS.md (edited since seite wrote it); it describes a feature seite no longer ships (`/seite` covers it), so remove it when you no longer need it."
            ),
        })
        .collect();
    let actions = match pruned {
        Some(_) => vec![UpgradeAction::PruneLegacySections {
            root: root.to_path_buf(),
            description: "Project instructions (removed sections for the retired /theme-builder, /brand-identity, /landing-page skills and the serve REPL)".into(),
        }],
        None => vec![],
    };
    (actions, notices)
}

/// Bring `.seite/docs/` up to date (see [`harness::local_docs_plan`]): write
/// missing and (for another seite version's docs) changed pages, delete
/// pages this version doesn't ship.
fn check_local_docs(root: &Path) -> Vec<UpgradeAction> {
    let (files, removes) = harness::local_docs_plan(root);
    let mut actions: Vec<UpgradeAction> = removes
        .into_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            UpgradeAction::Remove {
                path,
                description: format!("{rel} (no longer part of seite's docs)"),
            }
        })
        .collect();
    let pages = files
        .iter()
        .filter(|(p, _)| p.extension().is_some_and(|e| e == "md"))
        .count();
    if !files.is_empty() {
        let version = meta::format_version(meta::binary_version());
        let description = if pages == 0 {
            format!(
                "{}/ (version stamp for seite {version})",
                harness::LOCAL_DOCS_DIR
            )
        } else {
            format!(
                "{}/ ({} doc page{} for seite {version})",
                harness::LOCAL_DOCS_DIR,
                pages,
                if pages == 1 { "" } else { "s" },
            )
        };
        actions.push(UpgradeAction::WriteFiles { description, files });
    }
    actions
}

/// Add seite's machine-local paths (`.seite/docs/`, `.seite/dev.json`) to
/// `.gitignore` when missing (add-only; created if there is none).
fn check_gitignore_local_state(root: &Path) -> Vec<UpgradeAction> {
    let path = root.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let present = |entry: &str| {
        let bare = entry.trim_start_matches('/').trim_end_matches('/');
        existing.lines().any(|l| {
            let l = l.trim();
            l.trim_start_matches('/').trim_end_matches('/') == bare
        })
    };
    let missing: Vec<&str> = harness::GITIGNORE_LOCAL_STATE
        .iter()
        .copied()
        .filter(|e| !present(e))
        .collect();
    if missing.is_empty() {
        return vec![];
    }
    let mut content = String::new();
    if !existing.is_empty() && !existing.ends_with('\n') {
        content.push('\n');
    }
    for entry in &missing {
        content.push_str(entry);
        content.push('\n');
    }
    vec![UpgradeAction::Append {
        path,
        content,
        description: format!(".gitignore ({})", missing.join(", ")),
    }]
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
    let template_dir = crate::config::SiteConfig::load(&root.join("seite.toml"))
        .map(|c| c.build.template_dir)
        .unwrap_or_else(|_| "templates".to_string());
    let base_html = root.join(template_dir).join("base.html");
    if !base_html.exists() {
        return vec![]; // Using bundled theme — auto-upgraded
    }

    let content = match fs::read_to_string(&base_html) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Skip if already has Atom autodiscovery (directly or via seite's head
    // partial), or is an unedited bundled theme that `check_bundled_theme_base`
    // replaces with the current, partial-based version.
    if content.contains("application/atom+xml")
        || content.contains("seite/head.html")
        || legacy_bundled_theme(&content).is_some()
    {
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

/// Keep AGENTS.md's per-agent setup table and rules index in sync with the
/// agent selection (the table shows MCP config when `mcp`).
fn check_instructions_blocks(root: &Path, agents: &[Agent], mcp: bool) -> Vec<UpgradeAction> {
    let path = project_instructions_path(root);
    let Ok(content) = fs::read_to_string(&path) else {
        return vec![];
    };
    let features = SiteFeatures::detect(root);
    let blocks = [
        (
            InstructionsBlock {
                id: harness::MCP_SETUP_BLOCK,
                body: harness::mcp_setup_table(agents, mcp),
                heading: "## Agent setup",
                skip_if_contains: None,
            },
            "Project instructions (per-agent setup table)",
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

/// Replace `old` in an allow list (only an exact match, i.e. the rule seite
/// generated) with `new` at the same position, skipping rules already there.
/// `true` when `old` was found.
fn replace_rule(allow: &mut Vec<serde_json::Value>, old: &str, new: &[&str]) -> bool {
    let Some(pos) = allow.iter().position(|v| v == old) else {
        return false;
    };
    allow.remove(pos);
    let mut at = pos;
    for rule in new {
        if !allow.iter().any(|v| v == rule) {
            allow.insert(at, serde_json::json!(rule));
            at += 1;
        }
    }
    true
}

/// Create `.cursor/cli.json`, or add seite's allow rules to an existing one
/// (other rules, and any deny list, are kept). `Mcp(seite:*)` only with `mcp`.
fn check_cursor_cli_json(root: &Path, mcp: bool) -> Vec<UpgradeAction> {
    let rel = ".cursor/cli.json";
    let path = root.join(rel);
    if !path.exists() {
        return vec![UpgradeAction::Create {
            path,
            content: harness::pretty_json(&harness::cursor_cli_json(mcp)),
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
    let narrow = harness::cursor_shell_allow();
    let narrow_refs: Vec<&str> = narrow.iter().map(String::as_str).collect();
    if replace_rule(allow, harness::CURSOR_LEGACY_SHELL_ALLOW, &narrow_refs) {
        additions.push(format!(
            "Narrowed {} to the allowed seite subcommands in {rel}",
            harness::CURSOR_LEGACY_SHELL_ALLOW
        ));
    }
    let mcp_rule = mcp.then(|| harness::CURSOR_MCP_ALLOW.to_string());
    for rule in mcp_rule.into_iter().chain(harness::cursor_allow()) {
        if !allow.iter().any(|v| *v == rule) {
            additions.push(format!("Added {rule} to {rel}"));
            allow.push(serde_json::json!(rule));
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

/// Create `opencode.json`, or merge into an existing one: the permission
/// block only when there's no `permission` key, and (with `mcp`) `mcp.seite`
/// if missing. Every other key is kept.
fn check_opencode_json(root: &Path, mcp: bool) -> Vec<UpgradeAction> {
    let path = root.join("opencode.json");
    if !path.exists() {
        if root.join("opencode.jsonc").exists() {
            human::warning(
                "opencode.jsonc exists; add seite's permissions to it manually (see seite.sh/docs/agent)",
            );
            return vec![];
        }
        return vec![UpgradeAction::Create {
            path,
            content: harness::pretty_json(&harness::opencode_json(mcp)),
            description: "opencode.json (permissions for OpenCode)".into(),
        }];
    }
    let Some(mut json) = read_json_object(&path) else {
        human::warning(
            "opencode.json isn't plain JSON (comments?); add seite's permissions to it manually",
        );
        return vec![];
    };
    let mut additions = Vec::new();
    if mcp {
        let mcp_block = json
            .entry("mcp".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if let Some(mcp_block) = mcp_block.as_object_mut() {
            if !mcp_block.contains_key(harness::MCP_SERVER) {
                mcp_block.insert(
                    harness::MCP_SERVER.to_string(),
                    harness::opencode_mcp_entry(),
                );
                additions.push("Added mcp.seite to opencode.json".to_string());
            }
        }
    }
    if !json.contains_key("permission") {
        json.insert("permission".to_string(), harness::opencode_permission());
        additions.push("Added seite permission defaults to opencode.json".to_string());
    } else if let Some(bash) = json
        .get_mut("permission")
        .and_then(|p| p.get_mut("bash"))
        .and_then(serde_json::Value::as_object_mut)
    {
        // seite 0.20's `"seite theme*": "allow"` also allowed `theme create`
        // (a nested agent session): swap it, if untouched, for the narrow rules.
        if bash.get(harness::OPENCODE_LEGACY_THEME_RULE) == Some(&serde_json::json!("allow")) {
            let entries: Vec<(String, serde_json::Value)> =
                std::mem::take(bash).into_iter().collect();
            for (key, value) in entries {
                if key == harness::OPENCODE_LEGACY_THEME_RULE {
                    for rule in harness::OPENCODE_THEME_RULES {
                        bash.insert((*rule).to_string(), serde_json::json!("allow"));
                    }
                } else {
                    bash.insert(key, value);
                }
            }
            additions.push(format!(
                "Narrowed \"{}\" to `seite theme apply|list` in opencode.json",
                harness::OPENCODE_LEGACY_THEME_RULE
            ));
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

/// Create `.codex/config.toml`, or add `[mcp_servers.seite]` to an existing
/// one with a comment-preserving edit (only with `--mcp`).
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

/// Create an instructions shim (`CLAUDE.md`, `GEMINI.md`) that imports
/// AGENTS.md, for an agent that needs one, when the project has none.
fn check_instructions_shim(root: &Path, agent: Agent) -> Vec<UpgradeAction> {
    let Some(name) = agent.spec().instructions_shim else {
        return vec![];
    };
    let shim = root.join(name);
    if shim.exists() || !root.join("AGENTS.md").exists() {
        return vec![];
    }
    vec![UpgradeAction::Create {
        path: shim,
        content: crate::cli::agent_instructions::CLAUDE_SHIM.to_string(),
        description: format!("{name} (@AGENTS.md import for {})", agent.label()),
    }]
}

/// Every harness file the selected agents need, merged into what exists.
/// MCP config is only added or changed with `mcp`.
fn check_agent_harness(root: &Path, agents: &[Agent], mcp: bool) -> Vec<UpgradeAction> {
    let features = SiteFeatures::detect(root);
    let mut actions = Vec::new();

    for &agent in agents {
        actions.extend(check_instructions_shim(root, agent));
    }
    if agents.contains(&Agent::Claude) {
        actions.extend(check_claude_config(root, mcp));
        actions.extend(check_claude_rules(root));
        actions.extend(check_launch_json(root));
        for skill in harness::skills(features) {
            actions.extend(check_skill(root, harness::CLAUDE_SKILLS_DIR, skill));
        }
    }
    if agents.contains(&Agent::Cursor) {
        if mcp {
            actions.extend(check_mcp_servers_json(root, ".cursor/mcp.json", "Cursor"));
        }
        actions.extend(check_cursor_cli_json(root, mcp));
        actions.extend(missing_rules(
            root,
            ".cursor/rules",
            "mdc",
            harness::cursor_rule,
        ));
    }
    if agents.contains(&Agent::Codex) && mcp {
        actions.extend(check_codex_config(root));
    }
    if agents.contains(&Agent::Opencode) {
        actions.extend(check_opencode_json(root, mcp));
    }
    if agents.contains(&Agent::Gemini) && mcp {
        actions.extend(check_mcp_servers_json(
            root,
            ".gemini/settings.json",
            "Gemini CLI",
        ));
    }
    actions.extend(check_command_files(root, agents));
    if harness::uses_shared_skills(agents) {
        for skill in harness::skills(features) {
            actions.extend(check_skill(root, harness::SHARED_SKILLS_DIR, skill));
        }
    }
    if harness::uses_shared_rules(agents) {
        actions.extend(missing_rules(
            root,
            ".agents/rules",
            "md",
            harness::claude_rule,
        ));
    }
    actions.extend(check_local_docs(root));
    actions.extend(check_gitignore_local_state(root));
    let show_mcp = mcp || harness::mcp_configured(root, agents);
    actions.extend(check_instructions_blocks(root, agents, show_mcp));
    actions
}

/// Hooks for each selected agent, tier by tier. A tier not recorded in
/// `installed` (`.seite/config.json` `hooks_installed`: `<agent>` = turn-end,
/// `<agent>:post-edit` = after-edit) is added and recorded; a recorded tier is
/// never re-added, so a hook the user deleted (or turned off) stays deleted.
/// Recorded tiers still in place get refreshed (the missing-binary wrapper).
/// Projects from seite 0.20 record only the turn-end tier: they get the
/// after-edit tier only while their turn-end hook is still in place.
/// Returns the new record. Tiers whose config can't be merged are left
/// unrecorded so a later upgrade retries them.
fn add_stop_hooks(
    root: &Path,
    agents: &[Agent],
    installed: &[String],
    actions: &mut Vec<UpgradeAction>,
) -> Vec<String> {
    use crate::cli::harness_hooks::{installed_hooks, record_tier, tier_recorded, HookKind};

    let mut record = installed.to_vec();
    for &agent in agents {
        let stop_recorded = tier_recorded(installed, agent, HookKind::Stop);
        let mut kinds: Vec<HookKind> = Vec::new();
        if !stop_recorded {
            kinds.push(HookKind::Stop);
        }
        if !tier_recorded(installed, agent, HookKind::PostEdit)
            && (!stop_recorded || installed_hooks(root, agent).0)
        {
            kinds.push(HookKind::PostEdit);
        }
        if stop_recorded {
            refresh_hooks(root, agent, actions);
        }
        if kinds.is_empty() {
            continue;
        }
        if add_hook_tiers(root, agent, &kinds, actions) {
            for kind in kinds {
                record_tier(&mut record, agent, kind);
            }
        }
    }
    record
}

/// Rewrite 0.20's bare hook commands of a recorded agent to the wrapped form.
fn refresh_hooks(root: &Path, agent: Agent, actions: &mut Vec<UpgradeAction>) {
    use crate::cli::harness_hooks::{hook_path, merge_hook_kinds};
    if agent == Agent::Opencode {
        return;
    }
    let path = root.join(hook_path(agent));
    if actions
        .iter()
        .any(|a| a.replaced_path() == Some(path.as_path()))
    {
        return;
    }
    let Some(mut doc) = read_json_object(&path) else {
        return;
    };
    // Merging tiers that are already present only rewrites bare commands.
    let (stop, post_edit) = crate::cli::harness_hooks::installed_hooks(root, agent);
    let present: Vec<_> = [
        (stop, crate::cli::harness_hooks::HookKind::Stop),
        (post_edit, crate::cli::harness_hooks::HookKind::PostEdit),
    ]
    .into_iter()
    .filter_map(|(on, kind)| on.then_some(kind))
    .collect();
    if let Ok(true) = merge_hook_kinds(agent, &mut doc, &present) {
        actions.push(UpgradeAction::MergeJson {
            path,
            merged: serde_json::Value::Object(doc),
            additions: vec![format!(
                "Updated the seite check hooks in {}",
                hook_path(agent)
            )],
        });
    }
}

/// Queue `agent`'s `kinds` hooks: merged into a pending action on the same
/// file (e.g. the `.claude/settings.json` the settings step creates), else
/// created or merged on disk. `false` when the file can't be merged safely.
fn add_hook_tiers(
    root: &Path,
    agent: Agent,
    kinds: &[crate::cli::harness_hooks::HookKind],
    actions: &mut Vec<UpgradeAction>,
) -> bool {
    use crate::cli::harness_hooks::{
        hook_command, hook_path, is_outdated_opencode_plugin, merge_hook_kinds, opencode_plugin,
        HookKind,
    };
    let merge_hook = |agent, doc: &mut serde_json::Map<String, serde_json::Value>| {
        merge_hook_kinds(agent, doc, kinds)
    };

    let rel = hook_path(agent);
    let path = root.join(rel);
    let added = format!("Added the seite check hooks to {rel}");
    let unmergeable = || {
        human::warning(&format!(
            "{rel} can't be merged automatically; add a {} hook running `{}` by hand",
            agent.label(),
            hook_command(agent, HookKind::Stop)
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
                            description.push_str(" + seite check hooks");
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

    if agent == Agent::Opencode {
        // One plugin file holds both tiers.
        let existing = fs::read_to_string(&path).ok();
        if existing.is_none() && fs::symlink_metadata(&path).is_err() {
            actions.push(UpgradeAction::Create {
                path,
                content: opencode_plugin().to_string(),
                description: format!("{rel} (seite check hooks for {})", agent.label()),
            });
        } else if kinds.contains(&HookKind::PostEdit)
            && existing.is_some_and(|t| is_outdated_opencode_plugin(&t))
        {
            actions.push(UpgradeAction::Create {
                path,
                content: opencode_plugin().to_string(),
                description: format!("{rel} (adds the after-edit check)"),
            });
        }
        // Otherwise a plugin file already sits at our path; leave it alone.
        return true;
    }
    if !path.exists() {
        let mut doc = serde_json::Map::new();
        let _ = merge_hook(agent, &mut doc);
        actions.push(UpgradeAction::Create {
            path,
            content: pretty_json(&serde_json::Value::Object(doc)),
            description: format!("{rel} (seite check hooks for {})", agent.label()),
        });
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
    fn note(v: (u64, u64, u64), body: &str) -> ReleaseNote {
        ReleaseNote {
            version: v,
            title: format!("v{}", meta::format_version(v)),
            body: body.to_string(),
        }
    }

    fn entries() -> Vec<ReleaseNote> {
        // Newest first, like the embedded changelog.
        vec![
            note((0, 22, 0), "## Breaking\n\nRename `x` to `y`."),
            note((0, 21, 1), "fix"),
            note((0, 21, 0), "feature"),
            note((0, 20, 0), "old"),
        ]
    }

    fn site(from: (u64, u64, u64), changes: &[&str]) -> SiteOutcome {
        SiteOutcome {
            name: None,
            from,
            changes: changes.iter().map(|c| c.to_string()).collect(),
            notices: Vec::new(),
        }
    }

    #[test]
    fn test_select_notes_range_is_exclusive_of_from_and_inclusive_of_to() {
        let all = entries();
        let (notes, unknown) = select_notes(&all, (0, 20, 0), (0, 21, 1));
        let versions: Vec<_> = notes.iter().map(|n| n.version).collect();
        assert_eq!(versions, vec![(0, 21, 0), (0, 21, 1)]);
        assert!(!unknown);
    }

    #[test]
    fn test_select_notes_up_to_date_is_empty() {
        let all = entries();
        assert!(select_notes(&all, (0, 22, 0), (0, 22, 0)).0.is_empty());
        assert!(select_notes(&all, (0, 23, 0), (0, 22, 0)).0.is_empty());
    }

    #[test]
    fn test_select_notes_unknown_version_gives_everything_up_to_binary() {
        let all = entries();
        let (notes, unknown) = select_notes(&all, (0, 0, 0), (0, 21, 1));
        assert!(unknown);
        let versions: Vec<_> = notes.iter().map(|n| n.version).collect();
        assert_eq!(versions, vec![(0, 20, 0), (0, 21, 0), (0, 21, 1)]);
    }

    #[test]
    fn test_unknown_version_notes_are_capped_keeping_newest() {
        let big = "x".repeat(20 * 1024);
        let all = vec![
            note((0, 22, 0), &big),
            note((0, 21, 0), &big),
            note((0, 20, 0), &big),
        ];
        let brief = build_brief(&[site((0, 0, 0), &[])], &all, (0, 22, 0)).unwrap();
        assert!(brief.contains("### v0.22.0"));
        assert!(brief.contains("### v0.21.0"));
        assert!(
            !brief.contains("### v0.20.0"),
            "oldest dropped over the cap"
        );
        assert!(brief.contains("omitted for size"));
    }

    #[test]
    fn test_launch_argv_is_short_and_never_auto_approves() {
        use crate::cli::agent::Harness;
        let brief = "SECRET BRIEF TEXT | & > \"quoted\"\nline two";
        assert!(!LAUNCH_PROMPT.contains('\n'));
        assert!(LAUNCH_PROMPT.len() < 120);
        for h in [
            Harness::Claude,
            Harness::Codex,
            Harness::Opencode,
            Harness::Cursor,
            Harness::Gemini,
        ] {
            let args = h.interactive_args(Some(LAUNCH_PROMPT));
            assert!(args.iter().any(|a| a == LAUNCH_PROMPT), "{}", h.name());
            for a in &args {
                assert!(!a.contains(brief) && !a.contains("SECRET"));
                for bad in ["--auto", "--force", "yolo", "dangerously", "--approve-mcps"] {
                    assert!(!a.contains(bad), "{}: {a}", h.name());
                }
            }
        }
    }

    #[test]
    fn test_demote_headings_caps_at_six_levels() {
        assert_eq!(demote_headings("##### deep", 3), "###### deep");
    }

    #[test]
    fn test_demote_headings_skips_code_fences() {
        let out = demote_headings("## A\n```\n# not a heading\n```\ntext", 2);
        assert_eq!(out, "#### A\n```\n# not a heading\n```\ntext");
    }

    #[test]
    fn test_build_brief_lists_changes_notes_and_instructions() {
        let brief = build_brief(
            &[site((0, 20, 0), &["Update skill"])],
            &entries(),
            (0, 22, 0),
        )
        .unwrap();
        assert!(brief.contains("from v0.20.0 to v0.22.0"));
        assert!(brief.contains("- Update skill"));
        assert!(brief.contains("### v0.21.0"));
        assert!(brief.contains("### v0.22.0"));
        assert!(brief.contains("#### Breaking"));
        assert!(!brief.contains("### v0.20.0"));
        assert!(
            brief.find("### v0.21.0").unwrap() < brief.find("### v0.22.0").unwrap(),
            "oldest first"
        );
        assert!(brief.contains("`seite check`") && brief.contains("`seite build`"));
        assert!(brief.contains("Keep the site's own customizations"));
    }

    #[test]
    fn test_build_brief_unknown_version_says_so() {
        let brief = build_brief(&[site((0, 0, 0), &[])], &entries(), (0, 22, 0)).unwrap();
        assert!(brief.contains("an unknown version"));
        assert!(brief.contains("every release note is included"));
        assert!(brief.contains("### v0.22.0"));
        assert!(brief.contains("### v0.21.1"));
        assert!(brief.contains("### v0.20.0"));
    }

    #[test]
    fn test_build_brief_none_when_nothing_changed_and_no_notes() {
        assert!(build_brief(&[site((0, 22, 0), &[])], &entries(), (0, 22, 0)).is_none());
        assert!(build_brief(&[], &entries(), (0, 22, 0)).is_none());
        // Changes alone are enough.
        assert!(build_brief(&[site((0, 22, 0), &["x"])], &entries(), (0, 22, 0)).is_some());
    }

    #[test]
    fn test_build_brief_workspace_sections_per_site() {
        let mut a = site((0, 21, 0), &["a change"]);
        a.name = Some("blog".into());
        let mut b = site((0, 20, 0), &[]);
        b.name = Some("docs".into());
        let brief = build_brief(&[a, b], &entries(), (0, 22, 0)).unwrap();
        assert!(brief.contains("## Site `blog`"));
        assert!(brief.contains("## Site `docs`"));
        assert!(brief.contains("workspace"));
    }

    #[test]
    fn test_choose_delivery() {
        assert_eq!(choose_delivery(true, false, true), Delivery::Print);
        assert_eq!(choose_delivery(false, true, true), Delivery::Print);
        assert_eq!(choose_delivery(false, false, false), Delivery::Print);
        assert_eq!(choose_delivery(false, false, true), Delivery::Launch);
    }

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
    fn test_check_claude_config_mcp_no_claude_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_claude_config(tmp.path(), true);
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
    fn test_check_claude_config_mcp_migrates_legacy_settings() {
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
        let actions = check_claude_config(tmp.path(), true);
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
        assert!(check_claude_config(tmp.path(), true).is_empty());
    }

    #[test]
    fn test_check_claude_config_mcp_merges_into_existing_mcp_json() {
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
        apply_json_actions(check_claude_config(tmp.path(), true));
        let mcp = read_json(&tmp.path().join(".mcp.json"));
        assert_eq!(mcp["mcpServers"]["mine"]["command"], "mine");
        assert_eq!(mcp["mcpServers"]["seite"]["command"], "seite");
        assert!(check_claude_config(tmp.path(), true).is_empty());
    }

    #[test]
    fn test_check_claude_config_mcp_keeps_legacy_servers_if_mcp_json_malformed() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join(".mcp.json"), "not json").unwrap();
        write_settings(
            tmp.path(),
            serde_json::json!({ "mcpServers": { "other": { "command": "other" } } }),
        );
        apply_json_actions(check_claude_config(tmp.path(), true));
        assert_eq!(
            fs::read_to_string(tmp.path().join(".mcp.json")).unwrap(),
            "not json"
        );
        let settings = read_json(&tmp.path().join(".claude/settings.json"));
        assert_eq!(settings["mcpServers"]["other"]["command"], "other");
    }

    #[test]
    fn test_check_claude_config_mcp_fresh_init_is_noop() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".mcp.json"),
            serde_json::to_string_pretty(&crate::cli::harness::mcp_json()).unwrap(),
        )
        .unwrap();
        write_settings(tmp.path(), crate::cli::harness::claude_settings(true));
        assert!(check_claude_config(tmp.path(), true).is_empty());
    }

    #[test]
    fn test_check_claude_config_mcp_malformed_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".mcp.json"),
            serde_json::to_string_pretty(&crate::cli::harness::mcp_json()).unwrap(),
        )
        .unwrap();
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("settings.json"), "not json").unwrap();
        let actions = check_claude_config(tmp.path(), true);
        assert!(actions.is_empty()); // malformed JSON, don't touch
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
        assert!(check_opencode_json(tmp.path(), true).is_empty());

        // opencode.jsonc is never shadowed by a new opencode.json
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("opencode.jsonc"), "{}").unwrap();
        assert!(check_opencode_json(tmp.path(), true).is_empty());
    }

    #[test]
    fn test_check_opencode_json_merges_and_is_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("opencode.json"), r#"{"theme":"dark"}"#).unwrap();
        apply_json_actions(check_opencode_json(tmp.path(), true));
        let json = read_json(&tmp.path().join("opencode.json"));
        assert_eq!(json["theme"], "dark");
        assert_eq!(json["mcp"]["seite"]["type"], "local");
        assert_eq!(json["permission"]["bash"]["*"], "ask");
        assert!(check_opencode_json(tmp.path(), true).is_empty());
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
        apply_json_actions(check_cursor_cli_json(tmp.path(), true));
        let json = read_json(&tmp.path().join(".cursor/cli.json"));
        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow[0], "Shell(git)");
        assert!(allow.iter().any(|v| v == "Mcp(seite:*)"));
        assert_eq!(json["permissions"]["deny"][0], "Shell(rm)");
        assert_eq!(json["editor"]["vimMode"], true);
        assert!(check_cursor_cli_json(tmp.path(), true).is_empty());
    }

    #[test]
    fn test_check_cursor_cli_json_creates_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = check_cursor_cli_json(tmp.path(), true);
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
        let mut actions = check_claude_config(tmp.path(), true);
        let before = actions.len();
        let record = add_stop_hooks(tmp.path(), &[Agent::Claude], &[], &mut actions);
        assert_eq!(record, ["claude", "claude:post-edit"]);
        // Merged into the MCP step's pending settings.json action, not a second one.
        assert_eq!(actions.len(), before);
        apply_json_actions(actions);
        let settings = read_json(&tmp.path().join(".claude/settings.json"));
        assert_eq!(
            stop_command(&settings),
            crate::cli::harness_hooks::hook_command(
                Agent::Claude,
                crate::cli::harness_hooks::HookKind::Stop
            )
        );
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
        let mut actions = check_claude_config(tmp.path(), true);
        add_stop_hooks(tmp.path(), &[Agent::Claude], &[], &mut actions);
        let described: Vec<String> = actions.iter().flat_map(|a| a.describe()).collect();
        assert!(
            described.iter().any(|d| d.contains("seite check hooks")),
            "{described:?}"
        );
        apply_json_actions(actions);
        let settings = read_json(&tmp.path().join(".claude/settings.json"));
        assert_eq!(
            stop_command(&settings),
            crate::cli::harness_hooks::hook_command(
                Agent::Claude,
                crate::cli::harness_hooks::HookKind::Stop
            )
        );
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
        assert_eq!(record, ["cursor", "cursor:post-edit"]);
        apply_json_actions(actions);
        let hooks = read_json(&root.join(".cursor/hooks.json"));
        assert_eq!(hooks["hooks"]["afterFileEdit"][0]["command"], "fmt");
        assert_eq!(
            hooks["hooks"]["stop"][0]["command"],
            crate::cli::harness_hooks::hook_command(
                Agent::Cursor,
                crate::cli::harness_hooks::HookKind::Stop
            )
        );

        // Already present: recorded without a change.
        let mut actions = Vec::new();
        let record = add_stop_hooks(root, &[Agent::Cursor], &[], &mut actions);
        assert!(actions.is_empty());
        assert_eq!(record, ["cursor", "cursor:post-edit"]);
    }

    #[test]
    fn test_check_default_templates_refreshes_untouched_post_template() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("templates")).unwrap();
        // DEFAULT_POST as v0.20.x shipped it (f7a5557..0dc5dbc).
        let legacy = include_str!("../../tests/fixtures/legacy-post-0.20.html");
        assert!(DEFAULT_TEMPLATES[0]
            .hashes
            .contains(&template_hash(legacy).as_str()));
        fs::write(
            root.join("templates/post.html"),
            legacy.replace('\n', "\r\n"),
        )
        .unwrap();
        let (actions, notices) = check_default_templates(root);
        assert!(notices.is_empty());
        match actions.as_slice() {
            [UpgradeAction::Update { content, .. }] => {
                assert_eq!(content, crate::templates::DEFAULT_POST)
            }
            other => panic!("expected one update, got {}", other.len()),
        }

        // Current default: nothing to do.
        fs::write(
            root.join("templates/post.html"),
            crate::templates::DEFAULT_POST,
        )
        .unwrap();
        let (actions, notices) = check_default_templates(root);
        assert!(actions.is_empty() && notices.is_empty());

        // Edited copy: kept, with a notice.
        fs::write(
            root.join("templates/post.html"),
            format!("{legacy}<!-- mine -->"),
        )
        .unwrap();
        let (actions, notices) = check_default_templates(root);
        assert!(actions.is_empty());
        assert_eq!(notices.len(), 1);
        assert!(notices[0].text.contains("templates/post.html"));
    }

    #[test]
    fn current_default_templates_are_in_hash_table() {
        for t in DEFAULT_TEMPLATES {
            let hash = template_hash(t.current);
            assert_eq!(
                t.hashes.last(),
                Some(&hash.as_str()),
                "templates/{} changed: append \"{hash}\" to its DEFAULT_TEMPLATES hashes",
                t.file
            );
            assert!(t.hashes.iter().all(|h| h.len() == 64));
            // An edited copy of the current default needs no notice.
            assert!(
                t.has_change.iter().any(|m| t.current.contains(m)),
                "{}",
                t.file
            );
        }
        let files: Vec<&str> = DEFAULT_TEMPLATES.iter().map(|t| t.file).collect();
        assert_eq!(files, ["post.html", "index.html", "trust-index.html"]);
    }

    #[test]
    fn test_check_default_templates_refreshes_untouched_index_templates() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("templates")).unwrap();
        // DEFAULT_INDEX / DEFAULT_TRUST_INDEX as v0.20.x shipped them.
        let index = include_str!("../../tests/fixtures/legacy-index-0.20.html");
        let trust = include_str!("../../tests/fixtures/legacy-trust-index-0.20.html");
        fs::write(root.join("templates/index.html"), index).unwrap();
        fs::write(root.join("templates/trust-index.html"), trust).unwrap();
        let (actions, notices) = check_default_templates(root);
        assert!(notices.is_empty());
        let updated: Vec<(String, &str)> = actions
            .iter()
            .map(|a| match a {
                UpgradeAction::Update { path, content, .. } => (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    content.as_str(),
                ),
                _ => panic!("expected updates"),
            })
            .collect();
        assert_eq!(
            updated,
            [
                ("index.html".to_string(), crate::templates::DEFAULT_INDEX),
                (
                    "trust-index.html".to_string(),
                    crate::templates::DEFAULT_TRUST_INDEX
                ),
            ]
        );

        // Edited copies: kept, one notice each.
        fs::write(
            root.join("templates/index.html"),
            format!("{index}<!-- mine -->"),
        )
        .unwrap();
        fs::write(
            root.join("templates/trust-index.html"),
            format!("{trust}<!-- mine -->"),
        )
        .unwrap();
        let (actions, notices) = check_default_templates(root);
        assert!(actions.is_empty());
        assert_eq!(notices.len(), 2);
        assert!(notices[0].text.contains("templates/index.html"));
        assert!(notices[1].text.contains("templates/trust-index.html"));

        // Edited copies that already carry the change: silent.
        fs::write(
            root.join("templates/index.html"),
            format!("{}<!-- mine -->", crate::templates::DEFAULT_INDEX),
        )
        .unwrap();
        fs::remove_file(root.join("templates/trust-index.html")).unwrap();
        let (actions, notices) = check_default_templates(root);
        assert!(actions.is_empty() && notices.is_empty());
    }

    #[test]
    fn test_legacy_body_hash_matches_every_rendering() {
        let theme_builder = include_str!("../../tests/fixtures/legacy-skills/theme-builder-v2.md");
        let design = include_str!("../../tests/fixtures/legacy-skills/design-prompts.md");
        assert!(is_unmodified_legacy("theme-builder", theme_builder));
        // Portable vs Claude frontmatter, CRLF line endings: same body.
        let body = theme_builder.split_once("\n---\n").unwrap().1;
        let other_fm = format!("---\nname: theme-builder\ndescription: x\n---\n{body}");
        assert!(is_unmodified_legacy("theme-builder", &other_fm));
        assert!(is_unmodified_legacy(
            "theme-builder",
            &theme_builder.replace('\n', "\r\n")
        ));
        // Rules: Claude `paths:` and Cursor MDC frontmatter.
        assert!(is_unmodified_legacy(
            "design-prompts",
            &format!("---\npaths:\n  - \"templates/**\"\n---\n{design}")
        ));
        assert!(is_unmodified_legacy(
            "design-prompts",
            &format!("---\ndescription: \"d\"\nglobs: \"templates/**\"\nalwaysApply: false\n---\n{design}")
        ));
        // Edited, or the wrong name: not ours to delete.
        assert!(!is_unmodified_legacy(
            "theme-builder",
            &theme_builder.replace("Phase 1", "Step 1")
        ));
        assert!(!is_unmodified_legacy("brand-identity", theme_builder));
        for (_, hashes) in LEGACY_BODY_HASHES {
            assert!(hashes.iter().all(|h| h.len() == 64));
        }
    }

    #[test]
    fn test_check_claude_config_without_mcp_leaves_mcp_alone() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            serde_json::json!({
                "permissions": { "allow": ["Read"] },
                "mcpServers": { "seite": { "command": "seite", "args": ["mcp"] } }
            }),
        );
        let actions = check_claude_config(tmp.path(), false);
        assert!(actions
            .iter()
            .all(|a| a.replaced_path() != Some(tmp.path().join(".mcp.json").as_path())));
        apply_json_actions(actions);
        let settings = read_json(&tmp.path().join(".claude/settings.json"));
        assert!(settings.get("mcpServers").is_some(), "legacy block kept");
        assert!(settings.get("enabledMcpjsonServers").is_none());
        let allow = settings["permissions"]["allow"].as_array().unwrap();
        assert!(!allow.iter().any(|v| v == "mcp__seite"));
        assert!(allow.iter().any(|v| v == "Bash(seite context)"));
        assert!(!tmp.path().join(".mcp.json").exists());
    }

    // --- bundled themes → seite partials ---

    #[test]
    fn current_bundled_themes_are_in_legacy_table() {
        for theme in crate::themes::all() {
            let hash = template_hash(theme.base_html);
            let listed = LEGACY_THEME_HASHES
                .iter()
                .find(|(name, _)| *name == theme.name)
                .map(|(_, hashes)| hashes.contains(&hash.as_str()))
                .unwrap_or(false);
            assert!(
                listed,
                "src/themes/{name}.tera changed: append \"{hash}\" to the \"{name}\" list in \
                 LEGACY_THEME_HASHES (src/cli/upgrade.rs), so sites with the previous version \
                 keep upgrading",
                name = theme.name
            );
        }
    }

    #[test]
    fn test_legacy_theme_hashes_are_unique_across_themes() {
        let mut seen = std::collections::HashSet::new();
        let mut count = 0;
        for (name, hashes) in LEGACY_THEME_HASHES {
            assert!(crate::themes::by_name(name).is_some(), "{name}");
            for hash in *hashes {
                assert_eq!(hash.len(), 64, "{name}: {hash}");
                assert!(seen.insert(*hash), "{hash} listed twice");
                count += 1;
            }
        }
        // Every version of every bundled theme since the first release.
        assert!(count >= 185, "{count}");
    }

    fn write_base(root: &Path, content: &str) {
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("templates/base.html"), content).unwrap();
    }

    #[test]
    fn test_check_bundled_theme_base_replaces_every_unedited_version() {
        // The 0.20.x themes (pre-partials) and dark as 0.2.4 shipped it.
        let mut cases: Vec<(&str, &str)> = vec![(
            "dark",
            include_str!("../../tests/fixtures/legacy-themes/dark-0ef6f5f.tera"),
        )];
        for name in [
            "default",
            "minimal",
            "dark",
            "docs",
            "brutalist",
            "bento",
            "landing",
            "terminal",
            "magazine",
            "academic",
        ] {
            let path = format!(
                "{}/tests/fixtures/themes-pre-partials/{name}.tera",
                env!("CARGO_MANIFEST_DIR")
            );
            let content: &'static str =
                Box::leak(fs::read_to_string(path).unwrap().into_boxed_str());
            cases.push((name, content));
        }
        for (name, legacy) in cases {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path();
            // CRLF checkouts hash the same.
            write_base(root, &legacy.replace('\n', "\r\n"));
            let (actions, notices) = check_bundled_theme_base(root);
            assert!(notices.is_empty(), "{name}");
            match actions.as_slice() {
                [UpgradeAction::Update {
                    content,
                    description,
                    ..
                }] => {
                    assert_eq!(content, crate::themes::by_name(name).unwrap().base_html);
                    assert!(content.contains(r#"{% include "seite/head.html" %}"#));
                    assert!(description.contains(name), "{description}");
                }
                other => panic!("{name}: expected one update, got {}", other.len()),
            }
        }
    }

    #[test]
    fn test_check_bundled_theme_base_keeps_current_and_edited() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // No base.html: nothing to do.
        let (actions, notices) = check_bundled_theme_base(root);
        assert!(actions.is_empty() && notices.is_empty());

        // Current themes (blank included): nothing to do.
        for theme in crate::themes::all() {
            write_base(root, theme.base_html);
            let (actions, notices) = check_bundled_theme_base(root);
            assert!(actions.is_empty() && notices.is_empty(), "{}", theme.name);
        }

        // A custom design that includes the partials: nothing to do.
        write_base(
            root,
            "<html><head>{% include \"seite/head.html\" %}</head><body></body></html>",
        );
        let (actions, notices) = check_bundled_theme_base(root);
        assert!(actions.is_empty() && notices.is_empty());

        // An edited legacy theme: kept, with the adoption snippet.
        let legacy = include_str!("../../tests/fixtures/legacy-themes/dark-0ef6f5f.tera");
        write_base(root, &format!("{legacy}<!-- mine -->"));
        let (actions, notices) = check_bundled_theme_base(root);
        assert!(actions.is_empty());
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].key, "theme-partials:templates/base.html");
        assert!(notices[0].text.contains("Kept templates/base.html"));
        assert!(notices[0]
            .text
            .contains(r#"{% include "seite/head.html" %}"#));
        assert!(notices[0]
            .text
            .contains(r#"{% include "seite/scripts.html" %}"#));
    }

    #[test]
    fn test_atom_hint_skips_partial_based_base() {
        let tmp = tempfile::tempdir().unwrap();
        write_base(tmp.path(), crate::themes::default().base_html);
        assert!(check_atom_autodiscovery_template(tmp.path()).is_empty());
    }

    #[test]
    fn test_atom_hint_skips_unedited_bundled_theme_and_uses_template_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("CLAUDE.md"), "# Project").unwrap();
        // dark as 0.2.4 shipped it has RSS but no Atom link; upgrade replaces
        // it, so no hint about adding Atom by hand.
        let legacy = include_str!("../../tests/fixtures/legacy-themes/dark-0ef6f5f.tera");
        assert!(!legacy.contains("atom"));
        write_base(root, legacy);
        assert!(check_atom_autodiscovery_template(root).is_empty());

        // The configured template_dir is the one checked.
        fs::write(
            root.join("seite.toml"),
            "collections = []\n[site]\ntitle = \"T\"\n\n[build]\ntemplate_dir = \"layouts\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("layouts")).unwrap();
        fs::write(
            root.join("layouts/base.html"),
            r#"<link rel="alternate" type="application/rss+xml" href="/feed.xml">"#,
        )
        .unwrap();
        assert_eq!(check_atom_autodiscovery_template(root).len(), 1);
        fs::remove_file(root.join("layouts/base.html")).unwrap();
        assert!(check_atom_autodiscovery_template(root).is_empty());
    }
}
