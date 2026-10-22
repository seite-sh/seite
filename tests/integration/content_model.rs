//! Typed frontmatter schemas, scheduled publishing, and `seite check --external`.

use super::common::*;

/// A posts site whose schema requires `extra.author`, bounds
/// `extra.rating`, enumerates `extra.status` and caps `description`.
fn schema_site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Schema", "posts");
    let site = tmp.path().join("site");
    let mut toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    // Replace the preset schema with a stricter one.
    let start = toml.find("[collections.fields.description]").unwrap();
    let end = toml.find("[build]").unwrap();
    toml.replace_range(
        start..end,
        r#"[collections.fields.description]
type = "string"
max = 20
description = "Short summary"

[collections.fields.author]
type = "string"
required = true
default = "Ada"
description = "Who wrote it"

[collections.fields.rating]
type = "integer"
min = 1
max = 5

[collections.fields.status]
type = "enum"
enum = ["draft", "published"]

"#,
    );
    fs::write(site.join("seite.toml"), toml).unwrap();
    // The sample post predates this schema.
    for entry in fs::read_dir(site.join("content/posts")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    site
}

const BAD_POST: &str = "---\ntitle: Bad\ndescription: this description is far too long\nextra:\n  rating: 9\n  status: pending\n---\nBody\n";

fn diagnostics_of(doc: &serde_json::Value) -> Vec<serde_json::Value> {
    doc["data"]["diagnostics"]
        .as_array()
        .or_else(|| doc["error"]["diagnostics"].as_array())
        .cloned()
        .unwrap_or_default()
}

fn schema_diags(doc: &serde_json::Value) -> Vec<serde_json::Value> {
    diagnostics_of(doc)
        .into_iter()
        .filter(|d| d["code"] == "frontmatter-schema")
        .collect()
}

fn assert_bad_post_diagnostics(diags: &[serde_json::Value], doc: &serde_json::Value) {
    let find = |needle: &str| {
        diags
            .iter()
            .find(|d| d["message"].as_str().unwrap().contains(needle))
            .unwrap_or_else(|| panic!("no diagnostic mentioning {needle}: {doc}"))
    };
    let desc = find("`description`");
    assert_eq!(desc["line"], 3, "{desc}");
    assert_eq!(desc["severity"], "error");
    assert!(desc["message"].as_str().unwrap().contains("too long"));
    assert!(desc["hint"].as_str().unwrap().starts_with("Short summary"));
    assert!(desc["file"]
        .as_str()
        .unwrap()
        .ends_with("content/posts/2024-01-01-bad.md"));
    let author = find("`extra.author`");
    assert_eq!(author["line"], 4, "points at `extra:` {author}");
    assert!(author["hint"].as_str().unwrap().contains("Who wrote it"));
    assert_eq!(find("`extra.rating`")["line"], 5);
    let status = find("`extra.status`");
    assert_eq!(status["line"], 6);
    assert!(status["message"]
        .as_str()
        .unwrap()
        .contains("must be one of `draft`, `published`"));
    assert_eq!(diags.len(), 4, "{doc}");
}

#[test]
fn test_check_reports_frontmatter_schema_violations_with_lines() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(&site, "content/posts/2024-01-01-bad.md", BAD_POST);
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    assert_bad_post_diagnostics(&schema_diags(&doc), &doc);

    // Human output is compiler-style with the hint.
    let output = page_cmd().arg("check").current_dir(&site).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("content/posts/2024-01-01-bad.md:5: error[frontmatter-schema]: field `extra.rating` is too large"),
        "{stderr}"
    );
}

#[test]
fn test_build_fails_on_frontmatter_schema_violations() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(&site, "content/posts/2024-01-01-bad.md", BAD_POST);
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(!ok, "{doc}");
    assert_bad_post_diagnostics(&schema_diags(&doc), &doc);
}

#[test]
fn test_build_skips_schema_of_drafts_unless_included() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(
        &site,
        "content/posts/2024-01-01-bad.md",
        &BAD_POST.replace("title: Bad", "title: Bad\ndraft: true"),
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    let (ok, doc) = run_json(&site, &["build", "--drafts"]);
    assert!(!ok, "{doc}");
}

#[test]
fn test_check_file_reports_frontmatter_schema() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(&site, "content/posts/2024-01-01-bad.md", BAD_POST);
    let (ok, doc) = run_json(
        &site,
        &["check", "--file", "content/posts/2024-01-01-bad.md"],
    );
    assert!(!ok);
    assert_bad_post_diagnostics(&schema_diags(&doc), &doc);
}

#[test]
fn test_check_post_edit_hook_reports_frontmatter_schema() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(&site, "content/posts/2024-01-01-bad.md", BAD_POST);
    let path = site
        .join("content/posts/2024-01-01-bad.md")
        .to_string_lossy()
        .to_string();
    let output = page_cmd()
        .args(["check", "--hook", "claude", "--post-edit"])
        .current_dir(&site)
        .env_remove("CLAUDE_PROJECT_DIR")
        .write_stdin(
            serde_json::json!({ "tool_name": "Edit", "tool_input": { "file_path": path } })
                .to_string(),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let context = doc["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        context.contains("2024-01-01-bad.md:6: error[frontmatter-schema]"),
        "{context}"
    );
}

#[test]
fn test_check_strict_fields_warns_about_unknown_keys() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replacen(
            "default_template = \"post.html\"\n",
            "default_template = \"post.html\"\nstrict_fields = true\n",
            1,
        ),
    )
    .unwrap();
    write_site_file(
        &site,
        "content/posts/2024-01-01-ok.md",
        "---\ntitle: Ok\ncolour: red\nextra:\n  author: Me\n  athor: typo\n---\nBody\n",
    );
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "warnings only: {doc}");
    let unknown: Vec<_> = diagnostics_of(&doc)
        .into_iter()
        .filter(|d| d["code"] == "frontmatter-schema-unknown")
        .collect();
    assert_eq!(unknown.len(), 2, "{doc}");
    assert_eq!(unknown[0]["line"], 3);
    assert_eq!(unknown[1]["line"], 6);
    assert_eq!(unknown[1]["hint"], "did you mean `author`?");
    let (ok, _) = run_json(&site, &["check", "--strict"]);
    assert!(!ok);
}

#[test]
fn test_check_config_rejects_invalid_field_schema_with_line() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replace("enum = [\"draft\", \"published\"]\n", "requird = true\n"),
    )
    .unwrap();
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    let d = &diagnostics_of(&doc)[0];
    assert_eq!(d["code"], "config-invalid", "{doc}");
    assert!(d["message"]
        .as_str()
        .unwrap()
        .contains("[collections.fields.status]"));
    let line = d["line"].as_u64().unwrap() as usize;
    let text = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert_eq!(
        text.lines().nth(line - 1).unwrap(),
        "[collections.fields.status]"
    );

    // With the enum fixed, the typo is an unknown-key warning.
    fs::write(
        site.join("seite.toml"),
        text.replace("requird = true\n", "enum = [\"a\"]\nrequird = true\n"),
    )
    .unwrap();
    let (_, doc) = run_json(&site, &["check"]);
    let unknown: Vec<_> = diagnostics_of(&doc)
        .into_iter()
        .filter(|d| d["code"] == "config-unknown-key")
        .collect();
    assert_eq!(unknown.len(), 1, "{doc}");
    assert_eq!(unknown[0]["hint"], "did you mean `required`?");
}

#[test]
fn test_new_writes_schema_defaults_and_placeholders() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replace(
            "[collections.fields.rating]\ntype = \"integer\"\n",
            "[collections.fields.rating]\ntype = \"integer\"\nrequired = true\n",
        ),
    )
    .unwrap();
    let (ok, doc) = run_json(&site, &["new", "post", "Hello Schema"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["fill_in"], serde_json::json!(["rating"]));
    let path = doc["data"]["path"].as_str().unwrap();
    let text = fs::read_to_string(path).unwrap();
    assert!(
        text.contains("extra:\n  author: Ada\n  rating: null\n"),
        "{text}"
    );
    let problems = doc["data"]["schema_problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{doc}");
    assert!(problems[0]["message"]
        .as_str()
        .unwrap()
        .contains("missing required field `extra.rating`"));
    // `seite check` points at the same field until it is filled in.
    let (ok, _) = run_json(&site, &["check"]);
    assert!(!ok);
}

#[test]
fn test_mcp_create_content_validates_schema() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_create_content",
        serde_json::json!({
            "collection": "posts",
            "title": "Rated",
            "extra": { "rating": 11 }
        }),
    );
    assert!(is_error, "{text}");
    assert!(text.contains("field `extra.rating` is too large"), "{text}");
    assert!(text.contains("nothing was written"), "{text}");
    assert!(!site.join("content/posts").read_dir().unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("rated")));

    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_create_content",
        serde_json::json!({
            "collection": "posts",
            "title": "Rated",
            "extra": { "rating": 4, "status": "draft" }
        }),
    );
    assert!(!is_error, "{text}");
    let result: serde_json::Value = serde_json::from_str(&text).unwrap();
    let file = fs::read_to_string(site.join(result["path"].as_str().unwrap())).unwrap();
    assert!(file.contains("author: Ada"), "default applied: {file}");
}

#[test]
fn test_mcp_update_frontmatter_validates_schema_and_routes_custom_fields() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(
        &site,
        "content/posts/2024-01-01-p.md",
        "---\ntitle: P\nextra:\n  author: Me\n---\nBody\n",
    );
    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_update_frontmatter",
        serde_json::json!({ "path": "content/posts/2024-01-01-p.md", "set": { "status": "pending" } }),
    );
    assert!(is_error, "{text}");
    assert!(
        text.contains("must be one of `draft`, `published`"),
        "{text}"
    );

    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_update_frontmatter",
        serde_json::json!({ "path": "content/posts/2024-01-01-p.md", "set": { "status": "published" } }),
    );
    assert!(!is_error, "{text}");
    assert!(text.contains("stored as `extra.status`"), "{text}");
    let file = fs::read_to_string(site.join("content/posts/2024-01-01-p.md")).unwrap();
    assert!(file.contains("status: published"), "{file}");
    assert!(!file.contains("\nstatus:"), "not at the top level: {file}");

    // Removing a required field is refused.
    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_update_frontmatter",
        serde_json::json!({ "path": "content/posts/2024-01-01-p.md", "unset": ["author"] }),
    );
    assert!(is_error, "{text}");
    assert!(
        text.contains("missing required field `extra.author`"),
        "{text}"
    );
}

#[test]
fn test_templates_and_context_include_collection_schemas() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let (ok, doc) = run_json(&site, &["templates"]);
    assert!(ok, "{doc}");
    let posts = &doc["data"]["collection_schemas"][0];
    assert_eq!(posts["collection"], "posts");
    let author = posts["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "author")
        .unwrap();
    assert_eq!(author["location"], "extra.author");
    assert_eq!(author["required"], true);
    let (ok, doc) = run_json(&site, &["context"]);
    assert!(ok, "{doc}");
    assert_eq!(
        doc["data"]["collections"][0]["schema"]["fields"][1]["name"],
        "author"
    );
}

#[test]
fn test_init_writes_preset_schemas_and_site_checks_clean() {
    let tmp = TempDir::new().unwrap();
    init_site(
        &tmp,
        "site",
        "Presets",
        "posts,docs,pages,changelog,roadmap",
    );
    let site = tmp.path().join("site");
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert!(toml.contains("[collections.fields.description]\ntype = \"string\"\nmax = 300"));
    assert!(toml.contains("[collections.fields.weight]"));
    assert!(
        !toml.contains("required"),
        "presets require nothing: {toml}"
    );
    let (ok, doc) = run_json(&site, &["check", "--strict"]);
    assert!(ok, "{doc}");
}

#[test]
fn test_upgrade_does_not_add_schemas() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Old", "posts");
    let site = tmp.path().join("site");
    // A site from before schemas: no [collections.fields.*] tables.
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    let start = toml.find("[collections.fields.").unwrap();
    let end = toml.find("[build]").unwrap();
    let mut old = toml.clone();
    old.replace_range(start..end, "");
    fs::write(site.join("seite.toml"), &old).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .assert()
        .success();
    let after = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert!(!after.contains("[collections.fields."), "{after}");
}

// --- scheduled publishing ---

fn scheduled_site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Sched", "posts");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "content/posts/2999-01-01-futurepost.md",
        "---\ntitle: Futurepost\ndescription: Futuredesc\ntags: [futuretag]\n---\nFuturebody\n",
    );
    write_site_file(
        &site,
        "content/posts/oldpost.md",
        "---\ntitle: Oldpost\ndate: 2001-01-01\nexpires: 2002-01-01\ndescription: Olddesc\ntags: [oldtag]\n---\nOldbody\n",
    );
    write_site_file(
        &site,
        "content/posts/2020-01-01-livepost.md",
        "---\ntitle: Livepost\ndate: 2020-01-01\nexpires: 2999-01-01\ntags: [livetag]\n---\nLivebody\n",
    );
    site
}

/// Every file under `dist` whose contents mention `needle`.
fn files_mentioning(dist: &std::path::Path, needle: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dist.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if fs::read(&path)
                .map(|b| String::from_utf8_lossy(&b).contains(needle))
                .unwrap_or(false)
                || path.to_string_lossy().contains(needle)
            {
                found.push(path.strip_prefix(dist).unwrap().display().to_string());
            }
        }
    }
    found
}

#[test]
fn test_build_excludes_future_and_expired_content_from_every_output() {
    let tmp = TempDir::new().unwrap();
    let site = scheduled_site(&tmp);
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    let dist = site.join("dist");
    for needle in [
        "futurepost",
        "Futurepost",
        "futuretag",
        "Oldpost",
        "oldpost",
        "oldtag",
    ] {
        let found = files_mentioning(&dist, needle);
        assert!(found.is_empty(), "{needle} leaked into {found:?}");
    }
    // The live post is everywhere: page, markdown, feeds, sitemap, search, llms, tags.
    let live = files_mentioning(&dist, "livepost");
    for expected in [
        "posts/livepost.html",
        "posts/livepost.md",
        "feed.xml",
        "atom.xml",
        "sitemap.xml",
        "search-index.json",
        "llms.txt",
        "llms-full.txt",
    ] {
        assert!(
            live.iter().any(|f| f == expected),
            "{expected} missing from {live:?}"
        );
    }
    assert!(!files_mentioning(&dist, "livetag").is_empty());
}

#[test]
fn test_build_future_flag_and_config_include_scheduled_content() {
    let tmp = TempDir::new().unwrap();
    let site = scheduled_site(&tmp);
    let (ok, doc) = run_json(&site, &["build", "--future"]);
    assert!(ok, "{doc}");
    assert!(site.join("dist/posts/futurepost.html").exists());
    assert!(!files_mentioning(&site.join("dist"), "futuretag").is_empty());
    assert!(
        !site.join("dist/posts/oldpost.html").exists(),
        "expired stays out"
    );

    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replace("[build]\n", "[build]\nfuture = true\n"),
    )
    .unwrap();
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    assert!(site.join("dist/posts/futurepost.html").exists());
}

#[test]
fn test_build_timezone_decides_what_today_is() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Tz", "posts");
    let site = tmp.path().join("site");
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    let with_tz = |tz: &str| {
        fs::write(
            site.join("seite.toml"),
            toml.replace(
                "author = \"\"\n",
                &format!("author = \"\"\ntimezone = \"{tz}\"\n"),
            ),
        )
        .unwrap();
    };
    // Today at UTC+14 is always a later date than today at UTC-12.
    with_tz("+14:00");
    let (_, stats) = run_json(&site, &["stats"]);
    let east_today = stats["data"]["today"].as_str().unwrap().to_string();
    write_site_file(
        &site,
        "content/posts/tzpost.md",
        &format!("---\ntitle: Tzpost\ndate: {east_today}\n---\nx\n"),
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    assert!(site.join("dist/posts/tzpost.html").exists());
    with_tz("-12:00");
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    assert!(!site.join("dist/posts/tzpost.html").exists());
    // IANA names work; nonsense is a located config error.
    with_tz("Pacific/Kiritimati");
    assert!(run_json(&site, &["build"]).0);
    with_tz("Mars/Olympus");
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    let d = &diagnostics_of(&doc)[0];
    assert_eq!(d["code"], "config-invalid");
    assert!(d["message"].as_str().unwrap().contains("IANA"), "{d}");
    assert_eq!(d["line"], 7, "{d}");
}

#[test]
fn test_check_renders_scheduled_content() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Sched", "posts");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "content/posts/2999-01-01-later.md",
        "---\ntitle: Later\n---\n\n{{< youtub(id=\"x\") >}}\n",
    );
    // The build leaves the scheduled post out, so it passes...
    assert!(run_json(&site, &["build"]).0);
    // ...but check validates it before the cron publishes it.
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    assert!(diagnostics_of(&doc)
        .iter()
        .any(|d| d["code"] == "shortcode-unknown"));
}

#[test]
fn test_stats_and_context_report_expired_and_next_scheduled() {
    let tmp = TempDir::new().unwrap();
    let site = scheduled_site(&tmp);
    let (ok, doc) = run_json(&site, &["stats"]);
    assert!(ok, "{doc}");
    let data = &doc["data"];
    assert_eq!(data["totals"]["future_dated"], 1);
    assert_eq!(data["totals"]["expired"], 1);
    assert_eq!(data["collections"][0]["expired"][0]["date"], "2002-01-01");
    assert_eq!(data["next_scheduled"]["title"], "Futurepost");
    assert_eq!(data["next_scheduled"]["date"], "2999-01-01");
    let output = page_cmd()
        .arg("context")
        .current_dir(&site)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("next scheduled: 2999-01-01 (Futurepost)"),
        "{stdout}"
    );
}

// --- the daily cron in the generated workflow ---

#[test]
fn test_init_workflow_has_daily_cron() {
    for target in ["github-pages", "cloudflare", "netlify"] {
        let tmp = TempDir::new().unwrap();
        page_cmd()
            .args([
                "init",
                "site",
                "--title",
                "T",
                "--description",
                "",
                "--deploy-target",
                target,
                "--collections",
                "posts",
            ])
            .current_dir(tmp.path())
            .assert()
            .success();
        let wf = fs::read_to_string(tmp.path().join("site/.github/workflows/deploy.yml")).unwrap();
        assert!(wf.contains("  schedule:\n"), "{target}: {wf}");
        assert!(wf.contains("- cron: '17 5 * * *'"), "{target}");
    }
}

fn set_project_version(site: &std::path::Path, version: &str) {
    let path = site.join(".seite/config.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    meta["version"] = version.into();
    fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
}

const CRON_BLOCK: &str = "  schedule:\n    # Daily rebuild: publishes scheduled posts (future `date:`) and removes expired ones\n    - cron: '17 5 * * *'\n";

#[test]
fn test_upgrade_adds_cron_to_unmodified_workflow() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "T", "posts");
    let site = tmp.path().join("site");
    let wf_path = site.join(".github/workflows/deploy.yml");
    let generated = fs::read_to_string(&wf_path).unwrap();
    assert!(generated.contains(CRON_BLOCK));
    fs::write(&wf_path, generated.replace(CRON_BLOCK, "")).unwrap();
    set_project_version(&site, "0.20.1");
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&wf_path).unwrap(), generated);
}

#[test]
fn test_upgrade_leaves_modified_workflow_and_explains_the_cron() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "T", "posts");
    let site = tmp.path().join("site");
    let wf_path = site.join(".github/workflows/deploy.yml");
    let edited = fs::read_to_string(&wf_path)
        .unwrap()
        .replace(CRON_BLOCK, "")
        .replace("Build site", "Build my site");
    fs::write(&wf_path, &edited).unwrap();
    set_project_version(&site, "0.20.1");
    let output = page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(&wf_path).unwrap(), edited);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(all.contains("- cron: '17 5 * * *'"), "{all}");
}

// --- seite check --external ---

/// Local HTTP server: /ok 200, /missing 404, /slow (sleeps 3 s).
fn link_server() -> String {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            std::thread::spawn(move || {
                let code = match req.url() {
                    "/ok" => 200,
                    "/slow" => {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        200
                    }
                    _ => 404,
                };
                let _ = req.respond(tiny_http::Response::empty(code));
            });
        }
    });
    format!("http://127.0.0.1:{port}")
}

#[test]
fn test_check_external_reports_broken_links_with_source_lines() {
    let base = link_server();
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Ext", "posts");
    let site = tmp.path().join("site");
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        format!("{toml}\n[check]\nexternal_timeout = 1\nexternal_skip = [\"skipme.test\", \"fonts.googleapis.com\"]\n"),
    )
    .unwrap();
    write_site_file(
        &site,
        "content/posts/2024-01-01-links.md",
        &format!(
            "---\ntitle: Links\n---\n\nIntro.\n\n[ok]({base}/ok)\n[bad]({base}/missing)\n[slow]({base}/slow)\n[li](https://www.linkedin.com/in/someone)\n[skip](https://docs.skipme.test/x)\n"
        ),
    );
    // Not run by default (no network, no cache).
    let (ok, _) = run_json(&site, &["check"]);
    assert!(ok);
    assert!(!site.join(".seite/cache/links.json").exists());

    let (ok, doc) = run_json(&site, &["check", "--external"]);
    assert!(ok, "warnings only: {doc}");
    let diags = diagnostics_of(&doc);
    let broken = diags
        .iter()
        .find(|d| d["code"] == "external-link-broken")
        .unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(broken["file"], "content/posts/2024-01-01-links.md");
    assert_eq!(broken["line"], 8);
    assert_eq!(broken["severity"], "warning");
    assert!(broken["message"].as_str().unwrap().contains("404"));
    let slow = diags
        .iter()
        .find(|d| d["code"] == "external-link-unreachable")
        .unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(slow["line"], 9);
    assert!(slow["message"].as_str().unwrap().contains("timed out"));
    let ext = &doc["data"]["external"];
    assert!(
        ext["skipped"].as_u64().unwrap() >= 3,
        "linkedin + skip list + own URLs: {ext}"
    );
    assert_eq!(
        ext["checked"], 3,
        "only the local server is requested: {ext}"
    );
    assert_eq!(ext["broken"], 1);
    assert_eq!(ext["unreachable"], 1);

    let cache = fs::read_to_string(site.join(".seite/cache/links.json")).unwrap();
    assert!(cache.contains(&format!("{base}/ok")), "{cache}");
    let gitignore = fs::read_to_string(site.join(".gitignore")).unwrap();
    assert!(gitignore.contains("/.seite/cache/"));

    // Second run: the working link comes from the cache; --strict fails.
    let (ok, doc) = run_json(&site, &["check", "--external", "--strict"]);
    assert!(!ok);
    assert_eq!(doc["data"]["external"]["cached"], 1, "{doc}");
    assert!(diagnostics_of(&doc)
        .iter()
        .any(|d| d["code"] == "external-link-broken" && d["severity"] == "error"));
}
