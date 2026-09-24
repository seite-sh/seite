use super::common::*;

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
fn test_upgrade_adds_landing_page_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove the skill to simulate an older project
    let skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    fs::remove_file(&skill_path).unwrap();
    assert!(!skill_path.exists());

    // Downgrade the project version so the upgrade step applies
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.0");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("landing-page"));

    assert!(
        skill_path.exists(),
        "upgrade should create the landing-page skill"
    );
    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("name: landing-page"));
}

#[test]
fn test_upgrade_updates_outdated_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Skill Update", "posts,pages");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    assert!(skill_path.exists());

    // Write an older version of the skill (version 1)
    fs::write(
        &skill_path,
        "---\nname: landing-page\ndescription: old\n# seite-skill-version: 1\n---\nOld content\n",
    )
    .unwrap();

    // Downgrade the project version so the upgrade step applies
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.0");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("updated v1 \u{2192} v3"));

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(
        content.contains("seite-skill-version: 3"),
        "upgrade should replace skill with newer version"
    );
    assert!(
        content.contains("Phase 1: Messaging"),
        "upgraded skill should have full content"
    );
}

#[test]
fn test_upgrade_migrates_old_homepage_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Migration Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove the new skill and create an old homepage skill to simulate pre-rename project
    let new_skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    fs::remove_file(&new_skill_path).unwrap();
    let old_skill_dir = site_dir.join(".claude/skills/homepage");
    fs::create_dir_all(&old_skill_dir).unwrap();
    fs::write(
        old_skill_dir.join("SKILL.md"),
        "---\nname: homepage\ndescription: old\n# seite-skill-version: 1\n---\nOld content\n",
    )
    .unwrap();

    // Downgrade the project version
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), "0.1.0");
    fs::write(&meta_path, &updated).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("landing-page"));

    // New skill should exist
    assert!(
        new_skill_path.exists(),
        "upgrade should create the new landing-page skill"
    );
    let content = fs::read_to_string(&new_skill_path).unwrap();
    assert!(content.contains("name: landing-page"));

    // Old skill should still be there (non-destructive)
    assert!(
        old_skill_dir.join("SKILL.md").exists(),
        "upgrade should not delete the old homepage skill"
    );
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

    // Simulate a pre-MCP project by removing the MCP config and version stamp
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();
    fs::remove_file(site_dir.join(".mcp.json")).unwrap();

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

    // Run upgrade
    page_cmd()
        .args(["upgrade", "--force"])
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
fn test_upgrade_adds_mcp_section_before_migrating_to_agents_md() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CLAUDE.md Upgrade", "posts,pages");
    let site_dir = tmp.path().join("site");

    // init now includes MCP section, so verify it's there
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(
        agents_md.contains("## MCP Server"),
        "init-generated AGENTS.md should include MCP section"
    );
    assert!(
        agents_md.contains("## Commands"),
        "AGENTS.md should have Commands section from init"
    );

    // Simulate an older project whose CLAUDE.md doesn't have the MCP section.
    fs::remove_file(site_dir.join("AGENTS.md")).unwrap();
    let older_md = "# My Site\n\n## Commands\n\n```bash\nseite build\n```\n";
    fs::write(site_dir.join("CLAUDE.md"), older_md).unwrap();

    // Remove version stamp to trigger upgrade
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let updated = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(
        updated.contains("## MCP Server"),
        "upgrade should retain MCP guidance added before migration"
    );
    // Original content should still be there
    assert!(
        updated.contains("## Commands"),
        "upgrade should preserve existing project instructions"
    );
    assert_eq!(
        fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n"
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
        .args(["upgrade", "--force"])
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
    fs::remove_file(site_dir.join(".mcp.json")).unwrap();
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
        .args(["upgrade", "--force"])
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
        .args(["upgrade", "--check"])
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
    init_site(&tmp, "site", "Version Stamp", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Simulate a project whose files are already up to date but whose
    // .seite/config.json records an older version (e.g. 0.4.0 → current).
    let meta_path = site_dir.join(".seite/config.json");
    fs::write(
        &meta_path,
        r#"{"version": "0.4.0", "initialized_at": "2026-01-01T00:00:00+00:00"}"#,
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
        .args(["--json", "upgrade", "--force", "--agents", "all"])
        .current_dir(&site_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = json_stdout(&output);
    assert_eq!(
        doc["data"]["agents"],
        serde_json::json!(["claude", "codex", "opencode", "cursor"])
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
    assert!(site_dir
        .join(".agents/skills/brand-identity/SKILL.md")
        .exists());
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("`.codex/config.toml`"));
    assert!(agents_md.contains("(Cursor: same guides as `.cursor/rules/*.mdc`)"));
    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!(["claude", "codex", "opencode", "cursor"])
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
    fs::remove_dir_all(site_dir.join(".codex")).unwrap();
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

    // Sites without a stored selection are treated as "all" and get it recorded.
    let meta_path = site_dir.join(".seite/config.json");
    let mut meta = read_json_file(&meta_path);
    meta.as_object_mut().unwrap().remove("agents");
    fs::write(&meta_path, meta.to_string()).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!(["claude", "codex", "opencode", "cursor"])
    );
    assert_eq!(harness_snapshot(&site_dir), fresh);

    // Deselecting keeps files but stops maintaining them.
    page_cmd()
        .args(["upgrade", "--force", "--agents", "claude,codex"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No longer maintaining files for OpenCode, Cursor",
        ));
    assert!(site_dir.join("opencode.json").exists());
    assert!(site_dir.join(".cursor/mcp.json").exists());
    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!(["claude", "codex"])
    );
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(
        !agents_md.contains("`opencode.json`"),
        "table follows selection"
    );
    fs::remove_file(site_dir.join(".cursor/mcp.json")).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
    assert!(!site_dir.join(".cursor/mcp.json").exists());
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
