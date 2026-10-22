//! `seite hooks on|off|status`: manage the `seite check` hooks seite installs
//! in each coding agent's config (see `cli::harness_hooks`).
//!
//! `on` installs both tiers (post-edit and turn-end) and records the agent in
//! `.seite/config.json` `hooks_installed`; `off` removes only seite's entries
//! and keeps the record, so `seite upgrade` doesn't add them back; `status`
//! reads the config files themselves.

use std::path::Path;

use clap::{Args, Subcommand};
use serde_json::{json, Map, Value};

use crate::cli::harness::{self, Agent};
use crate::cli::harness_hooks::{
    has_seite_hook, hook_path, installed_hooks, merge_hook, opencode_plugin, remove_hooks,
};
use crate::meta;
use crate::output::human;

#[derive(Args)]
pub struct HooksArgs {
    #[command(subcommand)]
    pub command: HooksCommand,
}

#[derive(Subcommand)]
pub enum HooksCommand {
    /// Show which agents have seite's hooks installed
    Status {
        /// Agents to show (comma-separated, or all). Default: the project's agents
        #[arg(long, value_name = "LIST")]
        agents: Option<String>,
    },
    /// Install the post-edit and turn-end `seite check` hooks
    On {
        /// Agents to install for (comma-separated, or all). Default: the project's agents
        #[arg(long, value_name = "LIST")]
        agents: Option<String>,
    },
    /// Remove seite's hooks (other hooks are kept); `seite upgrade` won't re-add them
    Off {
        /// Agents to remove them from (comma-separated, or all). Default: the project's agents
        #[arg(long, value_name = "LIST")]
        agents: Option<String>,
    },
}

/// What happened to one agent's hooks.
#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Installed,
    Removed,
    Unchanged,
    /// The config file exists but can't be merged safely.
    Skipped(String),
}

pub fn run(args: &HooksArgs) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    if !root.join("seite.toml").exists() {
        anyhow::bail!(
            "No seite.toml found in current directory. Run this from a seite project root."
        );
    }
    match &args.command {
        HooksCommand::Status { agents } => {
            let agents = resolve_agents(&root, agents.as_deref())?;
            let rows = status(&root, &agents);
            for (agent, (stop, post)) in &rows {
                let state = match (stop, post) {
                    (true, true) => "installed".to_string(),
                    (false, false) => "missing".to_string(),
                    (stop, post) => format!(
                        "partial (turn-end: {}, after-edit: {})",
                        on_off(*stop),
                        on_off(*post)
                    ),
                };
                human::info(&format!(
                    "{:<12} {state}  ({})",
                    agent.label(),
                    hook_path(*agent)
                ));
            }
            crate::output::json::set_data(json!({
                "agents": rows.iter().map(|(a, (stop, post))| json!({
                    "agent": a.id(),
                    "file": hook_path(*a),
                    "stop": stop,
                    "post_edit": post,
                })).collect::<Vec<_>>()
            }));
        }
        HooksCommand::On { agents } => {
            let agents = resolve_agents(&root, agents.as_deref())?;
            let changes = apply(&root, &agents, true)?;
            report(&changes);
            record_installed(&root, &changes)?;
            set_change_data(&changes);
        }
        HooksCommand::Off { agents } => {
            let agents = resolve_agents(&root, agents.as_deref())?;
            let changes = apply(&root, &agents, false)?;
            report(&changes);
            set_change_data(&changes);
        }
    }
    Ok(())
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

/// `--agents`, else the project's stored selection, else every agent.
fn resolve_agents(root: &Path, flag: Option<&str>) -> anyhow::Result<Vec<Agent>> {
    if let Some(list) = flag {
        return harness::parse_agent_list(list);
    }
    Ok(meta::load(root)
        .and_then(|m| m.agents)
        .map(|ids| harness::from_ids(&ids))
        .filter(|a| !a.is_empty())
        .unwrap_or_else(|| Agent::ALL.to_vec()))
}

/// `(stop, post_edit)` hook state per agent.
pub fn status(root: &Path, agents: &[Agent]) -> Vec<(Agent, (bool, bool))> {
    agents
        .iter()
        .map(|&a| (a, installed_hooks(root, a)))
        .collect()
}

/// Install (`on`) or remove seite's hooks for `agents` under `root`.
pub fn apply(root: &Path, agents: &[Agent], on: bool) -> anyhow::Result<Vec<(Agent, Change)>> {
    let mut changes = Vec::new();
    for &agent in agents {
        let change = if on {
            install(root, agent)?
        } else {
            uninstall(root, agent)?
        };
        changes.push((agent, change));
    }
    Ok(changes)
}

fn install(root: &Path, agent: Agent) -> anyhow::Result<Change> {
    let rel = hook_path(agent);
    let path = root.join(rel);
    if agent == Agent::Opencode {
        let existing = std::fs::read_to_string(&path).ok();
        if existing.as_deref() == Some(opencode_plugin()) {
            return Ok(Change::Unchanged);
        }
        if existing.as_deref().is_some_and(|t| !has_seite_hook(t)) {
            return Ok(Change::Skipped(format!(
                "{rel} exists and isn't seite's plugin"
            )));
        }
        write(&path, opencode_plugin())?;
        return Ok(Change::Installed);
    }
    let mut doc = if path.exists() {
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        {
            Some(Value::Object(doc)) => doc,
            _ => return Ok(Change::Skipped(format!("{rel} is not a JSON object"))),
        }
    } else {
        Map::new()
    };
    match merge_hook(agent, &mut doc) {
        Ok(true) => {
            write(&path, &harness::pretty_json(&Value::Object(doc)))?;
            Ok(Change::Installed)
        }
        Ok(false) => Ok(Change::Unchanged),
        Err(_) => Ok(Change::Skipped(format!(
            "{rel} has a `hooks` value seite can't extend"
        ))),
    }
}

fn uninstall(root: &Path, agent: Agent) -> anyhow::Result<Change> {
    let path = root.join(hook_path(agent));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Change::Unchanged);
    };
    if agent == Agent::Opencode {
        if has_seite_hook(&text) {
            std::fs::remove_file(&path)?;
            return Ok(Change::Removed);
        }
        return Ok(Change::Unchanged);
    }
    let Ok(Value::Object(mut doc)) = serde_json::from_str::<Value>(&text) else {
        return Ok(Change::Unchanged);
    };
    if remove_hooks(&mut doc) {
        std::fs::write(&path, harness::pretty_json(&Value::Object(doc)))?;
        return Ok(Change::Removed);
    }
    Ok(Change::Unchanged)
}

fn write(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

fn report(changes: &[(Agent, Change)]) {
    for (agent, change) in changes {
        let rel = hook_path(*agent);
        match change {
            Change::Installed => {
                human::success(&format!("{}: hooks installed in {rel}", agent.label()))
            }
            Change::Removed => {
                human::success(&format!("{}: hooks removed from {rel}", agent.label()))
            }
            Change::Unchanged => human::info(&format!("{}: nothing to change", agent.label())),
            Change::Skipped(why) => human::warning(&format!("{}: skipped ({why})", agent.label())),
        }
    }
}

fn set_change_data(changes: &[(Agent, Change)]) {
    crate::output::json::set_data(json!({
        "agents": changes.iter().map(|(a, c)| json!({
            "agent": a.id(),
            "file": hook_path(*a),
            "change": match c {
                Change::Installed => "installed",
                Change::Removed => "removed",
                Change::Unchanged => "unchanged",
                Change::Skipped(_) => "skipped",
            },
        })).collect::<Vec<_>>()
    }));
}

/// Record agents whose hooks are now in place in `hooks_installed` (when the
/// project has `.seite/config.json`).
fn record_installed(root: &Path, changes: &[(Agent, Change)]) -> anyhow::Result<()> {
    let Some(mut project) = meta::load(root) else {
        return Ok(());
    };
    let mut ids = project.hooks_installed.take().unwrap_or_default();
    for (agent, change) in changes {
        if matches!(change, Change::Installed | Change::Unchanged)
            && !ids.iter().any(|id| id == agent.id())
        {
            ids.push(agent.id().to_string());
        }
    }
    project.hooks_installed = Some(ids);
    meta::write(root, &project)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_off_status_round_trip_keeps_other_hooks() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".claude")).unwrap();
        std::fs::write(
            root.join(".claude/settings.json"),
            r#"{"permissions":{"allow":["Read"]},"hooks":{"Stop":[{"hooks":[{"type":"command","command":"other"}]}]}}"#,
        )
        .unwrap();

        assert!(status(root, &Agent::ALL)
            .iter()
            .all(|(_, s)| *s == (false, false)));
        let on = apply(root, &Agent::ALL, true).unwrap();
        assert!(on.iter().all(|(_, c)| *c == Change::Installed), "{on:?}");
        assert!(status(root, &Agent::ALL)
            .iter()
            .all(|(_, s)| *s == (true, true)));
        // Idempotent.
        assert!(apply(root, &Agent::ALL, true)
            .unwrap()
            .iter()
            .all(|(_, c)| *c == Change::Unchanged));

        let off = apply(root, &Agent::ALL, false).unwrap();
        assert!(off.iter().all(|(_, c)| *c == Change::Removed), "{off:?}");
        assert!(status(root, &Agent::ALL)
            .iter()
            .all(|(_, s)| *s == (false, false)));
        let settings: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0]["command"], "other");
        assert_eq!(settings["permissions"]["allow"][0], "Read");
        assert!(!root.join(".opencode/plugins/seite-check.js").exists());
    }

    #[test]
    fn install_skips_foreign_or_malformed_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codex")).unwrap();
        std::fs::write(root.join(".codex/hooks.json"), "not json").unwrap();
        assert!(matches!(
            install(root, Agent::Codex).unwrap(),
            Change::Skipped(_)
        ));
        std::fs::create_dir_all(root.join(".opencode/plugins")).unwrap();
        std::fs::write(root.join(".opencode/plugins/seite-check.js"), "// mine").unwrap();
        assert!(matches!(
            install(root, Agent::Opencode).unwrap(),
            Change::Skipped(_)
        ));
        assert_eq!(uninstall(root, Agent::Opencode).unwrap(), Change::Unchanged);
        assert_eq!(uninstall(root, Agent::Gemini).unwrap(), Change::Unchanged);
    }

    #[test]
    fn record_installed_adds_ids_once() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        // No project metadata: nothing recorded, no error.
        record_installed(root, &[(Agent::Codex, Change::Installed)]).unwrap();
        assert!(meta::load(root).is_none());

        meta::write(
            root,
            &meta::PageMeta {
                hooks_installed: Some(vec!["claude".into()]),
                ..meta::PageMeta::current()
            },
        )
        .unwrap();
        record_installed(
            root,
            &[
                (Agent::Claude, Change::Unchanged),
                (Agent::Codex, Change::Installed),
                (Agent::Cursor, Change::Skipped("x".into())),
            ],
        )
        .unwrap();
        assert_eq!(
            meta::load(root).unwrap().hooks_installed.unwrap(),
            ["claude", "codex"]
        );
    }
}
