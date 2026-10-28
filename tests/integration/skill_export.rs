//! `seite skill export` and the copies of the `seite` skill and Claude Code
//! plugin checked into this repository (`skills/`, `plugins/seite/`,
//! `.claude-plugin/marketplace.json`), which `npx skills add seite-sh/seite`
//! and `/plugin marketplace add seite-sh/seite` install from.

use super::common::*;
use seite::cli::skill_export::{self, ExportFormat, REGENERATE_COMMAND};

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The checked-in copy must be exactly what a fresh render produces: same
/// file set (no missing, no stale files), same bytes.
#[test]
fn test_skill_export_checked_in_copy_is_current() {
    let root = repo_root();
    let mut problems = Vec::new();
    for (rel, expected) in skill_export::render(ExportFormat::Repo) {
        match fs::read_to_string(root.join(&rel)) {
            Ok(actual) if actual == expected => {}
            Ok(_) => problems.push(format!("differs: {rel}")),
            Err(_) => problems.push(format!("missing: {rel}")),
        }
    }
    for stale in skill_export::stale_files(&root, ExportFormat::Repo) {
        problems.push(format!(
            "stale: {}",
            stale.strip_prefix(&root).unwrap_or(&stale).display()
        ));
    }
    assert!(
        problems.is_empty(),
        "the checked-in skill/plugin copy is out of date:\n  {}\nRegenerate it with `{REGENERATE_COMMAND}` and commit the result.",
        problems.join("\n  ")
    );
}

#[test]
fn test_skill_export_cli_writes_skills_format() {
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("out");
    page_cmd()
        .args(["skill", "export"])
        .arg(&out)
        .assert()
        .success();
    let skill = fs::read_to_string(out.join("seite/SKILL.md")).unwrap();
    assert!(
        skill.starts_with("---\nname: seite\ndescription: "),
        "{skill}"
    );
    assert!(!skill.contains("argument-hint"));
    assert!(out.join("seite/reference/check.md").is_file());
    assert!(!out.join(".claude-plugin").exists());
}

#[test]
fn test_skill_export_cli_claude_plugin_json_output() {
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("plugin");
    let output = page_cmd()
        .args(["--json", "skill", "export", "--format", "claude-plugin"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    let total = skill_export::render(ExportFormat::ClaudePlugin).len();
    assert_eq!(doc["data"]["files"], total);
    assert_eq!(doc["data"]["written"], total);
    for rel in [
        ".claude-plugin/plugin.json",
        "hooks/hooks.json",
        "skills/seite/SKILL.md",
        "skills/seite/reference/deploy.md",
    ] {
        assert!(out.join(rel).is_file(), "{rel}");
    }
    assert!(!out.join(".mcp.json").exists());
    // A second export changes nothing.
    let again = page_cmd()
        .args(["--json", "skill", "export", "--format", "claude-plugin"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(json_stdout(&again)["data"]["written"], 0);
}

/// Plugin hooks fire in every project. Outside a seite site the hook must
/// exit 0 and print nothing, with or without hook input.
#[test]
fn test_check_hook_silent_outside_site() {
    let tmp = TempDir::new().unwrap();
    for args in [
        vec!["check", "--hook", "claude"],
        vec!["check", "--hook", "claude", "--post-edit"],
        vec!["check", "--hook", "copilot"],
    ] {
        for stdin in [
            String::new(),
            format!(
                r#"{{"cwd":{:?},"stop_hook_active":false,"tool_input":{{"file_path":"x.md"}}}}"#,
                tmp.path().display().to_string()
            ),
        ] {
            let output = page_cmd()
                .args(&args)
                .current_dir(tmp.path())
                .env_remove("CLAUDE_PROJECT_DIR")
                .env_remove("CURSOR_PROJECT_DIR")
                .env_remove("GEMINI_PROJECT_DIR")
                .write_stdin(stdin)
                .output()
                .unwrap();
            assert!(output.status.success(), "{args:?}");
            assert!(output.stdout.is_empty(), "{args:?}: {:?}", output.stdout);
            assert!(output.stderr.is_empty(), "{args:?}: {:?}", output.stderr);
        }
    }
}

/// Inside a site whose `.claude/settings.json` already runs seite's hooks
/// (`seite init`), the plugin's copy (`CLAUDE_PLUGIN_ROOT` set) stays quiet;
/// the project's own copy still reports.
#[test]
fn test_check_hook_plugin_copy_defers_to_project_hook() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Hook Dedupe", "posts");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/broken.md"),
        "---\ntitle: [unclosed\n---\nbody\n",
    )
    .unwrap();
    let run = |plugin_root: Option<&str>| {
        let mut cmd = page_cmd();
        cmd.args(["check", "--hook", "claude"])
            .current_dir(&site)
            .env_remove("CLAUDE_PROJECT_DIR")
            .env_remove("CURSOR_PROJECT_DIR")
            .env_remove("GEMINI_PROJECT_DIR")
            .env_remove("CLAUDE_PLUGIN_ROOT")
            .write_stdin(r#"{"stop_hook_active":false}"#);
        if let Some(root) = plugin_root {
            cmd.env("CLAUDE_PLUGIN_ROOT", root);
        }
        cmd.output().unwrap()
    };
    let project = run(None);
    assert!(project.status.success());
    assert!(
        String::from_utf8_lossy(&project.stdout).contains("additionalContext"),
        "{}",
        String::from_utf8_lossy(&project.stdout)
    );
    let plugin = run(Some("/plugins/seite"));
    assert!(plugin.status.success());
    assert!(plugin.stdout.is_empty());

    // Without the project hook, the plugin's copy reports.
    page_cmd()
        .args(["hooks", "off", "--agents", "claude"])
        .current_dir(&site)
        .assert()
        .success();
    let plugin = run(Some("/plugins/seite"));
    assert!(String::from_utf8_lossy(&plugin.stdout).contains("additionalContext"));
}
