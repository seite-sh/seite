use super::common::*;

// --- Context Rules (.claude/rules/) ---

#[test]
fn test_init_creates_rules_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Rules Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Verify rules directory exists
    assert!(site_dir.join(".claude/rules").is_dir());

    // Verify always-present rules files with frontmatter
    let expected = [
        "seo-requirements.md",
        "templates.md",
        "i18n.md",
        "data-files.md",
        "shortcodes.md",
        "config-reference.md",
        "features.md",
        "design-prompts.md",
    ];
    for name in &expected {
        let path = site_dir.join(format!(".claude/rules/{name}"));
        assert!(path.exists(), "rules file {name} should exist");
        let content = fs::read_to_string(&path).unwrap();
        assert!(
            content.starts_with("---\npaths:\n"),
            "rules file {name} should have frontmatter"
        );
    }
}

#[test]
fn test_init_lean_agents_md() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Lean Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();

    // Detail content should NOT be in AGENTS.md (now in rules)
    assert!(
        !agents_md.contains("### Every page `<head>` MUST include"),
        "SEO requirements detail should be in rules, not AGENTS.md"
    );
    assert!(
        !agents_md.contains("### Template Variables"),
        "Template variables table should be in rules, not AGENTS.md"
    );

    // Essential sections should still be present
    assert!(agents_md.contains("## Commands"));
    assert!(agents_md.contains("## Collections"));
    assert!(agents_md.contains("## Content Format"));
    assert!(agents_md.contains("## Key Conventions"));
    assert!(agents_md.contains(".claude/rules/"));

    // Line count should be well under 250
    let line_count = agents_md.lines().count();
    assert!(
        line_count < 250,
        "AGENTS.md should be under 250 lines, got {line_count}"
    );
}

#[test]
fn test_init_rules_no_trust_without_trust_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Trust Rules", "posts,pages");
    let site_dir = tmp.path().join("site");

    assert!(
        !site_dir.join(".claude/rules/trust-center.md").exists(),
        "trust-center rule should not exist without trust collection"
    );
}

#[test]
fn test_init_creates_theme_builder_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Theme Skill Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/theme-builder/SKILL.md");
    assert!(
        skill_path.exists(),
        "theme-builder skill should be created on init"
    );

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("name: theme-builder"));
    assert!(content.contains("seite-skill-version:"));
    assert!(content.contains("Phase 1: Understand the Vision"));
}

#[test]
fn test_init_creates_theme_builder_skill_without_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Pages", "posts,docs");
    let site_dir = tmp.path().join("site");

    // Theme builder is unconditional — should exist even without pages collection
    let skill_path = site_dir.join(".claude/skills/theme-builder/SKILL.md");
    assert!(
        skill_path.exists(),
        "theme-builder skill should be created even without pages collection"
    );
}

#[test]
fn test_init_agents_md_has_theme_builder_pointer() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Pointer Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(
        agents_md.contains("/theme-builder"),
        "AGENTS.md should mention /theme-builder skill"
    );
}

#[test]
fn test_upgrade_adds_theme_builder_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade TB", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove the skill to simulate an older project
    let skill_path = site_dir.join(".claude/skills/theme-builder/SKILL.md");
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
        .stdout(predicate::str::contains("theme-builder"));

    assert!(
        skill_path.exists(),
        "upgrade should create the theme-builder skill"
    );
    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("name: theme-builder"));
}

#[test]
fn test_upgrade_updates_outdated_theme_builder_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "TB Update", "posts,pages");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/theme-builder/SKILL.md");
    assert!(skill_path.exists());

    // Write an older version of the skill
    fs::write(
        &skill_path,
        "---\nname: theme-builder\ndescription: old\n# seite-skill-version: 0\n---\nOld content\n",
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
        .stdout(predicate::str::contains("theme-builder"));

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(
        content.contains("seite-skill-version: 1"),
        "upgrade should replace skill with newer version"
    );
    assert!(
        content.contains("Phase 1: Understand the Vision"),
        "upgraded skill should have full content"
    );
}
