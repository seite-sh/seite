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
    // Canned design palettes are gone (the theme workflow lives in /seite theme).
    assert!(!site_dir.join(".claude/rules/design-prompts.md").exists());
}

#[test]
fn test_init_lean_agents_md() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Lean Test", "posts,docs,pages");
    let site_dir = tmp.path().join("site");

    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();

    // Detail lives in rules, the skill, and .seite/docs — not AGENTS.md.
    for removed in [
        "### Every page `<head>` MUST include",
        "### Template Variables",
        "## Commands",
        "Dev Server REPL",
        "seite_create_content",
        "## Documentation",
        "Theme Builder Skill",
        "MCP Server",
    ] {
        assert!(!agents_md.contains(removed), "{removed}");
    }

    for kept in [
        "## Workflow",
        "seite context",
        "/seite <command>",
        ".seite/docs/",
        "seite docs <query>",
        "## Layout",
        "## Collections",
        "## Content format",
        "## Rules",
        "{{ page.content | safe }}",
        "relative `.md` paths",
        "Never edit `dist/`",
        "{{% end %}}",
        "## Verify",
        "once more, then stop",
        "<!-- seite:agent-setup -->",
        "<!-- seite:context-rules -->",
        ".claude/rules/",
    ] {
        assert!(agents_md.contains(kept), "AGENTS.md missing {kept}");
    }
    assert!(!agents_md.contains("MANDATORY"));
}

#[test]
fn test_init_agents_md_size_budget() {
    // Always-loaded context stays small (target ~3.5 KB for a 3-collection
    // site; the setup table grows with the agent count).
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "one",
            "--title",
            "Budget",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,docs,pages",
            "--agents",
            "claude",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
    let one = fs::read_to_string(tmp.path().join("one/AGENTS.md")).unwrap();
    assert!(
        one.len() <= 3584,
        "AGENTS.md is {} bytes:\n{one}",
        one.len()
    );

    init_site(&tmp, "all", "Budget", "posts,docs,pages");
    let all = fs::read_to_string(tmp.path().join("all/AGENTS.md")).unwrap();
    assert!(
        all.len() <= 4608,
        "AGENTS.md is {} bytes:\n{all}",
        all.len()
    );
}

#[test]
fn test_init_writes_version_matched_local_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Docs", "posts");
    let docs = tmp.path().join("site/.seite/docs");
    for slug in ["getting-started", "templates", "agent", "cli-reference"] {
        assert!(docs.join(format!("{slug}.md")).is_file(), "{slug}");
    }
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
fn test_init_creates_one_routed_skill_with_references() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Skill Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    for dir in [".claude/skills", ".agents/skills"] {
        let skill = fs::read_to_string(site_dir.join(format!("{dir}/seite/SKILL.md"))).unwrap();
        assert!(skill.contains("seite-skill-version: 3"), "{dir}");
        for name in [
            "check",
            "new",
            "preview",
            "build",
            "deploy",
            "theme",
            "brand",
            "landing",
            "collection",
            "upgrade",
            "hooks",
        ] {
            let path = site_dir.join(format!("{dir}/seite/reference/{name}.md"));
            assert!(path.is_file(), "{}", path.display());
        }
        // The old standalone skills are folded into /seite.
        for old in ["theme-builder", "brand-identity", "landing-page"] {
            assert!(
                !site_dir.join(format!("{dir}/{old}")).exists(),
                "{dir}/{old}"
            );
        }
    }
    // OpenCode's /seite wrapper and Gemini's custom command route to it.
    let opencode = fs::read_to_string(site_dir.join(".opencode/commands/seite.md")).unwrap();
    assert!(opencode.contains("skill({ name: \"seite\" })"));
    let gemini = fs::read_to_string(site_dir.join(".gemini/commands/seite.toml")).unwrap();
    assert!(gemini.contains("{{args}}"));
}

/// Downgrade the project's recorded version so version-gated steps run.
fn downgrade(site_dir: &std::path::Path, to: &str) {
    let meta_path = site_dir.join(".seite/config.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let updated = meta_content.replace(env!("CARGO_PKG_VERSION"), to);
    fs::write(&meta_path, &updated).unwrap();
}

#[test]
fn test_upgrade_replaces_outdated_seite_skill_and_references() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Skill Update", "posts,pages");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/seite/SKILL.md");
    fs::write(
        &skill_path,
        "---\nname: seite\ndescription: old\n# seite-skill-version: 1\n---\nOld content\n",
    )
    .unwrap();
    let reference = site_dir.join(".claude/skills/seite/reference/check.md");
    fs::write(&reference, "stale").unwrap();
    let missing = site_dir.join(".agents/skills/seite/reference/hooks.md");
    fs::remove_file(&missing).unwrap();

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("updated v1 → v3"));

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("seite-skill-version: 3"));
    assert!(fs::read_to_string(&reference)
        .unwrap()
        .starts_with("# /seite check"));
    assert!(missing.is_file(), "missing reference restored");
}

#[test]
fn test_upgrade_removes_unmodified_legacy_skills_and_keeps_modified() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Legacy Skills", "posts,pages");
    let site_dir = tmp.path().join("site");
    let theme_builder = include_str!("../fixtures/legacy-skills/theme-builder-v2.md");
    let design_prompts = include_str!("../fixtures/legacy-skills/design-prompts.md");

    // Unmodified copies, as seite 0.20 wrote them (Claude + shared dirs,
    // and the design-prompts rule in Claude and Cursor formats).
    let write = |rel: &str, content: &str| {
        let path = site_dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    };
    write(".claude/skills/theme-builder/SKILL.md", theme_builder);
    write(".agents/skills/theme-builder/SKILL.md", theme_builder);
    write(
        ".claude/rules/design-prompts.md",
        &format!("---\npaths:\n  - \"templates/**\"\n---\n{design_prompts}"),
    );
    write(
        ".cursor/rules/design-prompts.mdc",
        &format!("---\ndescription: \"x\"\nglobs: \"templates/**\"\nalwaysApply: false\n---\n{design_prompts}"),
    );
    // A modified copy: kept.
    write(
        ".claude/skills/brand-identity/SKILL.md",
        "---\nname: brand-identity\ndescription: mine\n---\n\nMy own brand workflow.\n",
    );

    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Removed .claude/skills/theme-builder/",
        ))
        .stdout(predicate::str::contains(
            "Kept .claude/skills/brand-identity",
        ));

    assert!(!site_dir.join(".claude/skills/theme-builder").exists());
    assert!(!site_dir.join(".agents/skills/theme-builder").exists());
    assert!(!site_dir.join(".claude/rules/design-prompts.md").exists());
    assert!(!site_dir.join(".cursor/rules/design-prompts.mdc").exists());
    assert!(site_dir
        .join(".claude/skills/brand-identity/SKILL.md")
        .is_file());

    // An edited copy of a bundled skill is kept too.
    write(
        ".claude/skills/theme-builder/SKILL.md",
        &theme_builder.replace("Theme Builder", "My Theme Builder"),
    );
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert!(site_dir
        .join(".claude/skills/theme-builder/SKILL.md")
        .is_file());
}

#[test]
fn test_upgrade_refreshes_local_docs_from_older_version() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Docs Refresh", "posts");
    let site_dir = tmp.path().join("site");
    let doc = site_dir.join(".seite/docs/templates.md");
    fs::write(&doc, "old docs").unwrap();
    let missing = site_dir.join(".seite/docs/agent.md");
    fs::remove_file(&missing).unwrap();

    // Same version: only the missing page comes back; local edits stay.
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success();
    assert!(missing.is_file());
    assert_eq!(fs::read_to_string(&doc).unwrap(), "old docs");

    // From an older seite: every page is refreshed.
    downgrade(&site_dir, "0.20.0");
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(".seite/docs/"));
    assert_ne!(fs::read_to_string(&doc).unwrap(), "old docs");
}
