use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Stdio;

use clap::Args;
use walkdir::WalkDir;

use crate::config::{ResolvedPaths, SiteConfig};
use crate::content;
use crate::error::PageError;
use crate::output::human;
use crate::platform::npm_cmd;

#[derive(Args)]
pub struct AgentArgs {
    /// Prompt for the agent (omit for interactive chat)
    pub prompt: Option<String>,

    /// Run a single prompt and exit (no follow-up conversation)
    #[arg(long)]
    pub once: bool,

    /// Coding-agent harness to drive: claude, codex, opencode, or cursor.
    /// Defaults to `SEITE_AGENT` when set, otherwise claude.
    #[arg(long, value_name = "HARNESS")]
    pub with: Option<String>,
}

/// Tools the agent may use without prompting, scoped to content/theme work.
/// `mcp__seite` allows every tool of the site's `seite` MCP server. `Bash` is
/// limited to the `seite` CLI itself and a handful of read-only git/ls
/// commands — no general shell access.
const AGENT_ALLOWED_TOOLS: &str = "Read,Write,Edit,Glob,Grep,mcp__seite,\
    Bash(seite:*),Bash(git status:*),Bash(git diff:*),Bash(git log:*),Bash(ls:*)";

/// Coding-agent harnesses `seite agent --with` can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Harness {
    Claude,
    Codex,
    Opencode,
    Cursor,
}

/// Every harness name `--with`/`SEITE_AGENT` accepts.
const VALID_HARNESSES: [&str; 4] = ["claude", "codex", "opencode", "cursor"];

/// Extra context each [`Harness`]'s argument builders need, gathered once in
/// [`run`] and threaded through so the builders stay pure and testable.
struct HarnessOpts<'a> {
    /// `--allowedTools` value. Only meaningful to [`Harness::Claude`].
    allowed_tools: &'a str,
    /// `--mcp-config .mcp.json`, when present. Only meaningful to
    /// [`Harness::Claude`].
    mcp_args: &'a [String],
    /// The global `--yes` flag: the opt-in for a harness's own auto-approve
    /// flag (`opencode run --auto`, `cursor-agent --force --approve-mcps`).
    auto: bool,
}

impl Harness {
    fn parse(name: &str) -> Option<Harness> {
        match name {
            "claude" => Some(Harness::Claude),
            "codex" => Some(Harness::Codex),
            "opencode" => Some(Harness::Opencode),
            "cursor" => Some(Harness::Cursor),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::Codex => "codex",
            Harness::Opencode => "opencode",
            Harness::Cursor => "cursor",
        }
    }

    /// The executable to spawn.
    fn binary(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::Codex => "codex",
            Harness::Opencode => "opencode",
            Harness::Cursor => "cursor-agent",
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
        }
    }

    /// Args to run one prompt non-interactively and exit.
    fn one_shot_args(self, prompt: &str, context: &str, opts: &HarnessOpts<'_>) -> Vec<String> {
        match self {
            Harness::Claude => {
                let mut args = vec![
                    "-p".to_string(),
                    prompt.to_string(),
                    "--append-system-prompt".to_string(),
                    context.to_string(),
                    "--allowedTools".to_string(),
                    opts.allowed_tools.to_string(),
                ];
                args.extend(opts.mcp_args.iter().cloned());
                args
            }
            Harness::Codex => vec![
                "exec".to_string(),
                "--sandbox".to_string(),
                "workspace-write".to_string(),
                with_context(context, prompt),
            ],
            Harness::Opencode => {
                let mut args = vec!["run".to_string(), with_context(context, prompt)];
                if opts.auto {
                    args.push("--auto".to_string());
                }
                args
            }
            Harness::Cursor => {
                let mut args = vec![
                    "-p".to_string(),
                    with_context(context, prompt),
                    "--output-format".to_string(),
                    "text".to_string(),
                ];
                if opts.auto {
                    args.push("--force".to_string());
                    args.push("--approve-mcps".to_string());
                }
                args
            }
        }
    }

    /// Args to start an interactive session, optionally seeded with an
    /// initial prompt. `Claude`'s own initial-prompt flow is handled
    /// separately (streaming chat with session resume), so it ignores
    /// `prompt` here and only ever runs with `None`.
    fn interactive_args(
        self,
        prompt: Option<&str>,
        context: &str,
        opts: &HarnessOpts<'_>,
    ) -> Vec<String> {
        match self {
            Harness::Claude => {
                let mut args = vec![
                    "--append-system-prompt".to_string(),
                    context.to_string(),
                    "--allowedTools".to_string(),
                    opts.allowed_tools.to_string(),
                ];
                args.extend(opts.mcp_args.iter().cloned());
                args
            }
            Harness::Codex => match prompt {
                Some(p) => vec![with_context(context, p)],
                None => Vec::new(),
            },
            Harness::Opencode => match prompt {
                Some(p) => vec!["--prompt".to_string(), with_context(context, p)],
                None => Vec::new(),
            },
            Harness::Cursor => match prompt {
                Some(p) => vec![with_context(context, p)],
                None => Vec::new(),
            },
        }
    }
}

/// Prepend the dynamic site context to a prompt for harnesses with no
/// system-prompt flag, clearly delimited so it reads as background, not an
/// instruction from the user.
fn with_context(context: &str, prompt: &str) -> String {
    format!("<seite-project-context>\n{context}\n</seite-project-context>\n\n{prompt}")
}

/// Resolve which harness to drive: `--with`, else `SEITE_AGENT`, else claude.
fn resolve_harness(cli_value: Option<&str>) -> anyhow::Result<Harness> {
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

/// `--mcp-config .mcp.json` when the project declares MCP servers there, so
/// the agent gets the seite MCP server without an interactive approval.
fn mcp_config_args() -> Vec<String> {
    mcp_config_args_for(std::path::Path::new(".mcp.json"))
}

fn mcp_config_args_for(mcp_json: &std::path::Path) -> Vec<String> {
    if mcp_json.is_file() {
        vec!["--mcp-config".to_string(), ".mcp.json".to_string()]
    } else {
        Vec::new()
    }
}

pub fn run(args: &AgentArgs) -> anyhow::Result<()> {
    let harness = resolve_harness(args.with.as_deref())?;
    ensure_installed(harness)?;

    let config = SiteConfig::load(&PathBuf::from("seite.toml"))?;
    let paths = config.resolve_paths(&std::env::current_dir()?);
    let context = build_dynamic_context(&config, &paths);

    let mcp_args = mcp_config_args();
    let opts = HarnessOpts {
        allowed_tools: AGENT_ALLOWED_TOOLS,
        mcp_args: &mcp_args,
        auto: crate::cli::prompt::assume_yes(),
    };

    match (&args.prompt, args.once, harness) {
        // Claude's non-once prompt path keeps the richer streaming chat loop
        // with session resume, so follow-up messages stay in the same session.
        (Some(prompt), false, Harness::Claude) => {
            let session_id = run_streaming(prompt, None, &context, opts.allowed_tools)?;
            if let Some(sid) = session_id {
                chat_loop(&sid, opts.allowed_tools)?;
            }
            Ok(())
        }
        (Some(prompt), true, _) => {
            human::info(&format!("Starting {} agent...", harness.name()));
            let cmd_args = harness.one_shot_args(prompt, &context, &opts);
            run_to_completion(harness, &cmd_args)
        }
        (Some(prompt), false, _) => {
            human::info(&format!(
                "Starting interactive {} agent session...",
                harness.name()
            ));
            let cmd_args = harness.interactive_args(Some(prompt), &context, &opts);
            run_interactive(harness, &cmd_args)
        }
        (None, _, _) => {
            human::info(&format!(
                "Starting interactive {} agent session...",
                harness.name()
            ));
            human::info("The agent has full context about your site. Type your requests.");
            let cmd_args = harness.interactive_args(None, &context, &opts);
            run_interactive(harness, &cmd_args)
        }
    }
}

/// Run a harness to completion (one-shot mode) and report the outcome.
fn run_to_completion(harness: Harness, cmd_args: &[String]) -> anyhow::Result<()> {
    let status = npm_cmd(harness.binary())
        .args(cmd_args)
        .status()
        .map_err(|e| PageError::Agent(format!("failed to run {}: {e}", harness.binary())))?;

    if !status.success() {
        return Err(
            PageError::Agent(format!("{} exited with non-zero status", harness.binary())).into(),
        );
    }
    human::success("Agent finished. Preview with `seite serve` or generate with `seite build`.");
    Ok(())
}

/// Hand the terminal to a harness's own interactive session.
fn run_interactive(harness: Harness, cmd_args: &[String]) -> anyhow::Result<()> {
    let status = npm_cmd(harness.binary())
        .args(cmd_args)
        .status()
        .map_err(|e| PageError::Agent(format!("failed to run {}: {e}", harness.binary())))?;

    if !status.success() {
        human::info("Agent session ended.");
    }
    Ok(())
}

/// Run a prompt with streaming JSON output, displaying events in real-time.
/// Claude-only: the other harnesses have no equivalent streaming/session
/// protocol.
/// Returns the session ID for follow-up messages.
fn run_streaming(
    prompt: &str,
    session_id: Option<&str>,
    system_prompt: &str,
    allowed_tools: &str,
) -> anyhow::Result<Option<String>> {
    let mut cmd = npm_cmd("claude");
    cmd.args(["-p", prompt])
        .args(["--output-format", "stream-json"])
        .args(["--verbose"])
        .args(["--allowedTools", allowed_tools])
        .args(mcp_config_args())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    // First message gets the system prompt; follow-ups use --resume
    match session_id {
        Some(sid) => {
            cmd.args(["--resume", sid]);
        }
        None => {
            cmd.args(["--append-system-prompt", system_prompt]);
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| PageError::Agent(format!("failed to run claude: {e}")))?;

    let stdout = child.stdout.take().expect("stdout piped");
    let reader = io::BufReader::new(stdout);

    let mut result_session_id: Option<String> = None;
    let mut in_text = false;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.is_empty() {
            continue;
        }

        let json: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match event_type {
            "assistant" => {
                let content = json
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_array());

                if let Some(blocks) = content {
                    for block in blocks {
                        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match block_type {
                            "thinking" => {
                                if in_text {
                                    println!();
                                    in_text = false;
                                }
                                if let Some(thinking) =
                                    block.get("thinking").and_then(|t| t.as_str())
                                {
                                    if !thinking.is_empty() {
                                        let styled = console::style("thinking").dim().italic();
                                        let preview = truncate(thinking, 200);
                                        println!("  {styled}: {preview}");
                                    }
                                }
                            }
                            "tool_use" => {
                                if in_text {
                                    println!();
                                    in_text = false;
                                }
                                let name =
                                    block.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                                let input = block.get("input").cloned().unwrap_or_default();
                                let detail = summarize_tool_input(name, &input);
                                let styled = console::style(format!("tool: {name}")).cyan();
                                println!("  {styled} {detail}");
                            }
                            "text" => {
                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                    if !text.is_empty() {
                                        print!("{text}");
                                        let _ = io::stdout().flush();
                                        in_text = true;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "result" => {
                if in_text {
                    println!();
                    in_text = false;
                }
                if let Some(sid) = json.get("session_id").and_then(|s| s.as_str()) {
                    result_session_id = Some(sid.to_string());
                }
            }
            _ => {}
        }
    }

    if in_text {
        println!();
    }

    let _ = child.wait();

    Ok(result_session_id)
}

/// Interactive chat loop: prompt the user, send follow-ups to the same session.
fn chat_loop(session_id: &str, allowed_tools: &str) -> anyhow::Result<()> {
    println!();
    human::info("Chat session active. Type follow-up messages, or \"done\" to exit.");
    println!();

    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        print!("you> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(_) => break,
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line, "done" | "quit" | "exit" | "stop") {
            human::info("Agent session ended.");
            break;
        }

        println!();
        // Drop stdin lock so the child can use stdin if needed
        drop(reader);
        match run_streaming(line, Some(session_id), "", allowed_tools) {
            Ok(_) => {}
            Err(e) => {
                human::error(&format!("Agent error: {e}"));
                break;
            }
        }
        println!();
        // Re-acquire stdin lock
        reader = stdin.lock();
    }

    Ok(())
}

/// Verify the chosen harness's binary is on `PATH` before doing any other
/// work, so the error names the harness and how to install it.
fn ensure_installed(harness: Harness) -> anyhow::Result<()> {
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

/// Truncate a string to approximately max_len characters, adding "..." if truncated.
/// Uses char boundaries to avoid panicking on multi-byte UTF-8.
fn truncate(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() <= max_len {
        s
    } else {
        let end = s
            .char_indices()
            .nth(max_len)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..end])
    }
}

/// Produce a short summary of a tool invocation for display.
fn summarize_tool_input(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Read" => input
            .get("file_path")
            .and_then(|p| p.as_str())
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "Write" => input
            .get("file_path")
            .and_then(|p| p.as_str())
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "Edit" => input
            .get("file_path")
            .and_then(|p| p.as_str())
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "Glob" => input
            .get("pattern")
            .and_then(|p| p.as_str())
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "Grep" => input
            .get("pattern")
            .and_then(|p| p.as_str())
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "Bash" => input
            .get("command")
            .and_then(|c| c.as_str())
            .map(|c| truncate(c, 80))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Build the dynamic, per-site context passed to the coding agent.
///
/// This deliberately excludes anything the project's `AGENTS.md` already
/// covers (content format, file naming conventions, available `seite`
/// commands, shortcodes, …) — Claude Code and other AGENTS.md-aware harnesses
/// load that on their own, so repeating it here would just be wasted context
/// and a second copy that can drift. What's left is context the agent can't
/// get cheaply any other way: the live site config, collection layout,
/// existing content, and template list, all read fresh from `seite.toml` and
/// the content directory.
///
/// Public so the REPL in serve.rs can reuse it via `agent::run`.
pub fn build_dynamic_context(config: &SiteConfig, paths: &ResolvedPaths) -> String {
    let mut prompt = String::with_capacity(2048);

    prompt.push_str(&format!(
        "You are an AI assistant helping manage a static site built with the `seite` CLI tool.\n\n\
         Project conventions (content format, file naming, available commands, shortcodes) \
         are in AGENTS.md, already loaded — read it if you haven't.\n\n\
         ## Site Configuration\n\
         - Title: {}\n\
         - Base URL: {}\n\
         - Language: {}\n\n",
        config.site.title, config.site.base_url, config.site.language,
    ));

    prompt.push_str("## Collections\n\n");
    prompt.push_str("| Collection | Directory | URL prefix | Dated | Nested |\n");
    prompt.push_str("|---|---|---|---|---|\n");
    for c in &config.collections {
        prompt.push_str(&format!(
            "| {} | `content/{}/` | `{}` | {} | {} |\n",
            c.label, c.directory, c.url_prefix, c.has_date, c.nested,
        ));
    }
    prompt.push('\n');

    prompt.push_str("## Existing Content\n\n");
    for c in &config.collections {
        let items = scan_collection_content(paths, c);
        prompt.push_str(&format!("### {} ({} items)\n", c.label, items.len()));
        for item in items.iter().take(50) {
            prompt.push_str(&format!("- {}\n", item));
        }
        if items.len() > 50 {
            prompt.push_str(&format!("- ... and {} more\n", items.len() - 50));
        }
        prompt.push('\n');
    }

    prompt.push_str("## Templates\n\n");
    for name in list_templates(paths) {
        prompt.push_str(&format!("- `templates/{name}`\n"));
    }
    prompt.push('\n');

    prompt
}

/// Scan a collection's content directory and return a summary of each item.
fn scan_collection_content(
    paths: &ResolvedPaths,
    collection: &crate::config::CollectionConfig,
) -> Vec<String> {
    let collection_dir = paths.content.join(&collection.directory);
    let mut items = Vec::new();

    if !collection_dir.exists() {
        return items;
    }

    for entry in WalkDir::new(&collection_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        let path = entry.path();
        if let Ok((fm, _body)) = content::parse_content_file(path) {
            let mut summary = fm.title.clone();
            if let Some(date) = fm.date {
                summary = format!("{summary} ({date})");
            }
            if !fm.tags.is_empty() {
                summary = format!("{summary} [{}]", fm.tags.join(", "));
            }
            if fm.draft {
                summary = format!("{summary} (draft)");
            }
            items.push(summary);
        }
    }

    items
}

/// List template files in the templates directory.
fn list_templates(paths: &ResolvedPaths) -> Vec<String> {
    let mut names = Vec::new();

    if !paths.templates.exists() {
        return names;
    }

    for entry in WalkDir::new(&paths.templates)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if let Some(name) = entry
            .path()
            .strip_prefix(&paths.templates)
            .ok()
            .and_then(|p| p.to_str())
        {
            names.push(name.to_string());
        }
    }

    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts<'a>(allowed_tools: &'a str, mcp_args: &'a [String], auto: bool) -> HarnessOpts<'a> {
        HarnessOpts {
            allowed_tools,
            mcp_args,
            auto,
        }
    }

    // --- allowed tools ---

    #[test]
    fn test_agent_allows_seite_mcp_tools() {
        assert!(AGENT_ALLOWED_TOOLS.split(',').any(|t| t == "mcp__seite"));
    }

    #[test]
    fn test_agent_allowed_tools_has_no_unrestricted_bash() {
        // "Bash" on its own (unrestricted shell) must never appear as a bare
        // entry — only scoped `Bash(...)` patterns are allowed.
        assert!(!AGENT_ALLOWED_TOOLS.split(',').any(|t| t == "Bash"));
    }

    #[test]
    fn test_agent_allowed_tools_scopes_bash_to_seite_and_readonly_git() {
        for pattern in [
            "Bash(seite:*)",
            "Bash(git status:*)",
            "Bash(git diff:*)",
            "Bash(git log:*)",
            "Bash(ls:*)",
        ] {
            assert!(
                AGENT_ALLOWED_TOOLS.contains(pattern),
                "expected {AGENT_ALLOWED_TOOLS} to contain {pattern}"
            );
        }
    }

    #[test]
    fn test_mcp_config_args_only_when_file_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join(".mcp.json");
        assert!(mcp_config_args_for(&path).is_empty());
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(
            mcp_config_args_for(&path),
            vec!["--mcp-config".to_string(), ".mcp.json".to_string()]
        );
    }

    // --- dynamic context (AGENTS.md dedupe) ---

    fn test_config_and_paths(tmp: &std::path::Path) -> (SiteConfig, ResolvedPaths) {
        let config_content = "[site]\ntitle = \"Test Site\"\ndescription = \"A test\"\nbase_url = \"http://localhost:3000\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\ndefault_template = \"post.html\"\nhas_date = true\nnested = false\nurl_prefix = \"/posts\"\n";
        std::fs::write(tmp.join("seite.toml"), config_content).unwrap();
        let config = SiteConfig::load(&tmp.join("seite.toml")).unwrap();
        let paths = config.resolve_paths(tmp);
        (config, paths)
    }

    #[test]
    fn test_build_dynamic_context_contains_site_and_collection_info() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (config, paths) = test_config_and_paths(tmp.path());

        let prompt = build_dynamic_context(&config, &paths);
        assert!(prompt.contains("Test Site"));
        assert!(prompt.contains("## Collections"));
        assert!(prompt.contains("## Existing Content"));
        assert!(prompt.contains("## Templates"));
        // Collection table carries dir/url-prefix/dated/nested.
        assert!(prompt.contains("content/posts/"));
        assert!(prompt.contains("/posts"));
        assert!(prompt.contains("true"));
    }

    #[test]
    fn test_build_dynamic_context_points_to_agents_md() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (config, paths) = test_config_and_paths(tmp.path());

        let prompt = build_dynamic_context(&config, &paths);
        assert!(prompt.contains("AGENTS.md"));
    }

    #[test]
    fn test_build_dynamic_context_drops_agents_md_duplicated_sections() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (config, paths) = test_config_and_paths(tmp.path());

        let prompt = build_dynamic_context(&config, &paths);
        for removed in [
            "## Content Format",
            "## File Naming Conventions",
            "## Available Commands",
            "## Shortcodes",
            "## Important Notes",
            "## Installed Extensions",
        ] {
            assert!(
                !prompt.contains(removed),
                "expected {removed} to be removed"
            );
        }
    }

    // --- harness resolution ---

    #[test]
    fn test_harness_parse_all_known_names() {
        assert_eq!(Harness::parse("claude"), Some(Harness::Claude));
        assert_eq!(Harness::parse("codex"), Some(Harness::Codex));
        assert_eq!(Harness::parse("opencode"), Some(Harness::Opencode));
        assert_eq!(Harness::parse("cursor"), Some(Harness::Cursor));
        assert_eq!(Harness::parse("bogus"), None);
    }

    #[test]
    fn test_harness_binaries() {
        assert_eq!(Harness::Claude.binary(), "claude");
        assert_eq!(Harness::Codex.binary(), "codex");
        assert_eq!(Harness::Opencode.binary(), "opencode");
        assert_eq!(Harness::Cursor.binary(), "cursor-agent");
    }

    #[test]
    fn test_resolve_harness_prefers_cli_flag_over_env() {
        let harness =
            resolve_harness_with_env(Some("codex"), Some("opencode".to_string())).unwrap();
        assert_eq!(harness, Harness::Codex);
    }

    #[test]
    fn test_resolve_harness_falls_back_to_env() {
        let harness = resolve_harness_with_env(None, Some("cursor".to_string())).unwrap();
        assert_eq!(harness, Harness::Cursor);
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
    }

    // --- claude arg vectors ---

    #[test]
    fn test_claude_one_shot_args() {
        let mcp = vec!["--mcp-config".to_string(), ".mcp.json".to_string()];
        let o = opts("Read,Write", &mcp, false);
        let args = Harness::Claude.one_shot_args("do it", "CTX", &o);
        assert_eq!(
            args,
            vec![
                "-p",
                "do it",
                "--append-system-prompt",
                "CTX",
                "--allowedTools",
                "Read,Write",
                "--mcp-config",
                ".mcp.json",
            ]
        );
    }

    #[test]
    fn test_claude_interactive_args_ignores_yes() {
        let mcp: Vec<String> = Vec::new();
        let o = opts("Read,Write", &mcp, true);
        let args = Harness::Claude.interactive_args(None, "CTX", &o);
        assert_eq!(
            args,
            vec![
                "--append-system-prompt",
                "CTX",
                "--allowedTools",
                "Read,Write"
            ]
        );
    }

    // --- codex arg vectors ---

    #[test]
    fn test_codex_one_shot_args_uses_workspace_write_sandbox() {
        let mcp: Vec<String> = Vec::new();
        let o = opts("", &mcp, false);
        let args = Harness::Codex.one_shot_args("do it", "CTX", &o);
        assert_eq!(args[0], "exec");
        assert_eq!(args[1], "--sandbox");
        assert_eq!(args[2], "workspace-write");
        assert!(args[3].contains("CTX"));
        assert!(args[3].contains("do it"));
    }

    #[test]
    fn test_codex_interactive_args_with_and_without_prompt() {
        let mcp: Vec<String> = Vec::new();
        let o = opts("", &mcp, false);
        assert!(Harness::Codex.interactive_args(None, "CTX", &o).is_empty());

        let with_prompt = Harness::Codex.interactive_args(Some("do it"), "CTX", &o);
        assert_eq!(with_prompt.len(), 1);
        assert!(with_prompt[0].contains("CTX"));
        assert!(with_prompt[0].contains("do it"));
    }

    // --- opencode arg vectors ---

    #[test]
    fn test_opencode_one_shot_args_default_no_auto_flag() {
        let mcp: Vec<String> = Vec::new();
        let o = opts("", &mcp, false);
        let args = Harness::Opencode.one_shot_args("do it", "CTX", &o);
        assert_eq!(args[0], "run");
        assert!(args[1].contains("do it"));
        assert!(!args.iter().any(|a| a == "--auto"));
    }

    #[test]
    fn test_opencode_one_shot_args_yes_adds_auto_flag() {
        let mcp: Vec<String> = Vec::new();
        let o = opts("", &mcp, true);
        let args = Harness::Opencode.one_shot_args("do it", "CTX", &o);
        assert!(args.iter().any(|a| a == "--auto"));
    }

    #[test]
    fn test_opencode_interactive_args_with_and_without_prompt() {
        let mcp: Vec<String> = Vec::new();
        let o = opts("", &mcp, false);
        assert!(Harness::Opencode
            .interactive_args(None, "CTX", &o)
            .is_empty());

        let with_prompt = Harness::Opencode.interactive_args(Some("do it"), "CTX", &o);
        assert_eq!(with_prompt[0], "--prompt");
        assert!(with_prompt[1].contains("do it"));
    }

    // --- cursor arg vectors ---

    #[test]
    fn test_cursor_one_shot_args_default_no_force() {
        let mcp: Vec<String> = Vec::new();
        let o = opts("", &mcp, false);
        let args = Harness::Cursor.one_shot_args("do it", "CTX", &o);
        assert_eq!(args[0], "-p");
        assert!(args[1].contains("do it"));
        assert_eq!(args[2], "--output-format");
        assert_eq!(args[3], "text");
        assert!(!args.iter().any(|a| a == "--force"));
        assert!(!args.iter().any(|a| a == "--approve-mcps"));
    }

    #[test]
    fn test_cursor_one_shot_args_yes_adds_force_and_approve_mcps() {
        let mcp: Vec<String> = Vec::new();
        let o = opts("", &mcp, true);
        let args = Harness::Cursor.one_shot_args("do it", "CTX", &o);
        assert!(args.iter().any(|a| a == "--force"));
        assert!(args.iter().any(|a| a == "--approve-mcps"));
    }

    #[test]
    fn test_cursor_interactive_args_with_and_without_prompt() {
        let mcp: Vec<String> = Vec::new();
        let o = opts("", &mcp, false);
        assert!(Harness::Cursor.interactive_args(None, "CTX", &o).is_empty());

        let with_prompt = Harness::Cursor.interactive_args(Some("do it"), "CTX", &o);
        assert_eq!(with_prompt.len(), 1);
        assert!(with_prompt[0].contains("do it"));
    }
}
