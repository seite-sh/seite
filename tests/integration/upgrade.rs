use super::common::*;
use seite::cli::harness::Agent;
use seite::cli::harness_hooks::{hook_command, HookKind};

#[test]
fn test_upgrade_migrates_claude_md_to_agents_md() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Instruction Migration", "posts,pages");
    let site_dir = tmp.path().join("site");
    let legacy_instructions = "# Legacy site\n\nKeep this custom instruction.\n";

    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::write(site_dir.join("CLAUDE.md"), legacy_instructions).unwrap();

    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let outdated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.17.1");
    fs::write(&meta_path, outdated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Existing instructions are kept verbatim; upgrade only appends the
    // seite-owned per-agent MCP table and rules index after them.
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(
        agents_md.starts_with(legacy_instructions),
        "upgrade should preserve all existing project instructions: {agents_md}"
    );
    assert!(agents_md.contains("<!-- seite:agent-setup -->"));
    assert_eq!(
        fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n",
        "upgrade should replace the migrated file with an import shim"
    );
}

#[test]
fn test_upgrade_preserves_claude_specific_instructions_when_agents_md_exists() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Instruction Compatibility", "posts,pages");
    let site_dir = tmp.path().join("site");
    let canonical = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    fs::write(
        site_dir.join("CLAUDE.md"),
        "# Claude-specific\n\nKeep this Claude-only instruction.\n",
    )
    .unwrap();

    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let outdated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.17.1");
    fs::write(&meta_path, outdated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(site_dir.join("AGENTS.md")).unwrap(),
        canonical,
        "upgrade should not overwrite an existing canonical file"
    );
    let claude_md = fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap();
    assert!(claude_md.starts_with("@AGENTS.md\n"));
    assert!(claude_md.contains("Keep this Claude-only instruction."));
    assert_eq!(claude_md.matches("@AGENTS.md").count(), 1);
}

#[test]
fn test_upgrade_creates_claude_shim_when_only_agents_md_exists() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Instruction Shim", "posts,pages");
    let site_dir = tmp.path().join("site");
    fs::remove_file(site_dir.join("CLAUDE.md")).unwrap();

    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let outdated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.17.1");
    fs::write(&meta_path, outdated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n"
    );
}

#[test]
fn test_upgrade_normalizes_duplicate_agents_imports() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Instruction Normalization", "posts,pages");
    let site_dir = tmp.path().join("site");
    fs::write(
        site_dir.join("CLAUDE.md"),
        "@AGENTS.md\n\nKeep this Claude-only instruction.\n@AGENTS.md\n",
    )
    .unwrap();

    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let outdated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.17.1");
    fs::write(&meta_path, outdated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let claude_md = fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap();
    assert_eq!(claude_md.matches("@AGENTS.md").count(), 1);
    assert!(claude_md.contains("Keep this Claude-only instruction."));
}

#[test]
fn test_upgrade_migrates_instructions_created_by_an_older_step() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Late Instruction Migration", "posts,pages");
    let site_dir = tmp.path().join("site");
    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::remove_file(site_dir.join("CLAUDE.md")).unwrap();
    fs::write(
        site_dir.join("templates/base.html"),
        "<!doctype html><html><head></head><body></body></html>",
    )
    .unwrap();

    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let outdated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.7.0");
    fs::write(&meta_path, outdated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("Custom Template: Atom Autodiscovery"));
    assert_eq!(
        fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n"
    );
}

#[test]
fn test_upgrade_does_not_stamp_version_when_instruction_migration_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Failed Instruction Migration", "posts,pages");
    let site_dir = tmp.path().join("site");
    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::remove_file(site_dir.join("CLAUDE.md")).unwrap();
    fs::create_dir(site_dir.join("CLAUDE.md")).unwrap();
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .failure();

    assert!(
        !site_dir.join(".seite/config.json").exists(),
        "the version marker must only be written after every upgrade succeeds"
    );
}

#[test]
fn test_upgrade_rejects_agents_md_directory_without_stamping_version() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Invalid Agent Instructions", "posts,pages");
    let site_dir = tmp.path().join("site");
    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::create_dir(site_dir.join("AGENTS.md")).unwrap();
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .failure();

    assert!(
        !site_dir.join(".seite/config.json").exists(),
        "an invalid AGENTS.md path must not be stamped as migrated"
    );
}

#[cfg(unix)]
#[test]
fn test_upgrade_rejects_symlinked_instructions_before_applying_actions() {
    use std::os::unix::fs::symlink;

    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Symlinked Agent Instructions", "posts,pages");
    let site_dir = tmp.path().join("site");
    let external_instructions = tmp.path().join("external-instructions.md");
    let original_instructions = "# External instructions\n";
    fs::write(&external_instructions, original_instructions).unwrap();

    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::remove_file(site_dir.join("CLAUDE.md")).unwrap();
    symlink(&external_instructions, site_dir.join("CLAUDE.md")).unwrap();

    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let outdated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.7.0");
    fs::write(&meta_path, &outdated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .failure();

    assert_eq!(
        fs::read_to_string(&external_instructions).unwrap(),
        original_instructions,
        "upgrade must reject a symlink before older actions can write through it"
    );
    assert!(!site_dir.join("AGENTS.md").exists());
    assert_eq!(fs::read_to_string(meta_path).unwrap(), outdated);
}

#[cfg(unix)]
#[test]
fn test_upgrade_rejects_read_only_instructions_before_applying_actions() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Read-only Agent Instructions", "posts,pages");
    let site_dir = tmp.path().join("site");
    let claude_path = site_dir.join("CLAUDE.md");
    let original_instructions = "# Legacy instructions\n";

    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::write(&claude_path, original_instructions).unwrap();
    fs::set_permissions(&claude_path, fs::Permissions::from_mode(0o444)).unwrap();
    fs::remove_file(site_dir.join(".claude/settings.json")).unwrap();

    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let outdated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.0.9");
    fs::write(&meta_path, &outdated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .failure();

    assert_eq!(
        fs::read_to_string(&claude_path).unwrap(),
        original_instructions
    );
    assert!(
        !site_dir.join(".claude/settings.json").exists(),
        "upgrade must reject read-only instructions before applying unrelated actions"
    );
    assert_eq!(fs::read_to_string(meta_path).unwrap(), outdated);
}

#[cfg(unix)]
#[test]
fn test_upgrade_rejects_unreadable_instructions_before_applying_actions() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Unreadable Agent Instructions", "posts,pages");
    let site_dir = tmp.path().join("site");
    let claude_path = site_dir.join("CLAUDE.md");
    let original_instructions = "# Legacy instructions\n";

    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::write(&claude_path, original_instructions).unwrap();
    fs::set_permissions(&claude_path, fs::Permissions::from_mode(0o200)).unwrap();
    fs::write(
        site_dir.join("templates/base.html"),
        r#"<link rel="alternate" type="application/rss+xml" href="/feed.xml">"#,
    )
    .unwrap();

    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let outdated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.7.0");
    fs::write(&meta_path, &outdated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .failure();

    fs::set_permissions(&claude_path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        fs::read_to_string(&claude_path).unwrap(),
        original_instructions,
        "upgrade must reject unreadable instructions before an append can truncate them"
    );
    assert!(!site_dir.join("AGENTS.md").exists());
    assert_eq!(fs::read_to_string(meta_path).unwrap(), outdated);
}

#[test]
fn test_upgrade_rejects_non_utf8_instructions_before_applying_actions() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Non-UTF-8 Agent Instructions", "posts,pages");
    let site_dir = tmp.path().join("site");
    let claude_path = site_dir.join("CLAUDE.md");
    let original_instructions = b"# Legacy instructions\n\xff\xfe\n";

    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::write(&claude_path, original_instructions).unwrap();
    fs::write(
        site_dir.join("templates/base.html"),
        r#"<link rel="alternate" type="application/rss+xml" href="/feed.xml">"#,
    )
    .unwrap();

    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let outdated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.7.0");
    fs::write(&meta_path, &outdated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .failure();

    assert_eq!(
        fs::read(&claude_path).unwrap(),
        original_instructions,
        "upgrade must reject invalid UTF-8 before an append can replace it"
    );
    assert!(!site_dir.join("AGENTS.md").exists());
    assert_eq!(fs::read_to_string(meta_path).unwrap(), outdated);
}

#[test]
fn test_upgrade_on_fresh_project_is_noop() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Up To Date", "posts,pages");
    let site_dir = tmp.path().join("site");

    // A freshly initialized project should already be up to date
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

#[test]
fn test_upgrade_adds_mcp_to_existing_project() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade MCP", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Simulate a pre-MCP project by removing the version stamp. MCP is
    // opt-in, so init wrote no MCP config.
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();
    assert!(!site_dir.join(".mcp.json").exists());

    // Write a .claude/settings.json WITHOUT mcpServers
    fs::write(
        site_dir.join(".claude/settings.json"),
        r#"{
  "permissions": {
    "allow": ["Read", "Glob", "Grep"],
    "deny": ["Read(.env)"]
  }
}
"#,
    )
    .unwrap();

    // Run upgrade with MCP opted in
    page_cmd()
        .args(["upgrade", "--force", "--mcp"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(".mcp.json"));

    // Verify MCP was added where Claude Code reads it
    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site_dir.join(".mcp.json")).unwrap()).unwrap();
    assert!(
        mcp.pointer("/mcpServers/seite").is_some(),
        "upgrade should add mcpServers.seite to .mcp.json"
    );
    let content = fs::read_to_string(site_dir.join(".claude/settings.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(settings.get("mcpServers").is_none());
    assert_eq!(settings["enabledMcpjsonServers"][0], "seite");

    // Verify existing permissions were preserved
    assert!(
        settings
            .pointer("/permissions/allow")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().any(|v| v.as_str() == Some("Read")))
            .unwrap_or(false),
        "upgrade should preserve existing permissions"
    );

    // Verify .seite/config.json was created
    assert!(
        site_dir.join(".seite/config.json").exists(),
        "upgrade should create .seite/config.json"
    );
}

#[test]
fn test_upgrade_installs_private_collections_rule() {
    // Existing sites get the rule when they run `seite upgrade`.
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Gated Upgrade", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Simulate a project from before the rule existed.
    fs::remove_file(site_dir.join(".claude/rules/private-collections.md")).unwrap();
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(
        site_dir
            .join(".claude/rules/private-collections.md")
            .exists(),
        "upgrade should install the private-collections rule"
    );
}

#[test]
fn test_upgrade_check_mode_exits_nonzero() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Check Mode", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove version stamp to make it look outdated
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["upgrade", "--check"])
        .current_dir(&site_dir)
        .assert()
        .failure(); // exit 1 = upgrades needed
}

#[test]
fn test_upgrade_check_mode_succeeds_when_current() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Check Current", "posts,pages");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["upgrade", "--check"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

#[test]
fn test_upgrade_preserves_existing_mcp_servers() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Preserve MCP", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove version stamp to trigger upgrade
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    // Write settings with a DIFFERENT MCP server (e.g., user has their own)
    fs::write(
        site_dir.join(".claude/settings.json"),
        r#"{
  "permissions": { "allow": ["Read"] },
  "mcpServers": {
    "custom-server": {
      "command": "my-tool",
      "args": ["serve"]
    }
  }
}
"#,
    )
    .unwrap();

    page_cmd()
        .args(["upgrade", "--force", "--mcp"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Both MCP servers end up in .mcp.json (the file Claude Code reads)
    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site_dir.join(".mcp.json")).unwrap()).unwrap();
    assert!(
        mcp.pointer("/mcpServers/seite").is_some(),
        "upgrade should keep the seite MCP server"
    );
    assert_eq!(
        mcp.pointer("/mcpServers/custom-server/command")
            .and_then(|v| v.as_str()),
        Some("my-tool"),
        "upgrade should move existing MCP servers into .mcp.json"
    );
}

#[test]
fn test_upgrade_migrates_legacy_settings_json() {
    // Sites created before .mcp.json support carried `mcpServers` in
    // .claude/settings.json, which Claude Code never loads.
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Legacy MCP", "posts,pages");
    let site_dir = tmp.path().join("site");
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    fs::write(
        &meta_path,
        meta_content.replace(env!("CARGO_PKG_VERSION"), "0.19.0"),
    )
    .unwrap();
    fs::write(
        site_dir.join(".claude/settings.json"),
        r#"{
  "permissions": {
    "allow": ["Read", "Write(static/**)", "Bash(seite build:*)"],
    "deny": ["Read(.env)"]
  },
  "mcpServers": { "seite": { "command": "seite", "args": ["mcp"] } }
}
"#,
    )
    .unwrap();

    page_cmd()
        .args(["upgrade", "--force", "--mcp"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(".mcp.json"))
        .stdout(predicate::str::contains("mcp__seite"));

    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site_dir.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["seite"]["command"], "seite");

    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(site_dir.join(".claude/settings.json")).unwrap())
            .unwrap();
    assert!(settings.get("mcpServers").is_none());
    assert_eq!(
        settings["enabledMcpjsonServers"],
        serde_json::json!(["seite"])
    );
    let allow: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for rule in [
        "Read",
        "Bash(seite build:*)",
        "mcp__seite",
        "Edit(static/**)",
        "Edit(seite.toml)",
    ] {
        assert!(allow.contains(&rule), "missing {rule}: {allow:?}");
    }
    assert_eq!(settings["permissions"]["deny"][0], "Read(.env)");

    // Idempotent: nothing left to migrate.
    page_cmd()
        .args(["upgrade", "--check", "--mcp"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[test]
fn test_upgrade_outside_project_fails() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["upgrade"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No seite.toml"));
}

#[test]
fn test_upgrade_idempotent() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Idempotent", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove version stamp so first upgrade does work
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    // First upgrade
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Second upgrade should be a no-op
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

#[test]
fn test_upgrade_creates_rules_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade Rules", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove rules files to simulate old project
    fs::remove_dir_all(site_dir.join(".claude/rules")).unwrap();
    assert!(!site_dir.join(".claude/rules").exists());

    // Reset version to trigger upgrade
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify rules were created
    assert!(site_dir.join(".claude/rules/seo-requirements.md").exists());
    assert!(site_dir.join(".claude/rules/templates.md").exists());
    assert!(site_dir.join(".claude/rules/i18n.md").exists());
    assert!(site_dir.join(".claude/rules/features.md").exists());
}

#[test]
fn test_upgrade_stamps_version_even_when_no_actions() {
    let tmp = TempDir::new().unwrap();
    // The agents a project from before the stored selection was set up for.
    init_site_with_agents(&tmp, "site", "claude,codex,opencode,cursor,gemini");
    let site_dir = tmp.path().join("site");

    // Simulate a project whose files are already up to date but whose
    // .seite/config.json records an older version (e.g. 0.20.0 → current).
    let meta_path = site_dir.join(".seite/config.json");
    fs::write(
        &meta_path,
        r#"{"version": "0.20.0", "initialized_at": "2026-01-01T00:00:00+00:00"}"#,
    )
    .unwrap();

    // Upgrade should report "up to date" (no file actions) but still stamp the version
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));

    // Verify the version was updated to the current binary version
    let content = fs::read_to_string(&meta_path).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
        meta["version"].as_str().unwrap(),
        env!("CARGO_PKG_VERSION"),
        ".seite/config.json version should be stamped to current binary version"
    );

    // Verify initialized_at was preserved
    assert_eq!(
        meta["initialized_at"].as_str().unwrap(),
        "2026-01-01T00:00:00+00:00",
        "initialized_at should be preserved from original config"
    );
}

#[test]
fn test_upgrade_adds_harness_files_without_clobbering() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");

    // User-owned configs for the other agents, written before seite manages them.
    fs::write(
        site_dir.join("opencode.json"),
        r#"{"$schema":"https://opencode.ai/config.json","model":"anthropic/claude-x","mcp":{"other":{"type":"local","command":["other"]}},"permission":{"edit":"deny"}}"#,
    )
    .unwrap();
    fs::create_dir_all(site_dir.join(".cursor")).unwrap();
    fs::write(
        site_dir.join(".cursor/mcp.json"),
        r#"{"mcpServers":{"other":{"command":"other"}}}"#,
    )
    .unwrap();
    fs::create_dir_all(site_dir.join(".codex")).unwrap();
    let codex_before = "# my codex notes — keep me\nmodel = \"gpt-x\" # pinned\n\n[mcp_servers.other]\ncommand = \"other\"\n";
    fs::write(site_dir.join(".codex/config.toml"), codex_before).unwrap();

    let output = page_cmd()
        .args(["--json", "upgrade", "--force", "--agents", "all", "--mcp"])
        .current_dir(&site_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    assert_eq!(
        doc["data"]["agents"],
        serde_json::json!([
            "claude",
            "codex",
            "opencode",
            "cursor",
            "gemini",
            "copilot",
            "antigravity"
        ])
    );

    let opencode = read_json_file(&site_dir.join("opencode.json"));
    assert_eq!(opencode["model"], "anthropic/claude-x");
    assert_eq!(opencode["mcp"]["other"]["command"][0], "other");
    assert_eq!(opencode["mcp"]["seite"]["command"][1], "mcp");
    assert_eq!(
        opencode["permission"],
        serde_json::json!({"edit": "deny"}),
        "an existing permission block must not be replaced"
    );

    let cursor = read_json_file(&site_dir.join(".cursor/mcp.json"));
    assert_eq!(cursor["mcpServers"]["other"]["command"], "other");
    assert_eq!(cursor["mcpServers"]["seite"]["command"], "seite");

    let codex = fs::read_to_string(site_dir.join(".codex/config.toml")).unwrap();
    assert!(
        codex.starts_with(codex_before),
        "codex config rewritten: {codex}"
    );
    let parsed: toml::Value = toml::from_str(&codex).unwrap();
    assert_eq!(
        parsed["mcp_servers"]["other"]["command"].as_str(),
        Some("other")
    );
    assert_eq!(
        parsed["mcp_servers"]["seite"]["command"].as_str(),
        Some("seite")
    );

    assert!(site_dir.join(".cursor/rules/templates.mdc").exists());
    assert!(site_dir.join(".agents/skills/seite/SKILL.md").exists());
    let gemini = read_json_file(&site_dir.join(".gemini/settings.json"));
    assert_eq!(gemini["mcpServers"]["seite"]["command"], "seite");
    assert!(
        gemini["hooks"]["AfterAgent"].is_array(),
        "gemini hooks kept alongside MCP: {gemini}"
    );
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("`.codex/config.toml`"));
    assert!(agents_md.contains("(Cursor: same guides as `.cursor/rules/*.mdc`; GitHub Copilot: same guides as `.github/instructions/*.instructions.md`; Antigravity CLI: same guides as `.agents/rules/*.md`)"));
    assert!(site_dir.join(".github/hooks/seite.json").exists());
    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!([
            "claude",
            "codex",
            "opencode",
            "cursor",
            "gemini",
            "copilot",
            "antigravity"
        ])
    );
}

#[test]
fn test_upgrade_is_idempotent_for_harness_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Idempotent Harness", "posts,pages");
    let site_dir = tmp.path().join("site");
    let fresh = harness_snapshot(&site_dir);

    // A fresh project needs nothing.
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert_eq!(harness_snapshot(&site_dir), fresh);

    // Missing files for selected agents come back exactly as init wrote them;
    // a second run is a no-op.
    fs::remove_file(site_dir.join("opencode.json")).unwrap();
    fs::remove_file(site_dir.join(".cursor/rules/templates.mdc")).unwrap();
    // (Not `.codex/hooks.json`: a deleted stop hook stays deleted, see
    // test_upgrade_stop_hooks_respect_opt_out.)
    fs::remove_file(site_dir.join(".agents/skills/seite/SKILL.md")).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("opencode.json"));
    assert_eq!(harness_snapshot(&site_dir), fresh);
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));

    // Sites without a stored selection get seite 0.20's agent set recorded
    // (Copilot stays opt-in; its existing files are kept, never deleted),
    // plus a one-time notice that Copilot is available.
    let meta_path = site_dir.join(".seite/config.json");
    let mut meta = read_json_file(&meta_path);
    meta.as_object_mut().unwrap().remove("agents");
    fs::write(&meta_path, meta.to_string()).unwrap();
    // Only AGENTS.md's setup table and rules index drop the Copilot rows.
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("per-agent setup table"))
        .stdout(predicate::str::contains(
            "--agents claude,codex,opencode,cursor,gemini,copilot",
        ));
    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!(["claude", "codex", "opencode", "cursor", "gemini"])
    );
    let without_agents_md = |files: Vec<(String, String)>| -> Vec<(String, String)> {
        files
            .into_iter()
            .filter(|(p, _)| p != "AGENTS.md")
            .collect()
    };
    assert_eq!(
        without_agents_md(harness_snapshot(&site_dir)),
        without_agents_md(fresh.clone())
    );
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(!agents_md.contains("| GitHub Copilot |"));
    // The notice is shown once.
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"))
        .stdout(predicate::str::contains("GitHub Copilot is supported").not());

    // Deselecting keeps files but stops maintaining them.
    page_cmd()
        .args(["upgrade", "--force", "--agents", "claude,codex"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No longer maintaining files for OpenCode, Cursor, Gemini CLI",
        ));
    assert!(site_dir.join("opencode.json").exists());
    assert!(site_dir.join(".cursor/hooks.json").exists());
    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!(["claude", "codex"])
    );
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(
        !agents_md.contains("`opencode.json`"),
        "table follows selection"
    );
    fs::remove_file(site_dir.join(".cursor/rules/templates.mdc")).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert!(!site_dir.join(".cursor/rules/templates.mdc").exists());
}

#[test]
fn test_upgrade_adds_seite_skill_and_opencode_command() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude,opencode");
    let site_dir = tmp.path().join("site");
    let fresh = harness_snapshot(&site_dir);
    let command = site_dir.join(".opencode/commands/seite.md");
    assert!(command.exists());
    assert!(site_dir.join(".claude/skills/seite/SKILL.md").exists());
    assert!(site_dir.join(".agents/skills/seite/SKILL.md").exists());

    // A site from before the seite skill: both come back exactly as init
    // wrote them, and an outdated command wrapper is refreshed.
    fs::remove_dir_all(site_dir.join(".claude/skills/seite")).unwrap();
    fs::remove_dir_all(site_dir.join(".agents/skills/seite")).unwrap();
    fs::write(&command, "---\ndescription: old\n---\nold\n").unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(".opencode/commands/seite.md"));
    assert_eq!(harness_snapshot(&site_dir), fresh);
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));

    // A newer (user-bumped) wrapper is left alone.
    let custom = "---\ndescription: mine\n# seite-skill-version: 99\n---\nmine\n";
    fs::write(&command, custom).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&command).unwrap(), custom);

    // Deselecting OpenCode stops maintaining the wrapper.
    fs::remove_file(&command).unwrap();
    page_cmd()
        .args(["upgrade", "--force", "--agents", "claude"])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert!(!command.exists());
}

#[test]
fn test_upgrade_fixes_deploy_workflow_cargo_install() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Deploy Fix", "posts");
    let site_dir = tmp.path().join("site");

    let workflow_path = site_dir.join(".github/workflows/deploy.yml");
    assert!(workflow_path.exists());

    // Verify the freshly generated workflow does NOT have cargo install
    let fresh = fs::read_to_string(&workflow_path).unwrap();
    assert!(
        !fresh.contains("cargo install --path ."),
        "fresh init should not use cargo install"
    );

    // Simulate an old workflow that uses cargo install
    let old_workflow = r#"name: Deploy to GitHub Pages

on:
  push:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Install seite
        run: cargo install --path .
      - name: Build site
        run: seite build
"#;
    fs::write(&workflow_path, old_workflow).unwrap();

    // Downgrade the project version so the upgrade step applies
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.5");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("deploy.yml"));

    let content = fs::read_to_string(&workflow_path).unwrap();
    assert!(
        !content.contains("cargo install --path ."),
        "upgrade should remove cargo install"
    );
    assert!(
        content.contains("curl -fsSL https://seite.sh/install.sh | sh"),
        "upgrade should use shell installer"
    );
    assert!(
        !content.contains("VERSION="),
        "upgrade should not pin seite version (installs latest)"
    );
    assert!(
        content.contains("seite build"),
        "workflow should still build the site"
    );
}

// --- upgrade command tests ---

#[test]
fn test_upgrade_force() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "upf", "Upgrade Force", "posts");
    let site_dir = tmp.path().join("upf");

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

fn stop_hook_ids(site_dir: &std::path::Path) -> serde_json::Value {
    read_json_file(&site_dir.join(".seite/config.json"))["hooks_installed"].clone()
}

#[test]
fn test_init_writes_stop_hooks_for_selected_agents() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Hooks", "posts");
    let site_dir = tmp.path().join("site");
    let settings = read_json_file(&site_dir.join(".claude/settings.json"));
    assert_eq!(
        settings["hooks"]["Stop"][0]["hooks"][0]["command"],
        hook_command(Agent::Claude, HookKind::Stop)
    );
    assert_eq!(
        settings["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
        hook_command(Agent::Claude, HookKind::PostEdit)
    );
    assert_eq!(
        settings["hooks"]["PostToolUse"][0]["matcher"],
        "Write|Edit|MultiEdit"
    );
    let codex = read_json_file(&site_dir.join(".codex/hooks.json"));
    assert_eq!(
        codex["hooks"]["Stop"][0]["hooks"][0]["command"],
        hook_command(Agent::Codex, HookKind::Stop)
    );
    assert_eq!(
        codex["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
        hook_command(Agent::Codex, HookKind::PostEdit)
    );
    let cursor = read_json_file(&site_dir.join(".cursor/hooks.json"));
    assert_eq!(cursor["version"], 1);
    assert_eq!(
        cursor["hooks"]["stop"][0]["command"],
        hook_command(Agent::Cursor, HookKind::Stop)
    );
    assert_eq!(
        cursor["hooks"]["postToolUse"][0]["command"],
        hook_command(Agent::Cursor, HookKind::PostEdit)
    );
    let gemini = read_json_file(&site_dir.join(".gemini/settings.json"));
    assert_eq!(
        gemini["hooks"]["AfterAgent"][0]["hooks"][0]["command"],
        hook_command(Agent::Gemini, HookKind::Stop)
    );
    assert_eq!(
        gemini["hooks"]["AfterTool"][0]["hooks"][0]["command"],
        hook_command(Agent::Gemini, HookKind::PostEdit)
    );
    let plugin = fs::read_to_string(site_dir.join(".opencode/plugins/seite-check.js")).unwrap();
    assert!(plugin.contains("seite check --hook opencode"));
    assert!(plugin.contains("tool.execute.after"));
    assert_eq!(
        stop_hook_ids(&site_dir),
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
            "gemini:post-edit",
            "copilot",
            "copilot:post-edit",
            "antigravity",
            "antigravity:post-edit",
        ])
    );

    init_site_with_agents(&tmp, "codex-only", "codex");
    let site_dir = tmp.path().join("codex-only");
    assert!(site_dir.join(".codex/hooks.json").exists());
    assert!(!site_dir.join(".cursor").exists() && !site_dir.join(".opencode").exists());
    assert_eq!(
        stop_hook_ids(&site_dir),
        serde_json::json!(["codex", "codex:post-edit"])
    );
}

#[test]
fn test_upgrade_adds_stop_hooks_to_existing_project() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Hooks", "posts");
    let site_dir = tmp.path().join("site");
    let fresh = harness_snapshot(&site_dir);

    // A project from before hooks: no record, no hook files, a user-owned
    // Cursor hook config and Claude settings without hooks.
    let meta_path = site_dir.join(".seite/config.json");
    let mut meta = read_json_file(&meta_path);
    meta.as_object_mut().unwrap().remove("hooks_installed");
    fs::write(&meta_path, meta.to_string()).unwrap();
    let settings_path = site_dir.join(".claude/settings.json");
    let mut settings = read_json_file(&settings_path);
    settings.as_object_mut().unwrap().remove("hooks");
    fs::write(
        &settings_path,
        format!("{}\n", serde_json::to_string_pretty(&settings).unwrap()),
    )
    .unwrap();
    fs::remove_file(site_dir.join(".codex/hooks.json")).unwrap();
    fs::remove_dir_all(site_dir.join(".opencode")).unwrap();
    fs::write(
        site_dir.join(".cursor/hooks.json"),
        r#"{"version":1,"hooks":{"afterFileEdit":[{"command":"fmt"}]}}"#,
    )
    .unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Added the seite check hooks to .claude/settings.json",
        ))
        .stdout(predicate::str::contains(
            ".codex/hooks.json (seite check hooks for Codex CLI)",
        ));

    let cursor = read_json_file(&site_dir.join(".cursor/hooks.json"));
    assert_eq!(cursor["hooks"]["afterFileEdit"][0]["command"], "fmt");
    assert_eq!(
        cursor["hooks"]["stop"][0]["command"],
        hook_command(Agent::Cursor, HookKind::Stop)
    );
    assert_eq!(
        cursor["hooks"]["postToolUse"][0]["command"],
        hook_command(Agent::Cursor, HookKind::PostEdit)
    );
    assert_eq!(
        stop_hook_ids(&site_dir),
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
            "gemini:post-edit",
            "copilot",
            "copilot:post-edit",
            "antigravity",
            "antigravity:post-edit",
        ])
    );
    // Everything except the user's own Cursor hook file matches a fresh init
    // (the Claude hook lands in settings.json exactly where init puts it).
    let without_cursor_hooks = |files: Vec<(String, String)>| {
        files
            .into_iter()
            .filter(|(p, _)| p != ".cursor/hooks.json")
            .collect::<Vec<_>>()
    };
    assert_eq!(
        without_cursor_hooks(harness_snapshot(&site_dir)),
        without_cursor_hooks(fresh)
    );

    // Idempotent.
    let before = harness_snapshot(&site_dir);
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert_eq!(harness_snapshot(&site_dir), before);
}

#[test]
fn test_upgrade_stop_hooks_respect_opt_out() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Hooks", "posts");
    let site_dir = tmp.path().join("site");

    // The user removes two hooks.
    let settings_path = site_dir.join(".claude/settings.json");
    let mut settings = read_json_file(&settings_path);
    settings.as_object_mut().unwrap().remove("hooks");
    fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .unwrap();
    fs::remove_file(site_dir.join(".cursor/hooks.json")).unwrap();
    let before = harness_snapshot(&site_dir);

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert_eq!(harness_snapshot(&site_dir), before);
    assert!(read_json_file(&settings_path).get("hooks").is_none());
    assert!(!site_dir.join(".cursor/hooks.json").exists());
}

#[test]
fn test_upgrade_leaves_malformed_cursor_configs_untouched_with_warning() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");
    fs::create_dir_all(site_dir.join(".cursor")).unwrap();
    let cli_json = "{ \"permissions\": { \"allow\": [ // comment\n] }";
    let mcp_json = "[\"not\", \"an object\"]";
    fs::write(site_dir.join(".cursor/cli.json"), cli_json).unwrap();
    fs::write(site_dir.join(".cursor/mcp.json"), mcp_json).unwrap();

    for _ in 0..2 {
        let output = page_cmd()
            .args(["upgrade", "--force", "--mcp", "--agents", "claude,cursor"])
            .current_dir(&site_dir)
            .output()
            .unwrap();
        assert!(output.status.success());
        let all = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            all.contains(".cursor/cli.json is not a JSON object; add seite's Cursor permissions to it manually"),
            "{all}"
        );
        assert!(
            all.contains(
                ".cursor/mcp.json is not a JSON object; add the seite MCP server to it manually"
            ),
            "{all}"
        );
        // Unparseable user files are never rewritten…
        assert_eq!(
            fs::read_to_string(site_dir.join(".cursor/cli.json")).unwrap(),
            cli_json
        );
        assert_eq!(
            fs::read_to_string(site_dir.join(".cursor/mcp.json")).unwrap(),
            mcp_json
        );
        // …and don't block the rest of the Cursor setup.
        assert!(site_dir.join(".cursor/rules/templates.mdc").is_file());
    }
}

#[test]
fn test_upgrade_merges_cursor_cli_json_keeping_user_rules_and_order() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");
    fs::create_dir_all(site_dir.join(".cursor")).unwrap();
    fs::write(
        site_dir.join(".cursor/cli.json"),
        r#"{"zeta":1,"permissions":{"deny":["Shell(rm)"],"allow":["Shell(ls)","Mcp(seite:*)"]},"alpha":true}"#,
    )
    .unwrap();

    page_cmd()
        .args(["upgrade", "--force", "--agents", "claude,cursor"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Added Write(content/**) to .cursor/cli.json",
        ))
        .stdout(predicate::str::contains("Added Mcp(seite:*)").not());

    let text = fs::read_to_string(site_dir.join(".cursor/cli.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    // User keys and rules kept, in their original order; seite's appended once.
    let keys: Vec<&String> = doc.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["zeta", "permissions", "alpha"]);
    assert_eq!(doc["permissions"]["deny"], serde_json::json!(["Shell(rm)"]));
    let allow: Vec<&str> = doc["permissions"]["allow"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(allow[..2], ["Shell(ls)", "Mcp(seite:*)"]);
    assert_eq!(
        allow.iter().filter(|r| **r == "Mcp(seite:*)").count(),
        1,
        "{allow:?}"
    );
    assert!(allow.contains(&"Write(seite.toml)"), "{allow:?}");

    // Idempotent: nothing left to add.
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert_eq!(
        fs::read_to_string(site_dir.join(".cursor/cli.json")).unwrap(),
        text
    );
}

#[test]
fn test_upgrade_stop_hook_unmergeable_settings_warns_and_retries_later() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Hooks", "posts");
    let site_dir = tmp.path().join("site");
    let meta_path = site_dir.join(".seite/config.json");
    let mut meta = read_json_file(&meta_path);
    meta.as_object_mut().unwrap().remove("hooks_installed");
    fs::write(&meta_path, meta.to_string()).unwrap();
    let settings_path = site_dir.join(".claude/settings.json");
    let broken = "{ \"permissions\": { \"allow\": [] }, }";
    fs::write(&settings_path, broken).unwrap();

    let output = page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        all.contains(&format!(
            ".claude/settings.json can't be merged automatically; add a Claude Code hook running `{}` by hand",
            hook_command(Agent::Claude, HookKind::Stop)
        )),
        "{all}"
    );
    assert_eq!(fs::read_to_string(&settings_path).unwrap(), broken);
    let ids = stop_hook_ids(&site_dir);
    assert!(
        !ids.as_array().unwrap().iter().any(|id| id == "claude"),
        "claude hook not recorded as installed: {ids}"
    );

    // Once the user fixes the file, the next upgrade adds the hook.
    fs::write(&settings_path, "{ \"permissions\": { \"allow\": [] } }").unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let settings = read_json_file(&settings_path);
    assert!(
        settings["hooks"]["Stop"]
            .to_string()
            .contains(&hook_command(Agent::Claude, HookKind::Stop)),
        "{settings}"
    );
    assert!(stop_hook_ids(&site_dir)
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "claude"));
}

#[test]
fn test_upgrade_old_project_injects_minify_into_build_section() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Old Minify", "posts");
    let site_dir = tmp.path().join("site");
    let toml_path = site_dir.join("seite.toml");
    let without: String = fs::read_to_string(&toml_path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim_start().starts_with("minify ="))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(without.contains("[build]"));
    fs::write(&toml_path, &without).unwrap();
    let meta_path = site_dir.join(".seite/config.json");
    let meta = fs::read_to_string(&meta_path).unwrap();
    fs::write(&meta_path, meta.replace(env!("CARGO_PKG_VERSION"), "0.6.0")).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("enabled minify = true in [build]"));

    let after = fs::read_to_string(&toml_path).unwrap();
    assert_eq!(
        after,
        without.replacen("[build]", "[build]\nminify = true", 1)
    );
    let parsed: toml::Value = toml::from_str(&after).unwrap();
    assert_eq!(parsed["build"]["minify"].as_bool(), Some(true));
}

#[test]
fn test_upgrade_old_project_without_claude_keeps_claude_files_out() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "codex,opencode");
    let site_dir = tmp.path().join("site");
    assert!(!site_dir.join(".claude").exists());
    assert!(!site_dir.join(".mcp.json").exists());
    fs::remove_dir_all(site_dir.join(".agents/rules")).unwrap();
    let meta_path = site_dir.join(".seite/config.json");
    let meta = fs::read_to_string(&meta_path).unwrap();
    fs::write(&meta_path, meta.replace(env!("CARGO_PKG_VERSION"), "0.6.0")).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Historical steps (MCP setup, Claude skills) must not recreate Claude
    // Code files for a project that doesn't use Claude Code.
    assert!(!site_dir.join(".claude").exists());
    assert!(!site_dir.join(".mcp.json").exists());
    // The shared rules directory Codex/OpenCode read is restored.
    assert!(site_dir.join(".agents/rules/templates.md").is_file());
}

/// Point `.seite/config.json` at an older version so `upgrade` has work to do.
fn set_project_version(site_dir: &std::path::Path, version: &str) {
    let meta_path = site_dir.join(".seite/config.json");
    let meta = fs::read_to_string(&meta_path).unwrap();
    fs::write(&meta_path, meta.replace(env!("CARGO_PKG_VERSION"), version)).unwrap();
}

#[test]
fn test_upgrade_without_mcp_leaves_existing_mcp_config_untouched() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No MCP", "posts,pages");
    let site_dir = tmp.path().join("site");
    assert!(!site_dir.join(".mcp.json").exists(), "init is MCP opt-in");

    let mcp_json = r#"{
  "mcpServers": {
    "custom-server": { "command": "my-tool", "args": ["serve"] }
  }
}
"#;
    let settings = r#"{
  "permissions": { "allow": ["Read"] },
  "mcpServers": { "legacy": { "command": "legacy-tool" } }
}
"#;
    fs::write(site_dir.join(".mcp.json"), mcp_json).unwrap();
    fs::write(site_dir.join(".claude/settings.json"), settings).unwrap();
    set_project_version(&site_dir, "0.19.0");

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(site_dir.join(".mcp.json")).unwrap(),
        mcp_json,
        "an existing .mcp.json must stay byte-identical without --mcp"
    );
    // Permissions may be topped up, but the legacy server block stays put and
    // no MCP enablement is added.
    let after = read_json_file(&site_dir.join(".claude/settings.json"));
    assert_eq!(
        after["mcpServers"],
        serde_json::json!({ "legacy": { "command": "legacy-tool" } }),
        "legacy mcpServers must not be migrated without --mcp"
    );
    assert!(after.get("enabledMcpjsonServers").is_none(), "{after}");
    assert!(!after.to_string().contains("mcp__seite"), "{after}");
    // No other agent gets MCP config either.
    assert!(!site_dir.join(".cursor/mcp.json").exists());
    assert!(!site_dir.join(".codex/config.toml").exists());
    let opencode = read_json_file(&site_dir.join("opencode.json"));
    assert!(opencode.get("mcp").is_none(), "{opencode}");
    let gemini = read_json_file(&site_dir.join(".gemini/settings.json"));
    assert!(gemini.get("mcpServers").is_none(), "{gemini}");
}

#[test]
fn test_upgrade_mcp_flag_adds_mcp_config_for_selected_agents() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "With MCP", "posts,pages");
    let site_dir = tmp.path().join("site");
    set_project_version(&site_dir, "0.19.0");

    page_cmd()
        .args([
            "upgrade",
            "--force",
            "--mcp",
            "--agents",
            "claude,codex,gemini",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    let mcp = read_json_file(&site_dir.join(".mcp.json"));
    assert_eq!(mcp["mcpServers"]["seite"]["command"], "seite");
    let settings = read_json_file(&site_dir.join(".claude/settings.json"));
    assert_eq!(
        settings["enabledMcpjsonServers"],
        serde_json::json!(["seite"])
    );
    assert!(settings["permissions"]["allow"]
        .to_string()
        .contains("mcp__seite"));
    let codex = fs::read_to_string(site_dir.join(".codex/config.toml")).unwrap();
    assert!(codex.contains("[mcp_servers.seite]"), "{codex}");
    let gemini = read_json_file(&site_dir.join(".gemini/settings.json"));
    assert_eq!(gemini["mcpServers"]["seite"]["command"], "seite");
    // Agents outside the selection get nothing.
    assert!(!site_dir.join(".cursor/mcp.json").exists());
    assert!(read_json_file(&site_dir.join("opencode.json"))
        .get("mcp")
        .is_none());

    // The per-agent table gains an MCP column once MCP is configured.
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("MCP config"), "{agents_md}");
}

#[test]
fn test_upgrade_merges_launch_json_add_only() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");
    let launch = site_dir.join(".claude/launch.json");
    assert!(launch.exists(), "init writes launch.json for Claude");

    // An existing file with another configuration gains `seite`, add-only.
    fs::write(
        &launch,
        r#"{"version":"0.0.1","configurations":[{"name":"web","runtimeExecutable":"npm","runtimeArgs":["run","dev"],"port":5173}]}"#,
    )
    .unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(".claude/launch.json"));
    let doc = read_json_file(&launch);
    let configs = doc["configurations"].as_array().unwrap();
    assert_eq!(configs.len(), 2);
    assert_eq!(configs[0]["name"], "web");
    assert_eq!(configs[0]["port"], 5173);
    let seite = configs
        .iter()
        .find(|c| c["name"] == "seite")
        .expect("seite configuration added");
    assert_eq!(seite["runtimeExecutable"], "seite");

    // Idempotent, and a deleted file is recreated.
    let before = fs::read_to_string(&launch).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert_eq!(fs::read_to_string(&launch).unwrap(), before);
    fs::remove_file(&launch).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let doc = read_json_file(&launch);
    assert_eq!(doc["configurations"][0]["name"], "seite");
}

#[test]
fn test_upgrade_adds_post_edit_tier_to_020_hooks_and_respects_hooks_off() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");

    // A 0.20-style site: only the bare Stop hook, recorded as installed.
    let settings_path = site_dir.join(".claude/settings.json");
    let mut settings = read_json_file(&settings_path);
    settings["hooks"] = serde_json::json!({
        "Stop": [{ "hooks": [{
            "type": "command",
            "command": "seite check --hook claude",
            "timeout": 60
        }] }]
    });
    fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .unwrap();
    set_project_version(&site_dir, "0.20.0");
    // seite 0.20 recorded a bare id: the turn-end tier only.
    let meta_path = site_dir.join(".seite/config.json");
    let mut meta = read_json_file(&meta_path);
    meta["hooks_installed"] = serde_json::json!(["claude"]);
    fs::write(&meta_path, meta.to_string()).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let settings = read_json_file(&settings_path);
    let stop = settings["hooks"]["Stop"].to_string();
    assert!(stop.contains(&hook_command(Agent::Claude, HookKind::Stop)));
    assert_eq!(
        settings["hooks"]["Stop"].as_array().unwrap().len(),
        1,
        "the bare command is wrapped in place, not duplicated: {stop}"
    );
    assert!(
        !stop.contains("\"seite check --hook claude\""),
        "bare command should be replaced: {stop}"
    );
    assert_eq!(
        settings["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
        hook_command(Agent::Claude, HookKind::PostEdit)
    );
    assert_eq!(
        settings["hooks"]["PostToolUse"][0]["matcher"],
        "Write|Edit|MultiEdit"
    );

    // After `seite hooks off`, an upgrade does not bring them back.
    page_cmd()
        .args(["hooks", "off", "--agents", "claude"])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert!(read_json_file(&settings_path).get("hooks").is_none());
    set_project_version(&site_dir, "0.20.1");
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert!(
        read_json_file(&settings_path).get("hooks").is_none(),
        "hooks off must survive upgrade"
    );
    assert_eq!(
        stop_hook_ids(&site_dir),
        serde_json::json!(["claude", "claude:post-edit"])
    );
}

#[test]
fn test_upgrade_keeps_a_deleted_post_edit_hook_deleted() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Hooks", "posts");
    let site_dir = tmp.path().join("site");
    let settings_path = site_dir.join(".claude/settings.json");
    let mut settings = read_json_file(&settings_path);
    settings["hooks"]
        .as_object_mut()
        .unwrap()
        .remove("PostToolUse");
    fs::write(
        &settings_path,
        format!("{}\n", serde_json::to_string_pretty(&settings).unwrap()),
    )
    .unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let settings = read_json_file(&settings_path);
    assert!(settings["hooks"].get("PostToolUse").is_none(), "{settings}");
    assert!(settings["hooks"].get("Stop").is_some());
}

/// A 0.20.1 settings.json needs both its permissions narrowed and its bare
/// Stop hook wrapped: one upgrade must do both (the hook refresh goes into the
/// pending settings rewrite), leaving nothing for a second run.
#[test]
fn test_upgrade_020_claude_settings_converge_in_one_run() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");
    fs::write(
        site_dir.join(".claude/settings.json"),
        include_str!("../fixtures/claude-settings-0.20.1.json"),
    )
    .unwrap();
    set_project_version(&site_dir, "0.20.1");
    let meta_path = site_dir.join(".seite/config.json");
    let mut meta = read_json_file(&meta_path);
    meta["hooks_installed"] = serde_json::json!(["claude"]);
    fs::write(&meta_path, meta.to_string()).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let settings = read_json_file(&site_dir.join(".claude/settings.json"));
    let stop = settings["hooks"]["Stop"].to_string();
    assert!(
        stop.contains(&hook_command(Agent::Claude, HookKind::Stop)),
        "{stop}"
    );
    assert!(!stop.contains("\"seite check --hook claude\""), "{stop}");
    assert!(
        !settings["permissions"]["allow"]
            .to_string()
            .contains("Bash(seite theme:*)"),
        "{settings}"
    );
    page_cmd()
        .args(["upgrade", "--check"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

#[test]
fn test_upgrade_narrows_legacy_theme_permissions() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Perms", "posts");
    let site_dir = tmp.path().join("site");
    // The permission files seite 0.20.1 generated (tests/fixtures/*-0.20.1.json).
    let mut settings: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/claude-settings-0.20.1.json")).unwrap();
    settings["permissions"]["allow"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("Bash(my-tool:*)"));
    fs::write(site_dir.join(".claude/settings.json"), settings.to_string()).unwrap();
    fs::write(
        site_dir.join("opencode.json"),
        include_str!("../fixtures/opencode-0.20.1.json"),
    )
    .unwrap();
    fs::write(
        site_dir.join(".cursor/cli.json"),
        include_str!("../fixtures/cursor-cli-0.20.1.json"),
    )
    .unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Narrowed"));

    let allow =
        read_json_file(&site_dir.join(".claude/settings.json"))["permissions"]["allow"].to_string();
    assert!(!allow.contains("Bash(seite theme:*)"), "{allow}");
    assert!(allow.contains("Bash(seite theme apply:*)"), "{allow}");
    assert!(allow.contains("Bash(my-tool:*)"), "{allow}");
    assert!(!allow.contains("seite agent"), "{allow}");
    let bash = read_json_file(&site_dir.join("opencode.json"))["permission"]["bash"].clone();
    assert!(bash.get("seite theme*").is_none(), "{bash}");
    assert_eq!(bash["seite theme apply*"], "allow");
    assert_eq!(bash["*"], "ask");
    let cursor =
        read_json_file(&site_dir.join(".cursor/cli.json"))["permissions"]["allow"].to_string();
    assert!(!cursor.contains("\"Shell(seite)\""), "{cursor}");
    assert!(cursor.contains("Shell(seite:theme apply*)"), "{cursor}");

    // A rule the user changed is theirs: left alone.
    let mut oc = read_json_file(&site_dir.join("opencode.json"));
    oc["permission"]["bash"]["seite theme*"] = serde_json::json!("ask");
    fs::write(site_dir.join("opencode.json"), oc.to_string()).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let bash = read_json_file(&site_dir.join("opencode.json"))["permission"]["bash"].clone();
    assert_eq!(bash["seite theme*"], "ask");
}

#[cfg(unix)]
#[test]
fn test_upgrade_refuses_to_write_through_a_symlinked_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Links", "posts");
    let site_dir = tmp.path().join("site");
    let outside = tmp.path().join("outside.md");
    fs::write(&outside, "# seite-skill-version: 1\nmine\n").unwrap();
    let skill = site_dir.join(".claude/skills/seite/SKILL.md");
    fs::remove_file(&skill).unwrap();
    std::os::unix::fs::symlink(&outside, &skill).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("symlink"))
        .stderr(predicate::str::contains(".claude/skills/seite/SKILL.md"));
    assert_eq!(
        fs::read_to_string(&outside).unwrap(),
        "# seite-skill-version: 1\nmine\n"
    );

    // A symlinked parent directory is refused too.
    fs::remove_file(&skill).unwrap();
    let outside_dir = tmp.path().join("outside-dir");
    fs::create_dir_all(&outside_dir).unwrap();
    fs::remove_dir_all(site_dir.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(&outside_dir, site_dir.join(".claude/skills")).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("symlink"));
    assert!(fs::read_dir(&outside_dir).unwrap().next().is_none());
}

#[test]
fn test_upgrade_keeps_edited_legacy_sections_with_one_time_notice() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Fixture Site", "posts,docs,pages");
    let site_dir = tmp.path().join("site");
    let edited = include_str!("../fixtures/agents-md-0.20.1.md").replace(
        "Run `/theme-builder` to create",
        "Run `/theme-builder` (our fork) to create",
    );
    fs::write(site_dir.join("AGENTS.md"), &edited).unwrap();
    let notice = "Kept the \"Theme Builder Skill\" section";

    // --check never prints the notice.
    page_cmd()
        .args(["upgrade", "--check"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stdout(predicate::str::contains(notice).not());
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(notice));
    let upgraded = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(upgraded.contains("(our fork)"));
    assert!(!upgraded.contains("### Brand Identity Builder Skill"));
    assert!(!upgraded.contains("### Dev Server REPL"));
    // Shown once.
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(notice).not());
}

#[test]
fn test_upgrade_kept_legacy_skill_notice_prints_once() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");
    let dir = site_dir.join(".claude/skills/theme-builder");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: theme-builder\n---\nmine\n",
    )
    .unwrap();
    let run = |args: &[&str]| {
        let out = page_cmd()
            .args(args)
            .current_dir(&site_dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    assert!(!run(&["upgrade", "--check"]).contains("Kept .claude/skills/theme-builder"));
    assert!(run(&["upgrade", "--force"]).contains("Kept .claude/skills/theme-builder"));
    assert!(!run(&["upgrade", "--force"]).contains("Kept .claude/skills/theme-builder"));
    assert!(dir.join("SKILL.md").exists());
}

/// A fresh site whose recorded version is rolled back to `version`.
fn site_recorded_at(tmp: &TempDir, version: &str) -> std::path::PathBuf {
    init_site(tmp, "site", "Brief Site", "posts,pages");
    let site_dir = tmp.path().join("site");
    let meta_path = site_dir.join(".seite/config.json");
    let content = fs::read_to_string(&meta_path).unwrap();
    fs::write(
        &meta_path,
        content.replace(env!("CARGO_PKG_VERSION"), version),
    )
    .unwrap();
    site_dir
}

#[test]
fn test_upgrade_agent_in_session_prints_brief_with_notes() {
    let tmp = TempDir::new().unwrap();
    let site_dir = site_recorded_at(&tmp, "0.19.0");
    // Inside an agent session (and without a PATH agent) nothing is launched.
    let out = page_cmd()
        .env("CLAUDECODE", "1")
        .env("PATH", "")
        .args(["upgrade", "--agent", "--force"])
        .current_dir(&site_dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("# seite upgrade: migration brief"),
        "{stdout}"
    );
    assert!(stdout.contains("from v0.19.0 to v"), "{stdout}");
    assert!(stdout.contains("### v0.20.0"), "{stdout}");
    assert!(stdout.contains("### v0.20.2"), "{stdout}");
    assert!(!stdout.contains("### v0.19.0"), "{stdout}");
    assert!(stdout.contains("## What to do"), "{stdout}");
    assert!(stdout.contains("`seite check`"), "{stdout}");
    // The normal upgrade ran first.
    let meta = fs::read_to_string(site_dir.join(".seite/config.json")).unwrap();
    assert!(meta.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_upgrade_agent_without_terminal_prints_brief() {
    let tmp = TempDir::new().unwrap();
    let site_dir = site_recorded_at(&tmp, "0.20.0");
    // No agent env and no TTY: print instead of launching.
    page_cmd()
        .env("PATH", "")
        .args(["upgrade", "--agent", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("migration brief"));
}

#[test]
fn test_upgrade_agent_unknown_version_includes_latest_only() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Unknown", "posts");
    let site_dir = tmp.path().join("site");
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();
    let out = page_cmd()
        .env("CLAUDECODE", "1")
        .args(["upgrade", "--agent", "--force"])
        .current_dir(&site_dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("an unknown version"), "{stdout}");
    assert!(
        stdout.contains("every release note is included"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("### v{}", env!("CARGO_PKG_VERSION"))),
        "{stdout}"
    );
    // Older notes are either included or, once the notes outgrow the size
    // cap, replaced by a pointer to the full release notes.
    assert!(
        stdout.contains("### v0.20.0") || stdout.contains("older ones omitted for size"),
        "{stdout}"
    );
}

#[test]
fn test_upgrade_agent_with_check_is_an_error() {
    let tmp = TempDir::new().unwrap();
    let site_dir = site_recorded_at(&tmp, "0.19.0");
    page_cmd()
        .args(["upgrade", "--agent", "--check"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("can't be combined with --check"));
}

#[test]
fn test_upgrade_agent_json_puts_brief_in_data() {
    let tmp = TempDir::new().unwrap();
    let site_dir = site_recorded_at(&tmp, "0.19.0");
    let (ok, doc) = run_json(&site_dir, &["upgrade", "--agent", "--force"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["agent_delivery"], "printed");
    let brief = doc["data"]["brief"].as_str().unwrap();
    assert!(brief.contains("### v0.20.0"), "{brief}");
    // The upgrade's own payload is kept next to the brief.
    let mut keys: Vec<&str> = doc["data"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "agent_delivery",
            "agents",
            "applied",
            "brief",
            "brief_file",
            "changes",
            "unmaintained_agents",
            "up_to_date",
            "version"
        ]
    );
    assert_eq!(doc["data"]["brief_file"], ".seite/upgrade-brief.md");
    assert!(site_dir.join(".seite/upgrade-brief.md").is_file());
}

#[test]
fn test_upgrade_agent_up_to_date_says_so() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Current", "posts");
    let site_dir = tmp.path().join("site");
    let (ok, doc) = run_json(&site_dir, &["upgrade", "--agent", "--force"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["agent_delivery"], "none");
    assert!(doc["data"]["brief"].is_null());
}

#[test]
fn test_upgrade_agent_rejects_unknown_harness_before_upgrading() {
    let tmp = TempDir::new().unwrap();
    let site_dir = site_recorded_at(&tmp, "0.19.0");
    page_cmd()
        .args(["upgrade", "--agent", "--with", "nope", "--force"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown agent harness"));
    let meta = fs::read_to_string(site_dir.join(".seite/config.json")).unwrap();
    assert!(meta.contains("0.19.0"), "must not have upgraded: {meta}");
}

#[test]
fn test_upgrade_with_requires_agent() {
    let tmp = TempDir::new().unwrap();
    let site_dir = site_recorded_at(&tmp, "0.19.0");
    page_cmd()
        .args(["upgrade", "--with", "claude"])
        .current_dir(&site_dir)
        .assert()
        .failure();
}

#[test]
fn test_init_and_upgrade_copilot_harness() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "copilot");
    let site_dir = tmp.path().join("site");
    let hooks = read_json_file(&site_dir.join(".github/hooks/seite.json"));
    assert_eq!(hooks["version"], 1);
    assert_eq!(
        hooks["hooks"]["Stop"][0]["bash"],
        hook_command(Agent::Copilot, HookKind::Stop)
    );
    assert!(site_dir
        .join(".github/instructions/templates.instructions.md")
        .is_file());
    assert!(site_dir.join(".agents/skills/seite/SKILL.md").is_file());
    assert!(!site_dir.join("CLAUDE.md").exists());
    let fresh = harness_snapshot(&site_dir);

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert_eq!(harness_snapshot(&site_dir), fresh);

    // Missing files come back exactly as init wrote them; then a no-op.
    fs::remove_file(site_dir.join(".github/instructions/templates.instructions.md")).unwrap();
    fs::remove_file(site_dir.join(".agents/skills/seite/SKILL.md")).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert_eq!(harness_snapshot(&site_dir), fresh);
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));

    // `seite hooks off` removes Copilot's handlers (keyed on `bash`).
    page_cmd()
        .args(["hooks", "off", "--agents", "copilot"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let text = fs::read_to_string(site_dir.join(".github/hooks/seite.json")).unwrap();
    assert!(!text.contains("seite check --hook"), "{text}");
}

#[test]
fn test_init_and_upgrade_antigravity_harness() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "antigravity");
    let site_dir = tmp.path().join("site");
    let hooks = read_json_file(&site_dir.join(".agents/hooks.json"));
    let ours = &hooks["seite-check"];
    assert_eq!(
        ours["Stop"][0]["command"],
        hook_command(Agent::Antigravity, HookKind::Stop)
    );
    assert_eq!(
        ours["PostInvocation"][0]["command"],
        hook_command(Agent::Antigravity, HookKind::PostEdit)
    );
    let rule = fs::read_to_string(site_dir.join(".agents/rules/templates.md")).unwrap();
    assert!(
        rule.starts_with("---\ntrigger: glob\nglobs: \"templates/**\"\n"),
        "{rule}"
    );
    assert!(site_dir.join(".agents/skills/seite/SKILL.md").is_file());
    assert!(!site_dir.join("GEMINI.md").exists());
    assert!(!site_dir.join("CLAUDE.md").exists());
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("| Antigravity CLI | `.agents/hooks.json` |"));
    assert!(agents_md.contains("Antigravity CLI loads them automatically"));
    let fresh = harness_snapshot(&site_dir);

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert_eq!(harness_snapshot(&site_dir), fresh);

    // `seite hooks off|on` removes and restores both tiers.
    let status = |site: &std::path::Path| {
        let output = page_cmd()
            .args(["--json", "hooks", "status", "--agents", "antigravity"])
            .current_dir(site)
            .output()
            .unwrap();
        json_stdout(&output)["data"]["agents"][0].clone()
    };
    assert_eq!(status(&site_dir)["post_edit"], true);
    page_cmd()
        .args(["hooks", "off", "--agents", "antigravity"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let text = fs::read_to_string(site_dir.join(".agents/hooks.json")).unwrap();
    assert!(!text.contains("seite check --hook"), "{text}");
    assert_eq!(status(&site_dir)["stop"], false);
    page_cmd()
        .args(["hooks", "on", "--agents", "antigravity"])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert_eq!(harness_snapshot(&site_dir), fresh);
}

#[test]
fn test_upgrade_adds_antigravity_and_refreshes_shared_rules() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "codex");
    let site_dir = tmp.path().join("site");
    // `.agents/rules/` as seite wrote it before 0.21 (Claude's `paths:`
    // frontmatter, which Antigravity would discard).
    let old = "---\npaths:\n  - \"data/**\"\n---\n";
    let rule_path = site_dir.join(".agents/rules/data-files.md");
    let current = fs::read_to_string(&rule_path).unwrap();
    let body = current.splitn(3, "---\n").nth(2).unwrap();
    fs::write(&rule_path, format!("{old}{body}")).unwrap();

    page_cmd()
        .args([
            "upgrade",
            "--force",
            "--agents",
            "codex,antigravity",
            "--mcp",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&rule_path).unwrap(), current);
    assert!(site_dir.join(".agents/hooks.json").is_file());
    let mcp = read_json_file(&site_dir.join(".agents/mcp_config.json"));
    assert_eq!(mcp["mcpServers"]["seite"]["command"], "seite");
    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!(["codex", "antigravity"])
    );
    page_cmd()
        .args(["upgrade", "--force", "--mcp"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

/// `seite check --hook antigravity` end to end, with the hook input agy
/// 1.4.10 sends.
#[test]
fn test_check_hook_antigravity_protocol() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "antigravity");
    let site_dir = tmp.path().join("site");
    let root = site_dir.canonicalize().unwrap();
    let ws = root.display().to_string();
    let hook = |args: &[&str], input: serde_json::Value| -> String {
        let mut full = vec!["check", "--hook", "antigravity"];
        full.extend_from_slice(args);
        let output = page_cmd()
            .args(&full)
            // Antigravity runs hooks in the directory of hooks.json.
            .current_dir(site_dir.join(".agents"))
            .write_stdin(input.to_string())
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    let stop = serde_json::json!({
        "conversationId": "conv-1",
        "executionNum": 0,
        "fullyIdle": true,
        "error": "",
        "terminationReason": "NO_TOOL_CALL",
        "workspacePaths": [ws],
    });
    // Clean site: nothing printed, the agent stops.
    assert_eq!(hook(&[], stop.clone()), "");

    // The agent writes a broken post: PostToolUse records it silently,
    // PostInvocation hands the error back.
    let broken = root.join("content/posts/2026-01-01-broken.md");
    fs::write(&broken, "---\ntitle: Broken: yes\n---\nBody.\n").unwrap();
    let tool = serde_json::json!({
        "conversationId": "conv-1",
        "stepIdx": 2,
        "toolCall": { "name": "write_file", "args": { "Path": broken.display().to_string(), "Content": "x" } },
        "workspacePaths": [ws],
    });
    assert_eq!(hook(&["--post-edit"], tool), "");
    let invocation = serde_json::json!({
        "conversationId": "conv-1",
        "invocationNum": 0,
        "initialNumSteps": 1,
        "workspacePaths": [ws],
    });
    let out: serde_json::Value =
        serde_json::from_str(&hook(&["--post-edit"], invocation.clone())).unwrap();
    let message = out["injectSteps"][0]["ephemeralMessage"].as_str().unwrap();
    assert!(
        message.contains("content/posts/2026-01-01-broken.md:2"),
        "{message}"
    );
    assert!(message.contains("frontmatter-parse"), "{message}");
    // Each edit is reported once.
    assert_eq!(hook(&["--post-edit"], invocation), "");

    // Turn end: continue once with the errors, then let the agent stop.
    let out: serde_json::Value = serde_json::from_str(&hook(&[], stop.clone())).unwrap();
    assert_eq!(out["decision"], "continue");
    assert!(out["reason"]
        .as_str()
        .unwrap()
        .contains("frontmatter-parse"));
    assert_eq!(hook(&[], stop.clone()), "");
    // Next turn: continues again (still broken); another conversation too.
    assert!(hook(&[], stop.clone()).contains("\"continue\""));
    let mut other = stop.clone();
    other["conversationId"] = "conv-2".into();
    assert!(hook(&[], other).contains("\"continue\""));
    // Errored or cancelled turns, and background work, never continue.
    for (key, value) in [
        ("terminationReason", serde_json::json!("USER_CANCELED")),
        ("fullyIdle", serde_json::json!(false)),
        ("error", serde_json::json!("boom")),
    ] {
        let mut input = stop.clone();
        input["conversationId"] = "conv-3".into();
        input[key] = value;
        assert_eq!(hook(&[], input), "", "{key}");
    }
    // A continued run that ends abnormally still clears the marker, so the
    // next normal turn end is checked again.
    let mut conv4 = stop.clone();
    conv4["conversationId"] = "conv-4".into();
    assert!(hook(&[], conv4.clone()).contains("\"continue\""));
    let mut aborted = conv4.clone();
    aborted["terminationReason"] = "MAX_INVOCATIONS".into();
    assert_eq!(hook(&[], aborted), "");
    assert!(hook(&[], conv4).contains("\"continue\""));
}

#[test]
fn test_upgrade_adds_copilot_to_claude_site_with_shared_mcp() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["upgrade", "--force", "--agents", "claude,copilot", "--mcp"])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert!(site_dir.join(".github/hooks/seite.json").is_file());
    assert!(site_dir
        .join(".github/instructions/seo-requirements.instructions.md")
        .is_file());
    // Copilot shares Claude Code's `.mcp.json`; no `.github/mcp.json`.
    let mcp = read_json_file(&site_dir.join(".mcp.json"));
    assert_eq!(mcp["mcpServers"]["seite"]["command"], "seite");
    assert!(!site_dir.join(".github/mcp.json").exists());
    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!(["claude", "copilot"])
    );
    page_cmd()
        .args(["upgrade", "--force", "--mcp"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

/// A pre-0.19 project with only CLAUDE.md gets AGENTS.md from the migration
/// step and the GEMINI.md shim that imports it in the same run.
#[test]
fn test_upgrade_pre_019_creates_shims_in_one_run() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude,gemini");
    let site_dir = tmp.path().join("site");
    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::remove_file(site_dir.join("GEMINI.md")).unwrap();
    fs::write(
        site_dir.join("CLAUDE.md"),
        "# My site\n\nWrite posts in content/posts.\n",
    )
    .unwrap();
    set_project_version(&site_dir, "0.18.0");

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("AGENTS.md").exists());
    assert!(fs::read_to_string(site_dir.join("GEMINI.md"))
        .unwrap()
        .contains("@AGENTS.md"));
    page_cmd()
        .args(["upgrade", "--check"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

#[test]
fn test_upgrade_020_workspace_site_gets_agents_md_and_shims() {
    // A workspace as 0.20.2's `seite workspace init` + `workspace add blog`
    // left it: seite.toml only, no AGENTS.md/CLAUDE.md, no .seite/config.json.
    let tmp = TempDir::new().unwrap();
    let site_dir = add_020_workspace_site(tmp.path(), "blog");

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.starts_with("# blog\n"), "{agents_md}");
    assert!(agents_md.contains("<!-- seite:context-rules -->"));
    assert!(agents_md.contains("`.claude/rules/`"));
    // Shims for the agents maintained by default (0.20's set: Claude, Gemini).
    for shim in ["CLAUDE.md", "GEMINI.md"] {
        assert_eq!(
            fs::read_to_string(site_dir.join(shim)).unwrap(),
            "@AGENTS.md\n",
            "{shim}"
        );
    }
    assert!(site_dir.join(".claude/rules/features.md").is_file());

    // Converged: nothing left to do, at the workspace root or in the site.
    for dir in [tmp.path(), site_dir.as_path()] {
        page_cmd()
            .args(["upgrade", "--check"])
            .current_dir(dir)
            .assert()
            .success();
    }
}

#[test]
fn test_upgrade_keeps_existing_claude_md_without_writing_agents_md_over_it() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let site_dir = tmp.path().join("site");
    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    fs::write(site_dir.join("CLAUDE.md"), "# My own guidance\n").unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    // A site with its own CLAUDE.md is left to the user (pre-0.19 sites to
    // the migration): no generated AGENTS.md beside it. (The seite blocks
    // go into CLAUDE.md, the project's instructions file.)
    let claude = fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap();
    assert!(claude.starts_with("# My own guidance\n"), "{claude}");
    assert!(!site_dir.join("AGENTS.md").exists());
}

#[test]
fn test_upgrade_refreshes_unedited_020_rules_and_keeps_edited_ones() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude,cursor");
    let site_dir = tmp.path().join("site");
    // features.md / features.mdc exactly as `seite init` 0.20.2 wrote them
    // (they still say search is always basic).
    let claude_rule = site_dir.join(".claude/rules/features.md");
    let cursor_rule = site_dir.join(".cursor/rules/features.mdc");
    let current_claude = fs::read_to_string(&claude_rule).unwrap();
    let current_cursor = fs::read_to_string(&cursor_rule).unwrap();
    fs::write(
        &claude_rule,
        include_str!("../fixtures/rules-0.20.2/features.md"),
    )
    .unwrap();
    fs::write(
        &cursor_rule,
        include_str!("../fixtures/rules-0.20.2/features.mdc"),
    )
    .unwrap();
    // An edited rule: kept, with a one-time notice.
    let edited_rule = site_dir.join(".claude/rules/templates.md");
    let edited = format!(
        "{}\nOur house style: no inline styles.\n",
        fs::read_to_string(&edited_rule).unwrap()
    );
    fs::write(&edited_rule, &edited).unwrap();
    let notice = "Kept .claude/rules/templates.md (edited since seite wrote it)";

    page_cmd()
        .args(["upgrade", "--check"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stdout(predicate::str::contains(".claude/rules/features.md"))
        .stdout(predicate::str::contains(notice).not());
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(notice));

    assert_eq!(fs::read_to_string(&claude_rule).unwrap(), current_claude);
    assert_eq!(fs::read_to_string(&cursor_rule).unwrap(), current_cursor);
    assert_eq!(fs::read_to_string(&edited_rule).unwrap(), edited);

    // Converged, and the notice is shown once.
    page_cmd()
        .args(["upgrade", "--check"])
        .current_dir(&site_dir)
        .assert()
        .success();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(notice).not());
}
