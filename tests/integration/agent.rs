use super::common::*;

// --- agent --with harness selection ---

#[test]
fn test_agent_with_bogus_harness_errors_with_suggestion() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["agent", "--with", "claud", "hello"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unknown agent harness")
                .and(predicate::str::contains("did you mean 'claude'")),
        );
}

#[test]
fn test_agent_with_codex_not_on_path_errors_clearly() {
    let tmp = TempDir::new().unwrap();
    let empty_path_dir = TempDir::new().unwrap();

    page_cmd()
        .args(["agent", "--with", "codex", "hello"])
        .current_dir(tmp.path())
        .env("PATH", empty_path_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("codex").and(predicate::str::contains("not installed")));
}

// --- agent --with: exact argv handed to each harness (fake binaries on PATH) ---

#[cfg(unix)]
mod fake_harness {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A site plus a PATH dir holding fake harness binaries that record
    /// their argv (one file per invocation) and exit with `$FAKE_EXIT`.
    struct Fake {
        tmp: TempDir,
        bin: TempDir,
        calls: TempDir,
    }

    impl Fake {
        fn new() -> Self {
            let tmp = TempDir::new().unwrap();
            init_site(&tmp, "site", "Fake Harness Site", "posts,pages");
            let bin = TempDir::new().unwrap();
            let script = "#!/bin/sh\n\
                if [ \"$1\" = \"--version\" ]; then echo fake; exit 0; fi\n\
                n=$(ls \"$FAKE_CALLS\" | wc -l | tr -d ' ')\n\
                for a in \"$@\"; do printf '%s\\0' \"$a\"; done > \"$FAKE_CALLS/$n\"\n\
                if [ -n \"$FAKE_STDOUT\" ]; then printf '%s\\n' \"$FAKE_STDOUT\"; fi\n\
                exit ${FAKE_EXIT:-0}\n";
            for name in ["claude", "codex", "opencode", "cursor-agent", "gemini"] {
                let path = bin.path().join(name);
                fs::write(&path, script).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            }
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
                .env_remove("SEITE_AGENT")
                .env_remove("SEITE_YES")
                .write_stdin("");
            cmd
        }

        /// argv of every recorded harness invocation, in call order.
        fn calls(&self) -> Vec<Vec<String>> {
            let mut files: Vec<(usize, std::path::PathBuf)> = fs::read_dir(self.calls.path())
                .unwrap()
                .flatten()
                .map(|e| (e.file_name().to_str().unwrap().parse().unwrap(), e.path()))
                .collect();
            files.sort();
            files
                .into_iter()
                .map(|(_, p)| {
                    let raw = fs::read(p).unwrap();
                    raw.split(|b| *b == 0)
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
    }

    #[test]
    fn test_agent_claude_once_passes_the_prompt_as_is() {
        let fake = Fake::new();
        fake.cmd(&["agent", "--once", "write a post"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Agent finished"));
        // No injected context, tool allowlist, or MCP config: the project's
        // own settings apply.
        assert_eq!(fake.only_call(), ["-p", "write a post"]);
    }

    #[test]
    fn test_agent_claude_interactive_session_argv() {
        let fake = Fake::new();
        fake.cmd(&["agent"])
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Starting interactive claude agent session",
            ));
        assert_eq!(fake.only_call(), Vec::<String>::new());

        // A prompt seeds the interactive session (`claude "<prompt>"`).
        let fake = Fake::new();
        fake.cmd(&["agent", "draft a post"]).assert().success();
        assert_eq!(fake.only_call(), ["draft a post"]);
    }

    #[test]
    fn test_agent_codex_once_uses_workspace_write_sandbox() {
        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "codex", "--once", "fix links"])
            .assert()
            .success();
        assert_eq!(
            fake.only_call(),
            ["exec", "--sandbox", "workspace-write", "fix links"]
        );
    }

    #[test]
    fn test_agent_harness_selected_by_seite_agent_env() {
        let fake = Fake::new();
        fake.cmd(&["agent", "--once", "hi"])
            .env("SEITE_AGENT", "codex")
            .assert()
            .success();
        assert_eq!(fake.only_call()[0], "exec");
    }

    #[test]
    fn test_agent_yes_never_adds_auto_approve_flags() {
        for (harness, expected) in [
            ("cursor", &["-p", "tidy", "--output-format", "text"][..]),
            ("opencode", &["run", "tidy"][..]),
            ("gemini", &["-p", "tidy"][..]),
        ] {
            let fake = Fake::new();
            fake.cmd(&["--yes", "agent", "--with", harness, "--once", "tidy"])
                .env("SEITE_YES", "1")
                .assert()
                .success();
            assert_eq!(fake.only_call(), expected, "{harness}");
        }
    }

    #[test]
    fn test_agent_auto_approve_ignored_without_a_terminal() {
        // Tests have no TTY on stdin: the flag is refused with a warning.
        let fake = Fake::new();
        fake.cmd(&[
            "agent",
            "--with",
            "cursor",
            "--once",
            "--auto-approve",
            "tidy",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("--auto-approve ignored"));
        assert_eq!(fake.only_call(), ["-p", "tidy", "--output-format", "text"]);
    }

    #[test]
    fn test_agent_refuses_to_nest_inside_an_agent_session() {
        for (var, agent) in [
            ("CLAUDECODE", "Claude Code"),
            ("CODEX_THREAD_ID", "Codex"),
            ("GEMINI_CLI", "Gemini CLI"),
            ("OPENCODE", "OpenCode"),
            ("CURSOR_AGENT", "Cursor"),
        ] {
            let fake = Fake::new();
            fake.cmd(&["agent", "--once", "hi"])
                .env(var, "1")
                .assert()
                .failure()
                .stderr(predicate::str::contains(format!("already inside {agent}")));
            assert!(fake.calls().is_empty(), "{var}");

            let fake = Fake::new();
            fake.cmd(&["theme", "create", "dark teal"])
                .env(var, "1")
                .assert()
                .failure()
                .stderr(predicate::str::contains(format!("already inside {agent}")))
                .stderr(predicate::str::contains("/seite theme dark teal"));
            assert!(fake.calls().is_empty(), "{var}");
        }
    }

    #[test]
    fn test_agent_opencode_argv_once_and_interactive() {
        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "opencode", "--once", "go"])
            .assert()
            .success();
        assert_eq!(fake.only_call(), ["run", "go"]);

        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "opencode", "go"])
            .assert()
            .success();
        assert_eq!(fake.only_call(), ["--prompt", "go"]);

        // No prompt: a bare interactive session.
        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "opencode"])
            .assert()
            .success();
        assert_eq!(fake.only_call(), Vec::<String>::new());
    }

    #[test]
    fn test_agent_gemini_argv_once_and_interactive() {
        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "gemini", "--once", "go"])
            .assert()
            .success();
        assert_eq!(fake.only_call(), ["-p", "go"]);

        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "gemini", "go"])
            .assert()
            .success();
        assert_eq!(fake.only_call(), ["-i", "go"]);
    }

    #[test]
    fn test_theme_create_runs_the_seite_skill_without_impeccable() {
        let fake = Fake::new();
        fake.cmd(&["theme", "create", "coral brutalist"])
            .assert()
            .success()
            .stdout(predicate::str::contains("seite skill install impeccable"));
        assert_eq!(fake.only_call(), ["-p", "/seite theme coral brutalist"]);

        let fake = Fake::new();
        fake.cmd(&["theme", "create", "coral brutalist"])
            .env("SEITE_AGENT", "codex")
            .assert()
            .success();
        assert_eq!(
            fake.only_call(),
            [
                "exec",
                "--sandbox",
                "workspace-write",
                "$seite theme coral brutalist"
            ]
        );
    }

    #[test]
    fn test_theme_create_points_to_impeccable_when_installed() {
        let fake = Fake::new();
        fs::create_dir_all(fake.site().join(".claude/skills/impeccable")).unwrap();
        fake.cmd(&["theme", "create", "coral brutalist"])
            .assert()
            .success()
            .stdout(predicate::str::contains("/impeccable coral brutalist"));
        // No agent is started.
        assert!(fake.calls().is_empty());
    }
    #[test]
    fn test_agent_once_harness_failure_fails_seite() {
        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "codex", "--once", "hi"])
            .env("FAKE_EXIT", "3")
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "codex exited with non-zero status",
            ))
            .stdout(predicate::str::contains("Agent finished").not());
    }

    #[test]
    fn test_agent_interactive_harness_exit_is_not_an_error() {
        let fake = Fake::new();
        // Quitting an interactive session with a non-zero status is normal.
        fake.cmd(&["agent", "--with", "cursor"])
            .env("FAKE_EXIT", "1")
            .assert()
            .success()
            .stdout(predicate::str::contains("Agent session ended."));
    }
}
