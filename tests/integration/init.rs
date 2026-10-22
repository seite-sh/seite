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
fn test_init_without_mcp_writes_no_mcp_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Plain", "posts,pages");
    let site_dir = tmp.path().join("site");

    for absent in [".mcp.json", ".cursor/mcp.json", ".codex/config.toml"] {
        assert!(!site_dir.join(absent).exists(), "{absent} should not exist");
    }
    let settings = read_json_file(&site_dir.join(".claude/settings.json"));
    assert!(settings.get("enabledMcpjsonServers").is_none());
    assert!(!settings.to_string().contains("mcp__seite"));
    assert!(read_json_file(&site_dir.join("opencode.json"))
        .get("mcp")
        .is_none());
    assert!(!read_json_file(&site_dir.join(".cursor/cli.json"))
        .to_string()
        .contains("Mcp(seite"));
    assert!(read_json_file(&site_dir.join(".gemini/settings.json"))
        .get("mcpServers")
        .is_none());
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(!agents_md.contains("MCP"), "{agents_md}");
}

#[test]
fn test_init_with_mcp_wires_every_agent() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "site",
            "--title",
            "MCP",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts",
            "--mcp",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
    let site_dir = tmp.path().join("site");

    // Claude Code reads project MCP servers from .mcp.json only.
    let mcp = read_json_file(&site_dir.join(".mcp.json"));
    assert_eq!(mcp["mcpServers"]["seite"]["command"], "seite");
    assert_eq!(
        mcp["mcpServers"]["seite"]["args"],
        serde_json::json!(["mcp"])
    );
    let settings = read_json_file(&site_dir.join(".claude/settings.json"));
    assert!(settings.get("mcpServers").is_none());
    assert_eq!(
        settings["enabledMcpjsonServers"],
        serde_json::json!(["seite"])
    );
    assert!(settings["permissions"]["allow"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "mcp__seite"));
    assert_eq!(
        read_json_file(&site_dir.join(".cursor/mcp.json"))["mcpServers"]["seite"]["args"][0],
        "mcp"
    );
    let codex: toml::Value =
        toml::from_str(&fs::read_to_string(site_dir.join(".codex/config.toml")).unwrap()).unwrap();
    assert_eq!(
        codex["mcp_servers"]["seite"]["command"].as_str(),
        Some("seite")
    );
    assert_eq!(
        read_json_file(&site_dir.join("opencode.json"))["mcp"]["seite"]["type"],
        "local"
    );
    let gemini = read_json_file(&site_dir.join(".gemini/settings.json"));
    assert_eq!(gemini["mcpServers"]["seite"]["command"], "seite");
    assert!(gemini["hooks"].get("AfterAgent").is_some());

    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    for needle in ["MCP config", "`.mcp.json`", "cursor-agent mcp enable seite"] {
        assert!(agents_md.contains(needle), "AGENTS.md missing {needle}");
    }
}

#[test]
fn test_init_settings_json_permissions() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Settings", "posts,pages");
    let site_dir = tmp.path().join("site");

    let settings = read_json_file(&site_dir.join(".claude/settings.json"));
    let allow: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for rule in [
        "Edit(static/**)",
        "Edit(seite.toml)",
        "Write(static/**)",
        "Bash(seite check:*)",
        "Bash(seite context)",
    ] {
        assert!(allow.contains(&rule), "missing allow rule {rule}");
    }
    // Hooks of both tiers.
    assert!(settings["hooks"].get("Stop").is_some());
    assert_eq!(
        settings["hooks"]["PostToolUse"][0]["matcher"],
        "Write|Edit|MultiEdit"
    );
}

#[test]
fn test_init_writes_claude_launch_json() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "site", "claude");
    let launch = read_json_file(&tmp.path().join("site/.claude/launch.json"));
    assert_eq!(launch["version"], "0.0.1");
    assert_eq!(
        launch["configurations"][0],
        serde_json::json!({
            "name": "seite",
            "runtimeExecutable": "seite",
            "runtimeArgs": ["serve", "--no-repl", "--port", "3000"],
            "port": 3000
        })
    );
    init_site_with_agents(&tmp, "codex", "codex");
    assert!(!tmp.path().join("codex/.claude").exists());
}

#[test]
fn test_init_uses_agents_md_as_canonical_instructions() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Agent Instructions", "posts,pages");
    let site_dir = tmp.path().join("site");

    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(
        agents_md.contains("## Workflow"),
        "AGENTS.md should contain the generated project instructions"
    );
    assert!(
        agents_md.contains("## Agent setup"),
        "AGENTS.md should contain the per-agent setup table"
    );

    for shim in ["CLAUDE.md", "GEMINI.md"] {
        assert_eq!(
            fs::read_to_string(site_dir.join(shim)).unwrap(),
            "@AGENTS.md\n",
            "{shim} should only import the canonical AGENTS.md"
        );
    }
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
    assert!(site_dir.join(".claude/settings.json").exists());
    assert!(site_dir.join(".claude/rules/templates.md").exists());
    assert!(site_dir.join(".claude/skills/seite/SKILL.md").exists());
    assert_eq!(
        fs::read_to_string(site_dir.join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n"
    );

    // Cursor: .mdc rules with globs from the same source
    let mdc = fs::read_to_string(site_dir.join(".cursor/rules/templates.mdc")).unwrap();
    assert!(mdc.contains("globs: \"templates/**\"\nalwaysApply: false\n"));
    let claude_rule = fs::read_to_string(site_dir.join(".claude/rules/templates.md")).unwrap();
    let body = |s: &str| s.splitn(3, "---\n").nth(2).unwrap().to_string();
    assert_eq!(body(&mdc), body(&claude_rule), "rules share one source");
    assert!(site_dir.join(".cursor/cli.json").exists());

    // Codex: hooks (no MCP config without --mcp)
    assert!(site_dir.join(".codex/hooks.json").exists());

    // OpenCode
    let opencode = read_json_file(&site_dir.join("opencode.json"));
    assert_eq!(opencode["permission"]["bash"]["seite build*"], "allow");

    // Gemini CLI
    assert!(site_dir.join(".gemini/settings.json").exists());
    assert!(site_dir.join(".gemini/commands/seite.toml").exists());

    // Shared skill for Codex / Cursor / OpenCode / Gemini: same references
    assert_eq!(
        fs::read_to_string(site_dir.join(".agents/skills/seite/reference/theme.md")).unwrap(),
        fs::read_to_string(site_dir.join(".claude/skills/seite/reference/theme.md")).unwrap()
    );

    // AGENTS.md lists every agent's setup and the rules index
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    for needle in [
        "`.claude/settings.json`",
        "`.codex/hooks.json`",
        "`.cursor/cli.json`",
        "`opencode.json`",
        "`.gemini/settings.json`",
        "- `templates/**`: `seo-requirements.md`",
    ] {
        assert!(agents_md.contains(needle), "AGENTS.md missing {needle}");
    }

    assert_eq!(
        stored_agents(&site_dir),
        serde_json::json!(["claude", "codex", "opencode", "cursor", "gemini"])
    );
}

#[test]
fn test_init_agents_subset() {
    let tmp = TempDir::new().unwrap();
    init_site_with_agents(&tmp, "claude-only", "claude");
    let site_dir = tmp.path().join("claude-only");
    assert!(site_dir.join(".claude/settings.json").exists());
    assert!(site_dir.join(".claude/rules/templates.md").exists());
    for absent in [
        ".cursor",
        ".codex",
        ".agents",
        "opencode.json",
        ".gemini",
        "GEMINI.md",
        ".mcp.json",
    ] {
        assert!(!site_dir.join(absent).exists(), "{absent} should not exist");
    }
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("`.claude/settings.json`"));
    assert!(!agents_md.contains("opencode.json") && !agents_md.contains(".codex/"));
    assert!(!agents_md.contains("Cursor"));
    assert_eq!(stored_agents(&site_dir), serde_json::json!(["claude"]));

    // Codex only: no Claude files; rules land in the neutral .agents/rules/
    init_site_with_agents(&tmp, "codex-only", "codex");
    let site_dir = tmp.path().join("codex-only");
    assert!(site_dir.join(".codex/hooks.json").exists());
    assert!(site_dir.join(".agents/skills/seite/SKILL.md").exists());
    assert!(site_dir.join(".agents/rules/templates.md").exists());
    for absent in [
        ".claude",
        ".mcp.json",
        "CLAUDE.md",
        ".cursor",
        "opencode.json",
        ".codex/config.toml",
    ] {
        assert!(!site_dir.join(absent).exists(), "{absent} should not exist");
    }
    let agents_md = fs::read_to_string(site_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("Detailed guides live in `.agents/rules/`"));
    assert!(!agents_md.contains(".claude/"));

    // Gemini only: GEMINI.md shim, settings with hooks, the /seite command.
    init_site_with_agents(&tmp, "gemini-only", "gemini");
    let site_dir = tmp.path().join("gemini-only");
    assert_eq!(
        fs::read_to_string(site_dir.join("GEMINI.md")).unwrap(),
        "@AGENTS.md\n"
    );
    let settings = read_json_file(&site_dir.join(".gemini/settings.json"));
    assert_eq!(
        settings["hooks"]["AfterTool"][0]["matcher"],
        "write_file|replace"
    );
    assert!(site_dir.join(".gemini/commands/seite.toml").exists());
    assert!(site_dir.join(".agents/skills/seite/SKILL.md").exists());
    assert!(!site_dir.join(".claude").exists());
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

#[test]
fn test_init_agents_md_mentions_seite_workflow_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Workflow", "posts");
    let agents_md = fs::read_to_string(tmp.path().join("site/AGENTS.md")).unwrap();
    assert!(agents_md.contains("`/seite <command>`"), "{agents_md}");
    let skill = fs::read_to_string(tmp.path().join("site/.agents/skills/seite/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\nname: seite\n"));
}
