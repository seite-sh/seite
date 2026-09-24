use super::common::*;

// --- init command ---

#[test]
fn test_init_creates_project_structure() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "mysite",
            "--title",
            "My Site",
            "--description",
            "A test site",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created new site in 'mysite'"));

    let root = tmp.path().join("mysite");
    assert!(root.join("seite.toml").exists());
    assert!(root.join("content/posts").is_dir());
    assert!(root.join("content/pages").is_dir());
    assert!(root.join("templates/base.html").exists());
    assert!(root.join("templates/index.html").exists());
    assert!(root.join("templates/post.html").exists());
    assert!(root.join("templates/page.html").exists());
    assert!(root.join("static").is_dir());

    // Verify .gitignore
    assert!(root.join(".gitignore").exists());
    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(gitignore.contains("/dist"));

    // Verify seite.toml content
    let config_content = fs::read_to_string(root.join("seite.toml")).unwrap();
    assert!(config_content.contains("title = \"My Site\""));
    assert!(config_content.contains("[[collections]]"));

    // Verify sample post exists
    let posts: Vec<_> = fs::read_dir(root.join("content/posts"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(posts.len(), 1);
    let post_content = fs::read_to_string(posts[0].path()).unwrap();
    assert!(post_content.contains("title: Hello World"));
}

#[test]
fn test_init_with_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mysite", "Doc Site", "posts,docs,pages");

    let root = tmp.path().join("mysite");
    assert!(root.join("content/docs").is_dir());
    assert!(root.join("templates/doc.html").exists());

    let config_content = fs::read_to_string(root.join("seite.toml")).unwrap();
    assert!(config_content.contains("name = \"docs\""));
}

#[test]
fn test_init_fails_if_dir_exists() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir(tmp.path().join("existing")).unwrap();

    page_cmd()
        .args([
            "init",
            "existing",
            "--title",
            "Test",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_init_github_pages_creates_workflow() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "mysite",
            "--title",
            "Workflow Test",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let workflow = tmp.path().join("mysite/.github/workflows/deploy.yml");
    assert!(
        workflow.exists(),
        "GitHub Actions workflow should be created"
    );

    let content = fs::read_to_string(&workflow).unwrap();
    assert!(content.contains("Deploy to GitHub Pages"));
    assert!(content.contains("deploy-pages"));
    assert!(content.contains("seite build"));
}

#[test]
fn test_init_cloudflare_creates_workflow() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "mysite",
            "--title",
            "CF Test",
            "--description",
            "",
            "--deploy-target",
            "cloudflare",
            "--collections",
            "posts",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Should have .github/workflows/deploy.yml for Cloudflare
    let workflow_path = tmp.path().join("mysite/.github/workflows/deploy.yml");
    assert!(workflow_path.exists());
    let content = fs::read_to_string(&workflow_path).unwrap();
    assert!(content.contains("Cloudflare"));
    assert!(content.contains("wrangler-action"));
}

#[test]
fn test_init_netlify_target() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "mysite",
            "--title",
            "Netlify Init",
            "--description",
            "",
            "--deploy-target",
            "netlify",
            "--collections",
            "posts",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let config = fs::read_to_string(tmp.path().join("mysite/seite.toml")).unwrap();
    assert!(
        config.contains("netlify"),
        "config should contain netlify deploy target"
    );

    // Should generate netlify.toml
    let netlify_toml = tmp.path().join("mysite/netlify.toml");
    assert!(netlify_toml.exists());
    let content = fs::read_to_string(&netlify_toml).unwrap();
    assert!(content.contains("[build]"));
    assert!(content.contains("seite build"));

    // Should also generate GitHub Actions workflow
    let workflow_path = tmp.path().join("mysite/.github/workflows/deploy.yml");
    assert!(workflow_path.exists());
    let workflow = fs::read_to_string(&workflow_path).unwrap();
    assert!(workflow.contains("Netlify"));
}

#[test]
fn test_init_creates_data_directory() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Init Data Dir", "posts,pages");
    let site_dir = tmp.path().join("site");

    assert!(
        site_dir.join("data").exists(),
        "init should create data/ directory"
    );
    assert!(site_dir.join("data").is_dir());
}

// ── Project metadata & upgrade ──────────────────────────────────────

#[test]
fn test_init_creates_page_meta() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Meta Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // .seite/config.json should exist
    let meta_path = site_dir.join(".seite/config.json");
    assert!(
        meta_path.exists(),
        ".seite/config.json should be created by init"
    );

    let content = fs::read_to_string(&meta_path).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(
        meta.get("version").is_some(),
        "meta should have a version field"
    );
    assert!(
        meta.get("initialized_at").is_some(),
        "meta should have an initialized_at timestamp"
    );
}

#[test]
fn test_init_creates_mcp_server_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "MCP Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Claude Code reads project MCP servers from .mcp.json only.
    let mcp_path = site_dir.join(".mcp.json");
    assert!(mcp_path.exists(), "init should write .mcp.json");
    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
    assert_eq!(
        mcp.pointer("/mcpServers/seite/command")
            .and_then(|v| v.as_str()),
        Some("seite"),
        "MCP command should be 'seite'"
    );
    assert_eq!(
        mcp.pointer("/mcpServers/seite/args")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>()),
        Some(vec!["mcp"]),
        "MCP args should be ['mcp']"
    );
}

#[test]
fn test_init_settings_json_approves_mcp_without_mcp_servers() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "MCP Settings", "posts,pages");
    let site_dir = tmp.path().join("site");

    let content = fs::read_to_string(site_dir.join(".claude/settings.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(
        settings.get("mcpServers").is_none(),
        "settings.json must not carry mcpServers (Claude Code ignores it there)"
    );
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
        "mcp__seite",
        "Edit(static/**)",
        "Edit(seite.toml)",
        "Write(static/**)",
    ] {
        assert!(allow.contains(&rule), "missing allow rule {rule}");
    }
}

#[test]
fn test_init_uses_agents_md_as_canonical_instructions() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Agent Instructions", "posts,pages");
    let site_dir = tmp.path().join("site");

    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(
        agents_md.contains("## Commands"),
        "AGENTS.md should contain the generated project instructions"
    );
    assert!(
        agents_md.contains("## MCP Server"),
        "AGENTS.md should contain the generated MCP guidance"
    );

    let claude_md = fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap();
    assert_eq!(
        claude_md, "@AGENTS.md\n",
        "CLAUDE.md should only import the canonical AGENTS.md"
    );
}

#[test]
fn test_init_creates_landing_page_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Skill Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    assert!(
        skill_path.exists(),
        "landing-page skill should be created when pages collection is present"
    );

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(
        content.starts_with("---"),
        "SKILL.md should have YAML frontmatter"
    );
    assert!(
        content.contains("name: landing-page"),
        "SKILL.md should define skill name"
    );
    assert!(
        content.contains("Phase 1: Messaging"),
        "SKILL.md should contain messaging phase"
    );
}

#[test]
fn test_init_no_landing_page_skill_without_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Pages", "posts,docs");
    let site_dir = tmp.path().join("site");

    let skill_path = site_dir.join(".claude/skills/landing-page/SKILL.md");
    assert!(
        !skill_path.exists(),
        "landing-page skill should NOT be created without pages collection"
    );
}

#[test]
fn test_init_includes_private_collections_rule() {
    // New sites get guidance for discovery-only private collections and the
    // optional Cloudflare Pages password layer.
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Gated", "posts,pages");
    let site_dir = tmp.path().join("site");
    let rule = fs::read_to_string(site_dir.join(".claude/rules/private-collections.md")).unwrap();
    assert!(
        rule.contains("private = true"),
        "rule should cover the flag"
    );
    assert!(
        rule.contains("[access]"),
        "rule should cover password access"
    );
    assert!(
        rule.contains("seite access set-password"),
        "rule should explain secure password setup"
    );
    assert!(rule.contains("Cloudflare Pages"));
}

#[test]
fn test_init_agents_default_writes_all_harness_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "All Agents", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Claude Code
    assert_eq!(
        read_json_file(&site_dir.join(".mcp.json"))["mcpServers"]["seite"]["command"],
        "seite"
    );
    assert!(site_dir.join(".claude/settings.json").exists());
    assert!(site_dir.join(".claude/rules/templates.md").exists());
    assert!(site_dir
        .join(".claude/skills/theme-builder/SKILL.md")
        .exists());
    assert_eq!(
        fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n"
    );

    // Cursor: same MCP shape, .mdc rules with globs from the same source
    assert_eq!(
        read_json_file(&site_dir.join(".cursor/mcp.json"))["mcpServers"]["seite"]["args"][0],
        "mcp"
    );
    let mdc = fs::read_to_string(site_dir.join(".cursor/rules/templates.mdc")).unwrap();
    assert!(mdc.contains("globs: \"templates/**\"\nalwaysApply: false\n"));
    let claude_rule = fs::read_to_string(site_dir.join(".claude/rules/templates.md")).unwrap();
    let body = |s: &str| s.splitn(3, "---\n").nth(2).unwrap().to_string();
    assert_eq!(body(&mdc), body(&claude_rule), "rules share one source");

    // Codex
    let codex: toml::Value =
        toml::from_str(&fs::read_to_string(site_dir.join(".codex/config.toml")).unwrap()).unwrap();
    assert_eq!(
        codex["mcp_servers"]["seite"]["command"].as_str(),
        Some("seite")
    );

    // OpenCode
    let opencode = read_json_file(&site_dir.join("opencode.json"));
    assert_eq!(opencode["mcp"]["seite"]["type"], "local");
    assert_eq!(opencode["permission"]["bash"]["seite build*"], "allow");

    // Shared skills for Codex / Cursor / OpenCode
    assert_eq!(
        fs::read_to_string(site_dir.join(".agents/skills/theme-builder/SKILL.md")).unwrap(),
        fs::read_to_string(site_dir.join(".claude/skills/theme-builder/SKILL.md")).unwrap()
    );
    assert!(site_dir
        .join(".agents/skills/landing-page/SKILL.md")
        .exists());

    // AGENTS.md lists every agent's MCP config and the rules index
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    for needle in [
        "`.mcp.json`",
        "`.codex/config.toml`",
        "`.cursor/mcp.json`",
        "`opencode.json`",
        "cursor-agent mcp enable seite",
        "- `templates/**`: `seo-requirements.md`",
    ] {
        assert!(agents_md.contains(needle), "AGENTS.md missing {needle}");
    }

    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!(["claude", "codex", "opencode", "cursor"])
    );
}

#[test]
fn test_init_agents_subset() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "claude-only", "claude");
    let site_dir = tmp.path().join("claude-only");
    assert!(site_dir.join(".mcp.json").exists());
    assert!(site_dir.join(".claude/rules/templates.md").exists());
    for absent in [".cursor", ".codex", ".agents", "opencode.json"] {
        assert!(!site_dir.join(absent).exists(), "{absent} should not exist");
    }
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("`.mcp.json`"));
    assert!(!agents_md.contains("opencode.json") && !agents_md.contains(".codex/"));
    assert!(!agents_md.contains("Cursor"));
    assert_eq!(stored_agents(&site_dir), serde_json::json!(["claude"]));

    // Codex only: no Claude files; rules land in the neutral .agents/rules/
    init_site_with_agents(&tmp, "codex-only", "codex");
    let site_dir = tmp.path().join("codex-only");
    assert!(site_dir.join(".codex/config.toml").exists());
    assert!(site_dir
        .join(".agents/skills/theme-builder/SKILL.md")
        .exists());
    assert!(site_dir.join(".agents/rules/templates.md").exists());
    for absent in [
        ".claude",
        ".mcp.json",
        "CLAUDE.md",
        ".cursor",
        "opencode.json",
    ] {
        assert!(!site_dir.join(absent).exists(), "{absent} should not exist");
    }
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("Detailed guides live in `.agents/rules/`"));
    assert!(!agents_md.contains(".claude/"));
}

#[test]
fn test_init_agents_unknown_errors() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "site",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts",
            "--agents",
            "claude,cursr",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown agent 'cursr'"))
        .stderr(predicate::str::contains("did you mean 'cursor'"));
    assert!(!tmp.path().join("site").exists());
}

#[test]
fn test_init_with_contact_provider() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "ctsite",
            "--title",
            "Contact Site",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages",
            "--contact-provider",
            "formspree",
            "--contact-endpoint",
            "xpznqkdl",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let config = fs::read_to_string(tmp.path().join("ctsite/seite.toml")).unwrap();
    assert!(config.contains("[contact]"));
    assert!(config.contains("formspree"));
    assert!(config.contains("xpznqkdl"));

    // Should have created a contact page
    assert!(tmp.path().join("ctsite/content/pages/contact.md").exists());
    let contact_page =
        fs::read_to_string(tmp.path().join("ctsite/content/pages/contact.md")).unwrap();
    assert!(contact_page.contains("contact_form()"));
}

// ── Public directory feature ────────────────────────────────────────

#[test]
fn test_init_creates_public_directory() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Public Dir", "posts");
    let site_dir = tmp.path().join("site");
    assert!(site_dir.join("public").is_dir());
}

#[test]
fn test_init_gitignore_includes_dist_subdomains() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "gitig", "Gitignore Test", "posts");
    let site_dir = tmp.path().join("gitig");

    let gitignore = fs::read_to_string(site_dir.join(".gitignore")).unwrap();
    assert!(
        gitignore.contains("dist-subdomains"),
        ".gitignore should include dist-subdomains/"
    );
}
