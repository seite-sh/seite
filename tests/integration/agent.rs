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

    const ALLOWED_TOOLS: &str = "Read,Write,Edit,Glob,Grep,mcp__seite,\
        Bash(seite:*),Bash(git status:*),Bash(git diff:*),Bash(git log:*),Bash(ls:*)";

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
            for name in ["claude", "codex", "opencode", "cursor-agent"] {
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

    /// The site context prepended to prompts for harnesses without a
    /// system-prompt flag.
    fn assert_context_then_prompt(arg: &str, prompt: &str) {
        assert!(arg.starts_with("<seite-project-context>\n"), "{arg}");
        assert!(arg.contains("- Title: Fake Harness Site"), "{arg}");
        assert!(
            arg.ends_with(&format!("\n</seite-project-context>\n\n{prompt}")),
            "{arg}"
        );
    }

    #[test]
    fn test_agent_claude_once_argv_is_restricted_and_uses_mcp_config() {
        let fake = Fake::new();
        assert!(fake.site().join(".mcp.json").is_file());
        fake.cmd(&["agent", "--once", "write a post"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Agent finished"));
        let argv = fake.only_call();
        assert_eq!(argv.len(), 8, "{argv:?}");
        assert_eq!(argv[..2], ["-p", "write a post"]);
        assert_eq!(argv[2], "--append-system-prompt");
        assert!(
            argv[3].contains("- Title: Fake Harness Site"),
            "{}",
            argv[3]
        );
        assert_eq!(
            argv[4..],
            ["--allowedTools", ALLOWED_TOOLS, "--mcp-config", ".mcp.json"]
        );
    }

    #[test]
    fn test_agent_claude_without_mcp_json_omits_mcp_config() {
        let fake = Fake::new();
        fs::remove_file(fake.site().join(".mcp.json")).unwrap();
        fake.cmd(&["agent", "--with", "claude", "--once", "hi"])
            .assert()
            .success();
        let argv = fake.only_call();
        assert!(!argv.iter().any(|a| a == "--mcp-config"), "{argv:?}");
        assert_eq!(argv.last().unwrap(), ALLOWED_TOOLS);
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
        let argv = fake.only_call();
        assert_eq!(argv[0], "--append-system-prompt");
        assert_eq!(
            argv[2..],
            ["--allowedTools", ALLOWED_TOOLS, "--mcp-config", ".mcp.json"]
        );
    }

    #[test]
    fn test_agent_claude_prompt_streams_and_resumes_the_session() {
        let fake = Fake::new();
        let events = [
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Drafted the post."}]}}"#,
            r#"{"type":"result","session_id":"sess-42"}"#,
        ]
        .join("\n");
        fake.cmd(&["agent", "draft a post"])
            .env("FAKE_STDOUT", events)
            .write_stdin("now add tags\ndone\n")
            .assert()
            .success()
            .stdout(predicate::str::contains("Drafted the post."));
        let calls = fake.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        let first = &calls[0];
        assert_eq!(first[..2], ["-p", "draft a post"]);
        assert!(first
            .windows(2)
            .any(|w| w == ["--output-format", "stream-json"]));
        assert!(first.iter().any(|a| a == "--append-system-prompt"));
        assert!(!first.iter().any(|a| a == "--resume"));
        // The follow-up continues the same session instead of starting over.
        let second = &calls[1];
        assert_eq!(second[..2], ["-p", "now add tags"]);
        assert!(
            second.windows(2).any(|w| w == ["--resume", "sess-42"]),
            "{second:?}"
        );
        assert!(second
            .windows(2)
            .any(|w| w == ["--allowedTools", ALLOWED_TOOLS]));
        assert!(!second.iter().any(|a| a == "--append-system-prompt"));
    }

    #[test]
    fn test_agent_codex_once_uses_workspace_write_sandbox_and_context() {
        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "codex", "--once", "fix links"])
            .assert()
            .success();
        let argv = fake.only_call();
        assert_eq!(argv.len(), 4, "{argv:?}");
        assert_eq!(argv[..3], ["exec", "--sandbox", "workspace-write"]);
        assert_context_then_prompt(&argv[3], "fix links");
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
    fn test_agent_cursor_once_auto_approve_only_with_yes() {
        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "cursor", "--once", "tidy"])
            .assert()
            .success();
        let argv = fake.only_call();
        assert_eq!(argv.len(), 4, "{argv:?}");
        assert_eq!(argv[0], "-p");
        assert_context_then_prompt(&argv[1], "tidy");
        assert_eq!(argv[2..], ["--output-format", "text"]);

        let fake = Fake::new();
        fake.cmd(&["--yes", "agent", "--with", "cursor", "--once", "tidy"])
            .assert()
            .success();
        let argv = fake.only_call();
        assert_eq!(
            argv[2..],
            ["--output-format", "text", "--force", "--approve-mcps"]
        );
    }

    #[test]
    fn test_agent_opencode_argv_once_and_interactive() {
        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "opencode", "--once", "go"])
            .assert()
            .success();
        let argv = fake.only_call();
        assert_eq!(argv.len(), 2, "{argv:?}");
        assert_eq!(argv[0], "run");
        assert_context_then_prompt(&argv[1], "go");

        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "opencode", "go"])
            .assert()
            .success();
        let argv = fake.only_call();
        assert_eq!(argv[0], "--prompt");
        assert_context_then_prompt(&argv[1], "go");

        // No prompt: a bare interactive session.
        let fake = Fake::new();
        fake.cmd(&["agent", "--with", "opencode"])
            .assert()
            .success();
        assert_eq!(fake.only_call(), Vec::<String>::new());
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
