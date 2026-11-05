//! `seite agent`: a thin launcher for a coding agent in the current site.
//!
//! The agent gets its context from the project itself (AGENTS.md, the
//! `seite` skill, `seite context`), so the prompt is passed through as-is and
//! the agent runs with the project's own permission settings.

use clap::Args;

use crate::error::PageError;
use crate::output::human;
use crate::platform::npm_cmd;

#[derive(Args)]
pub struct AgentArgs {
    /// Prompt for the agent (omit for an interactive session)
    pub prompt: Option<String>,

    /// Run a single prompt and exit (no follow-up conversation)
    #[arg(long)]
    pub once: bool,

    /// Coding-agent harness to drive: claude, codex, opencode, cursor,
    /// gemini, copilot, or antigravity. Defaults to `SEITE_AGENT` when set, otherwise
    /// claude.
    #[arg(long, value_name = "HARNESS")]
    pub with: Option<String>,

    /// With --once: pass the harness's own auto-approve flag (opencode
    /// `--auto`, cursor `--force --approve-mcps`, gemini
    /// `--approval-mode=yolo`, copilot `--allow-all-tools`, antigravity
    /// `--dangerously-skip-permissions`). Honored only from an interactive terminal,
    /// never inside another agent's session; the global `--yes` never
    /// implies it.
    #[arg(long)]
    pub auto_approve: bool,
}

/// Environment variables that mark a process as running inside a coding
/// agent's session (set for the shell commands the agent runs, or its
/// hooks), as `(variable, agent)`. Checked in order; any non-empty value
/// counts.
const AGENT_SESSION_MARKERS: &[(&str, &str)] = &[
    // Claude Code sets CLAUDECODE=1 for every Bash tool command (verified
    // empirically in Claude Code's shell). CLAUDE_PROJECT_DIR is set for
    // hooks and MCP servers (https://code.claude.com/docs/en/hooks).
    ("CLAUDECODE", "Claude Code"),
    // Codex CLI injects CODEX_THREAD_ID into every shell tool command
    // (openai/codex codex-rs/core/src/exec_env.rs); CODEX_SANDBOX is set
    // for sandboxed commands on macOS and CODEX_SANDBOX_NETWORK_DISABLED
    // when the sandbox has network off (codex-rs/core/src/spawn.rs).
    ("CODEX_THREAD_ID", "Codex"),
    ("CODEX_SANDBOX", "Codex"),
    ("CODEX_SANDBOX_NETWORK_DISABLED", "Codex"),
    // Gemini CLI sets GEMINI_CLI=1 for shell tool commands
    // (google-gemini/gemini-cli packages/core/src/services/
    // shellExecutionService.ts); GEMINI_PROJECT_DIR is set for hooks.
    ("GEMINI_CLI", "Gemini CLI"),
    ("GEMINI_PROJECT_DIR", "Gemini CLI"),
    // OpenCode sets OPENCODE=1 in its own process env at startup, which its
    // shell commands inherit (seen in the 1.18 binary's strings; not in
    // published docs — unverified against source).
    ("OPENCODE", "OpenCode"),
    // Cursor's agent CLI references CURSOR_AGENT (2026.10 binary strings;
    // exact value unverified); CURSOR_PROJECT_DIR is set for its hooks
    // (https://cursor.com/docs/agent/hooks).
    ("CURSOR_AGENT", "Cursor"),
    ("CURSOR_PROJECT_DIR", "Cursor"),
    // Antigravity CLI (agy 1.4.10, verified): ANTIGRAVITY_AGENT=1 for shell
    // tool commands, ANTIGRAVITY_CONVERSATION_ID for those and for hooks.
    // Not in its docs (https://antigravity.google/docs/settings).
    ("ANTIGRAVITY_AGENT", "Antigravity CLI"),
    ("ANTIGRAVITY_CONVERSATION_ID", "Antigravity CLI"),
    // Last: Gemini CLI also exports this alias to its hooks.
    ("CLAUDE_PROJECT_DIR", "Claude Code"),
    // GitHub Copilot CLI: no documented variable marks its shell commands
    // (https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference
    // lists only user-set ones such as COPILOT_ALLOW_ALL), so a nested
    // session inside Copilot isn't detected.
];

/// The coding agent whose session this process runs in, if any.
pub fn agent_session() -> Option<&'static str> {
    agent_session_with(|var| std::env::var_os(var).filter(|v| !v.is_empty()).is_some())
}

/// Whether this process runs inside a coding agent's session.
pub fn in_agent_session() -> bool {
    agent_session().is_some()
}

fn agent_session_with(is_set: impl Fn(&str) -> bool) -> Option<&'static str> {
    AGENT_SESSION_MARKERS
        .iter()
        .find(|(var, _)| is_set(var))
        .map(|(_, agent)| *agent)
}

/// Refuse to launch a nested coding agent from inside an agent's session:
/// the child would run with seite's (or the harness's) approvals instead of
/// the outer session's, and `what` can be done directly there.
pub fn refuse_nested(what: &str) -> anyhow::Result<()> {
    refuse_nested_in(agent_session(), what)
}

fn refuse_nested_in(session: Option<&str>, what: &str) -> anyhow::Result<()> {
    match session {
        Some(agent) => Err(PageError::Agent(format!(
            "already inside {agent}; seite won't start a nested agent session. {what}"
        ))
        .into()),
        None => Ok(()),
    }
}

/// Whether `--auto-approve` takes effect: asked for, from an interactive
/// terminal, and not inside another agent's session.
fn auto_approve_effective(requested: bool, stdin_is_tty: bool, session: Option<&str>) -> bool {
    requested && stdin_is_tty && session.is_none()
}

/// Coding-agent harnesses `seite agent --with` can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Harness {
    Claude,
    Codex,
    Opencode,
    Cursor,
    Gemini,
    Copilot,
    Antigravity,
}

/// Every harness name `--with`/`SEITE_AGENT` accepts.
const VALID_HARNESSES: [&str; 7] = [
    "claude",
    "codex",
    "opencode",
    "cursor",
    "gemini",
    "copilot",
    "antigravity",
];

impl Harness {
    fn parse(name: &str) -> Option<Harness> {
        match name {
            "claude" => Some(Harness::Claude),
            "codex" => Some(Harness::Codex),
            "opencode" => Some(Harness::Opencode),
            "cursor" => Some(Harness::Cursor),
            "gemini" => Some(Harness::Gemini),
            "copilot" => Some(Harness::Copilot),
            "antigravity" => Some(Harness::Antigravity),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::Codex => "codex",
            Harness::Opencode => "opencode",
            Harness::Cursor => "cursor",
            Harness::Gemini => "gemini",
            Harness::Copilot => "copilot",
            Harness::Antigravity => "antigravity",
        }
    }

    /// The executable to spawn.
    fn binary(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::Codex => "codex",
            Harness::Opencode => "opencode",
            Harness::Cursor => "cursor-agent",
            Harness::Gemini => "gemini",
            Harness::Copilot => "copilot",
            Harness::Antigravity => "agy",
        }
    }

    /// Install instructions shown when [`binary`](Harness::binary) isn't on `PATH`.
    fn install_hint(self) -> &'static str {
        match self {
            Harness::Claude => {
                "npm install -g @anthropic-ai/claude-code (https://docs.claude.com/en/docs/claude-code)"
            }
            Harness::Codex => "npm install -g @openai/codex (https://github.com/openai/codex)",
            Harness::Opencode => {
                "curl -fsSL https://opencode.ai/install | bash (https://opencode.ai/docs)"
            }
            Harness::Cursor => {
                "curl https://cursor.com/install -fsS | bash (https://docs.cursor.com/cli)"
            }
            Harness::Gemini => {
                "npm install -g @google/gemini-cli (https://github.com/google-gemini/gemini-cli)"
            }
            Harness::Copilot => {
                "npm install -g @github/copilot (https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/install-copilot-cli)"
            }
            Harness::Antigravity => {
                "curl -fsSL https://antigravity.google/cli/install.sh | bash (https://antigravity.google/docs/getting-started?tab=cli)"
            }
        }
    }

    /// How the agent invokes the `seite` skill with `args` (`/seite …`, or
    /// `$seite …` in Codex).
    pub fn skill_prompt(self, args: &str) -> String {
        match self {
            Harness::Codex => format!("$seite {args}"),
            // Copilot runs a skill the prompt names ("Use the /frontend-design
            // skill to ..."); passing arguments is undocumented.
            // https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-skills
            Harness::Copilot => format!("Use the /seite skill: {args}"),
            _ => format!("/seite {args}"),
        }
    }

    /// Args to run one prompt non-interactively and exit. `auto` adds the
    /// harness's own auto-approve flag (`seite agent --auto-approve` only).
    fn one_shot_args(self, prompt: &str, auto: bool) -> Vec<String> {
        let mut args: Vec<String> = match self {
            // https://code.claude.com/docs/en/cli-reference (`-p`, print mode)
            Harness::Claude => vec!["-p".into(), prompt.into()],
            Harness::Codex => vec![
                "exec".into(),
                "--sandbox".into(),
                "workspace-write".into(),
                prompt.into(),
            ],
            Harness::Opencode => vec!["run".into(), prompt.into()],
            Harness::Cursor => vec![
                "-p".into(),
                prompt.into(),
                "--output-format".into(),
                "text".into(),
            ],
            // https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/cli-reference.md
            Harness::Gemini => vec!["-p".into(), prompt.into()],
            // `-p PROMPT`: run programmatically, exit when done.
            // https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference
            Harness::Copilot => vec!["-p".into(), prompt.into()],
            // `agy -p PROMPT`: print mode, one turn. Shell commands no user
            // allow rule covers are auto-denied there.
            // https://antigravity.google/docs/settings (`agy --help`)
            Harness::Antigravity => vec!["-p".into(), prompt.into()],
        };
        if auto {
            match self {
                Harness::Opencode => args.push("--auto".into()),
                Harness::Cursor => {
                    args.push("--force".into());
                    args.push("--approve-mcps".into());
                }
                Harness::Gemini => args.push("--approval-mode=yolo".into()),
                Harness::Copilot => args.push("--allow-all-tools".into()),
                Harness::Antigravity => args.push("--dangerously-skip-permissions".into()),
                Harness::Claude | Harness::Codex => {}
            }
        }
        args
    }

    /// Args to start an interactive session, optionally seeded with an
    /// initial prompt.
    pub(crate) fn interactive_args(self, prompt: Option<&str>) -> Vec<String> {
        let Some(p) = prompt else {
            return Vec::new();
        };
        match self {
            // `claude "<prompt>"` starts an interactive session with it.
            Harness::Claude | Harness::Codex | Harness::Cursor => vec![p.into()],
            Harness::Opencode => vec!["--prompt".into(), p.into()],
            // `-i` runs the prompt, then stays interactive (Gemini CLI,
            // Copilot CLI and Antigravity CLI alike).
            Harness::Gemini | Harness::Copilot | Harness::Antigravity => {
                vec!["-i".into(), p.into()]
            }
        }
    }
}

/// Resolve which harness to drive: `cli_value`, else `SEITE_AGENT`, else claude.
pub fn resolve_harness(cli_value: Option<&str>) -> anyhow::Result<Harness> {
    resolve_harness_with_env(cli_value, std::env::var("SEITE_AGENT").ok())
}

fn resolve_harness_with_env(
    cli_value: Option<&str>,
    env_value: Option<String>,
) -> anyhow::Result<Harness> {
    let name = cli_value
        .map(str::to_string)
        .or(env_value)
        .unwrap_or_else(|| "claude".to_string());

    Harness::parse(&name).ok_or_else(|| {
        let hint = human::suggest_match(&name, &VALID_HARNESSES);
        anyhow::anyhow!(
            "unknown agent harness '{}'. Valid options: {}{}",
            name,
            VALID_HARNESSES.join(", "),
            hint
        )
    })
}

pub fn run(args: &AgentArgs) -> anyhow::Result<()> {
    refuse_nested("Ask for the change directly in this session (the `/seite` skill covers seite's workflows).")?;
    let harness = resolve_harness(args.with.as_deref())?;
    ensure_installed(harness)?;
    // Only run inside a site, so the agent finds AGENTS.md and the skill.
    crate::config::SiteConfig::load(std::path::Path::new("seite.toml"))?;

    use std::io::IsTerminal;
    let auto = auto_approve_effective(
        args.auto_approve,
        std::io::stdin().is_terminal(),
        agent_session(),
    );
    if args.auto_approve && !auto {
        human::warning("--auto-approve ignored: it needs an interactive terminal");
    } else if auto && !args.once {
        human::warning("--auto-approve only applies with --once");
    }

    match (&args.prompt, args.once) {
        (Some(prompt), true) => run_once(harness, prompt, auto),
        (prompt, _) => {
            human::info(&format!(
                "Starting interactive {} agent session...",
                harness.name()
            ));
            run_interactive(harness, &harness.interactive_args(prompt.as_deref()))
        }
    }
}

/// Run `prompt` once with `harness` and wait for it to finish. `auto` adds
/// the harness's auto-approve flag (only `seite agent --auto-approve`).
pub fn run_once(harness: Harness, prompt: &str, auto: bool) -> anyhow::Result<()> {
    human::info(&format!("Starting {} agent...", harness.name()));
    let cmd_args = harness.one_shot_args(prompt, auto);
    let status = npm_cmd(harness.binary())
        .args(&cmd_args)
        .stdout(crate::output::child_stdout())
        .status()
        .map_err(|e| PageError::Agent(format!("failed to run {}: {e}", harness.binary())))?;

    if !status.success() {
        return Err(
            PageError::Agent(format!("{} exited with non-zero status", harness.binary())).into(),
        );
    }
    human::success("Agent finished. Preview with `seite serve` or check with `seite check`.");
    Ok(())
}

/// Start `harness` interactively, seeded with `prompt`. Never adds
/// auto-approve flags.
pub fn launch_interactive(harness: Harness, prompt: &str) -> anyhow::Result<()> {
    human::info(&format!(
        "Starting interactive {} agent session...",
        harness.name()
    ));
    run_interactive(harness, &harness.interactive_args(Some(prompt)))
}

/// Whether `harness`'s binary is on `PATH`, and how to install it if not.
pub fn install_hint_if_missing(harness: Harness) -> Option<String> {
    ensure_installed(harness).err().map(|e| e.to_string())
}

/// Hand the terminal to a harness's own interactive session.
fn run_interactive(harness: Harness, cmd_args: &[String]) -> anyhow::Result<()> {
    let status = npm_cmd(harness.binary())
        .args(cmd_args)
        .stdout(crate::output::child_stdout())
        .status()
        .map_err(|e| PageError::Agent(format!("failed to run {}: {e}", harness.binary())))?;

    if !status.success() {
        human::info("Agent session ended.");
    }
    Ok(())
}

/// Verify the chosen harness's binary is on `PATH` before doing any other
/// work, so the error names the harness and how to install it.
pub fn ensure_installed(harness: Harness) -> anyhow::Result<()> {
    match npm_cmd(harness.binary()).arg("--version").output() {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(PageError::Agent(format!(
            "{} is not installed or not on PATH. Install it with: {}",
            harness.binary(),
            harness.install_hint()
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- nested sessions + auto-approve ---

    #[test]
    fn test_agent_session_detection() {
        assert_eq!(agent_session_with(|_| false), None);
        assert_eq!(
            agent_session_with(|v| v == "CLAUDECODE"),
            Some("Claude Code")
        );
        assert_eq!(
            agent_session_with(|v| v == "CODEX_THREAD_ID"),
            Some("Codex")
        );
        assert_eq!(
            agent_session_with(|v| v == "GEMINI_CLI"),
            Some("Gemini CLI")
        );
        assert_eq!(agent_session_with(|v| v == "OPENCODE"), Some("OpenCode"));
        assert_eq!(agent_session_with(|v| v == "CURSOR_AGENT"), Some("Cursor"));
        // Gemini's hooks get a CLAUDE_PROJECT_DIR alias: Gemini wins.
        assert_eq!(
            agent_session_with(|v| v == "GEMINI_PROJECT_DIR" || v == "CLAUDE_PROJECT_DIR"),
            Some("Gemini CLI")
        );
    }

    #[test]
    fn test_refuse_nested_names_the_agent() {
        assert!(refuse_nested_in(None, "x").is_ok());
        let err = refuse_nested_in(Some("Codex"), "Run it directly.").unwrap_err();
        assert!(err.to_string().contains("already inside Codex"), "{err}");
        assert!(err.to_string().contains("Run it directly."), "{err}");
    }

    #[test]
    fn test_auto_approve_needs_tty_and_no_outer_session() {
        assert!(auto_approve_effective(true, true, None));
        assert!(!auto_approve_effective(false, true, None));
        assert!(!auto_approve_effective(true, false, None));
        assert!(!auto_approve_effective(true, true, Some("Claude Code")));
    }

    // --- harness resolution ---

    #[test]
    fn test_harness_parse_all_known_names() {
        for name in VALID_HARNESSES {
            assert_eq!(Harness::parse(name).unwrap().name(), name);
        }
        assert_eq!(Harness::parse("bogus"), None);
    }

    #[test]
    fn test_harness_binaries() {
        assert_eq!(Harness::Claude.binary(), "claude");
        assert_eq!(Harness::Codex.binary(), "codex");
        assert_eq!(Harness::Opencode.binary(), "opencode");
        assert_eq!(Harness::Cursor.binary(), "cursor-agent");
        assert_eq!(Harness::Gemini.binary(), "gemini");
        assert!(Harness::Gemini
            .install_hint()
            .contains("@google/gemini-cli"));
        assert_eq!(Harness::Copilot.binary(), "copilot");
        assert!(Harness::Copilot.install_hint().contains("@github/copilot"));
        assert_eq!(Harness::Antigravity.binary(), "agy");
        assert!(Harness::Antigravity
            .install_hint()
            .contains("antigravity.google/cli/install.sh"));
    }

    #[test]
    fn test_antigravity_harness_args() {
        assert_eq!(
            Harness::Antigravity.one_shot_args("hi", false),
            ["-p", "hi"]
        );
        assert_eq!(
            Harness::Antigravity.one_shot_args("hi", true),
            ["-p", "hi", "--dangerously-skip-permissions"]
        );
        assert_eq!(
            Harness::Antigravity.interactive_args(Some("hi")),
            ["-i", "hi"]
        );
        // Skills are slash commands in agy.
        assert_eq!(Harness::Antigravity.skill_prompt("check"), "/seite check");
        assert_eq!(
            agent_session_with(|v| v == "ANTIGRAVITY_AGENT"),
            Some("Antigravity CLI")
        );
        assert_eq!(
            agent_session_with(|v| v == "ANTIGRAVITY_CONVERSATION_ID"),
            Some("Antigravity CLI")
        );
    }

    #[test]
    fn test_copilot_harness_args() {
        assert_eq!(Harness::Copilot.one_shot_args("hi", false), ["-p", "hi"]);
        assert_eq!(
            Harness::Copilot.one_shot_args("hi", true),
            ["-p", "hi", "--allow-all-tools"]
        );
        assert_eq!(Harness::Copilot.interactive_args(Some("hi")), ["-i", "hi"]);
        assert_eq!(
            Harness::Copilot.skill_prompt("check"),
            "Use the /seite skill: check"
        );
    }

    #[test]
    fn test_resolve_harness_prefers_cli_flag_over_env() {
        let harness =
            resolve_harness_with_env(Some("codex"), Some("opencode".to_string())).unwrap();
        assert_eq!(harness, Harness::Codex);
    }

    #[test]
    fn test_resolve_harness_falls_back_to_env() {
        let harness = resolve_harness_with_env(None, Some("gemini".to_string())).unwrap();
        assert_eq!(harness, Harness::Gemini);
    }

    #[test]
    fn test_resolve_harness_defaults_to_claude() {
        let harness = resolve_harness_with_env(None, None).unwrap();
        assert_eq!(harness, Harness::Claude);
    }

    #[test]
    fn test_resolve_harness_unknown_suggests_close_match() {
        let err = resolve_harness_with_env(Some("claud"), None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown agent harness"));
        assert!(err.contains("did you mean 'claude'"));
        assert!(err.contains("gemini"));
    }

    // --- argv per harness: prompts pass through unchanged ---

    #[test]
    fn test_one_shot_args_per_harness() {
        let cases: [(Harness, &[&str]); 5] = [
            (Harness::Claude, &["-p", "do it"]),
            (
                Harness::Codex,
                &["exec", "--sandbox", "workspace-write", "do it"],
            ),
            (Harness::Opencode, &["run", "do it"]),
            (Harness::Cursor, &["-p", "do it", "--output-format", "text"]),
            (Harness::Gemini, &["-p", "do it"]),
        ];
        for (harness, expected) in cases {
            assert_eq!(
                harness.one_shot_args("do it", false),
                expected,
                "{harness:?}"
            );
        }
    }

    #[test]
    fn test_one_shot_args_yes_adds_auto_approve_flags() {
        assert_eq!(
            Harness::Opencode.one_shot_args("p", true),
            ["run", "p", "--auto"]
        );
        let cursor = Harness::Cursor.one_shot_args("p", true);
        assert!(cursor.iter().any(|a| a == "--force"));
        assert!(cursor.iter().any(|a| a == "--approve-mcps"));
        assert_eq!(
            Harness::Gemini.one_shot_args("p", true),
            ["-p", "p", "--approval-mode=yolo"]
        );
        // Claude and Codex rely on the project's own permission settings.
        assert_eq!(Harness::Claude.one_shot_args("p", true), ["-p", "p"]);
        assert_eq!(Harness::Codex.one_shot_args("p", true).len(), 4);
    }

    #[test]
    fn test_no_injected_context_or_tool_flags() {
        for name in VALID_HARNESSES {
            let harness = Harness::parse(name).unwrap();
            let all: Vec<String> = harness
                .one_shot_args("x", true)
                .into_iter()
                .chain(harness.interactive_args(Some("x")))
                .collect();
            for flag in ["--append-system-prompt", "--allowedTools", "--mcp-config"] {
                assert!(!all.iter().any(|a| a == flag), "{name}: {flag}");
            }
            assert!(!all.iter().any(|a| a.contains("seite-project-context")));
        }
    }

    #[test]
    fn test_interactive_args_per_harness() {
        for name in VALID_HARNESSES {
            assert!(Harness::parse(name)
                .unwrap()
                .interactive_args(None)
                .is_empty());
        }
        assert_eq!(Harness::Claude.interactive_args(Some("hi")), ["hi"]);
        assert_eq!(Harness::Codex.interactive_args(Some("hi")), ["hi"]);
        assert_eq!(Harness::Cursor.interactive_args(Some("hi")), ["hi"]);
        assert_eq!(
            Harness::Opencode.interactive_args(Some("hi")),
            ["--prompt", "hi"]
        );
        assert_eq!(Harness::Gemini.interactive_args(Some("hi")), ["-i", "hi"]);
    }

    #[test]
    fn test_skill_prompt_uses_dollar_for_codex() {
        assert_eq!(Harness::Claude.skill_prompt("theme x"), "/seite theme x");
        assert_eq!(Harness::Codex.skill_prompt("theme x"), "$seite theme x");
        assert_eq!(Harness::Gemini.skill_prompt("check"), "/seite check");
    }
}
