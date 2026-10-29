use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::cli::harness::{Agent, SkillFrontmatter};
use crate::config::{CollectionConfig, SiteConfig};
use crate::mcp::content_index;
use crate::output::human;

#[derive(Args)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: SkillCommand,
}

#[derive(Subcommand)]
pub enum SkillCommand {
    /// Install a skill pack or individual skill from a URL
    Install {
        /// Known pack name (e.g., "seomachine") or URL to a SKILL.md file
        source: String,

        /// Override the skill name (for URL installs)
        #[arg(long)]
        name: Option<String>,

        /// Coding agents to install a pack for (comma-separated, or `all`).
        /// Default: the agents in `.seite/config.json`, else all
        #[arg(long)]
        agents: Option<String>,
    },

    /// List installed skills and skill packs
    List,

    /// Remove an installed skill pack or individual skill
    Remove {
        /// Pack or skill name to remove
        name: String,
    },

    /// Render the bundled `seite` skill (and check hooks) for distribution:
    /// `skills` writes `<dir>/seite/` for the `skills` CLI; `claude-plugin`
    /// writes a Claude Code plugin into `<dir>`
    #[command(hide = true)]
    Export(crate::cli::skill_export::ExportArgs),

    /// Update installed skills from their original sources
    Update {
        /// Specific pack or skill to update (updates all if omitted)
        name: Option<String>,

        /// Coding agents to update a pack for (comma-separated, or `all`).
        /// Default: the agents the pack was installed for
        #[arg(long)]
        agents: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Known skill packs
// ---------------------------------------------------------------------------

/// SEOMachine's pinned release: the commit, and a SHA-256 for every file
/// seite downloads. `scripts/refresh-seomachine-pin.sh` rewrites it; it
/// changes only when seite itself is released, so `seite skill update` moves
/// to the pin of the running binary and never to the upstream `main`.
const SEOMACHINE_PIN: &str = include_str!("seomachine.pin");

struct SkillPack {
    name: &'static str,
    description: &'static str,
    /// The pin file text (see [`Pin::parse`]).
    pin: &'static str,
}

fn known_packs() -> &'static [SkillPack] {
    &[SkillPack {
        name: "seomachine",
        description:
            "SEO content research, writing, and optimization — by SEOMachine (seomachine.com)",
        pin: SEOMACHINE_PIN,
    }]
}

fn find_pack(name: &str) -> Option<&'static SkillPack> {
    known_packs().iter().find(|p| p.name == name)
}

/// A pinned upstream release: repo, commit, and the SHA-256 of every file.
#[derive(Debug, Clone)]
struct Pin {
    repo: String,
    commit: String,
    date: String,
    licence: String,
    files: Vec<PinnedFile>,
}

#[derive(Debug, Clone)]
struct PinnedFile {
    sha256: String,
    path: String,
}

impl Pin {
    /// Header comments (`# repo|commit|date|licence <value>`) then
    /// `<sha256>  <path>` lines.
    fn parse(text: &str) -> anyhow::Result<Pin> {
        let (mut repo, mut commit, mut date, mut licence) =
            (String::new(), String::new(), String::new(), String::new());
        let mut files = Vec::new();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if let Some(comment) = line.strip_prefix('#') {
                let comment = comment.trim();
                for (key, slot) in [
                    ("repo ", &mut repo),
                    ("commit ", &mut commit),
                    ("date ", &mut date),
                    ("licence ", &mut licence),
                ] {
                    if let Some(value) = comment.strip_prefix(key) {
                        *slot = value.trim().to_string();
                    }
                }
                continue;
            }
            let (hash, path) = line
                .split_once(char::is_whitespace)
                .map(|(h, p)| (h.trim(), p.trim()))
                .ok_or_else(|| anyhow::anyhow!("malformed pin line: {line}"))?;
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                anyhow::bail!("pin line has no SHA-256: {line}");
            }
            validate_pinned_path(path)?;
            files.push(PinnedFile {
                sha256: hash.to_ascii_lowercase(),
                path: path.to_string(),
            });
        }
        if repo.is_empty() || files.is_empty() {
            anyhow::bail!("pin is missing its repo or files");
        }
        if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            anyhow::bail!("pin commit must be a full 40-character SHA, got '{commit}'");
        }
        Ok(Pin {
            repo,
            commit,
            date,
            licence,
            files,
        })
    }
}

/// Pinned paths are repo-relative and never leave the install directories.
fn validate_pinned_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
    {
        anyhow::bail!("unsafe path in pin: {path}");
    }
    Ok(())
}

/// Every pack `seite skill install <name>` knows, impeccable included.
fn known_pack_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = known_packs().iter().map(|p| p.name).collect();
    names.push(IMPECCABLE);
    names
}

/// Description of a known pack (impeccable included).
fn known_pack_description(name: &str) -> Option<&'static str> {
    if name == IMPECCABLE {
        return Some(IMPECCABLE_DESCRIPTION);
    }
    find_pack(name).map(|p| p.description)
}

// ---------------------------------------------------------------------------
// impeccable (third-party npm installer)
// ---------------------------------------------------------------------------

/// Pack name of the impeccable design skill (<https://github.com/pbakaus/impeccable>).
///
/// Unlike `known_packs`, seite does not download its files: install and
/// update delegate to the package's own installer, which knows each agent's
/// layout. The installer is fetched (pinned, [`IMPECCABLE_VERSION`]) into a
/// fresh temp prefix from a temp working directory, so a project-local
/// `.npmrc` (e.g. a different registry) or `node_modules/.bin/impeccable`
/// can't swap it; only then is its binary run in the site root.
pub const IMPECCABLE: &str = "impeccable";

/// The impeccable version seite installs (`npm view impeccable version`,
/// 2026-10-22). Bump deliberately after reviewing a new release.
pub const IMPECCABLE_VERSION: &str = "4.7.0";

const IMPECCABLE_DESCRIPTION: &str =
    "Design skill for frontend interfaces — by impeccable (npm package `impeccable`)";

/// Manifest `source` recorded for impeccable.
const IMPECCABLE_SOURCE: &str = "npm:impeccable";

/// Skill folders impeccable creates (Claude's, and the shared `.agents/` one).
const IMPECCABLE_SKILL_DIRS: [&str; 2] = [".claude/skills/impeccable", ".agents/skills/impeccable"];

/// Whether impeccable is installed: recorded in the manifest, or its skill
/// folder exists for some agent (e.g. installed with `npx impeccable` directly).
pub fn impeccable_installed(root: &Path) -> bool {
    load_manifest(root).packs.contains_key(IMPECCABLE)
        || IMPECCABLE_SKILL_DIRS
            .iter()
            .any(|dir| root.join(dir).is_dir())
}

/// impeccable `--providers` for the agents recorded in `.seite/config.json`.
/// seite's agent ids map to impeccable provider names (`Agent::impeccable_provider`);
/// unknown ids are dropped, and no (known) agents means every agent seite supports.
fn impeccable_providers(agents: Option<&[String]>) -> Vec<&'static str> {
    let mut providers: Vec<&'static str> = Vec::new();
    for id in agents.unwrap_or_default() {
        if let Some(agent) = crate::cli::harness::Agent::from_id(id.trim()) {
            if !providers.contains(&agent.impeccable_provider()) {
                providers.push(agent.impeccable_provider());
            }
        }
    }
    if providers.is_empty() {
        providers = crate::cli::harness::Agent::ALL
            .iter()
            .map(|a| a.impeccable_provider())
            .collect();
    }
    providers
}

/// `npm` arguments that fetch the pinned installer into `prefix` (run with
/// `prefix` as the working directory, never the project).
fn impeccable_fetch_args(prefix: &Path) -> Vec<String> {
    vec![
        "install".into(),
        "--prefix".into(),
        prefix.display().to_string(),
        "--no-save".into(),
        "--no-package-lock".into(),
        "--no-audit".into(),
        "--no-fund".into(),
        format!("{IMPECCABLE}@{IMPECCABLE_VERSION}"),
    ]
}

/// The installer binary npm puts under `prefix`.
fn impeccable_bin(prefix: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "impeccable.cmd"
    } else {
        "impeccable"
    };
    prefix.join("node_modules").join(".bin").join(name)
}

/// Installer arguments that install impeccable into the project for
/// `providers`.
fn impeccable_install_args(providers: &[&str]) -> Vec<String> {
    vec![
        "install".into(),
        format!("--providers={}", providers.join(",")),
        "--scope=project".into(),
        "-y".into(),
    ]
}

/// Installer arguments that update the project's impeccable install
/// (`update` takes no `--providers`; it updates what is installed).
fn impeccable_update_args() -> Vec<String> {
    vec!["update".into(), "--scope=project".into(), "-y".into()]
}

/// How a run is described to the user and in `--json`.
fn impeccable_command_label(args: &[String]) -> String {
    format!("{IMPECCABLE}@{IMPECCABLE_VERSION} {}", args.join(" "))
}

/// The site root for impeccable: the nearest ancestor with `seite.toml`, so
/// the third-party installer never runs outside a site.
fn impeccable_root() -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    crate::mcp::find_project_root(&cwd).ok_or_else(|| {
        anyhow::anyhow!(
            "no seite.toml found in {} or its parents; run this from a seite site",
            cwd.display()
        )
    })
}

/// Fail with an install hint unless `npm` runs.
fn ensure_npm() -> anyhow::Result<()> {
    let ok = crate::platform::npm_cmd("npm")
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        anyhow::bail!(
            "npm not found: impeccable is installed with npm. Install Node.js (includes npm) from https://nodejs.org, then re-run this command"
        );
    }
    Ok(())
}

/// Check for npm, confirm, fetch the pinned installer into a temp prefix
/// (from a temp working directory), then run it with `args` in `root`.
/// Returns `false` when the user declined.
#[cfg_attr(coverage_nightly, coverage(off))]
fn run_impeccable(root: &Path, verb: &str, args: &[String]) -> anyhow::Result<bool> {
    ensure_npm()?;
    let question = format!(
        "`seite skill {verb} impeccable` runs the third-party npm package `{}` in {}, continue?",
        impeccable_command_label(args),
        root.display()
    );
    if !crate::cli::prompt::confirm(&question, true)? {
        human::info("Cancelled; nothing was changed.");
        return Ok(false);
    }
    let prefix = tempfile::TempDir::new()?;
    let status = crate::platform::npm_cmd("npm")
        .args(impeccable_fetch_args(prefix.path()))
        .current_dir(prefix.path())
        .stdin(std::process::Stdio::null())
        .stdout(crate::output::child_stdout())
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run npm: {e}"))?;
    let bin = impeccable_bin(prefix.path());
    if !status.success() || !bin.is_file() {
        anyhow::bail!("fetching {IMPECCABLE}@{IMPECCABLE_VERSION} with npm failed");
    }
    let status = std::process::Command::new(&bin)
        .args(args)
        .current_dir(root)
        .stdout(crate::output::child_stdout())
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run the impeccable installer: {e}"))?;
    if !status.success() {
        let code = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "a signal".into());
        anyhow::bail!("`{}` failed (exit {code})", impeccable_command_label(args));
    }
    Ok(true)
}

/// Record impeccable in the skill-packs manifest.
fn record_impeccable(root: &Path) -> anyhow::Result<()> {
    let mut manifest = load_manifest(root);
    manifest.packs.insert(
        IMPECCABLE.to_string(),
        PackEntry {
            source: IMPECCABLE_SOURCE.to_string(),
            branch: "external".to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            files: Vec::new(),
            ..Default::default()
        },
    );
    save_manifest(root, &manifest)?;
    ensure_gitignore_entries(root, &[".claude/.seite-skill-packs.json"])
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_install_impeccable(agents_flag: Option<&str>) -> anyhow::Result<()> {
    let root = impeccable_root()?;
    let agents = match agents_flag {
        Some(flag) => Some(crate::cli::harness::to_ids(
            &crate::cli::harness::parse_agent_list(flag)?,
        )),
        None => crate::meta::load(&root).and_then(|m| m.agents),
    };
    let providers = impeccable_providers(agents.as_deref());
    let args = impeccable_install_args(&providers);

    human::info(&format!(
        "Installing '{IMPECCABLE}': {IMPECCABLE_DESCRIPTION} (providers: {})",
        providers.join(", ")
    ));
    let ran = run_impeccable(&root, "install", &args)?;
    if ran {
        record_impeccable(&root)?;
        // Point impeccable's live mode at the site's base.html (add-only).
        if let Ok(config) = crate::config::SiteConfig::load(&root.join("seite.toml")) {
            let templates = config.resolve_paths(&root).templates;
            crate::cli::theme::configure_impeccable_live(&root, &templates)?;
        }
        human::success(
            "Installed 'impeccable'. In your agent, run `/impeccable init`, then `/impeccable <describe your design>` (Codex: `$impeccable`). For a fresh design: `seite theme new`.",
        );
    }
    crate::output::json::set_data(serde_json::json!({
        "pack": IMPECCABLE,
        "action": "install",
        "installed": ran,
        "providers": providers,
        "command": impeccable_command_label(&args),
    }));
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_update_impeccable(root: &Path) -> anyhow::Result<()> {
    let args = impeccable_update_args();
    human::info("Updating 'impeccable'...");
    let ran = run_impeccable(root, "update", &args)?;
    if ran {
        record_impeccable(root)?;
        human::success("Updated 'impeccable'.");
    }
    crate::output::json::set_data(serde_json::json!({
        "pack": IMPECCABLE,
        "action": "update",
        "updated": ran,
        "command": impeccable_command_label(&args),
    }));
    Ok(())
}

/// Whether `dir` looks like impeccable's skill folder: a `SKILL.md` whose
/// frontmatter names it `impeccable`.
fn looks_like_impeccable_skill(dir: &Path) -> bool {
    let Ok(content) = fs::read_to_string(dir.join("SKILL.md")) else {
        return false;
    };
    let Some(rest) = content.strip_prefix("---") else {
        return false;
    };
    let frontmatter = rest.split("\n---").next().unwrap_or("");
    frontmatter.lines().any(|line| {
        line.strip_prefix("name:")
            .map(|v| v.trim().trim_matches(|c| c == '"' || c == '\'') == IMPECCABLE)
            .unwrap_or(false)
    })
}

/// Remove impeccable: its manifest entry and the skill folders that look like
/// impeccable's. Other files its installer added (commands, agents, hooks)
/// are left in place.
fn remove_impeccable(root: &Path) -> anyhow::Result<()> {
    let mut manifest = load_manifest(root);
    let recorded = manifest.packs.remove(IMPECCABLE).is_some();

    let mut removed: Vec<&str> = Vec::new();
    let mut kept: Vec<&str> = Vec::new();
    for dir in IMPECCABLE_SKILL_DIRS {
        let path = root.join(dir);
        if fs::symlink_metadata(&path).is_err() {
            continue;
        }
        // Refuse a symlinked skill dir (or parent) before looking inside it.
        crate::safe_fs::guard(root, &path)?;
        if !path.is_dir() {
            continue;
        }
        if looks_like_impeccable_skill(&path) {
            crate::safe_fs::remove(root, &path)?;
            removed.push(dir);
        } else {
            kept.push(dir);
        }
    }

    if !recorded && removed.is_empty() && kept.is_empty() {
        anyhow::bail!("no skill or pack named '{IMPECCABLE}' is installed");
    }
    if recorded {
        save_manifest(root, &manifest)?;
    }
    for dir in &kept {
        human::warning(&format!(
            "kept {dir}: its SKILL.md is not impeccable's (no `name: impeccable`); remove it by hand if you want it gone"
        ));
    }
    human::success(&format!(
        "Removed 'impeccable'{}",
        if removed.is_empty() {
            String::new()
        } else {
            format!(" ({})", removed.join(", "))
        }
    ));
    human::info(
        "Other files impeccable's installer added (agent commands, subagents, hooks) were left in place.",
    );
    crate::output::json::set_data(serde_json::json!({
        "pack": IMPECCABLE,
        "action": "remove",
        "removed_dirs": removed,
        "kept_dirs": kept,
    }));
    Ok(())
}

// ---------------------------------------------------------------------------
// Source tracking
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct SkillPacksManifest {
    packs: HashMap<String, PackEntry>,
}

/// One installed pack. Entries written before pinning only have the first
/// four fields; the rest default, so old manifests keep loading.
#[derive(Serialize, Deserialize, Clone, Default)]
struct PackEntry {
    source: String,
    /// The pinned commit for downloaded packs (kept under its old name).
    branch: String,
    installed_at: String,
    files: Vec<String>,
    /// Pinned commit SHA (same as `branch` for new entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    /// Commit date of the pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    /// Agent ids the pack was installed for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    agents: Vec<String>,
    /// Files an earlier update left in place (edited by the user) and then
    /// stopped tracking; `remove` reports them instead of forgetting them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    kept: Vec<String>,
    /// SHA-256 of each installed file as written, to tell later edits apart
    /// from untouched files on update and remove.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    hashes: BTreeMap<String, String>,
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join(".claude").join(".seite-skill-packs.json")
}

fn load_manifest(root: &Path) -> SkillPacksManifest {
    let path = manifest_path(root);
    // Never read through a symlink planted in the project.
    if crate::safe_fs::guard(root, &path).is_err() {
        return SkillPacksManifest::default();
    }
    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        SkillPacksManifest::default()
    }
}

fn save_manifest(root: &Path, manifest: &SkillPacksManifest) -> anyhow::Result<()> {
    let path = manifest_path(root);
    if let Some(parent) = path.parent() {
        crate::safe_fs::create_dir_all(root, parent)?;
    }
    let json = serde_json::to_string_pretty(manifest)?;
    crate::safe_fs::write(root, &path, json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Public API for agent prompt integration
// ---------------------------------------------------------------------------

/// Summary of installed skill packs and context files, for agent prompt injection.
pub struct SkillPackSummary {
    pub packs: Vec<InstalledPackInfo>,
    pub context_files: Vec<String>,
    pub custom_skills: Vec<String>,
}

pub struct InstalledPackInfo {
    pub name: String,
    pub description: String,
    pub agents: Vec<String>,
    pub commands: Vec<String>,
    pub skills: Vec<String>,
}

/// Gather skill pack information for the agent system prompt.
pub fn gather_skill_summary(root: &Path) -> SkillPackSummary {
    let manifest = load_manifest(root);
    let bundled_skills: Vec<&str> = crate::cli::harness::SKILLS.iter().map(|s| s.name).collect();

    let mut packs = Vec::new();
    let mut all_pack_skills: Vec<String> = Vec::new();

    for (name, entry) in &manifest.packs {
        let description = known_pack_description(name)
            .map(str::to_string)
            .unwrap_or_else(|| format!("from {}", entry.source));

        let mut agents = Vec::new();
        let mut commands = Vec::new();
        let mut skills = Vec::new();

        for file in &entry.files {
            if let Some(rest) = file.strip_prefix(".claude/agents/") {
                if let Some(n) = rest.strip_suffix(".md") {
                    agents.push(n.to_string());
                }
            } else if let Some(rest) = file.strip_prefix(".claude/commands/") {
                if let Some(n) = rest.strip_suffix(".md") {
                    commands.push(n.to_string());
                }
            } else if let Some(rest) = file
                .strip_prefix(".claude/skills/")
                .or_else(|| file.strip_prefix(".agents/skills/"))
            {
                if let Some(n) = rest.strip_suffix("/SKILL.md") {
                    if !skills.iter().any(|s| s == n) {
                        skills.push(n.to_string());
                    }
                    all_pack_skills.push(n.to_string());
                }
            }
        }

        agents.sort();
        commands.sort();
        skills.sort();

        packs.push(InstalledPackInfo {
            name: name.clone(),
            description,
            agents,
            commands,
            skills,
        });
    }

    // Context files
    let context_dir = root.join("context");
    let mut context_files = Vec::new();
    if context_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&context_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        context_files.push(stem.to_string());
                    }
                }
            }
        }
    }
    context_files.sort();

    // Custom skills (not bundled, not in packs)
    let skills_dir = root.join(".claude").join("skills");
    let mut custom_skills = Vec::new();
    if skills_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if bundled_skills.contains(&name.as_str()) {
                    continue;
                }
                if all_pack_skills.contains(&name) {
                    continue;
                }
                if entry.path().join("SKILL.md").exists() {
                    custom_skills.push(name);
                }
            }
        }
    }
    custom_skills.sort();

    SkillPackSummary {
        packs,
        context_files,
        custom_skills,
    }
}

// ---------------------------------------------------------------------------
// HTTP download
// ---------------------------------------------------------------------------

/// Fetches one URL's body. Injected so tests never touch the network.
type Fetch<'a> = &'a dyn Fn(&str) -> anyhow::Result<Vec<u8>>;

fn download_text(url: &str) -> anyhow::Result<String> {
    let body = ureq::get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("download failed ({url}): {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| anyhow::anyhow!("failed to read response ({url}): {e}"))?;
    Ok(body)
}

fn download_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    ureq::get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("download failed ({url}): {e}"))?
        .body_mut()
        .read_to_vec()
        .map_err(|e| anyhow::anyhow!("failed to read response ({url}): {e}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn raw_github_url(repo: &str, branch: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{repo}/{branch}/{path}")
}

// ---------------------------------------------------------------------------
// Context injection
// ---------------------------------------------------------------------------

fn seite_context_preamble() -> &'static str {
    r#"## seite Integration Context

You are working inside a **seite** static site project (https://seite.sh). Adapt your workflow to use seite's content model and CLI:

### Content model
- Content lives in `content/{collection}/*.md` with YAML frontmatter
- Use `seite new post "Title" --tags tag1,tag2` to create a new post
- Set `draft: true` in frontmatter for work-in-progress drafts
- Run `seite build` to build the site, `seite serve` for live preview, `seite deploy` to publish

### Directory mapping (SEOMachine → seite)
| SEOMachine | seite equivalent |
|---|---|
| `drafts/` | `content/{collection}/` with `draft: true` in frontmatter |
| `published/` | `content/{collection}/` (no draft flag) |
| `research/` | `research/` (same) |
| `output/` | `output/` (same, for agent reports) |
| `topics/` | `topics/` (same, for idea capture) |

### Frontmatter format
```yaml
---
title: "SEO-Optimized Title (50-60 chars)"
date: 2026-03-12
description: "Compelling meta description with primary keyword (150-160 chars)"
tags:
  - keyword1
  - keyword2
image: /static/og-image.png
draft: false
extra:
  primary_keyword: "target keyword"
---
```

### SEO features (automatic)
seite themes automatically handle: canonical URLs, Open Graph tags, Twitter Cards, JSON-LD structured data (BlogPosting/Article/WebSite), hreflang tags (multilingual), robots.txt with AI crawler management, llms.txt + llms-full.txt for AI discovery, and raw markdown alongside HTML. Focus on content quality — technical SEO is built in.

### Publishing workflow
Instead of WordPress REST API, use:
1. Write content to `content/posts/` (or other collection)
2. Remove `draft: true` when ready to publish
3. Run `seite build` to verify
4. Run `seite deploy` to publish

### Site context
- Read `seite.toml` for site configuration (title, collections, base_url, language)
- Read `data/brand.yaml` for brand identity (if created via `/seite brand`)
- Read `AGENTS.md` for full site documentation
"#
}

fn seite_publish_draft_command() -> &'static str {
    r#"# Publish Draft — seite Edition

Publish a draft article from `content/` by removing the `draft: true` flag, building the site, and optionally deploying.

## Usage

`/publish-draft <file-path>`

Example: `/publish-draft content/posts/2026-03-12-rust-error-handling.md`

## Workflow

1. **Read the specified file** and verify it exists and has `draft: true` in frontmatter
2. **Run the content scrubber** — check for AI watermarks and telltale patterns (same as SEOMachine's `/scrub`). Fix any issues found
3. **Remove `draft: true`** from the YAML frontmatter (or set it to `draft: false`)
4. **Run `seite build`** to verify the site builds successfully with the new content
5. **Report the result** — show the article title, URL it will be published at, and word count
6. **Ask if user wants to deploy** — if yes, run `seite deploy`

## Important

- Do NOT use WordPress or any external CMS — this is a seite static site
- The article stays in its current location in `content/` — seite serves it from there
- After publishing, the article will appear at its URL (e.g., `/posts/rust-error-handling`)
- Run `seite serve` if the user wants to preview before deploying
"#
}

// ---------------------------------------------------------------------------
// Internal links map generation
// ---------------------------------------------------------------------------

/// Build the internal links map from the site's content. URLs come from the
/// build's own resolution (collection `url_prefix`, `slug:` overrides, date
/// prefixes, nested docs, translations), so they match the generated site.
/// Drafts, unparseable files, and private collections are left out.
fn generate_internal_links_map(root: &Path) -> anyhow::Result<String> {
    let config = SiteConfig::load(&root.join("seite.toml"))?;
    let paths = config.resolve_paths(root);

    let mut map = String::from("# Internal Links Map\n\n");
    map.push_str("Auto-generated by `seite skill install`. Update with `seite skill update`.\n\n");

    let mut collections: Vec<&CollectionConfig> =
        config.collections.iter().filter(|c| !c.private).collect();
    collections.sort_by(|a, b| a.name.cmp(&b.name));

    for collection in collections {
        // Subdomain collections are served from their own origin.
        let origin = if collection.subdomain.is_some() {
            config.subdomain_base_url(collection)
        } else {
            String::new()
        };

        let mut links: Vec<(String, String)> =
            content_index::scan_collection(&config, &paths, collection)
                .into_iter()
                .filter_map(|item| item.parsed.ok())
                .filter(|p| !p.frontmatter.draft && !is_index_slug(&p.slug))
                .map(|p| {
                    let url = format!("{origin}{}", p.url);
                    let title = if p.frontmatter.title.is_empty() {
                        p.slug
                    } else {
                        p.frontmatter.title
                    };
                    (title, url)
                })
                .collect();
        if links.is_empty() {
            continue;
        }
        links.sort();

        map.push_str(&format!("\n## {}\n\n", collection.name));
        for (title, url) in &links {
            map.push_str(&format!("- [{}]({})\n", title, url));
        }
    }

    Ok(map)
}

/// The build renders an item whose slug is exactly `index` as the homepage or
/// its collection's index page instead of a standalone page, so it has no URL
/// of its own. Nested `section/index` items are ordinary pages.
fn is_index_slug(slug: &str) -> bool {
    slug == "index"
}

/// Write `context/internal-links-map.md`. Refuses to write through a symlink.
/// If the site config can't be loaded, warn and leave any existing map
/// untouched. Returns whether it was written.
fn write_internal_links_map(root: &Path) -> anyhow::Result<bool> {
    match generate_internal_links_map(root) {
        Ok(map) => {
            crate::safe_fs::write(
                root,
                &root.join("context").join("internal-links-map.md"),
                map,
            )?;
            Ok(true)
        }
        Err(e) => {
            human::warning(&format!("Skipped internal links map: {e}"));
            Ok(false)
        }
    }
}

// ---------------------------------------------------------------------------
// .gitignore helpers
// ---------------------------------------------------------------------------

fn ensure_gitignore_entries(root: &Path, entries: &[&str]) -> anyhow::Result<()> {
    let gitignore_path = root.join(".gitignore");
    crate::safe_fs::guard(root, &gitignore_path)?;
    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    let mut additions = Vec::new();
    for entry in entries {
        if !existing.lines().any(|l| l.trim() == *entry) {
            additions.push(*entry);
        }
    }

    if !additions.is_empty() {
        let mut content = existing;
        if !content.ends_with('\n') && !content.is_empty() {
            content.push('\n');
        }
        content.push_str("\n# SEOMachine\n");
        for entry in &additions {
            content.push_str(entry);
            content.push('\n');
        }
        crate::safe_fs::write(root, &gitignore_path, content)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// AGENTS.md integration
// ---------------------------------------------------------------------------

const AGENTS_MD_MARKER_START: &str = "<!-- seite-seomachine-start -->";
const AGENTS_MD_MARKER_END: &str = "<!-- seite-seomachine-end -->";

fn append_agent_instructions_section(root: &Path, agents: &[Agent]) -> anyhow::Result<()> {
    crate::cli::agent_instructions::migrate(root)?;

    // Pack versions installed before AGENTS.md may have left a managed copy in
    // Claude-specific instructions. Remove it so Claude does not load both the
    // stale legacy section and the freshly generated canonical section.
    let claude_md_path = root.join("CLAUDE.md");
    if claude_md_path.is_file() {
        let claude_content = fs::read_to_string(&claude_md_path)?;
        let cleaned = remove_agent_instructions_section(&claude_content);
        if cleaned != claude_content {
            crate::cli::agent_instructions::write_atomic(&claude_md_path, &cleaned)?;
        }
    }

    let agents_md_path = root.join("AGENTS.md");
    let existing = if agents_md_path.exists() {
        fs::read_to_string(&agents_md_path)?
    } else {
        String::new()
    };

    // Remove any existing SEOMachine section
    let cleaned = remove_agent_instructions_section(&existing);

    let agent_note = if agents.contains(&Agent::Claude) {
        format!(
            "**Installed for:** {}. Subagents and slash commands are Claude Code-only; the other agents get the skills.\n\n**Key commands:** `/research`, `/write`, `/article`, `/optimize`, `/analyze-existing`, `/rewrite`, `/priorities`, `/publish-draft`, `/cluster`, `/performance-review`\n",
            agent_labels(agents)
        )
    } else {
        format!(
            "**Installed for:** {}. SEOMachine's subagents and slash commands are Claude Code-only and were not installed; ask your agent to use the installed skills (`seo-audit`, `copywriting`, `content-strategy`, `schema-markup`, ...).\n",
            agent_labels(agents)
        )
    };

    let section = format!(
        r#"
{AGENTS_MD_MARKER_START}
### SEOMachine Integration

This project has [SEOMachine](https://github.com/TheCraigHewitt/seomachine) installed for SEO content research, writing, and optimization.

{agent_note}
**Context files** in `context/` define your brand voice, target keywords, and SEO guidelines. Fill these in for best results.

**Directory layout:**
- `content/posts/` — articles (use `draft: true` for work-in-progress)
- `research/` — research briefs and analysis reports
- `output/` — agent optimization reports
- `topics/` — raw topic ideas
- `context/` — brand voice, style guide, target keywords, competitor analysis

**Analytics (optional):** Python scripts in `scripts/seo/` connect to Google Analytics, Search Console, and DataForSEO. Set up `.env` with API credentials. Run `pip install -r requirements-seo.txt` to install dependencies.

**Publishing:** Use `/publish-draft <file>` to remove draft status, build, and deploy. This replaces SEOMachine's WordPress integration with seite's native deploy.

{seite_preamble}
{AGENTS_MD_MARKER_END}
"#,
        seite_preamble = seite_context_preamble(),
        agent_note = agent_note
    );

    let mut result = cleaned;
    if !result.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }
    result.push_str(&section);

    crate::cli::agent_instructions::write_atomic(&agents_md_path, &result)?;
    crate::cli::agent_instructions::migrate(root)?;
    Ok(())
}

fn remove_agent_instructions_section(content: &str) -> String {
    if let (Some(start), Some(end)) = (
        content.find(AGENTS_MD_MARKER_START),
        content.find(AGENTS_MD_MARKER_END),
    ) {
        let end_pos = end + AGENTS_MD_MARKER_END.len();
        // Also strip trailing newline after marker
        let end_pos = if content[end_pos..].starts_with('\n') {
            end_pos + 1
        } else {
            end_pos
        };
        // Also strip leading newline before marker
        let start = if start > 0 && content.as_bytes()[start - 1] == b'\n' {
            start - 1
        } else {
            start
        };
        format!("{}{}", &content[..start], &content[end_pos..])
    } else {
        content.to_string()
    }
}

// ---------------------------------------------------------------------------
// .env.example scaffolding
// ---------------------------------------------------------------------------

fn scaffold_env_example(root: &Path) -> anyhow::Result<()> {
    let env_path = root.join(".env.example");
    if env_path.exists() {
        return Ok(());
    }
    crate::safe_fs::write(
        root,
        &env_path,
        r#"# SEOMachine Analytics Configuration
# Copy this file to .env and fill in your credentials.
# See: https://github.com/TheCraigHewitt/seomachine/blob/main/data-sources-setup.md

# Google Analytics 4
GA4_PROPERTY_ID=
GA4_CREDENTIALS_PATH=credentials/ga4-service-account.json

# Google Search Console
GSC_SITE_URL=
GSC_CREDENTIALS_PATH=credentials/gsc-service-account.json

# DataForSEO
DATAFORSEO_LOGIN=
DATAFORSEO_PASSWORD=

# WordPress (not used with seite — kept for SEOMachine compatibility)
# WP_URL=
# WP_USERNAME=
# WP_APP_PASSWORD=
"#,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// requirements-seo.txt scaffolding
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Name derivation and validation
// ---------------------------------------------------------------------------

fn derive_skill_name(url: &str, name_override: Option<&str>) -> String {
    match name_override {
        Some(n) => n.to_string(),
        None => {
            // Try to get a meaningful name from the URL path
            let segments: Vec<&str> = url.trim_end_matches('/').rsplit('/').collect();
            // For .../skills/content-strategy/SKILL.md → "content-strategy"
            // Check if the filename is literally "SKILL.md" (the standard name)
            if segments.len() >= 2 && segments[0].eq_ignore_ascii_case("skill.md") {
                return segments[1].to_string();
            }
            // For .../foo.md → "foo"
            let filename = segments.first().unwrap_or(&"skill");
            filename.strip_suffix(".md").unwrap_or(filename).to_string()
        }
    }
}

fn validate_skill_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || name.contains(std::path::is_separator) || name.contains("..") {
        return Err(anyhow::anyhow!("invalid skill name: '{}'", name));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// CLI dispatch
// ---------------------------------------------------------------------------

pub fn run(args: &SkillArgs) -> anyhow::Result<()> {
    match &args.command {
        SkillCommand::Install {
            source,
            name,
            agents,
        } => run_install(source, name.as_deref(), agents.as_deref()),
        SkillCommand::List => run_list(),
        SkillCommand::Remove { name } => run_remove(name),
        SkillCommand::Update { name, agents } => run_update(name.as_deref(), agents.as_deref()),
        SkillCommand::Export(export) => crate::cli::skill_export::run(export),
    }
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_install(
    source: &str,
    name_override: Option<&str>,
    agents_flag: Option<&str>,
) -> anyhow::Result<()> {
    if source == IMPECCABLE {
        return run_install_impeccable(agents_flag);
    }

    let root = std::env::current_dir()?;

    // Check if it's a known pack
    if let Some(pack) = find_pack(source) {
        return run_install_pack(&root, pack, agents_flag, &[]);
    }

    // Check if it looks like a URL
    if source.starts_with("http://") || source.starts_with("https://") {
        return run_install_url(&root, source, name_override);
    }

    Err(anyhow::anyhow!(
        "unknown skill pack '{}'. Known packs: {}. Or provide a URL to a SKILL.md file.",
        source,
        known_pack_names().join(", ")
    ))
}

/// Which coding agents a pack install targets: `--agents`, else the agents
/// the pack was installed for, else `.seite/config.json`, else all.
fn resolve_agents(
    root: &Path,
    flag: Option<&str>,
    recorded: &[String],
) -> anyhow::Result<Vec<Agent>> {
    if let Some(flag) = flag {
        return crate::cli::harness::parse_agent_list(flag);
    }
    let recorded = crate::cli::harness::from_ids(recorded);
    if !recorded.is_empty() {
        return Ok(recorded);
    }
    if let Some(ids) = crate::meta::load(root).and_then(|m| m.agents) {
        let configured = crate::cli::harness::from_ids(&ids);
        if !configured.is_empty() {
            return Ok(configured);
        }
    }
    Ok(Agent::ALL.to_vec())
}

/// Reduce a SKILL.md frontmatter to `name` and `description`, the portable
/// Agent Skills subset every agent accepts (same rule as the seite skill in
/// `.agents/skills/`). Keys are matched at column 0; indented lines, block
/// scalars (`>-`, `|`) and list items belong to the key above them, so nested
/// keys such as `metadata: {name: ..}` never leak through. Line endings are
/// preserved (CRLF files stay CRLF). A skill with no `name` gets
/// `fallback_name` (its directory name), which the spec requires. Content
/// without a closed frontmatter block is returned unchanged.
fn portable_skill_md(content: &str, fallback_name: &str) -> String {
    let Some(first) = content.split_inclusive('\n').next() else {
        return content.to_string();
    };
    if first.trim_end() != "---" {
        return content.to_string();
    }
    let eol = if first.ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut kept = String::new();
    let mut has_name = false;
    let mut keep = false;
    let mut consumed = first.len();
    let mut closed = false;
    for line in content[first.len()..].split_inclusive('\n') {
        consumed += line.len();
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        let continuation =
            line.starts_with([' ', '\t']) || line.trim().is_empty() || line.starts_with("- ");
        if !continuation {
            let key = line.split(':').next().unwrap_or("").trim();
            keep = key == "name" || key == "description";
            has_name |= key == "name";
        }
        if keep {
            kept.push_str(line);
            if !line.ends_with('\n') {
                kept.push_str(eol);
            }
        }
    }
    if !closed {
        return content.to_string();
    }
    let mut out = format!("---{eol}");
    if !has_name {
        out.push_str(&format!("name: {fallback_name}{eol}"));
    }
    out.push_str(&kept);
    out.push_str(&format!("---{eol}"));
    out.push_str(&content[consumed..]);
    out
}

/// `<name>` of `.claude/skills/<name>/SKILL.md`.
fn pinned_skill_name(path: &str) -> Option<&str> {
    let name = path
        .strip_prefix(".claude/skills/")?
        .strip_suffix("/SKILL.md")?;
    (!name.is_empty() && !name.contains('/')).then_some(name)
}

/// Distinct skills directories of the selected agents, with the frontmatter
/// each takes (`.claude/skills` for Claude, shared `.agents/skills` for the
/// rest), from `harness::PROVIDERS`.
fn skill_dirs(agents: &[Agent]) -> Vec<(&'static str, SkillFrontmatter)> {
    let mut dirs: Vec<(&'static str, SkillFrontmatter)> = Vec::new();
    for agent in agents {
        let spec = agent.spec();
        if !dirs.iter().any(|(d, _)| *d == spec.skills_dir) {
            dirs.push((spec.skills_dir, spec.skill_frontmatter));
        }
    }
    dirs
}

/// One file to place in the project.
struct Planned {
    dest: String,
    content: Vec<u8>,
    /// Template the user is meant to edit: never overwrite an existing file.
    keep_existing: bool,
    /// Recorded in the manifest (and so deleted by `remove`).
    track: bool,
}

/// Where each verified pinned file goes for the selected agents. Agents and
/// commands are Claude Code-only; skills go to every selected agent's skills
/// directory.
fn plan_files(files: &[(PinnedFile, Vec<u8>)], agents: &[Agent]) -> Vec<Planned> {
    let claude = agents.contains(&Agent::Claude);
    let dirs = skill_dirs(agents);
    let mut plan = Vec::new();
    let mut push = |dest: String, content: Vec<u8>, keep_existing: bool, track: bool| {
        plan.push(Planned {
            dest,
            content,
            keep_existing,
            track,
        });
    };
    for (file, bytes) in files {
        let path = file.path.as_str();
        if path.starts_with(".claude/agents/") || path.starts_with(".claude/commands/") {
            if claude {
                push(path.to_string(), bytes.clone(), false, true);
            }
        } else if let Some(name) = pinned_skill_name(path) {
            for (dir, frontmatter) in &dirs {
                let content = match frontmatter {
                    SkillFrontmatter::ClaudeCode => bytes.clone(),
                    SkillFrontmatter::Portable => std::str::from_utf8(bytes)
                        .map(|t| portable_skill_md(t, name).into_bytes())
                        .unwrap_or_else(|_| bytes.clone()),
                };
                push(format!("{dir}/{name}/SKILL.md"), content, false, true);
            }
        } else if path.starts_with("context/") {
            push(path.to_string(), bytes.clone(), true, true);
        } else if path == "data_sources/requirements.txt" {
            push(
                "requirements-seo.txt".to_string(),
                bytes.clone(),
                true,
                false,
            );
        } else {
            push(format!("scripts/seo/{path}"), bytes.clone(), false, true);
        }
    }
    if claude {
        push(
            ".claude/commands/publish-draft.md".to_string(),
            seite_publish_draft_command().as_bytes().to_vec(),
            false,
            true,
        );
    }
    plan
}

#[derive(Debug)]
struct Applied {
    files: Vec<String>,
    hashes: BTreeMap<String, String>,
    notes: Vec<String>,
    written: usize,
    /// Files this run created (not overwrote), for rollback.
    created: Vec<PathBuf>,
}

/// Write the plan. A file is overwritten only if it is new, identical, or
/// tracked by the previous install and still as installed; anything else the
/// user owns is kept with a note. On a write error, files created by this
/// run are removed again.
fn apply_plan(root: &Path, plan: &[Planned], prev: Option<&PackEntry>) -> anyhow::Result<Applied> {
    let mut created: Vec<PathBuf> = Vec::new();
    let result = apply_plan_inner(root, plan, prev, &mut created);
    match result {
        Ok(mut applied) => {
            applied.created = created;
            Ok(applied)
        }
        Err(e) => {
            rollback(root, &created);
            Err(e)
        }
    }
}

/// Best-effort undo of newly created files after a failed install. Files an
/// update overwrote are not restored: staging every write in a temp tree
/// would double the I/O for a failure that can only be a local I/O error,
/// since all content was already downloaded and hash-verified in memory.
fn rollback(root: &Path, created: &[PathBuf]) {
    for path in created {
        if fs::symlink_metadata(path).is_ok_and(|m| m.is_file()) {
            let _ = fs::remove_file(path);
            if let Some(parent) = path.parent() {
                if crate::safe_fs::guard(root, parent).is_ok() {
                    let _ = fs::remove_dir(parent);
                }
            }
        }
    }
}

fn apply_plan_inner(
    root: &Path,
    plan: &[Planned],
    prev: Option<&PackEntry>,
    created: &mut Vec<PathBuf>,
) -> anyhow::Result<Applied> {
    enum Act {
        Write,
        Adopt,
        Keep { modified: bool },
    }
    let mut applied = Applied {
        files: Vec::new(),
        hashes: BTreeMap::new(),
        notes: Vec::new(),
        written: 0,
        created: Vec::new(),
    };
    for item in plan {
        let path = root.join(&item.dest);
        crate::safe_fs::guard(root, &path)?;
        let new_hash = sha256_hex(&item.content);
        let tracked = prev.is_some_and(|e| e.files.iter().any(|f| f == &item.dest));
        let prev_hash = prev.and_then(|e| e.hashes.get(&item.dest)).cloned();
        let disk = match fs::symlink_metadata(&path) {
            Ok(m) if m.is_file() => Some(fs::read(&path)?),
            Ok(_) => anyhow::bail!("{} is not a regular file", path.display()),
            Err(_) => None,
        };
        let act = match &disk {
            None => Act::Write,
            Some(d) if sha256_hex(d) == new_hash => Act::Adopt,
            Some(_) if item.keep_existing => Act::Keep { modified: false },
            // No recorded hash (pre-0.21 install): cannot tell an edit from
            // an old release, so a differing file is kept.
            Some(d) if tracked => match &prev_hash {
                Some(h) if *h == sha256_hex(d) => Act::Write,
                _ => Act::Keep { modified: true },
            },
            Some(_) => Act::Keep { modified: false },
        };
        match act {
            Act::Write => {
                crate::safe_fs::write(root, &path, &item.content)?;
                if disk.is_none() {
                    created.push(path);
                }
                applied.written += 1;
                if item.track {
                    applied.files.push(item.dest.clone());
                    applied.hashes.insert(item.dest.clone(), new_hash);
                }
            }
            Act::Adopt => {
                if item.track {
                    applied.files.push(item.dest.clone());
                    applied.hashes.insert(item.dest.clone(), new_hash);
                }
            }
            Act::Keep { modified } => {
                if !item.keep_existing {
                    applied.notes.push(if modified {
                        format!(
                            "kept {} (differs from the pinned version and may be edited; delete it and run `seite skill update` to refresh)",
                            item.dest
                        )
                    } else {
                        format!("kept {} (existing file not installed by seite)", item.dest)
                    });
                }
                if tracked && item.track {
                    applied.files.push(item.dest.clone());
                    if let Some(h) = prev_hash {
                        applied.hashes.insert(item.dest.clone(), h);
                    }
                }
            }
        }
    }
    Ok(applied)
}

/// What happened to one manifest entry on remove or update.
#[derive(Debug, PartialEq, Eq)]
enum Removal {
    Removed,
    /// Edited (hash differs), or a legacy entry that cannot be verified.
    Kept,
    /// Not a path the pack can install, or not a regular file: never touched.
    Skipped,
}

/// Every path the pinned pack can install for any agent (tracked files
/// only). The manifest lives in the repository, so it is untrusted input:
/// only these paths are ever deleted on its say-so.
fn owned_paths(pack: &SkillPack) -> BTreeSet<String> {
    let Ok(pin) = Pin::parse(pack.pin) else {
        return BTreeSet::new();
    };
    let files: Vec<(PinnedFile, Vec<u8>)> =
        pin.files.iter().map(|f| (f.clone(), Vec::new())).collect();
    plan_files(&files, &Agent::ALL)
        .into_iter()
        .filter(|p| p.track)
        .map(|p| p.dest)
        .collect()
}

/// Remove a file the previous install placed if it is still as installed.
///
/// Deletes regular files only (never a directory or symlink, so a crafted
/// manifest cannot wipe `.git` or `content/`), and only paths in `owned`.
/// Entries without a recorded hash (installed before 0.21) are removed
/// when `legacy_ok`, except `context/` templates, which are the user's.
fn remove_if_unmodified(
    root: &Path,
    entry: &PackEntry,
    owned: &BTreeSet<String>,
    file: &str,
    legacy_ok: bool,
) -> anyhow::Result<Removal> {
    if !owned.contains(file) {
        return Ok(Removal::Skipped);
    }
    let path = root.join(file);
    let Ok(meta) = fs::symlink_metadata(&path) else {
        return Ok(Removal::Removed);
    };
    if !meta.is_file() {
        return Ok(Removal::Skipped);
    }
    crate::safe_fs::guard(root, &path)?;
    let unmodified = match (entry.hashes.get(file), fs::read(&path)) {
        (Some(h), Ok(bytes)) => *h == sha256_hex(&bytes),
        (Some(_), Err(_)) => false,
        // Manifests from before hashes were recorded cannot tell: `remove`
        // deletes (as it always did), an update keeps the file.
        (None, _) => legacy_ok && !file.starts_with("context/"),
    };
    if !unmodified {
        return Ok(Removal::Kept);
    }
    fs::remove_file(&path)?;
    if let Some(parent) = path.parent() {
        if crate::safe_fs::guard(root, parent).is_ok() {
            let _ = fs::remove_dir(parent);
        }
    }
    Ok(Removal::Removed)
}

/// Install (or update to the binary's pin) a downloaded pack for `agents`.
///
/// Every pinned file is fetched and checked against its SHA-256 before
/// anything is written, so a mismatch or failed download aborts with the
/// project untouched.
fn install_pack_with(
    root: &Path,
    pack: &SkillPack,
    agents: &[Agent],
    fetch: Fetch,
) -> anyhow::Result<Applied> {
    let pin = Pin::parse(pack.pin)?;
    crate::cli::agent_instructions::validate_paths(root)?;
    let short = &pin.commit[..7.min(pin.commit.len())];
    human::info(&format!(
        "Installing skill pack '{}': {} (pinned {short}, {}, {} licence)",
        pack.name,
        pack.description,
        pin.date.get(..10).unwrap_or(&pin.date),
        pin.licence
    ));
    human::info(&format!(
        "Downloading and verifying {} files...",
        pin.files.len()
    ));

    let mut verified: Vec<(PinnedFile, Vec<u8>)> = Vec::with_capacity(pin.files.len());
    for file in &pin.files {
        let url = raw_github_url(&pin.repo, &pin.commit, &file.path);
        let bytes = fetch(&url)?;
        let got = sha256_hex(&bytes);
        if got != file.sha256 {
            anyhow::bail!(
                "checksum mismatch for {} at {short} (expected {}, got {got}); nothing was installed",
                file.path,
                file.sha256
            );
        }
        verified.push((file.clone(), bytes));
    }

    let claude = agents.contains(&Agent::Claude);
    if !claude {
        human::info(&format!(
            "SEOMachine's subagents and slash commands are Claude Code-only; installing its skills for {} only.",
            agent_labels(agents)
        ));
    }

    let plan = plan_files(&verified, agents);
    let manifest = load_manifest(root);
    let prev = manifest.packs.get(pack.name).cloned();
    let mut applied = apply_plan(root, &plan, prev.as_ref())?;
    let created = std::mem::take(&mut applied.created);

    // Everything after this point rolls back the files created above if it
    // fails (see `rollback` for what is not undone).
    let finish = (|| -> anyhow::Result<Vec<String>> {
        scaffold_env_example(root)?;
        for dir in ["context", "research", "output", "topics"] {
            crate::safe_fs::create_dir_all(root, &root.join(dir))?;
        }
        ensure_gitignore_entries(
            root,
            &[
                "research/",
                "output/",
                "topics/",
                ".env",
                "credentials/",
                ".claude/.seite-skill-packs.json",
            ],
        )?;
        write_internal_links_map(root)?;
        append_agent_instructions_section(root, agents)?;

        // Files an earlier install placed that this one no longer does (an
        // agent dropped, or a file removed from the pin). Only paths the pack
        // can install are touched; edited ones are kept and remembered.
        let mut kept: Vec<String> = Vec::new();
        if let Some(prev) = &prev {
            let owned = owned_paths(pack);
            let planned: BTreeSet<&str> = plan.iter().map(|p| p.dest.as_str()).collect();
            for file in prev.files.iter().filter(|f| !planned.contains(f.as_str())) {
                match remove_if_unmodified(root, prev, &owned, file, false)? {
                    Removal::Removed => {}
                    Removal::Kept => {
                        applied
                            .notes
                            .push(format!("kept {file} (edited since install)"));
                        kept.push(file.clone());
                    }
                    Removal::Skipped => applied
                        .notes
                        .push(format!("ignored {file}: not a file this pack installs")),
                }
            }
            // Earlier leftovers still on disk stay on the list.
            for file in &prev.kept {
                if !kept.contains(file) && fs::symlink_metadata(root.join(file)).is_ok() {
                    kept.push(file.clone());
                }
            }
        }

        let mut manifest = load_manifest(root);
        manifest.packs.insert(
            pack.name.to_string(),
            PackEntry {
                source: format!("github:{}", pin.repo),
                branch: pin.commit.clone(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                files: applied.files.clone(),
                commit: Some(pin.commit.clone()),
                date: Some(pin.date.clone()),
                agents: crate::cli::harness::to_ids(agents),
                kept: kept.clone(),
                hashes: applied.hashes.clone(),
            },
        );
        save_manifest(root, &manifest)?;
        Ok(kept)
    })();
    if let Err(e) = finish {
        rollback(root, &created);
        return Err(e);
    }

    for note in &applied.notes {
        human::info(&format!("  {note}"));
    }
    human::success(&format!(
        "Installed '{}' at {short} for {} — {} files written, {} tracked",
        pack.name,
        agent_labels(agents),
        applied.written,
        applied.files.len()
    ));
    human::info("");
    human::info("Next steps:");
    human::info("  1. Fill in context/ files with your brand voice, keywords, and guidelines");
    human::info("  2. (Optional) Set up analytics: copy .env.example to .env, fill in API keys");
    human::info("  3. (Optional) Install Python deps: pip install -r requirements-seo.txt");
    if claude {
        human::info("  4. Start with: /research \"your topic\" or /write \"your topic\"");
    } else {
        human::info(
            "  4. Ask your agent to use the installed skills, e.g. seo-audit or copywriting",
        );
    }
    Ok(applied)
}

fn agent_labels(agents: &[Agent]) -> String {
    agents
        .iter()
        .map(|a| a.label())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_install_pack(
    root: &Path,
    pack: &SkillPack,
    agents_flag: Option<&str>,
    recorded: &[String],
) -> anyhow::Result<()> {
    let agents = resolve_agents(root, agents_flag, recorded)?;
    install_pack_with(root, pack, &agents, &download_bytes)?;
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_install_url(root: &Path, url: &str, name_override: Option<&str>) -> anyhow::Result<()> {
    let skill_name = derive_skill_name(url, name_override);
    validate_skill_name(&skill_name)?;

    human::info(&format!("Downloading skill from {}...", url));

    let body = download_text(url)?;

    // Basic validation — should look like markdown
    if body.is_empty() {
        return Err(anyhow::anyhow!("downloaded file is empty"));
    }

    let skill_dir = root.join(".claude").join("skills").join(&skill_name);
    crate::safe_fs::create_dir_all(root, &skill_dir)?;
    crate::safe_fs::write(root, &skill_dir.join("SKILL.md"), &body)?;

    // Save source URL for updates
    crate::safe_fs::write(root, &skill_dir.join(".source"), url)?;

    human::success(&format!(
        "Installed skill '{}' to .claude/skills/{}/SKILL.md",
        skill_name, skill_name
    ));

    Ok(())
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

fn run_list() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let skills_dir = root.join(".claude").join("skills");

    // Bundled skills (always present after init)
    let bundled: Vec<&str> = crate::cli::harness::SKILLS.iter().map(|s| s.name).collect();

    human::info("Bundled skills:");
    let mut bundled_present: Vec<&str> = Vec::new();
    for name in &bundled {
        let skill_path = skills_dir.join(name).join("SKILL.md");
        if skill_path.exists() {
            human::info(&format!("  {} (bundled)", name));
            bundled_present.push(name);
        }
    }

    // Installed packs
    let manifest = load_manifest(&root);
    let impeccable = impeccable_installed(&root);
    let mut installed: Vec<(&str, String)> = manifest
        .packs
        .iter()
        .map(|(name, entry)| {
            let detail = if entry.source == IMPECCABLE_SOURCE {
                format!("via npm (from {})", entry.source)
            } else {
                format!("{} files (from {})", entry.files.len(), entry.source)
            };
            (name.as_str(), detail)
        })
        .collect();
    if impeccable && !manifest.packs.contains_key(IMPECCABLE) {
        installed.push((
            IMPECCABLE,
            "skill folder found (installed outside seite)".into(),
        ));
    }
    installed.sort();
    if !installed.is_empty() {
        human::info("");
        human::info("Installed packs:");
        for (name, detail) in &installed {
            human::info(&format!("  {name} — {detail}"));
        }
    }

    // Other installed skills (not bundled, not part of a pack)
    let pack_skills: Vec<String> = manifest
        .packs
        .values()
        .flat_map(|e| {
            e.files.iter().filter_map(|f| {
                f.strip_prefix(".claude/skills/")
                    .and_then(|s| s.strip_suffix("/SKILL.md"))
                    .map(|s| s.to_string())
            })
        })
        .collect();

    let mut custom_skills = Vec::new();
    if skills_dir.exists() {
        if let Ok(entries) = fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if bundled.contains(&name.as_str())
                    || pack_skills.contains(&name)
                    || name == IMPECCABLE
                {
                    continue;
                }
                if entry.path().join("SKILL.md").exists() {
                    custom_skills.push(name);
                }
            }
        }
        custom_skills.sort();
        if !custom_skills.is_empty() {
            human::info("");
            human::info("Custom skills:");
            for name in &custom_skills {
                human::info(&format!("  {}", name));
            }
        }
    }

    // Available packs (not yet installed)
    let is_installed = |name: &str| {
        if name == IMPECCABLE {
            impeccable
        } else {
            manifest.packs.contains_key(name)
        }
    };
    let available: Vec<&str> = known_pack_names()
        .into_iter()
        .filter(|name| !is_installed(name))
        .collect();
    if !available.is_empty() {
        human::info("");
        human::info("Available packs (not installed):");
        for name in &available {
            human::info(&format!(
                "  {} — {}",
                name,
                known_pack_description(name).unwrap_or_default()
            ));
            human::info(&format!("    Install with: seite skill install {name}"));
        }
    }

    let mut packs: Vec<serde_json::Value> = known_pack_names()
        .into_iter()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "description": known_pack_description(name),
                "installed": is_installed(name),
                "source": manifest.packs.get(name).map(|e| e.source.clone()),
            })
        })
        .collect();
    // Packs recorded in the manifest that this binary no longer knows.
    let mut unknown: Vec<_> = manifest
        .packs
        .iter()
        .filter(|(name, _)| !known_pack_names().contains(&name.as_str()))
        .collect();
    unknown.sort_by(|a, b| a.0.cmp(b.0));
    for (name, entry) in unknown {
        packs.push(serde_json::json!({
            "name": name,
            "description": null,
            "installed": true,
            "source": entry.source,
        }));
    }
    crate::output::json::set_data(serde_json::json!({
        "bundled": bundled_present,
        "packs": packs,
        "custom_skills": custom_skills,
    }));

    Ok(())
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

fn run_remove(name: &str) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    remove_from(&root, name)
}

fn remove_from(root: &Path, name: &str) -> anyhow::Result<()> {
    if name == IMPECCABLE {
        return remove_impeccable(root);
    }

    let mut manifest = load_manifest(root);

    // Check if it's an installed pack
    if let Some(entry) = manifest.packs.remove(name) {
        crate::cli::agent_instructions::validate_paths(root)?;
        human::info(&format!("Removing skill pack '{}'...", name));

        let owned = find_pack(name).map(owned_paths).unwrap_or_default();
        let (mut removed, mut kept) = (0usize, 0usize);
        for file in &entry.files {
            match remove_if_unmodified(root, &entry, &owned, file, true)? {
                Removal::Removed => removed += 1,
                Removal::Kept => {
                    kept += 1;
                    human::info(&format!(
                        "  kept {file} ({})",
                        if file.starts_with("context/") && !entry.hashes.contains_key(file) {
                            "your template"
                        } else {
                            "edited since install"
                        }
                    ));
                }
                Removal::Skipped => human::warning(&format!(
                    "  ignored {file}: not a file this pack installs (the manifest is not trusted)"
                )),
            }
        }
        for file in &entry.kept {
            human::info(&format!(
                "  left {file} (edited, kept by an earlier update)"
            ));
        }

        // Remove generated sections from canonical and legacy locations so
        // packs installed before the AGENTS.md migration are cleaned up too.
        for filename in ["AGENTS.md", "CLAUDE.md"] {
            let path = root.join(filename);
            if path.exists() && path.is_file() {
                let content = fs::read_to_string(&path)?;
                let cleaned = remove_agent_instructions_section(&content);
                if cleaned != content {
                    crate::cli::agent_instructions::write_atomic(&path, &cleaned)?;
                }
            }
        }

        save_manifest(root, &manifest)?;
        human::success(&format!(
            "Removed pack '{}' ({removed} files removed, {kept} edited or template files kept).",
            name
        ));
        return Ok(());
    }

    // Check if it's a custom skill (a plain directory name under
    // .claude/skills/, never a path or a symlink)
    if !is_plain_name(name) {
        anyhow::bail!("'{}' is not a skill name", name);
    }
    let skill_dir = root.join(".claude").join("skills").join(name);
    if fs::symlink_metadata(&skill_dir).is_ok() {
        crate::safe_fs::remove(root, &skill_dir)?;
        human::success(&format!("Removed skill '{}'", name));
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "no skill or pack named '{}' is installed",
        name
    ))
}

/// A single path component: no separators, no `.`/`..`, not empty.
fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.contains('\0')
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_update(name: Option<&str>, agents_flag: Option<&str>) -> anyhow::Result<()> {
    if name == Some(IMPECCABLE) {
        let root = impeccable_root()?;
        if !impeccable_installed(&root) {
            anyhow::bail!(
                "impeccable is not installed; install it with `seite skill install impeccable`"
            );
        }
        return run_update_impeccable(&root);
    }

    let root = std::env::current_dir()?;
    let manifest = load_manifest(&root);

    if let Some(name) = name {
        // Update specific pack
        if let Some(entry) = manifest.packs.get(name) {
            if let Some(pack) = find_pack(name) {
                human::info(&format!("Updating pack '{}'...", name));
                return run_install_pack(&root, pack, agents_flag, &entry.agents);
            }
            return Err(anyhow::anyhow!(
                "pack '{}' was installed but is no longer in the known packs registry",
                name
            ));
        }

        // Check if it's a URL-installed skill with .source file
        let source_path = root
            .join(".claude")
            .join("skills")
            .join(name)
            .join(".source");
        if source_path.exists() {
            let url = fs::read_to_string(&source_path)?.trim().to_string();
            human::info(&format!("Updating skill '{}' from {}...", name, url));
            return run_install_url(&root, &url, Some(name));
        }

        return Err(anyhow::anyhow!(
            "no updatable skill or pack named '{}' found",
            name
        ));
    }

    // Update all packs. The links map tracks site content, not packs, so it is
    // regenerated whenever `context/` exists, even with no packs installed.
    let pack_names: Vec<String> = manifest.packs.keys().cloned().collect();
    let context_dir = root.join("context");
    if pack_names.is_empty() && !context_dir.exists() {
        human::info("No skill packs installed. Nothing to update.");
        return Ok(());
    }

    for pack_name in &pack_names {
        if pack_name == IMPECCABLE {
            run_update_impeccable(&root)?;
        } else if let Some(pack) = find_pack(pack_name) {
            human::info(&format!("Updating pack '{}'...", pack_name));
            let recorded = manifest
                .packs
                .get(pack_name)
                .map(|e| e.agents.clone())
                .unwrap_or_default();
            run_install_pack(&root, pack, agents_flag, &recorded)?;
        }
    }

    // Also regenerate internal links map
    let context_dir = root.join("context");
    if context_dir.exists() && write_internal_links_map(&root)? {
        human::info("Regenerated internal links map");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_known_pack() {
        assert!(find_pack("seomachine").is_some());
        assert!(find_pack("nonexistent").is_none());
    }

    #[test]
    fn test_derive_skill_name_from_url() {
        assert_eq!(
            derive_skill_name(
                "https://raw.githubusercontent.com/user/repo/main/.claude/skills/content-strategy/SKILL.md",
                None
            ),
            "content-strategy"
        );
    }

    #[test]
    fn test_derive_skill_name_plain_md() {
        assert_eq!(
            derive_skill_name("https://example.com/my-skill.md", None),
            "my-skill"
        );
    }

    #[test]
    fn test_derive_skill_name_override() {
        assert_eq!(
            derive_skill_name("https://example.com/whatever.md", Some("custom-name")),
            "custom-name"
        );
    }

    #[test]
    fn test_validate_skill_name() {
        assert!(validate_skill_name("seo-content").is_ok());
        assert!(validate_skill_name("my_skill").is_ok());
        assert!(validate_skill_name("").is_err());
        assert!(validate_skill_name("..").is_err());
        assert!(validate_skill_name("../evil").is_err());
    }

    #[test]
    fn test_manifest_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "test".to_string(),
            PackEntry {
                source: "github:user/repo".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec!["file1.md".to_string()],
                ..Default::default()
            },
        );
        save_manifest(tmp.path(), &manifest).unwrap();
        let loaded = load_manifest(tmp.path());
        assert!(loaded.packs.contains_key("test"));
        assert_eq!(loaded.packs["test"].files.len(), 1);
    }

    /// A site with posts under `/blog`, root-mounted pages, and a private
    /// collection, matching the shape of `seite-sh/seite.toml`.
    fn links_map_site() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("seite.toml"),
            r#"
[site]
title = "Test"
base_url = "https://example.com"

[[collections]]
name = "posts"
label = "Posts"
directory = "posts"
has_date = true
default_template = "post.html"
url_prefix = "/blog"

[[collections]]
name = "pages"
label = "Pages"
directory = "pages"
default_template = "page.html"
url_prefix = ""

[[collections]]
name = "docs"
label = "Docs"
directory = "docs"
nested = true
default_template = "doc.html"
url_prefix = "/docs"

[[collections]]
name = "members"
label = "Members"
directory = "members"
default_template = "page.html"
url_prefix = "/members"
private = true
"#,
        )
        .unwrap();
        tmp
    }

    fn write_content(root: &Path, rel: &str, body: &str) {
        let path = root.join("content").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn test_generate_internal_links_map_requires_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(generate_internal_links_map(tmp.path()).is_err());
    }

    #[test]
    fn test_generate_internal_links_map_uses_url_prefix() {
        let tmp = links_map_site();
        write_content(
            tmp.path(),
            "posts/2026-03-13-hugo-alternative.md",
            "---\ntitle: \"Hugo Alternative\"\n---\nBody",
        );
        let map = generate_internal_links_map(tmp.path()).unwrap();
        assert!(map.contains("## posts"));
        assert!(map.contains("- [Hugo Alternative](/blog/hugo-alternative)"));
        assert!(!map.contains("/posts/"));
    }

    #[test]
    fn test_generate_internal_links_map_empty_url_prefix() {
        let tmp = links_map_site();
        write_content(tmp.path(), "pages/about.md", "---\ntitle: About\n---\n");
        let map = generate_internal_links_map(tmp.path()).unwrap();
        assert!(map.contains("- [About](/about)"));
        assert!(!map.contains("/pages/about"));
    }

    #[test]
    fn test_generate_internal_links_map_slug_override() {
        let tmp = links_map_site();
        write_content(
            tmp.path(),
            "posts/2026-01-01-original-name.md",
            "---\ntitle: Renamed\nslug: custom-slug\n---\n",
        );
        let map = generate_internal_links_map(tmp.path()).unwrap();
        assert!(map.contains("- [Renamed](/blog/custom-slug)"));
        assert!(!map.contains("original-name"));
    }

    #[test]
    fn test_generate_internal_links_map_excludes_drafts() {
        let tmp = links_map_site();
        write_content(
            tmp.path(),
            "posts/2026-01-01-published.md",
            "---\ntitle: Published\n---\n",
        );
        write_content(
            tmp.path(),
            "posts/2026-01-02-wip.md",
            "---\ntitle: Work In Progress\ndraft: true\n---\n",
        );
        let map = generate_internal_links_map(tmp.path()).unwrap();
        assert!(map.contains("Published"));
        assert!(!map.contains("Work In Progress"));
        assert!(!map.contains("/blog/wip"));
    }

    #[test]
    fn test_generate_internal_links_map_excludes_private_collections() {
        let tmp = links_map_site();
        write_content(tmp.path(), "members/secret.md", "---\ntitle: Secret\n---\n");
        let map = generate_internal_links_map(tmp.path()).unwrap();
        assert!(!map.contains("## members"));
        assert!(!map.contains("Secret"));
    }

    #[test]
    fn test_generate_internal_links_map_nested_docs_and_index() {
        let tmp = links_map_site();
        write_content(
            tmp.path(),
            "docs/guides/setup.md",
            "---\ntitle: Setup Guide\n---\n",
        );
        write_content(tmp.path(), "pages/index.md", "---\ntitle: Home\n---\n");
        // Like the build: only a slug of exactly `index` is an index page.
        write_content(
            tmp.path(),
            "docs/guides/index.md",
            "---\ntitle: Guides Overview\n---\n",
        );
        write_content(
            tmp.path(),
            "docs/index.md",
            "---\ntitle: Welcome\nslug: welcome\n---\n",
        );
        write_content(
            tmp.path(),
            "docs/start.md",
            "---\ntitle: Docs Landing\nslug: index\n---\n",
        );
        let map = generate_internal_links_map(tmp.path()).unwrap();
        assert!(map.contains("- [Setup Guide](/docs/guides/setup)"));
        assert!(map.contains("- [Guides Overview](/docs/guides/index)"));
        assert!(map.contains("- [Welcome](/docs/welcome)"));
        assert!(!map.contains("Docs Landing"));
        assert!(!map.contains("Home"));
        // Collections with no linkable content get no section.
        assert!(!map.contains("## pages"));
    }

    #[cfg(unix)]
    #[test]
    fn test_write_internal_links_map_refuses_symlink() {
        use std::os::unix::fs::symlink;
        let tmp = links_map_site();
        let victim = tmp.path().join("victim.md");
        fs::write(&victim, "keep me").unwrap();
        fs::create_dir_all(tmp.path().join("context")).unwrap();
        symlink(&victim, tmp.path().join("context/internal-links-map.md")).unwrap();
        assert!(write_internal_links_map(tmp.path()).is_err());
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep me");
    }

    #[test]
    fn test_write_internal_links_map_keeps_existing_on_bad_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        let context = tmp.path().join("context");
        fs::create_dir_all(&context).unwrap();
        fs::write(context.join("internal-links-map.md"), "previous").unwrap();
        assert!(!write_internal_links_map(tmp.path()).unwrap());
        assert_eq!(
            fs::read_to_string(context.join("internal-links-map.md")).unwrap(),
            "previous"
        );
    }

    #[test]
    fn test_seite_context_preamble_nonempty() {
        let preamble = seite_context_preamble();
        assert!(preamble.contains("seite"));
        assert!(preamble.contains("frontmatter"));
    }

    #[test]
    fn test_agents_md_section_add_and_remove() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# My Project\n").unwrap();
        append_agent_instructions_section(tmp.path(), &Agent::ALL).unwrap();

        let content = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        assert!(content.contains("SEOMachine Integration"));
        assert!(content.contains(AGENTS_MD_MARKER_START));
        assert!(content.contains(AGENTS_MD_MARKER_END));
        assert!(content.contains("# My Project")); // Original content preserved

        // Remove
        let cleaned = remove_agent_instructions_section(&content);
        assert!(!cleaned.contains("SEOMachine Integration"));
        assert!(cleaned.contains("# My Project"));
    }

    #[test]
    fn test_ensure_gitignore_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        ensure_gitignore_entries(tmp.path(), &["research/", ".env"]).unwrap();
        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("research/"));
        assert!(content.contains(".env"));

        // Calling again shouldn't duplicate
        ensure_gitignore_entries(tmp.path(), &["research/", ".env"]).unwrap();
        let content2 = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_eq!(
            content2.matches("research/").count(),
            1,
            "should not duplicate entries"
        );
    }

    #[test]
    fn test_scaffold_env_example() {
        let tmp = tempfile::TempDir::new().unwrap();
        scaffold_env_example(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join(".env.example")).unwrap();
        assert!(content.contains("DATAFORSEO_LOGIN"));

        // Should not overwrite
        fs::write(tmp.path().join(".env.example"), "custom").unwrap();
        scaffold_env_example(tmp.path()).unwrap();
        let content2 = fs::read_to_string(tmp.path().join(".env.example")).unwrap();
        assert_eq!(content2, "custom");
    }

    #[test]
    fn test_run_list_no_skills_dir() {
        // Just verify it doesn't panic when no .claude/skills exists
        // We can't easily test the output without capturing stdout
        // but the function should not error
        let _result = run_list();
        // May fail if cwd doesn't exist, that's fine for this test
    }

    #[test]
    fn test_remove_nonexistent() {
        let result = run_remove("definitely-not-installed");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no skill or pack named"));
    }

    #[test]
    fn test_raw_github_url() {
        let url = raw_github_url(
            "TheCraigHewitt/seomachine",
            "main",
            ".claude/agents/editor.md",
        );
        assert_eq!(
            url,
            "https://raw.githubusercontent.com/TheCraigHewitt/seomachine/main/.claude/agents/editor.md"
        );
    }

    #[test]
    fn test_load_manifest_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let manifest = load_manifest(tmp.path());
        assert!(manifest.packs.is_empty());
    }

    #[test]
    fn test_load_manifest_corrupt_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join(".claude");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join(".seite-skill-packs.json"), "not valid json {{{").unwrap();
        let manifest = load_manifest(tmp.path());
        assert!(manifest.packs.is_empty()); // Falls back to default
    }

    #[test]
    fn test_remove_agent_instructions_section_no_markers() {
        let content = "# My Project\n\nSome content here.\n";
        let result = remove_agent_instructions_section(content);
        assert_eq!(result, content);
    }

    #[test]
    fn test_append_agent_instructions_no_existing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No AGENTS.md exists
        append_agent_instructions_section(tmp.path(), &Agent::ALL).unwrap();
        let content = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        assert!(content.contains("SEOMachine Integration"));
        assert!(content.contains(AGENTS_MD_MARKER_START));
        assert_eq!(
            fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap(),
            "@AGENTS.md\n"
        );
    }

    #[test]
    fn test_append_agent_instructions_migrates_legacy_claude_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("CLAUDE.md"),
            "# Legacy project\n\nKeep this instruction.\n",
        )
        .unwrap();

        append_agent_instructions_section(tmp.path(), &Agent::ALL).unwrap();

        let agents = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        assert!(agents.contains("# Legacy project"));
        assert!(agents.contains("Keep this instruction."));
        assert!(agents.contains("SEOMachine Integration"));
        assert_eq!(
            fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap(),
            "@AGENTS.md\n"
        );
    }

    #[test]
    fn test_append_agent_instructions_removes_legacy_claude_managed_section() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# Canonical project\n").unwrap();
        fs::write(
            tmp.path().join("CLAUDE.md"),
            format!(
                "@AGENTS.md\n\n# Claude only\n{AGENTS_MD_MARKER_START}\nOld pack content\n{AGENTS_MD_MARKER_END}\n"
            ),
        )
        .unwrap();

        append_agent_instructions_section(tmp.path(), &Agent::ALL).unwrap();

        let agents = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        assert_eq!(agents.matches(AGENTS_MD_MARKER_START).count(), 1);
        let claude = fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert!(claude.contains("# Claude only"));
        assert!(!claude.contains("Old pack content"));
        assert!(!claude.contains(AGENTS_MD_MARKER_START));
    }

    #[test]
    fn test_append_agent_instructions_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# My Project\n").unwrap();

        // Call twice
        append_agent_instructions_section(tmp.path(), &Agent::ALL).unwrap();
        append_agent_instructions_section(tmp.path(), &Agent::ALL).unwrap();

        let content = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        // Should have exactly one section
        assert_eq!(
            content.matches(AGENTS_MD_MARKER_START).count(),
            1,
            "should not duplicate section"
        );
        assert!(content.contains("# My Project"));
    }

    #[test]
    fn test_derive_skill_name_trailing_slash() {
        assert_eq!(
            derive_skill_name("https://example.com/my-skill.md/", None),
            "my-skill"
        );
    }

    #[test]
    fn test_derive_skill_name_no_extension() {
        assert_eq!(
            derive_skill_name("https://example.com/raw-name", None),
            "raw-name"
        );
    }

    #[test]
    fn test_validate_skill_name_with_separator() {
        // Platform-dependent path separator
        assert!(validate_skill_name("a/b").is_err());
    }

    #[test]
    fn test_seite_publish_draft_command_content() {
        let content = seite_publish_draft_command();
        assert!(content.contains("Publish Draft"));
        assert!(content.contains("seite build"));
        assert!(content.contains("seite deploy"));
        assert!(content.contains("draft: true"));
        // Should explicitly say NOT to use WordPress
        assert!(content.contains("Do NOT use WordPress"));
    }

    #[test]
    fn test_remove_custom_skill() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skill_dir = tmp.path().join(".claude").join("skills").join("my-custom");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# My Custom Skill\n").unwrap();

        let result = remove_from(tmp.path(), "my-custom");

        assert!(result.is_ok());
        assert!(!skill_dir.exists());
    }

    #[test]
    fn test_remove_pack_cleans_files_and_manifest() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pack = find_pack("seomachine").unwrap();
        let owned = owned_paths(pack);
        let pick = |prefix: &str| {
            owned
                .iter()
                .find(|p| p.starts_with(prefix))
                .unwrap()
                .clone()
        };
        let agent = pick(".claude/agents/");
        let command = pick(".claude/commands/");

        fs::write(
            tmp.path().join("AGENTS.md"),
            format!(
                "# Project\n\n{AGENTS_MD_MARKER_START}\nSEOMachine Integration\n{AGENTS_MD_MARKER_END}\n"
            ),
        )
        .unwrap();
        for file in [&agent, &command] {
            let path = tmp.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "# pack file").unwrap();
        }

        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "seomachine".to_string(),
            PackEntry {
                source: "github:TheCraigHewitt/seomachine".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec![agent.clone(), command.clone()],
                ..Default::default()
            },
        );
        save_manifest(tmp.path(), &manifest).unwrap();

        let result = remove_from(tmp.path(), "seomachine");

        assert!(result.is_ok());
        assert!(!tmp.path().join(&agent).exists());
        assert!(!tmp.path().join(&command).exists());

        // Manifest should no longer have the pack
        let updated = load_manifest(tmp.path());
        assert!(!updated.packs.contains_key("seomachine"));

        let instructions = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        assert!(instructions.contains("# Project"));
        assert!(!instructions.contains("SEOMachine Integration"));
    }

    /// The manifest is part of the project, so a cloned repository can list
    /// any path in it: remove must delete only files the pack installs.
    #[test]
    fn test_remove_pack_ignores_paths_the_pack_does_not_install() {
        let tmp = tempfile::TempDir::new().unwrap();
        let site = tmp.path().join("site");
        fs::create_dir_all(site.join("content/posts")).unwrap();
        fs::create_dir_all(site.join("context")).unwrap();
        fs::write(site.join("seite.toml"), "[site]\n").unwrap();
        fs::write(site.join(".env"), "SECRET=1\n").unwrap();
        fs::write(site.join("content/posts/a.md"), "# a").unwrap();
        let context = "context/brand-voice.md".to_string();
        fs::write(site.join(&context), "filled in by the user").unwrap();
        fs::write(tmp.path().join("outside.txt"), "outside").unwrap();
        let absolute = tmp.path().join("outside.txt").display().to_string();

        let mut manifest = SkillPacksManifest::default();
        let files = [
            "seite.toml",
            ".env",
            "content/posts/a.md",
            "content",
            "../outside.txt",
            absolute.as_str(),
            context.as_str(),
            ".claude/agents/../../seite.toml",
        ];
        for name in ["seomachine", "unknown-pack"] {
            manifest.packs.insert(
                name.to_string(),
                PackEntry {
                    source: "github:x/y".to_string(),
                    branch: "main".to_string(),
                    installed_at: "2026-01-01T00:00:00Z".to_string(),
                    files: files.iter().map(|f| f.to_string()).collect(),
                    ..Default::default()
                },
            );
        }
        save_manifest(&site, &manifest).unwrap();

        remove_from(&site, "seomachine").unwrap();
        remove_from(&site, "unknown-pack").unwrap();

        assert!(site.join("seite.toml").exists());
        assert!(site.join(".env").exists());
        assert!(site.join("content/posts/a.md").exists());
        assert!(site.join(&context).exists());
        assert!(tmp.path().join("outside.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_pack_refuses_pack_paths_behind_a_symlink() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::TempDir::new().unwrap();
        let site = tmp.path().join("site");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(site.join(".claude")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let pack = find_pack("seomachine").unwrap();
        let agent = owned_paths(pack)
            .into_iter()
            .find(|p| p.starts_with(".claude/agents/"))
            .unwrap();
        let name = agent.trim_start_matches(".claude/agents/").to_string();
        fs::write(outside.join(&name), "not the pack's").unwrap();
        symlink(&outside, site.join(".claude/agents")).unwrap();

        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "seomachine".to_string(),
            PackEntry {
                source: "github:x/y".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec![agent.clone()],
                ..Default::default()
            },
        );
        save_manifest(&site, &manifest).unwrap();

        let err = remove_from(&site, "seomachine").unwrap_err();
        assert!(format!("{err:#}").contains("symlink"), "{err:#}");
        assert!(outside.join(&name).exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_install_writes_refuse_symlinked_targets() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::TempDir::new().unwrap();
        let site = tmp.path().join("site");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(site.join(".claude")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let victim = outside.join("rc");
        fs::write(&victim, "keep me").unwrap();

        // A manifest symlinked to a file outside the project is never written.
        symlink(&victim, site.join(".claude/.seite-skill-packs.json")).unwrap();
        assert!(save_manifest(&site, &SkillPacksManifest::default()).is_err());
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep me");

        // Neither is anything under a symlinked install directory.
        symlink(&outside, site.join("context")).unwrap();
        let links_map = site.join("context").join("internal-links-map.md");
        assert!(crate::safe_fs::write(&site, &links_map, "x").is_err());
        assert!(!outside.join("internal-links-map.md").exists());
    }

    #[test]
    fn test_remove_custom_skill_rejects_paths() {
        let tmp = tempfile::TempDir::new().unwrap();
        let site = tmp.path().join("site");
        fs::create_dir_all(site.join(".claude/skills")).unwrap();
        fs::create_dir_all(site.join("content")).unwrap();
        for name in ["../../content", "..", "a/b", ""] {
            assert!(remove_from(&site, name).is_err(), "{name:?}");
        }
        assert!(site.join("content").exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_pack_rejects_symlinked_instructions_before_deleting_files() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::TempDir::new().unwrap();
        let external = tmp.path().join("external-agents.md");
        let original_instructions = format!(
            "# External\n\n{AGENTS_MD_MARKER_START}\nPack instructions\n{AGENTS_MD_MARKER_END}\n"
        );
        fs::write(&external, &original_instructions).unwrap();
        symlink(&external, tmp.path().join("AGENTS.md")).unwrap();

        let pack_file = tmp.path().join(".claude").join("agents").join("editor.md");
        fs::create_dir_all(pack_file.parent().unwrap()).unwrap();
        fs::write(&pack_file, "# Agent").unwrap();

        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "seomachine".to_string(),
            PackEntry {
                source: "github:user/repo".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec![".claude/agents/editor.md".to_string()],
                ..Default::default()
            },
        );
        save_manifest(tmp.path(), &manifest).unwrap();

        let result = remove_from(tmp.path(), "seomachine");

        assert!(result.is_err());
        assert!(
            pack_file.exists(),
            "pack files must remain after preflight fails"
        );
        assert!(load_manifest(tmp.path()).packs.contains_key("seomachine"));
        assert_eq!(fs::read_to_string(external).unwrap(), original_instructions);
    }

    #[test]
    fn test_ensure_gitignore_appends_to_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join(".gitignore"), "dist/\nnode_modules/\n").unwrap();
        ensure_gitignore_entries(tmp.path(), &["research/"]).unwrap();
        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("dist/"));
        assert!(content.contains("research/"));
    }

    #[test]
    fn test_ensure_gitignore_skips_existing_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join(".gitignore"), "research/\n").unwrap();
        ensure_gitignore_entries(tmp.path(), &["research/", ".env"]).unwrap();
        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_eq!(content.matches("research/").count(), 1);
        assert!(content.contains(".env"));
    }

    #[test]
    fn test_run_install_unknown_source() {
        let result = run_install("completely-unknown-pack", None, None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown skill pack"));
        assert!(err.contains("seomachine")); // Should suggest known packs
    }

    #[test]
    fn test_gather_skill_summary_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let summary = gather_skill_summary(tmp.path());
        assert!(summary.packs.is_empty());
        assert!(summary.context_files.is_empty());
        assert!(summary.custom_skills.is_empty());
    }

    #[test]
    fn test_gather_skill_summary_with_manifest() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "seomachine".to_string(),
            PackEntry {
                source: "github:TheCraigHewitt/seomachine".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec![
                    ".claude/agents/editor.md".to_string(),
                    ".claude/agents/seo-optimizer.md".to_string(),
                    ".claude/commands/research.md".to_string(),
                    ".claude/commands/write.md".to_string(),
                    ".claude/commands/publish-draft.md".to_string(),
                    ".claude/skills/content-strategy/SKILL.md".to_string(),
                ],
                ..Default::default()
            },
        );
        save_manifest(tmp.path(), &manifest).unwrap();

        let summary = gather_skill_summary(tmp.path());
        assert_eq!(summary.packs.len(), 1);
        assert_eq!(summary.packs[0].name, "seomachine");
        assert_eq!(summary.packs[0].agents.len(), 2);
        assert_eq!(summary.packs[0].commands.len(), 3);
        assert_eq!(summary.packs[0].skills.len(), 1);
        assert!(summary.packs[0].description.contains("SEO"));
    }

    #[test]
    fn test_gather_skill_summary_context_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let context_dir = tmp.path().join("context");
        fs::create_dir_all(&context_dir).unwrap();
        fs::write(context_dir.join("brand-voice.md"), "# Brand Voice").unwrap();
        fs::write(context_dir.join("target-keywords.md"), "# Keywords").unwrap();
        fs::write(context_dir.join("notes.txt"), "not markdown").unwrap();

        let summary = gather_skill_summary(tmp.path());
        assert_eq!(
            summary.context_files,
            vec!["brand-voice", "target-keywords"]
        );
    }

    #[test]
    fn test_gather_skill_summary_custom_skills() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".claude").join("skills");

        // Create a custom skill
        let custom = skills_dir.join("my-custom-skill");
        fs::create_dir_all(&custom).unwrap();
        fs::write(custom.join("SKILL.md"), "# My Skill").unwrap();

        // Create a bundled skill (should be excluded)
        let bundled = skills_dir.join("seite");
        fs::create_dir_all(&bundled).unwrap();
        fs::write(bundled.join("SKILL.md"), "# seite").unwrap();

        let summary = gather_skill_summary(tmp.path());
        assert_eq!(summary.custom_skills, vec!["my-custom-skill"]);
    }

    #[test]
    fn test_gather_skill_summary_unknown_pack() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "future-pack".to_string(),
            PackEntry {
                source: "github:someone/future-pack".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec![".claude/commands/cool.md".to_string()],
                ..Default::default()
            },
        );
        save_manifest(tmp.path(), &manifest).unwrap();

        let summary = gather_skill_summary(tmp.path());
        assert_eq!(summary.packs.len(), 1);
        assert!(summary.packs[0]
            .description
            .contains("github:someone/future-pack"));
    }

    // --- impeccable ---

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_impeccable_providers_map_recorded_agents() {
        assert_eq!(
            impeccable_providers(Some(&ids(&["claude", "codex"]))),
            ["claude", "codex"]
        );
        assert_eq!(
            impeccable_providers(Some(&ids(&["gemini", "cursor", "opencode"]))),
            ["gemini", "cursor", "opencode"]
        );
    }

    #[test]
    fn test_impeccable_providers_default_to_all_agents() {
        let all = ["claude", "codex", "opencode", "cursor", "gemini", "github"];
        assert_eq!(impeccable_providers(None), all);
        assert_eq!(impeccable_providers(Some(&[])), all);
        // Unknown ids are dropped; nothing known left means all.
        assert_eq!(impeccable_providers(Some(&ids(&["vim"]))), all);
    }

    #[test]
    fn test_impeccable_providers_drop_unknown_and_duplicates() {
        assert_eq!(
            impeccable_providers(Some(&ids(&["codex", "vim", "codex", " claude "]))),
            ["codex", "claude"]
        );
    }

    #[test]
    fn test_impeccable_install_args() {
        assert_eq!(
            impeccable_install_args(&["claude", "codex"]),
            [
                "install",
                "--providers=claude,codex",
                "--scope=project",
                "-y"
            ]
        );
    }

    #[test]
    fn test_impeccable_update_args_take_no_providers() {
        let args = impeccable_update_args();
        assert_eq!(args, ["update", "--scope=project", "-y"]);
        assert!(!args.iter().any(|a| a.starts_with("--providers")));
    }

    #[test]
    fn test_impeccable_fetch_is_pinned_and_confined_to_the_temp_prefix() {
        let prefix = tempfile::TempDir::new().unwrap();
        let args = impeccable_fetch_args(prefix.path());
        assert_eq!(args[0], "install");
        assert_eq!(args[1], "--prefix");
        assert_eq!(args[2], prefix.path().display().to_string());
        assert_eq!(
            args.last().unwrap(),
            &format!("impeccable@{IMPECCABLE_VERSION}")
        );
        assert!(args.contains(&"--no-save".to_string()));
        assert!(impeccable_bin(prefix.path()).starts_with(prefix.path()));
        assert!(impeccable_command_label(&impeccable_update_args()).starts_with("impeccable@"));
    }

    #[test]
    fn test_impeccable_is_a_known_pack() {
        assert!(known_pack_names().contains(&IMPECCABLE));
        assert!(known_pack_description(IMPECCABLE)
            .unwrap()
            .contains("impeccable"));
        assert!(known_pack_description("seomachine").is_some());
        assert!(known_pack_description("nope").is_none());
    }

    #[test]
    fn test_impeccable_installed_by_manifest_or_dir() {
        for dir in IMPECCABLE_SKILL_DIRS {
            let tmp = tempfile::TempDir::new().unwrap();
            assert!(!impeccable_installed(tmp.path()));
            fs::create_dir_all(tmp.path().join(dir)).unwrap();
            assert!(impeccable_installed(tmp.path()), "{dir}");
        }
        let tmp = tempfile::TempDir::new().unwrap();
        record_impeccable(tmp.path()).unwrap();
        assert!(impeccable_installed(tmp.path()));
        let entry = &load_manifest(tmp.path()).packs[IMPECCABLE];
        assert_eq!(entry.source, IMPECCABLE_SOURCE);
        assert_eq!(entry.branch, "external");
        assert!(entry.files.is_empty());
        let summary = gather_skill_summary(tmp.path());
        assert_eq!(summary.packs[0].name, IMPECCABLE);
        assert_eq!(summary.packs[0].description, IMPECCABLE_DESCRIPTION);
    }

    #[test]
    fn test_looks_like_impeccable_skill() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        assert!(!looks_like_impeccable_skill(dir));
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: impeccable\ndescription: x\n---\nbody\n",
        )
        .unwrap();
        assert!(looks_like_impeccable_skill(dir));
        fs::write(dir.join("SKILL.md"), "---\nname: \"impeccable\"\n---\n").unwrap();
        assert!(looks_like_impeccable_skill(dir));
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: my-design\n---\nname: impeccable\n",
        )
        .unwrap();
        assert!(!looks_like_impeccable_skill(dir));
        fs::write(dir.join("SKILL.md"), "# impeccable\n").unwrap();
        assert!(!looks_like_impeccable_skill(dir));
    }

    #[test]
    fn test_remove_impeccable_keeps_foreign_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        record_impeccable(root).unwrap();
        let ours = root.join(".claude/skills/impeccable");
        let foreign = root.join(".agents/skills/impeccable");
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&foreign).unwrap();
        fs::write(ours.join("SKILL.md"), "---\nname: impeccable\n---\n").unwrap();
        fs::write(foreign.join("SKILL.md"), "---\nname: other\n---\n").unwrap();

        remove_from(root, IMPECCABLE).unwrap();
        assert!(!ours.exists());
        assert!(foreign.exists());
        assert!(!load_manifest(root).packs.contains_key(IMPECCABLE));
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_refuses_symlinked_skill_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("site");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("SKILL.md"), "---\nname: impeccable\n---\n").unwrap();
        fs::create_dir_all(root.join(".claude/skills")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join(".claude/skills/impeccable")).unwrap();
        record_impeccable(&root).unwrap();
        let err = remove_from(&root, IMPECCABLE).unwrap_err();
        assert!(format!("{err:#}").contains("symlink"), "{err:#}");
        assert!(outside.join("SKILL.md").exists());

        // A symlinked custom skill dir (or a symlinked `.claude/skills`).
        std::os::unix::fs::symlink(&outside, root.join(".claude/skills/mine")).unwrap();
        assert!(remove_from(&root, "mine").is_err());
        assert!(outside.join("SKILL.md").exists());
    }

    #[test]
    fn test_remove_impeccable_not_installed_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = remove_from(tmp.path(), IMPECCABLE).unwrap_err();
        assert!(err.to_string().contains("no skill or pack named"));
    }

    // --- pinned install (fixture fetcher, no network) ---

    const FIXTURE_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
    const FIXTURE_SKILL: &str = "---\nname: seo-audit\nversion: 1.0.0\ndescription: Audit SEO issues,\n  across a whole site.\nmetadata:\n  tags: seo\n---\n\n# SEO Audit\n";

    fn fixture_files() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            (".claude/agents/editor.md", b"# Editor".to_vec()),
            (".claude/commands/write.md", b"# Write".to_vec()),
            (
                ".claude/skills/seo-audit/SKILL.md",
                FIXTURE_SKILL.as_bytes().to_vec(),
            ),
            ("context/brand-voice.md", b"# Brand".to_vec()),
            ("data_sources/requirements.txt", b"requests\n".to_vec()),
            ("data_sources/modules/scorer.py", b"print('x')\n".to_vec()),
        ]
    }

    fn fixture_pack(files: &[(&'static str, Vec<u8>)]) -> SkillPack {
        let mut pin = format!(
            "# repo owner/fixture\n# commit {FIXTURE_COMMIT}\n# date 2026-10-23T00:00:00Z\n# licence MIT\n"
        );
        for (path, bytes) in files {
            pin.push_str(&format!("{}  {path}\n", sha256_hex(bytes)));
        }
        SkillPack {
            name: "seomachine",
            description: "fixture",
            pin: Box::leak(pin.into_boxed_str()),
        }
    }

    fn serve(files: Vec<(&'static str, Vec<u8>)>) -> impl Fn(&str) -> anyhow::Result<Vec<u8>> {
        move |url: &str| {
            let prefix =
                format!("https://raw.githubusercontent.com/owner/fixture/{FIXTURE_COMMIT}/");
            let path = url
                .strip_prefix(&prefix)
                .ok_or_else(|| anyhow::anyhow!("unexpected url {url}"))?;
            files
                .iter()
                .find(|(p, _)| *p == path)
                .map(|(_, b)| b.clone())
                .ok_or_else(|| anyhow::anyhow!("404 {url}"))
        }
    }

    fn install_fixture(root: &Path, agents: &[Agent]) -> Applied {
        let files = fixture_files();
        let pack = fixture_pack(&files);
        install_pack_with(root, &pack, agents, &serve(files)).unwrap()
    }

    #[test]
    fn test_skill_bundled_seomachine_pin_is_valid() {
        let pin = Pin::parse(SEOMACHINE_PIN).unwrap();
        assert_eq!(pin.repo, "TheCraigHewitt/seomachine");
        assert_eq!(pin.commit.len(), 40);
        assert_eq!(pin.licence, "MIT");
        assert!(pin
            .files
            .iter()
            .any(|f| f.path == ".claude/agents/editor.md"));
        assert!(pin
            .files
            .iter()
            .any(|f| pinned_skill_name(&f.path) == Some("seo-audit")));
        assert!(pin
            .files
            .iter()
            .any(|f| f.path == "data_sources/requirements.txt"));
        // publish-draft is seite's own version, never downloaded.
        assert!(!pin
            .files
            .iter()
            .any(|f| f.path.ends_with("publish-draft.md")));
    }

    #[test]
    fn test_skill_pin_rejects_bad_lines() {
        let good = "# repo a/b\n# commit 0123456789abcdef0123456789abcdef01234567\n";
        let hash = "a".repeat(64);
        assert!(Pin::parse(&format!("{good}{hash}  ../etc/passwd\n")).is_err());
        assert!(Pin::parse(&format!("{good}abc  x.md\n")).is_err());
        assert!(Pin::parse(&format!("# repo a/b\n# commit main\n{hash}  x.md\n")).is_err());
        assert!(Pin::parse(&format!("{good}{hash}  x.md\n")).is_ok());
    }

    #[test]
    fn test_skill_hash_mismatch_aborts_with_nothing_written() {
        let tmp = tempfile::TempDir::new().unwrap();
        let files = fixture_files();
        let pack = fixture_pack(&files);
        let mut tampered = files.clone();
        tampered[4].1 = b"requests\nevil\n".to_vec();

        let err = install_pack_with(tmp.path(), &pack, &[Agent::Claude], &serve(tampered))
            .unwrap_err()
            .to_string();
        assert!(err.contains("checksum mismatch"), "{err}");
        assert!(err.contains("requirements.txt"), "{err}");
        assert_eq!(
            fs::read_dir(tmp.path()).unwrap().count(),
            0,
            "nothing may be written"
        );
    }

    #[test]
    fn test_skill_download_failure_aborts_with_nothing_written() {
        let tmp = tempfile::TempDir::new().unwrap();
        let files = fixture_files();
        let pack = fixture_pack(&files);
        let missing: Vec<_> = files.iter().take(3).cloned().collect();
        assert!(install_pack_with(tmp.path(), &pack, &[Agent::Claude], &serve(missing)).is_err());
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn test_skill_install_claude_only() {
        let tmp = tempfile::TempDir::new().unwrap();
        install_fixture(tmp.path(), &[Agent::Claude]);
        let root = tmp.path();

        assert!(root.join(".claude/agents/editor.md").is_file());
        assert!(root.join(".claude/commands/write.md").is_file());
        assert!(root.join(".claude/commands/publish-draft.md").is_file());
        assert!(root
            .join("scripts/seo/data_sources/modules/scorer.py")
            .is_file());
        assert!(root.join("requirements-seo.txt").is_file());
        assert!(root.join("context/brand-voice.md").is_file());
        assert!(!root.join(".agents").exists());
        // Claude keeps the full frontmatter.
        let skill = fs::read_to_string(root.join(".claude/skills/seo-audit/SKILL.md")).unwrap();
        assert_eq!(skill, FIXTURE_SKILL);

        let manifest = load_manifest(root);
        let entry = &manifest.packs["seomachine"];
        assert_eq!(entry.commit.as_deref(), Some(FIXTURE_COMMIT));
        assert_eq!(entry.agents, vec!["claude".to_string()]);
        assert!(entry
            .files
            .contains(&".claude/skills/seo-audit/SKILL.md".to_string()));
        assert_eq!(
            entry.hashes[".claude/agents/editor.md"],
            sha256_hex(b"# Editor")
        );
        let agents_md = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents_md.contains("Installed for:** Claude Code"));
        assert!(agents_md.contains("/research"));
    }

    #[test]
    fn test_skill_install_codex_and_gemini_use_shared_skills_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        install_fixture(tmp.path(), &[Agent::Codex, Agent::Gemini]);
        let root = tmp.path();

        let skill = fs::read_to_string(root.join(".agents/skills/seo-audit/SKILL.md")).unwrap();
        assert_eq!(
            skill,
            "---\nname: seo-audit\ndescription: Audit SEO issues,\n  across a whole site.\n---\n\n# SEO Audit\n"
        );
        assert!(!root.join(".claude/agents").exists());
        assert!(!root.join(".claude/commands").exists());
        assert!(!root.join(".claude/skills").exists());
        assert!(root
            .join("scripts/seo/data_sources/modules/scorer.py")
            .is_file());

        let manifest = load_manifest(root);
        let entry = &manifest.packs["seomachine"];
        assert_eq!(
            entry.agents,
            vec!["codex".to_string(), "gemini".to_string()]
        );
        assert!(!entry.files.iter().any(|f| f.starts_with(".claude/")));
        let agents_md = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents_md.contains("Codex, Gemini CLI") || agents_md.contains("Codex"));
        assert!(agents_md.contains("Claude Code-only"));
        assert!(!agents_md.contains("/research"));

        let summary = gather_skill_summary(root);
        assert_eq!(summary.packs[0].skills, vec!["seo-audit".to_string()]);
    }

    #[test]
    fn test_skill_install_claude_and_codex_write_both_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        install_fixture(tmp.path(), &[Agent::Claude, Agent::Codex]);
        let claude =
            fs::read_to_string(tmp.path().join(".claude/skills/seo-audit/SKILL.md")).unwrap();
        let shared =
            fs::read_to_string(tmp.path().join(".agents/skills/seo-audit/SKILL.md")).unwrap();
        assert!(claude.contains("version: 1.0.0"));
        assert!(!shared.contains("version:"));
    }

    #[test]
    fn test_skill_portable_frontmatter_without_frontmatter_is_unchanged() {
        assert_eq!(portable_skill_md("# Just text\n", "n"), "# Just text\n");
        assert_eq!(
            portable_skill_md("---\nname: x\nno close\n", "n"),
            "---\nname: x\nno close\n"
        );
    }

    #[test]
    fn test_skill_update_from_old_manifest_without_hashes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        // A pre-pinning install: branch only, no hashes, Claude paths.
        fs::create_dir_all(root.join(".claude/agents")).unwrap();
        fs::write(root.join(".claude/agents/editor.md"), "# Old editor").unwrap();
        fs::write(root.join(".claude/agents/retired.md"), "# Retired").unwrap();
        let old = r#"{"packs":{"seomachine":{"source":"github:owner/fixture","branch":"4e04990d","installed_at":"2026-03-05T00:00:00Z","files":[".claude/agents/editor.md",".claude/agents/retired.md"]}}}"#;
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(root.join(".claude/.seite-skill-packs.json"), old).unwrap();

        let files = fixture_files();
        let pack = fixture_pack(&files);
        install_pack_with(root, &pack, &[Agent::Claude, Agent::Codex], &serve(files)).unwrap();

        // No hash recorded: a differing legacy file is kept, not overwritten.
        assert_eq!(
            fs::read_to_string(root.join(".claude/agents/editor.md")).unwrap(),
            "# Old editor"
        );
        assert!(root.join(".agents/skills/seo-audit/SKILL.md").is_file());
        let entry = &load_manifest(root).packs["seomachine"];
        assert_eq!(entry.commit.as_deref(), Some(FIXTURE_COMMIT));
        assert!(!entry.hashes.is_empty());
        // No recorded hash: a stale legacy file is kept, not guessed at.
        assert!(root.join(".claude/agents/retired.md").is_file());
        assert!(!entry
            .files
            .contains(&".claude/agents/retired.md".to_string()));
    }

    #[test]
    fn test_skill_update_adopts_identical_legacy_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".claude/agents")).unwrap();
        fs::write(root.join(".claude/agents/editor.md"), "# Editor").unwrap();
        let old = r#"{"packs":{"seomachine":{"source":"github:owner/fixture","branch":"x","installed_at":"t","files":[".claude/agents/editor.md"]}}}"#;
        fs::write(root.join(".claude/.seite-skill-packs.json"), old).unwrap();
        install_fixture(root, &[Agent::Claude]);
        let entry = &load_manifest(root).packs["seomachine"];
        assert_eq!(
            entry.hashes[".claude/agents/editor.md"],
            sha256_hex(b"# Editor")
        );
    }

    #[test]
    fn test_skill_update_keeps_edited_files_and_drops_deselected_agents() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        install_fixture(root, &[Agent::Claude, Agent::Codex]);
        fs::write(root.join(".claude/agents/editor.md"), "# My editor").unwrap();

        let files = fixture_files();
        let pack = fixture_pack(&files);
        let applied = install_pack_with(root, &pack, &[Agent::Claude], &serve(files)).unwrap();

        assert_eq!(
            fs::read_to_string(root.join(".claude/agents/editor.md")).unwrap(),
            "# My editor"
        );
        assert!(applied.notes.iter().any(|n| n.contains("editor.md")));
        // The unedited codex copy is deleted, empty dirs included.
        assert!(!root.join(".agents/skills/seo-audit").exists());
        let entry = &load_manifest(root).packs["seomachine"];
        assert_eq!(
            entry.hashes[".claude/agents/editor.md"],
            sha256_hex(b"# Editor")
        );
    }

    #[test]
    fn test_skill_install_does_not_clobber_untracked_user_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".claude/commands")).unwrap();
        fs::write(root.join(".claude/commands/write.md"), "mine").unwrap();
        let applied = install_fixture(root, &[Agent::Claude]);
        assert_eq!(
            fs::read_to_string(root.join(".claude/commands/write.md")).unwrap(),
            "mine"
        );
        assert!(applied.notes.iter().any(|n| n.contains("write.md")));
        assert!(!load_manifest(root).packs["seomachine"]
            .files
            .contains(&".claude/commands/write.md".to_string()));
    }

    #[test]
    fn test_skill_remove_keeps_modified_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        install_fixture(root, &[Agent::Claude, Agent::Codex]);
        fs::write(root.join(".claude/agents/editor.md"), "# My editor").unwrap();

        remove_from(root, "seomachine").unwrap();

        assert_eq!(
            fs::read_to_string(root.join(".claude/agents/editor.md")).unwrap(),
            "# My editor"
        );
        assert!(!root.join(".claude/commands/write.md").exists());
        assert!(!root.join(".agents/skills/seo-audit").exists());
        assert!(!load_manifest(root).packs.contains_key("seomachine"));
        let agents_md = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(!agents_md.contains(AGENTS_MD_MARKER_START));
    }

    #[test]
    fn test_skill_manifest_backward_compat() {
        let old = r#"{"packs":{"seomachine":{"source":"github:a/b","branch":"abc","installed_at":"t","files":["x.md"]}}}"#;
        let manifest: SkillPacksManifest = serde_json::from_str(old).unwrap();
        let entry = &manifest.packs["seomachine"];
        assert!(entry.commit.is_none() && entry.agents.is_empty() && entry.hashes.is_empty());
        // Old fields only on the way out when the new ones are empty.
        let json = serde_json::to_string(entry).unwrap();
        assert!(!json.contains("hashes") && !json.contains("agents"));
    }

    #[test]
    fn test_skill_resolve_agents_precedence() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No config: all agents.
        assert_eq!(
            resolve_agents(tmp.path(), None, &[]).unwrap(),
            Agent::ALL.to_vec()
        );
        // Recorded beats default; the flag beats recorded.
        let recorded = vec!["codex".to_string()];
        assert_eq!(
            resolve_agents(tmp.path(), None, &recorded).unwrap(),
            vec![Agent::Codex]
        );
        assert_eq!(
            resolve_agents(tmp.path(), Some("claude"), &recorded).unwrap(),
            vec![Agent::Claude]
        );
        assert!(resolve_agents(tmp.path(), Some("nope"), &[]).is_err());
    }

    // --- hardening: the manifest is repository-controlled input ---

    fn write_manifest(root: &Path, json: &str) {
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(root.join(".claude/.seite-skill-packs.json"), json).unwrap();
    }

    #[test]
    fn test_skill_crafted_manifest_deletes_nothing_outside_the_pack() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("site");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/HEAD"), "ref").unwrap();
        fs::create_dir_all(root.join("content/posts")).unwrap();
        fs::write(root.join("content/posts/a.md"), "post").unwrap();
        fs::write(root.join("seite.toml"), "[site]").unwrap();
        fs::write(root.join(".env"), "SECRET=1").unwrap();
        fs::write(tmp.path().join("x"), "outside").unwrap();
        let victim = tmp.path().join("victim.txt");
        fs::write(&victim, "abs").unwrap();

        let files = [
            ".git",
            "content",
            "seite.toml",
            ".env",
            "../x",
            victim.to_str().unwrap(),
            ".claude",
            "context/../seite.toml",
        ];
        // Legacy (no hashes) and hashed variants.
        for hashed in [false, true] {
            let hashes: BTreeMap<String, String> = files
                .iter()
                .map(|f| (f.to_string(), sha256_hex(b"")))
                .collect();
            let mut manifest = SkillPacksManifest::default();
            manifest.packs.insert(
                "seomachine".to_string(),
                PackEntry {
                    files: files.iter().map(|f| f.to_string()).collect(),
                    hashes: if hashed { hashes } else { BTreeMap::new() },
                    ..Default::default()
                },
            );
            save_manifest(&root, &manifest).unwrap();
            remove_from(&root, "seomachine").unwrap();

            assert!(root.join(".git/HEAD").is_file());
            assert!(root.join("content/posts/a.md").is_file());
            assert!(root.join("seite.toml").is_file());
            assert!(root.join(".env").is_file());
            assert!(tmp.path().join("x").is_file());
            assert!(victim.is_file());
        }
    }

    #[test]
    fn test_skill_update_stale_loop_ignores_paths_the_pack_cannot_install() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("seite.toml"), "[site]").unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        let bad_hash = sha256_hex(b"[site]");
        let manifest = format!(
            r#"{{"packs":{{"seomachine":{{"source":"s","branch":"b","installed_at":"t","files":["seite.toml","content"],"hashes":{{"seite.toml":"{bad_hash}"}}}}}}}}"#
        );
        write_manifest(root, &manifest);
        let applied = install_fixture(root, &[Agent::Claude]);
        assert!(root.join("seite.toml").is_file());
        assert!(root.join("content").is_dir());
        assert!(applied
            .notes
            .iter()
            .any(|n| n.contains("ignored seite.toml")));
        assert!(!load_manifest(root).packs["seomachine"]
            .files
            .contains(&"seite.toml".to_string()));
    }

    #[test]
    fn test_skill_remove_never_deletes_a_directory_or_symlink_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        // A pack path that is a directory (owned name, wrong kind).
        fs::create_dir_all(root.join(".claude/agents/editor.md/inner")).unwrap();
        fs::write(root.join(".claude/agents/editor.md/inner/f"), "keep").unwrap();
        write_manifest(
            root,
            r#"{"packs":{"seomachine":{"source":"s","branch":"b","installed_at":"t","files":[".claude/agents/editor.md"]}}}"#,
        );
        remove_from(root, "seomachine").unwrap();
        assert!(root.join(".claude/agents/editor.md/inner/f").is_file());
    }

    #[test]
    fn test_skill_legacy_remove_keeps_context_templates() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("context")).unwrap();
        fs::write(root.join("context/brand-voice.md"), "my voice").unwrap();
        write_manifest(
            root,
            r#"{"packs":{"seomachine":{"source":"s","branch":"b","installed_at":"t","files":["context/brand-voice.md"]}}}"#,
        );
        remove_from(root, "seomachine").unwrap();
        assert_eq!(
            fs::read_to_string(root.join("context/brand-voice.md")).unwrap(),
            "my voice"
        );
    }

    #[test]
    fn test_skill_update_remembers_kept_files_for_remove() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        install_fixture(root, &[Agent::Claude, Agent::Codex]);
        fs::write(root.join(".agents/skills/seo-audit/SKILL.md"), "edited").unwrap();
        // Drop codex: its edited skill is kept and remembered.
        install_fixture(root, &[Agent::Claude]);
        let entry = load_manifest(root).packs.remove("seomachine").unwrap();
        assert_eq!(
            entry.kept,
            vec![".agents/skills/seo-audit/SKILL.md".to_string()]
        );
        assert!(!entry
            .files
            .contains(&".agents/skills/seo-audit/SKILL.md".to_string()));
        // A later update keeps remembering it while it exists.
        install_fixture(root, &[Agent::Claude]);
        assert_eq!(load_manifest(root).packs["seomachine"].kept.len(), 1);
    }

    #[test]
    fn test_skill_failure_after_apply_rolls_back_created_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        // `research` as a file makes the post-apply scaffolding fail.
        fs::write(root.join("research"), "not a dir").unwrap();
        let files = fixture_files();
        let pack = fixture_pack(&files);
        assert!(install_pack_with(root, &pack, &[Agent::Claude], &serve(files)).is_err());
        assert!(!root.join(".claude/agents/editor.md").exists());
        assert!(!root.join("requirements-seo.txt").exists());
        assert!(!root
            .join("scripts/seo/data_sources/modules/scorer.py")
            .exists());
        assert!(!load_manifest(root).packs.contains_key("seomachine"));
    }

    #[test]
    fn test_skill_portable_frontmatter_edge_cases() {
        // Folded scalar, nested metadata with its own name key.
        let src = "---\nname: a\nmetadata:\n  name: nested\n  tags: x\ndescription: >-\n  folded line one\n  line two\nversion: 2\n---\nBody\n";
        assert_eq!(
            portable_skill_md(src, "fb"),
            "---\nname: a\ndescription: >-\n  folded line one\n  line two\n---\nBody\n"
        );
        // CRLF stays CRLF.
        let crlf = "---\r\nname: a\r\nversion: 1\r\ndescription: d\r\n---\r\nBody\r\n";
        assert_eq!(
            portable_skill_md(crlf, "fb"),
            "---\r\nname: a\r\ndescription: d\r\n---\r\nBody\r\n"
        );
        // Missing name falls back to the directory name; a nested name does not count.
        let noname = "---\ndescription: d\nmetadata:\n  name: nested\n---\nBody\n";
        assert_eq!(
            portable_skill_md(noname, "seo-audit"),
            "---\nname: seo-audit\ndescription: d\n---\nBody\n"
        );
    }
}
