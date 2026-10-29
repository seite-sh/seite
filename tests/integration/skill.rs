use super::common::*;

// --- skill command ---

#[test]
fn test_skill_help() {
    page_cmd()
        .args(["skill", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("update"));
}

#[test]
fn test_skill_list_shows_bundled() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillsite", "Skill Test", "posts,pages");
    let site_dir = tmp.path().join("skillsite");

    page_cmd()
        .args(["skill", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("seite (bundled)"))
        .stdout(predicate::str::contains("theme-builder").not());
}

#[test]
fn test_skill_list_shows_available_packs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillpk", "Pack Test", "posts");
    let site_dir = tmp.path().join("skillpk");

    page_cmd()
        .args(["skill", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("seomachine"))
        .stdout(predicate::str::contains("Available packs"));
}

#[test]
fn test_skill_remove_nonexistent() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillrm", "Remove Test", "posts");
    let site_dir = tmp.path().join("skillrm");

    page_cmd()
        .args(["skill", "remove", "nonexistent-pack"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no skill or pack named"));
}

#[test]
fn test_skill_install_unknown_name() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillunk", "Unknown Test", "posts");
    let site_dir = tmp.path().join("skillunk");

    page_cmd()
        .args(["skill", "install", "nonexistent-pack-name"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown skill pack"));
}

#[test]
fn test_skill_remove_custom_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillcust", "Custom Test", "posts");
    let site_dir = tmp.path().join("skillcust");

    // Manually create a custom skill
    let skill_dir = site_dir.join(".claude").join("skills").join("my-custom");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# My Custom Skill\n").unwrap();

    // Verify it shows up in list
    page_cmd()
        .args(["skill", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("my-custom"));

    // Remove it
    page_cmd()
        .args(["skill", "remove", "my-custom"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify it's gone
    assert!(!skill_dir.exists());
}

#[test]
fn test_skill_update_nothing_installed() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillupd", "Update Test", "posts");
    let site_dir = tmp.path().join("skillupd");

    page_cmd()
        .args(["skill", "update"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing to update"));
}

// --- seomachine (pinned, hash-verified, per-agent) ---

#[test]
fn test_skill_install_help_lists_agents_flag() {
    page_cmd()
        .args(["skill", "install", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--agents"));
    page_cmd()
        .args(["skill", "update", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--agents"));
}

#[test]
fn test_skill_install_seomachine_rejects_unknown_agent_before_downloading() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillag", "Agents Test", "posts");
    let site_dir = tmp.path().join("skillag");

    page_cmd()
        .args(["skill", "install", "seomachine", "--agents", "emacs"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown agent"));
    assert!(!site_dir.join("context").exists());
    assert!(!site_dir.join(".claude/agents").exists());
}

#[test]
fn test_skill_remove_pack_keeps_edited_files_and_reads_old_manifest() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillrm", "Remove Test", "posts");
    let site_dir = tmp.path().join("skillrm");

    let agents = site_dir.join(".claude/agents");
    std::fs::create_dir_all(&agents).unwrap();
    std::fs::write(agents.join("editor.md"), "my edit").unwrap();
    std::fs::write(agents.join("cluster-strategist.md"), "as installed").unwrap();
    std::fs::write(agents.join("seo-optimizer.md"), "no hash recorded").unwrap();

    // `cluster-strategist.md` matches its recorded SHA-256; `editor.md` does not; `seo-optimizer.md`
    // comes from a manifest that predates hashes.
    let gone_hash = {
        // sha256("as installed")
        use sha2::{Digest, Sha256};
        Sha256::digest(b"as installed")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let manifest = format!(
        r#"{{"packs":{{"seomachine":{{"source":"github:a/b","branch":"abc","installed_at":"t","files":[".claude/agents/editor.md",".claude/agents/cluster-strategist.md",".claude/agents/seo-optimizer.md"],"hashes":{{".claude/agents/editor.md":"{}",".claude/agents/cluster-strategist.md":"{gone_hash}"}}}}}}}}"#,
        "0".repeat(64)
    );
    std::fs::write(site_dir.join(".claude/.seite-skill-packs.json"), manifest).unwrap();

    page_cmd()
        .args(["skill", "remove", "seomachine"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(agents.join("editor.md")).unwrap(),
        "my edit"
    );
    assert!(!agents.join("cluster-strategist.md").exists());
    assert!(!agents.join("seo-optimizer.md").exists());
}

// --- impeccable (pinned npm installer; fake npm on PATH) ---

#[test]
fn test_skill_list_shows_impeccable_available() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillimp", "Impeccable Test", "posts");
    let site_dir = tmp.path().join("skillimp");

    page_cmd()
        .args(["skill", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("impeccable"))
        .stdout(predicate::str::contains("seite skill install impeccable"));

    let (ok, doc) = run_json(&site_dir, &["skill", "list"]);
    assert!(ok, "{doc}");
    let packs = doc["data"]["packs"].as_array().unwrap();
    let imp = packs.iter().find(|p| p["name"] == "impeccable").unwrap();
    assert_eq!(imp["installed"], false);
    assert!(doc["data"]["bundled"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("seite")));
}

#[test]
fn test_skill_list_detects_impeccable_skill_dir() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillimpdir", "Impeccable Dir", "posts");
    let site_dir = tmp.path().join("skillimpdir");
    let dir = site_dir.join(".agents/skills/impeccable");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), "---\nname: impeccable\n---\n").unwrap();

    let (ok, doc) = run_json(&site_dir, &["skill", "list"]);
    assert!(ok, "{doc}");
    let packs = doc["data"]["packs"].as_array().unwrap();
    let imp = packs.iter().find(|p| p["name"] == "impeccable").unwrap();
    assert_eq!(imp["installed"], true);
}

#[cfg(unix)]
mod fake_npm {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A site (agents claude,codex) plus a PATH dir holding a fake `npm`. Its
    /// `install --prefix <dir> …` records its own argv and cwd (`npm-argv`,
    /// `npm-cwd`) and drops a fake `impeccable` installer into
    /// `<dir>/node_modules/.bin/`, which records its argv (one numbered file
    /// per run) and cwd and exits with `$FAKE_EXIT`.
    struct Fake {
        tmp: TempDir,
        bin: TempDir,
        calls: TempDir,
    }

    impl Fake {
        fn new() -> Self {
            let tmp = TempDir::new().unwrap();
            init_site_with_agents(&tmp, "site", "claude,codex");
            let bin = TempDir::new().unwrap();
            let installer = "#!/bin/sh\n\
                n=$(ls \"$FAKE_CALLS\" | grep -c '^[0-9]')\n\
                for a in \"$@\"; do printf '%s\\0' \"$a\"; done > \"$FAKE_CALLS/$n\"\n\
                pwd > \"$FAKE_CALLS/cwd\"\n\
                echo 'fake impeccable output'\n\
                exit ${FAKE_EXIT:-0}\n";
            let script = format!(
                "#!/bin/sh\n\
                if [ \"$1\" = \"--version\" ]; then echo 10.0.0; exit 0; fi\n\
                for a in \"$@\"; do printf '%s\\0' \"$a\"; done > \"$FAKE_CALLS/npm-argv\"\n\
                pwd > \"$FAKE_CALLS/npm-cwd\"\n\
                prefix=\n\
                while [ $# -gt 0 ]; do\n\
                  if [ \"$1\" = \"--prefix\" ]; then prefix=\"$2\"; fi\n\
                  shift\n\
                done\n\
                mkdir -p \"$prefix/node_modules/.bin\"\n\
                cat > \"$prefix/node_modules/.bin/impeccable\" <<'EOS'\n{installer}EOS\n\
                chmod +x \"$prefix/node_modules/.bin/impeccable\"\n"
            );
            let path = bin.path().join("npm");
            fs::write(&path, script).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            Fake {
                tmp,
                bin,
                calls: TempDir::new().unwrap(),
            }
        }

        fn site(&self) -> std::path::PathBuf {
            self.tmp.path().join("site")
        }

        fn cmd(&self, args: &[&str]) -> Command {
            let path = format!(
                "{}:{}",
                self.bin.path().display(),
                std::env::var("PATH").unwrap_or_default()
            );
            let mut cmd = page_cmd();
            cmd.args(args)
                .current_dir(self.site())
                .env("PATH", path)
                .env("FAKE_CALLS", self.calls.path())
                .env_remove("SEITE_YES")
                .write_stdin("");
            cmd
        }

        /// argv of every recorded installer run.
        fn calls(&self) -> Vec<Vec<String>> {
            let mut files: Vec<(usize, std::path::PathBuf)> = fs::read_dir(self.calls.path())
                .unwrap()
                .flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_str()?.parse().ok()?;
                    Some((n, e.path()))
                })
                .collect();
            files.sort();
            files
                .into_iter()
                .map(|(_, p)| {
                    fs::read(p)
                        .unwrap()
                        .split(|b| *b == 0)
                        .filter(|s| !s.is_empty())
                        .map(|s| String::from_utf8(s.to_vec()).unwrap())
                        .collect()
                })
                .collect()
        }

        fn only_call(&self) -> Vec<String> {
            let calls = self.calls();
            assert_eq!(calls.len(), 1, "{calls:?}");
            calls.into_iter().next().unwrap()
        }

        fn manifest(&self) -> serde_json::Value {
            read_json_file(&self.site().join(".claude/.seite-skill-packs.json"))
        }
    }

    #[test]
    fn test_skill_install_impeccable_runs_pinned_installer_for_recorded_agents() {
        let fake = Fake::new();
        fake.cmd(&["--yes", "skill", "install", "impeccable"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Installed 'impeccable'"));

        let argv = fake.only_call();
        assert_eq!(argv[0], "install", "{argv:?}");
        assert!(
            argv.contains(&"--providers=claude,codex".to_string()),
            "{argv:?}"
        );
        assert!(argv.contains(&"--scope=project".to_string()), "{argv:?}");
        assert_eq!(argv.last().map(String::as_str), Some("-y"), "{argv:?}");

        // The installer ran in the site root; npm fetched the pinned version
        // from a temp dir, never the project (whose .npmrc/node_modules it
        // must not see).
        let cwd = fs::read_to_string(fake.calls.path().join("cwd")).unwrap();
        assert_eq!(
            fs::canonicalize(cwd.trim()).unwrap(),
            fs::canonicalize(fake.site()).unwrap()
        );
        let npm_cwd = fs::read_to_string(fake.calls.path().join("npm-cwd")).unwrap();
        // (The temp dir is gone by now, so compare the path as recorded.)
        let npm_cwd = std::path::Path::new(npm_cwd.trim());
        assert!(
            !npm_cwd.starts_with(fs::canonicalize(fake.site()).unwrap())
                && !npm_cwd.starts_with(fake.site()),
            "{}",
            npm_cwd.display()
        );
        let npm_argv = String::from_utf8(fs::read(fake.calls.path().join("npm-argv")).unwrap())
            .unwrap()
            .replace('\0', " ");
        assert!(
            npm_argv.contains(&format!(
                "impeccable@{}",
                seite::cli::skill::IMPECCABLE_VERSION
            )),
            "{npm_argv}"
        );
        assert!(npm_argv.contains("--prefix"), "{npm_argv}");

        let entry = &fake.manifest()["packs"]["impeccable"];
        assert_eq!(entry["source"], "npm:impeccable");
        assert_eq!(entry["branch"], "external");
    }

    #[test]
    fn test_skill_install_impeccable_json_keeps_stdout_clean() {
        let fake = Fake::new();
        let output = fake
            .cmd(&["--json", "--yes", "skill", "install", "impeccable"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout is not one JSON document ({e}): {}",
                String::from_utf8_lossy(&output.stdout)
            )
        });
        assert_eq!(doc["data"]["pack"], "impeccable");
        assert_eq!(doc["data"]["installed"], true);
        assert_eq!(
            doc["data"]["providers"],
            serde_json::json!(["claude", "codex"])
        );

        let output = fake.cmd(&["--json", "skill", "list"]).output().unwrap();
        assert!(output.status.success());
        let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let packs = doc["data"]["packs"].as_array().unwrap();
        let imp = packs.iter().find(|p| p["name"] == "impeccable").unwrap();
        assert_eq!(imp["installed"], true);
        assert_eq!(imp["source"], "npm:impeccable");
    }

    #[test]
    fn test_skill_install_impeccable_defaults_to_all_agents() {
        let fake = Fake::new();
        let meta_path = fake.site().join(".seite/config.json");
        let mut meta = read_json_file(&meta_path);
        meta.as_object_mut().unwrap().remove("agents");
        fs::write(&meta_path, serde_json::to_string(&meta).unwrap()).unwrap();

        fake.cmd(&["--yes", "skill", "install", "impeccable"])
            .assert()
            .success();
        let argv = fake.only_call();
        assert!(
            argv.contains(&"--providers=claude,codex,opencode,cursor,gemini,github".to_string()),
            "{argv:?}"
        );
    }

    #[test]
    fn test_skill_install_impeccable_non_tty_uses_confirm_default() {
        // No --yes and no TTY: the confirmation defaults to yes.
        let fake = Fake::new();
        fake.cmd(&["skill", "install", "impeccable"])
            .assert()
            .success();
        assert_eq!(fake.calls().len(), 1);
    }

    #[test]
    fn test_skill_update_impeccable_runs_installer_update() {
        let fake = Fake::new();
        fake.cmd(&["--yes", "skill", "install", "impeccable"])
            .assert()
            .success();
        fake.cmd(&["--yes", "skill", "update", "impeccable"])
            .assert()
            .success();
        let calls = fake.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        let argv = &calls[1];
        assert_eq!(argv[0], "update", "{argv:?}");
        assert!(argv.contains(&"--scope=project".to_string()), "{argv:?}");
        assert!(argv.contains(&"-y".to_string()), "{argv:?}");
        assert!(
            !argv.iter().any(|a| a.starts_with("--providers")),
            "{argv:?}"
        );
    }

    #[test]
    fn test_skill_update_all_includes_impeccable() {
        let fake = Fake::new();
        fake.cmd(&["--yes", "skill", "install", "impeccable"])
            .assert()
            .success();
        fake.cmd(&["--yes", "skill", "update"]).assert().success();
        let calls = fake.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(calls[1][0], "update", "{calls:?}");
    }

    #[test]
    fn test_skill_update_impeccable_not_installed_errors() {
        let fake = Fake::new();
        fake.cmd(&["--yes", "skill", "update", "impeccable"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("not installed"));
        assert!(fake.calls().is_empty());
    }

    #[test]
    fn test_skill_install_impeccable_installer_failure_fails() {
        let fake = Fake::new();
        fake.cmd(&["--yes", "skill", "install", "impeccable"])
            .env("FAKE_EXIT", "1")
            .assert()
            .failure()
            .stderr(predicate::str::contains("failed"));
        assert_eq!(fake.calls().len(), 1);
        assert!(!fake.site().join(".claude/.seite-skill-packs.json").exists());
    }

    #[test]
    fn test_skill_install_impeccable_without_npm_mentions_nodejs() {
        let fake = Fake::new();
        let empty = TempDir::new().unwrap();
        page_cmd()
            .args(["--yes", "skill", "install", "impeccable"])
            .current_dir(fake.site())
            .env("PATH", empty.path())
            .assert()
            .failure()
            .stderr(predicate::str::contains("Node.js"));
    }

    #[test]
    fn test_skill_install_impeccable_outside_site_errors() {
        let fake = Fake::new();
        let elsewhere = TempDir::new().unwrap();
        fake.cmd(&["--yes", "skill", "install", "impeccable"])
            .current_dir(elsewhere.path())
            .assert()
            .failure()
            .stderr(predicate::str::contains("seite.toml"));
        assert!(fake.calls().is_empty());
    }

    #[test]
    fn test_skill_remove_impeccable_after_install() {
        let fake = Fake::new();
        fake.cmd(&["--yes", "skill", "install", "impeccable"])
            .assert()
            .success();
        let dir = fake.site().join(".claude/skills/impeccable");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "---\nname: impeccable\n---\n").unwrap();

        fake.cmd(&["skill", "remove", "impeccable"])
            .assert()
            .success();
        assert!(!dir.exists());
        assert!(fake.manifest()["packs"].get("impeccable").is_none());
    }
}

#[test]
fn test_skill_update_regenerates_links_map_without_packs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skilllinks", "Links Test", "posts,pages");
    let site_dir = tmp.path().join("skilllinks");

    // Serve posts under /blog, like seite-sh does.
    let config_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    let rewritten = config.replace("url_prefix = \"/posts\"", "url_prefix = \"/blog\"");
    assert_ne!(config, rewritten, "expected posts url_prefix in seite.toml");
    fs::write(&config_path, rewritten).unwrap();

    fs::write(
        site_dir.join("content/posts/2026-03-13-hugo-alternative.md"),
        "---\ntitle: \"Hugo Alternative\"\n---\nBody\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/pages/about.md"),
        "---\ntitle: About\n---\nBody\n",
    )
    .unwrap();
    fs::create_dir_all(site_dir.join("context")).unwrap();

    page_cmd()
        .args(["skill", "update"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Regenerated internal links map"))
        .stdout(predicate::str::contains("Nothing to update").not());

    let map = fs::read_to_string(site_dir.join("context/internal-links-map.md")).unwrap();
    assert!(map.contains("- [Hugo Alternative](/blog/hugo-alternative)"));
    assert!(map.contains("- [About](/about)"));
    assert!(!map.contains("/posts/hugo-alternative"));
    assert!(!map.contains("/pages/about"));
}
