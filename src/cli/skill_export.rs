//! `seite skill export <dir> [--format skills|claude-plugin|repo]`: render the
//! bundled `seite` skill (and the Claude Code check hooks) for distribution
//! outside `seite init`, from the same single source as the harness files
//! (`harness::SKILLS`, `src/scaffold/skill-seite/`, `harness_hooks`).
//!
//! - `skills`: `<dir>/seite/SKILL.md` + `reference/*.md`, portable frontmatter
//!   (`name`/`description`), the layout the `skills` CLI installs
//!   (`npx skills add seite-sh/seite` discovers `skills/<name>/SKILL.md` at a
//!   repo root and copies the whole directory;
//!   <https://github.com/vercel-labs/skills>).
//! - `claude-plugin`: a Claude Code plugin in `<dir>`: `.claude-plugin/plugin.json`,
//!   `skills/seite/` (Claude frontmatter, with `argument-hint`), and
//!   `hooks/hooks.json` (the two-tier `seite check --hook claude` hooks of
//!   `seite init`, with `--plugin`: silent outside a seite site, turn-end
//!   check only after an edit inside the site, standing down when the project
//!   runs its own; see `cli::harness_hooks`). No MCP server: coding agents
//!   use the CLI. <https://code.claude.com/docs/en/plugins-reference>
//! - `repo`: what this repository checks in, at `<dir>` = the repo root:
//!   `skills/seite/`, `plugins/seite/`, and the marketplace manifest
//!   `.claude-plugin/marketplace.json`. `tests/integration/skill_export.rs`
//!   fails when the checked-in copy differs from a fresh render.
//!
//! Files under the paths an export owns that it no longer renders are removed,
//! so a dropped reference file doesn't linger, but only inside a skill
//! directory whose `SKILL.md` carries seite's `# seite-skill-version` marker
//! (anything else is refused, so a mistyped target can't lose files).

use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};

use crate::cli::harness::{self, Skill, SkillFrontmatter};
use crate::output::human;

/// Regenerates the copy checked into this repository.
pub const REGENERATE_COMMAND: &str = "cargo run -- skill export . --format repo";

/// Plugin and marketplace name (`/plugin install seite@seite`).
pub const PLUGIN_NAME: &str = "seite";

/// Where the repository keeps the plugin, relative to its root.
pub const REPO_PLUGIN_DIR: &str = "plugins/seite";
/// Where the repository keeps the portable skill directory.
pub const REPO_SKILLS_DIR: &str = "skills";
/// The marketplace manifest at the repository root.
pub const MARKETPLACE_PATH: &str = ".claude-plugin/marketplace.json";

#[derive(Args)]
pub struct ExportArgs {
    /// Directory to write into (created when missing)
    #[arg(value_name = "DIR")]
    pub out: PathBuf,

    /// What to render
    #[arg(long, value_enum, default_value_t = ExportFormat::Skills)]
    pub format: ExportFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExportFormat {
    /// `<dir>/seite/` for the `skills` CLI (portable frontmatter)
    Skills,
    /// A Claude Code plugin rooted at `<dir>`
    ClaudePlugin,
    /// This repository's checked-in copy (`skills/`, `plugins/seite/`,
    /// `.claude-plugin/marketplace.json`)
    Repo,
}

/// Every file `format` renders, as (path relative to the export dir, content),
/// sorted by path.
pub fn render(format: ExportFormat) -> Vec<(String, String)> {
    let mut files = match format {
        ExportFormat::Skills => skill_dir_files("", SkillFrontmatter::Portable),
        ExportFormat::ClaudePlugin => plugin_files(""),
        ExportFormat::Repo => {
            let mut files = skill_dir_files(REPO_SKILLS_DIR, SkillFrontmatter::Portable);
            files.extend(plugin_files(REPO_PLUGIN_DIR));
            files.push((MARKETPLACE_PATH.to_string(), marketplace_json()));
            files
        }
    };
    files.sort();
    files
}

/// The paths (files or directories, relative to the export dir) an export
/// owns: anything under them that [`render`] doesn't produce is stale.
pub fn owned_paths(format: ExportFormat) -> Vec<String> {
    let skill_dirs = |base: &str| -> Vec<String> {
        harness::SKILLS.iter().map(|s| join(base, s.name)).collect()
    };
    match format {
        ExportFormat::Skills => skill_dirs(""),
        ExportFormat::ClaudePlugin => plugin_owned(""),
        ExportFormat::Repo => {
            let mut owned = skill_dirs(REPO_SKILLS_DIR);
            owned.extend(plugin_owned(REPO_PLUGIN_DIR));
            owned.push(MARKETPLACE_PATH.to_string());
            owned
        }
    }
}

fn plugin_owned(base: &str) -> Vec<String> {
    let mut owned = vec![
        join(base, ".claude-plugin/plugin.json"),
        join(base, "hooks/hooks.json"),
    ];
    owned.extend(
        harness::SKILLS
            .iter()
            .map(|s| join(base, &format!("skills/{}", s.name))),
    );
    owned
}

fn join(base: &str, rel: &str) -> String {
    if base.is_empty() {
        rel.to_string()
    } else {
        format!("{base}/{rel}")
    }
}

/// `<base>/<skill>/SKILL.md` and `<base>/<skill>/reference/*.md` for every
/// bundled skill. Every skill ships: a site that isn't set up by `seite init`
/// has no feature detection to gate on.
fn skill_dir_files(base: &str, frontmatter: SkillFrontmatter) -> Vec<(String, String)> {
    harness::SKILLS
        .iter()
        .flat_map(|s| skill_files(base, s, frontmatter))
        .collect()
}

fn skill_files(base: &str, skill: &Skill, frontmatter: SkillFrontmatter) -> Vec<(String, String)> {
    let dir = join(base, skill.name);
    let mut files = vec![(format!("{dir}/SKILL.md"), skill.render(frontmatter))];
    for r in skill.references {
        files.push((
            format!("{dir}/reference/{}.md", r.name),
            r.content.to_string(),
        ));
    }
    files
}

/// The Claude Code plugin rooted at `base`.
fn plugin_files(base: &str) -> Vec<(String, String)> {
    let mut files = vec![
        (
            join(base, ".claude-plugin/plugin.json"),
            harness::pretty_json(&plugin_json()),
        ),
        // `{"hooks": {"Stop": […], "PostToolUse": […]}}`: the hooks `seite
        // init` merges into `.claude/settings.json`, marked `--plugin`.
        (
            join(base, "hooks/hooks.json"),
            crate::cli::harness_hooks::plugin_hooks_json(),
        ),
    ];
    files.extend(skill_dir_files(
        &join(base, "skills"),
        SkillFrontmatter::ClaudeCode,
    ));
    files
}

const PLUGIN_DESCRIPTION: &str = "The /seite skill (write, check, audit, preview, build, deploy, theme) and automatic `seite check` hooks for seite static sites. Needs the seite CLI 0.21 or newer (https://seite.sh).";

/// `.claude-plugin/plugin.json`. `version` is seite's: Claude Code offers an
/// update when it changes.
pub fn plugin_json() -> serde_json::Value {
    serde_json::json!({
        "name": PLUGIN_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "description": PLUGIN_DESCRIPTION,
        "author": { "name": "seite", "url": env!("CARGO_PKG_HOMEPAGE") },
        "homepage": env!("CARGO_PKG_HOMEPAGE"),
        "repository": env!("CARGO_PKG_REPOSITORY"),
        "license": env!("CARGO_PKG_LICENSE"),
        "keywords": ["seite", "static-site-generator", "seo", "llms-txt"]
    })
}

/// The repository's `.claude-plugin/marketplace.json`, listing the plugin at
/// [`REPO_PLUGIN_DIR`] (a relative `source` must start with `./`). The
/// version lives only in plugin.json.
pub fn marketplace_json() -> String {
    harness::pretty_json(&serde_json::json!({
        "name": PLUGIN_NAME,
        "owner": { "name": "seite", "url": env!("CARGO_PKG_HOMEPAGE") },
        "description": "Claude Code plugin for seite, the AI-native static site generator.",
        "plugins": [{
            "name": PLUGIN_NAME,
            "source": format!("./{REPO_PLUGIN_DIR}"),
            "description": PLUGIN_DESCRIPTION,
            "category": "development",
            "homepage": env!("CARGO_PKG_HOMEPAGE")
        }]
    }))
}

/// Files under `dir`'s owned paths that `format` doesn't render, sorted.
pub fn stale_files(dir: &Path, format: ExportFormat) -> Vec<PathBuf> {
    let rendered: std::collections::BTreeSet<PathBuf> = render(format)
        .into_iter()
        .map(|(path, _)| dir.join(path))
        .collect();
    let mut stale = Vec::new();
    for owned in owned_paths(format) {
        let mut found = Vec::new();
        collect_files(&dir.join(owned), &mut found);
        stale.extend(found.into_iter().filter(|p| !rendered.contains(p)));
    }
    stale.sort();
    stale
}

/// Every regular file at or under `path` (symlinks are listed, not followed).
fn collect_files(path: &Path, out: &mut Vec<PathBuf>) {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    if meta.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_files(&entry.path(), out);
            }
        }
    } else {
        out.push(path.to_path_buf());
    }
}

/// Write [`render`] under `dir` and remove [`stale_files`]. Returns
/// (written, removed) counts; unchanged files are left untouched.
pub fn export(dir: &Path, format: ExportFormat) -> anyhow::Result<(usize, usize)> {
    std::fs::create_dir_all(dir)?;
    let stale = stale_files(dir, format);
    // Delete only inside directories that hold seite's own skill.
    for owned in owned_paths(format) {
        let owned = dir.join(owned);
        if owned.is_dir()
            && stale.iter().any(|p| p.starts_with(&owned))
            && !is_seite_skill_dir(&owned)
        {
            anyhow::bail!(
                "refusing to delete files in {}: its SKILL.md is not the seite skill (no `# seite-skill-version` line); pick an empty or seite-owned directory",
                owned.display()
            );
        }
    }
    for path in &stale {
        crate::safe_fs::remove(dir, path)?;
    }
    let mut written = 0;
    for (rel, content) in render(format) {
        let path = dir.join(&rel);
        if std::fs::read_to_string(&path).is_ok_and(|c| c == content) {
            continue;
        }
        crate::safe_fs::write(dir, &path, content)?;
        written += 1;
    }
    Ok((written, stale.len()))
}

/// Whether `dir/SKILL.md` is a seite skill (carries the version marker).
fn is_seite_skill_dir(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("SKILL.md")).is_ok_and(|t| harness::extract_version(&t) > 0)
}

pub fn run(args: &ExportArgs) -> anyhow::Result<()> {
    let (written, removed) = export(&args.out, args.format)?;
    let total = render(args.format).len();
    crate::output::json::set_data(serde_json::json!({
        "dir": args.out.display().to_string(),
        "files": total,
        "written": written,
        "removed": removed,
    }));
    human::success(&format!(
        "Exported {total} files to {} ({written} written, {removed} removed)",
        args.out.display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frontmatter(content: &str) -> serde_yaml_ng::Value {
        let fm = content
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n"))
            .map(|(fm, _)| fm)
            .unwrap();
        serde_yaml_ng::from_str(fm).unwrap()
    }

    fn file<'a>(files: &'a [(String, String)], path: &str) -> &'a str {
        &files.iter().find(|(p, _)| p == path).unwrap().1
    }

    #[test]
    fn test_skill_export_skills_format_is_portable() {
        let files = render(ExportFormat::Skills);
        let skill = file(&files, "seite/SKILL.md");
        let fm = frontmatter(skill);
        assert_eq!(fm["name"].as_str(), Some("seite"));
        assert!(fm["description"].as_str().is_some_and(|d| !d.is_empty()));
        // Only the spec keys: no `argument-hint` in the portable copy.
        assert_eq!(fm.as_mapping().unwrap().len(), 2, "{skill}");
        for r in harness::SEITE_REFERENCES {
            assert_eq!(
                file(&files, &format!("seite/reference/{}.md", r.name)),
                r.content
            );
        }
        assert_eq!(files.len(), 1 + harness::SEITE_REFERENCES.len());
    }

    #[test]
    fn test_skill_export_claude_plugin_layout() {
        let files = render(ExportFormat::ClaudePlugin);
        let manifest: serde_json::Value =
            serde_json::from_str(file(&files, ".claude-plugin/plugin.json")).unwrap();
        assert_eq!(manifest["name"], PLUGIN_NAME);
        assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
        let skill = file(&files, "skills/seite/SKILL.md");
        assert!(frontmatter(skill)["argument-hint"].as_str().is_some());
        // No MCP server by default.
        assert!(!files.iter().any(|(p, _)| p.contains("mcp")));

        let hooks: serde_json::Value =
            serde_json::from_str(file(&files, "hooks/hooks.json")).unwrap();
        let stop = hooks["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert_eq!(
            stop,
            "command -v seite >/dev/null 2>&1 && seite check --hook claude --plugin || true"
        );
        let post = &hooks["hooks"]["PostToolUse"][0];
        assert_eq!(post["matcher"], "Write|Edit|MultiEdit");
        assert_eq!(
            post["hooks"][0]["command"],
            "command -v seite >/dev/null 2>&1 && seite check --hook claude --post-edit --plugin || true"
        );
    }

    #[test]
    fn test_skill_export_repo_marketplace_points_at_plugin() {
        let files = render(ExportFormat::Repo);
        let market: serde_json::Value =
            serde_json::from_str(file(&files, MARKETPLACE_PATH)).unwrap();
        assert_eq!(market["name"], PLUGIN_NAME);
        let entry = &market["plugins"][0];
        assert_eq!(entry["name"], PLUGIN_NAME);
        let source = entry["source"].as_str().unwrap();
        assert_eq!(source, "./plugins/seite");
        let plugin_json = format!("{}/.claude-plugin/plugin.json", &source[2..]);
        assert!(files.iter().any(|(p, _)| *p == plugin_json));
        assert!(files.iter().any(|(p, _)| p == "skills/seite/SKILL.md"));
        // Every rendered file sits under a path the export owns.
        let owned = owned_paths(ExportFormat::Repo);
        for (path, _) in &files {
            assert!(
                owned
                    .iter()
                    .any(|o| path == o || path.starts_with(&format!("{o}/"))),
                "{path}"
            );
        }
    }

    #[test]
    fn test_skill_export_writes_and_prunes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("seite/reference")).unwrap();
        std::fs::write(dir.join("seite/reference/gone.md"), "old").unwrap();
        // Not seite's skill directory: nothing is deleted.
        std::fs::write(dir.join("seite/SKILL.md"), "---\nname: seite\n---\nmine\n").unwrap();
        let err = export(dir, ExportFormat::Skills).unwrap_err().to_string();
        assert!(err.contains("refusing to delete"), "{err}");
        assert!(dir.join("seite/reference/gone.md").exists());
        std::fs::write(dir.join("seite/SKILL.md"), "# seite-skill-version: 1\n").unwrap();
        std::fs::write(dir.join("unrelated.md"), "keep").unwrap();
        let (written, removed) = export(dir, ExportFormat::Skills).unwrap();
        assert_eq!(written, render(ExportFormat::Skills).len());
        assert_eq!(removed, 1);
        assert!(!dir.join("seite/reference/gone.md").exists());
        assert!(dir.join("unrelated.md").exists());
        // Idempotent.
        assert_eq!(export(dir, ExportFormat::Skills).unwrap(), (0, 0));
        assert!(stale_files(dir, ExportFormat::Skills).is_empty());
    }
}
