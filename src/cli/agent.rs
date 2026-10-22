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

    /// Coding-agent harness to drive: claude, codex, opencode, cursor, or
    /// gemini. Defaults to `SEITE_AGENT` when set, otherwise claude.
    #[arg(long, value_name = "HARNESS")]
    pub with: Option<String>,
}

/// Coding-agent harnesses `seite agent --with` can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Harness {
    Claude,
    Codex,
    Opencode,
    Cursor,
    Gemini,
}

/// Every harness name `--with`/`SEITE_AGENT` accepts.
const VALID_HARNESSES: [&str; 5] = ["claude", "codex", "opencode", "cursor", "gemini"];

impl Harness {
    fn parse(name: &str) -> Option<Harness> {
        match name {
            "claude" => Some(Harness::Claude),
            "codex" => Some(Harness::Codex),
            "opencode" => Some(Harness::Opencode),
            "cursor" => Some(Harness::Cursor),
            "gemini" => Some(Harness::Gemini),
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
        }
    }

    /// How the agent invokes the `seite` skill with `args` (`/seite …`, or
    /// `$seite …` in Codex).
    pub fn skill_prompt(self, args: &str) -> String {
        match self {
            Harness::Codex => format!("$seite {args}"),
            _ => format!("/seite {args}"),
        }
    }

    /// Args to run one prompt non-interactively and exit. `auto` is the
    /// global `--yes`: the opt-in for a harness's own auto-approve flag.
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
        };
        if auto {
            match self {
                Harness::Opencode => args.push("--auto".into()),
                Harness::Cursor => {
                    args.push("--force".into());
                    args.push("--approve-mcps".into());
                }
                Harness::Gemini => args.push("--approval-mode=yolo".into()),
                Harness::Claude | Harness::Codex => {}
            }
        }
        args
    }

    /// Args to start an interactive session, optionally seeded with an
    /// initial prompt.
    fn interactive_args(self, prompt: Option<&str>) -> Vec<String> {
        let Some(p) = prompt else {
            return Vec::new();
        };
        match self {
            // `claude "<prompt>"` starts an interactive session with it.
            Harness::Claude | Harness::Codex | Harness::Cursor => vec![p.into()],
            Harness::Opencode => vec!["--prompt".into(), p.into()],
            // `-i` runs the prompt, then stays interactive.
            Harness::Gemini => vec!["-i".into(), p.into()],
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
    let harness = resolve_harness(args.with.as_deref())?;
    ensure_installed(harness)?;
    // Only run inside a site, so the agent finds AGENTS.md and the skill.
    crate::config::SiteConfig::load(std::path::Path::new("seite.toml"))?;

    match (&args.prompt, args.once) {
        (Some(prompt), true) => run_once(harness, prompt),
        (prompt, _) => {
            human::info(&format!(
                "Starting interactive {} agent session...",
                harness.name()
            ));
            run_interactive(harness, &harness.interactive_args(prompt.as_deref()))
        }
    }
}

/// Run `prompt` once with `harness` and wait for it to finish.
pub fn run_once(harness: Harness, prompt: &str) -> anyhow::Result<()> {
    human::info(&format!("Starting {} agent...", harness.name()));
    let cmd_args = harness.one_shot_args(prompt, crate::cli::prompt::assume_yes());
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
