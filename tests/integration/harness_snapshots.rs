//! Snapshot tests of the coding-agent harness files `seite init` and
//! `seite upgrade` generate.
//!
//! Each case is one file, `tests/snapshots/harness/<case>.snap`: every
//! generated path under a `==> path <==` header, followed by its content. The
//! bodies of the long bundled rules and skills (verbatim `src/scaffold/*.md`)
//! are elided so the snapshots show layout, frontmatter, and configs; the
//! `seite` workflow skill is kept in full. Single files (not a mirrored tree)
//! keep dot-directories such as `.claude/skills` out of the repository, where
//! agents working on seite itself would discover them.
//!
//! Regenerate after an intended change with:
//!
//! ```sh
//! SEITE_UPDATE_SNAPSHOTS=1 cargo test --test integration harness_snapshot
//! ```

use super::common::*;
use seite::cli::harness::{self, Agent, SiteFeatures};

fn snapshot_path(case: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots/harness")
        .join(format!("{case}.snap"))
}

/// Replace the body of a bundled rule or skill (other than `seite`) with a
/// one-line marker.
fn elide(content: &str) -> String {
    for rule in harness::RULES {
        if let Some(head) = content.strip_suffix(rule.content) {
            return format!("{head}[… body of rule `{}` elided …]\n", rule.name);
        }
    }
    for skill in harness::SKILLS.iter().filter(|s| s.name != "seite") {
        let body = skill.content.split_once("\n---\n").map(|(_, b)| b);
        if let Some(head) = body.and_then(|b| content.strip_suffix(b)) {
            return format!("{head}[… body of skill `{}` elided …]\n", skill.name);
        }
    }
    content.to_string()
}

fn render(mut files: Vec<(String, String)>) -> String {
    files.sort();
    let mut out = String::new();
    for (path, content) in files {
        out.push_str(&format!("==> {path} <==\n{}", elide(&content)));
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// Compare `files` with the committed snapshot for `case`, or rewrite it
/// under `SEITE_UPDATE_SNAPSHOTS=1`.
fn assert_snapshot(case: &str, files: Vec<(String, String)>) {
    let actual = render(files);
    let path = snapshot_path(case);
    if std::env::var_os("SEITE_UPDATE_SNAPSHOTS").is_some_and(|v| v == "1") {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_default();
    if actual == expected {
        return;
    }
    let mut report = String::new();
    let (exp, act): (Vec<_>, Vec<_>) = (expected.lines().collect(), actual.lines().collect());
    let first = exp
        .iter()
        .zip(&act)
        .position(|(e, a)| e != a)
        .unwrap_or(exp.len().min(act.len()));
    for i in first.saturating_sub(3)..(first + 6) {
        let (e, a) = (exp.get(i), act.get(i));
        if e.is_none() && a.is_none() {
            break;
        }
        let mark = if e == a { " " } else { "!" };
        report.push_str(&format!(
            "{mark} {:>4} expected: {}\n{mark} {:>4} actual:   {}\n",
            i + 1,
            e.unwrap_or(&"<eof>"),
            i + 1,
            a.unwrap_or(&"<eof>")
        ));
    }
    panic!(
        "harness snapshot `{case}` differs ({}), first difference at line {}:\n{report}\nIf the change is intended, rerun with SEITE_UPDATE_SNAPSHOTS=1 and review the diff.",
        path.display(),
        first + 1
    );
}

/// Everything `seite init` generates for `agents` besides the site itself:
/// the planned harness files plus the two AGENTS.md blocks.
fn init_files(agents: &[Agent], mcp: bool) -> Vec<(String, String)> {
    let features = SiteFeatures::default();
    let mut files: Vec<(String, String)> = harness::plan(agents, features, mcp)
        .into_iter()
        .map(|f| (f.path, f.content))
        .collect();
    files.push((
        "AGENTS.md#agent-setup".into(),
        harness::mcp_setup_table(agents, mcp),
    ));
    files.push((
        "AGENTS.md#context-rules".into(),
        harness::rules_index(features, agents),
    ));
    files
}

#[test]
fn test_harness_snapshot_init_all_agents() {
    assert_snapshot("init-all", init_files(&Agent::ALL, false));
}

#[test]
fn test_harness_snapshot_init_all_agents_with_mcp() {
    assert_snapshot("init-all-mcp", init_files(&Agent::ALL, true));
}

#[test]
fn test_harness_snapshot_init_each_agent_alone() {
    for agent in Agent::ALL {
        assert_snapshot(&format!("init-{}", agent.id()), init_files(&[agent], false));
    }
}

/// A site as seite 0.18 left it: Claude-only, instructions in CLAUDE.md, the
/// MCP server in `.claude/settings.json` (never loaded), an old skill, and no
/// agent selection.
fn write_legacy_site(dir: &std::path::Path) {
    let write = |rel: &str, content: &str| {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    };
    write(
        "seite.toml",
        "[site]\ntitle = \"Legacy\"\ndescription = \"\"\nbase_url = \"http://localhost:3000\"\nlanguage = \"en\"\n\n[[collections]]\nname = \"posts\"\n\n[build]\noutput_dir = \"dist\"\nminify = true\n",
    );
    write(
        "CLAUDE.md",
        "# Legacy\n\nCustom instructions the user wrote.\n",
    );
    write(
        ".claude/settings.json",
        r#"{"permissions":{"allow":["Read","Bash(seite build:*)"]},"mcpServers":{"seite":{"command":"seite","args":["mcp"]}}}"#,
    );
    write(
        ".claude/skills/theme-builder/SKILL.md",
        "---\nname: theme-builder\ndescription: Old theme builder.\n---\n\nOld body.\n",
    );
    write(".seite/config.json", r#"{"version":"0.18.0"}"#);
}

/// Harness files (and the upgrade's reported changes) after `seite upgrade`.
fn upgrade_files(extra_args: &[&str]) -> Vec<(String, String)> {
    let tmp = TempDir::new().unwrap();
    let site = tmp.path().join("legacy");
    write_legacy_site(&site);
    let mut args = vec!["--json", "upgrade", "--force"];
    args.extend_from_slice(extra_args);
    let output = page_cmd().args(&args).current_dir(&site).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc = json_stdout(&output);
    let changes: Vec<String> = doc["data"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();

    let mut files = harness_snapshot(&site);
    files.push(("(upgrade changes)".into(), changes.join("\n")));
    files
}

#[test]
fn test_harness_snapshot_upgrade_legacy_claude_site() {
    assert_snapshot("upgrade-legacy-all", upgrade_files(&[]));
    assert_snapshot(
        "upgrade-legacy-claude",
        upgrade_files(&["--agents", "claude"]),
    );
}

/// A real AGENTS.md from `seite init` 0.20.1 (tests/fixtures/agents-md-0.20.1.md)
/// after `seite upgrade`: the sections for retired features are gone.
#[test]
fn test_harness_snapshot_upgrade_020_agents_md() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Fixture Site", "posts,docs,pages");
    let site_dir = tmp.path().join("site");
    let legacy = include_str!("../fixtures/agents-md-0.20.1.md");
    fs::write(site_dir.join("AGENTS.md"), legacy).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let upgraded = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    for gone in [
        "### Theme Builder Skill",
        "### Brand Identity Builder Skill",
        "### Landing Page Builder Skill",
        "### Dev Server REPL",
        "Run `/theme-builder`",
    ] {
        assert!(!upgraded.contains(gone), "{gone} still in:\n{upgraded}");
    }
    assert_eq!(upgraded.matches("/seite brand").count(), 1, "{upgraded}");
    assert!(upgraded.contains("## Contact Forms"));
    assert!(upgraded.contains("## Verify Your Change"));
    assert_snapshot(
        "upgrade-020-agents-md",
        vec![("AGENTS.md".to_string(), upgraded)],
    );
}
