use super::common::*;
use seite::cli::harness::Agent;
use seite::cli::harness_hooks::{hook_command, HookKind};

fn hooks_json(site_dir: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let mut full = vec!["--json", "hooks"];
    full.extend_from_slice(args);
    let output = page_cmd()
        .args(full)
        .current_dir(site_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    json_stdout(&output)
}

fn status_rows(site_dir: &std::path::Path, args: &[&str]) -> Vec<serde_json::Value> {
    let mut a = vec!["status"];
    a.extend_from_slice(args);
    let doc = hooks_json(site_dir, &a);
    assert_eq!(doc["command"], "hooks");
    doc["data"]["agents"].as_array().unwrap().clone()
}

fn row<'a>(rows: &'a [serde_json::Value], agent: &str) -> &'a serde_json::Value {
    rows.iter()
        .find(|r| r["agent"] == agent)
        .unwrap_or_else(|| panic!("no row for {agent}: {rows:?}"))
}

#[test]
fn test_hooks_status_reports_fresh_site_as_installed() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Hooks", "posts");
    let site_dir = tmp.path().join("site");

    let rows = status_rows(&site_dir, &[]);
    let ids: Vec<&str> = rows.iter().map(|r| r["agent"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        ["claude", "codex", "opencode", "cursor", "gemini", "copilot"]
    );
    for r in &rows {
        assert_eq!(r["stop"], true, "{r}");
        assert_eq!(r["post_edit"], true, "{r}");
    }
    assert_eq!(row(&rows, "claude")["file"], ".claude/settings.json");
    assert_eq!(row(&rows, "gemini")["file"], ".gemini/settings.json");
    assert_eq!(
        row(&rows, "opencode")["file"],
        ".opencode/plugins/seite-check.js"
    );

    page_cmd()
        .args(["hooks", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("installed"));
}

#[test]
fn test_hooks_status_default_follows_project_agents() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude,codex");
    let site_dir = tmp.path().join("site");

    let rows = status_rows(&site_dir, &[]);
    let ids: Vec<&str> = rows.iter().map(|r| r["agent"].as_str().unwrap()).collect();
    assert_eq!(ids, ["claude", "codex"]);

    // An explicit --agents overrides the stored selection.
    let rows = status_rows(&site_dir, &["--agents", "cursor"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["agent"], "cursor");
    assert_eq!(rows[0]["stop"], false);
    assert_eq!(rows[0]["post_edit"], false);
}

#[test]
fn test_hooks_status_reports_partial_install() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");

    // Drop only the post-edit tier.
    let path = site_dir.join(".claude/settings.json");
    let mut settings = read_json_file(&path);
    settings["hooks"]
        .as_object_mut()
        .unwrap()
        .remove("PostToolUse");
    fs::write(&path, serde_json::to_string_pretty(&settings).unwrap()).unwrap();

    let rows = status_rows(&site_dir, &[]);
    assert_eq!(rows[0]["stop"], true);
    assert_eq!(rows[0]["post_edit"], false);
    page_cmd()
        .args(["hooks", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("partial"));

    // `on` completes it.
    page_cmd()
        .args(["hooks", "on"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let rows = status_rows(&site_dir, &[]);
    assert_eq!(rows[0]["stop"], true);
    assert_eq!(rows[0]["post_edit"], true);
}

#[test]
fn test_hooks_off_and_on_round_trip_keeps_user_hooks() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude,codex,cursor,opencode,gemini");
    let site_dir = tmp.path().join("site");

    // User hooks next to seite's, in every JSON-configured agent.
    let user =
        |text: &str| serde_json::json!({ "hooks": [{ "type": "command", "command": text }] });
    let claude_path = site_dir.join(".claude/settings.json");
    let mut claude = read_json_file(&claude_path);
    claude["hooks"]["Stop"]
        .as_array_mut()
        .unwrap()
        .push(user("my-claude-stop"));
    claude["hooks"]["PreToolUse"] = serde_json::json!([user("my-pre")]);
    fs::write(&claude_path, serde_json::to_string_pretty(&claude).unwrap()).unwrap();

    let cursor_path = site_dir.join(".cursor/hooks.json");
    let mut cursor = read_json_file(&cursor_path);
    cursor["hooks"]["afterFileEdit"] = serde_json::json!([{ "command": "fmt" }]);
    cursor["hooks"]["stop"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "command": "my-cursor-stop" }));
    fs::write(&cursor_path, serde_json::to_string_pretty(&cursor).unwrap()).unwrap();

    let gemini_path = site_dir.join(".gemini/settings.json");
    let mut gemini = read_json_file(&gemini_path);
    gemini["theme"] = serde_json::json!("dark");
    fs::write(&gemini_path, serde_json::to_string_pretty(&gemini).unwrap()).unwrap();

    let doc = hooks_json(&site_dir, &["off"]);
    let changes = doc["data"]["agents"].as_array().unwrap();
    assert_eq!(changes.len(), 5);
    for c in changes {
        assert_eq!(c["change"], "removed", "{c}");
    }
    for r in status_rows(&site_dir, &[]) {
        assert_eq!(r["stop"], false, "{r}");
        assert_eq!(r["post_edit"], false, "{r}");
    }

    // Only seite's entries are gone.
    let claude = read_json_file(&claude_path);
    let stop = claude["hooks"]["Stop"].to_string();
    assert!(stop.contains("my-claude-stop"), "{stop}");
    assert!(!stop.contains("seite check"), "{stop}");
    assert!(claude["hooks"].get("PostToolUse").is_none(), "{claude}");
    assert!(claude["hooks"]["PreToolUse"].to_string().contains("my-pre"));
    let cursor = read_json_file(&cursor_path);
    assert_eq!(cursor["hooks"]["afterFileEdit"][0]["command"], "fmt");
    assert_eq!(cursor["hooks"]["stop"][0]["command"], "my-cursor-stop");
    assert_eq!(cursor["hooks"]["stop"].as_array().unwrap().len(), 1);
    let gemini = read_json_file(&gemini_path);
    assert_eq!(gemini["theme"], "dark");
    assert!(!gemini.to_string().contains("seite check"), "{gemini}");
    assert!(!site_dir.join(".opencode/plugins/seite-check.js").exists());
    // `off` keeps the record so `seite upgrade` doesn't re-add the hooks.
    assert_eq!(
        read_json_file(&site_dir.join(".seite/config.json"))["hooks_installed"],
        serde_json::json!([
            "claude",
            "claude:post-edit",
            "codex",
            "codex:post-edit",
            "opencode",
            "opencode:post-edit",
            "cursor",
            "cursor:post-edit",
            "gemini",
            "gemini:post-edit"
        ])
    );

    // A second `off` has nothing left to do.
    let doc = hooks_json(&site_dir, &["off"]);
    for c in doc["data"]["agents"].as_array().unwrap() {
        assert_eq!(c["change"], "unchanged", "{c}");
    }

    // `on` brings both tiers back next to the user's hooks.
    let doc = hooks_json(&site_dir, &["on"]);
    for c in doc["data"]["agents"].as_array().unwrap() {
        assert_eq!(c["change"], "installed", "{c}");
    }
    for r in status_rows(&site_dir, &[]) {
        assert_eq!(r["stop"], true, "{r}");
        assert_eq!(r["post_edit"], true, "{r}");
    }
    let claude = read_json_file(&claude_path);
    let stop = claude["hooks"]["Stop"].to_string();
    assert!(stop.contains("my-claude-stop"), "{stop}");
    assert!(stop.contains(&hook_command(Agent::Claude, HookKind::Stop)));
    assert!(claude["hooks"]["PostToolUse"]
        .to_string()
        .contains(&hook_command(Agent::Claude, HookKind::PostEdit)));
    let cursor = read_json_file(&cursor_path);
    assert_eq!(cursor["hooks"]["afterFileEdit"][0]["command"], "fmt");

    // Idempotent.
    let doc = hooks_json(&site_dir, &["on"]);
    for c in doc["data"]["agents"].as_array().unwrap() {
        assert_eq!(c["change"], "unchanged", "{c}");
    }
}

#[test]
fn test_hooks_off_with_agents_subset_only_touches_those_agents() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Hooks Subset", "posts");
    let site_dir = tmp.path().join("site");

    let doc = hooks_json(&site_dir, &["off", "--agents", "codex,gemini"]);
    let changes = doc["data"]["agents"].as_array().unwrap();
    let ids: Vec<&str> = changes
        .iter()
        .map(|c| c["agent"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["codex", "gemini"]);

    let rows = status_rows(&site_dir, &[]);
    for agent in ["codex", "gemini"] {
        assert_eq!(row(&rows, agent)["stop"], false, "{agent}");
        assert_eq!(row(&rows, agent)["post_edit"], false, "{agent}");
    }
    for agent in ["claude", "opencode", "cursor"] {
        assert_eq!(row(&rows, agent)["stop"], true, "{agent}");
        assert_eq!(row(&rows, agent)["post_edit"], true, "{agent}");
    }

    // Subset `on` restores only the named agent.
    hooks_json(&site_dir, &["on", "--agents", "gemini"]);
    let rows = status_rows(&site_dir, &[]);
    assert_eq!(row(&rows, "gemini")["stop"], true);
    assert_eq!(row(&rows, "codex")["stop"], false);
}

#[test]
fn test_hooks_on_creates_missing_hook_files() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");
    assert!(!site_dir.join(".codex/hooks.json").exists());

    hooks_json(&site_dir, &["on", "--agents", "codex,opencode"]);
    let codex = read_json_file(&site_dir.join(".codex/hooks.json"));
    assert_eq!(
        codex["hooks"]["Stop"][0]["hooks"][0]["command"],
        hook_command(Agent::Codex, HookKind::Stop)
    );
    assert_eq!(codex["hooks"]["PostToolUse"][0]["matcher"], "^apply_patch$");
    assert!(site_dir.join(".opencode/plugins/seite-check.js").is_file());
}

#[test]
fn test_hooks_leaves_unparseable_config_alone() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");
    fs::create_dir_all(site_dir.join(".codex")).unwrap();
    let broken = "{ not json";
    fs::write(site_dir.join(".codex/hooks.json"), broken).unwrap();

    let doc = hooks_json(&site_dir, &["on", "--agents", "codex"]);
    assert_eq!(doc["data"]["agents"][0]["change"], "skipped");
    assert_eq!(
        fs::read_to_string(site_dir.join(".codex/hooks.json")).unwrap(),
        broken
    );
}

#[test]
fn test_hooks_outside_project_fails() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["hooks", "status"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No seite.toml"));
}
