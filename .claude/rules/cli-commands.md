---
paths:
  - "src/cli/**"
---
# CLI Commands

- clap 4.5 with derive macros
- Each subcommand: `src/cli/{name}.rs` with `{Command}Args` + `pub fn run(args) -> anyhow::Result<()>`
- Interactive prompts go through `src/cli/prompt.rs` (wraps `dialoguer`), never called directly — it degrades safely under `--yes`/`-y`/`SEITE_YES=1` or a non-TTY: defaults are used where one exists, otherwise the command errors naming the missing flag
- Global `--json` prints exactly one document on stdout (`{"ok","command","data","warnings"}` or `{"ok":false,"error":{"message","chain"}}`, plus `data` if the command set it before failing); set a command's payload with `output::json::set_data()`. Rejected for `serve`, `agent`, `mcp`, `completions`, `self-update`

## Subcommands
init, new, build, check, serve, deploy, agent, theme, hooks, mcp, workspace, upgrade, contact, collection, access, skill, self-update, completions, perf, telemetry

## seite check
`src/cli/check.rs` runs config, frontmatter, shortcode, data-file, template, render, link, and asset validation via `check_site()` — the real build pipeline (`build::build_site`), but rendered into a `tempfile` scratch dir that's always discarded, so `dist/`/`dist-subdomains/` are never touched. Diagnostics come back as one sorted, deduplicated `Diagnostics` list (see `src/diagnostics.rs`); paths are relativized back to the site root (`relativize()` strips the scratch prefix from both `file` and any absolute paths quoted in the message). `--strict` also fails on warnings (e.g. `config-unknown-key`, `broken-link`); `--drafts` includes draft content. Exit 0 (no errors, or in `--strict` no diagnostics at all) / 1 otherwise; `--json` shape is `{"diagnostics": [...], "summary": {"errors", "warnings"}}` on success and `error.diagnostics` on failure. Output is compiler-style: `file:line:col: severity[code]: message` + an indented `hint:` line. In a workspace, `--site <name>` (or running at the workspace root, which has no `seite.toml`) goes through `check_workspace()`: each site via `check_site_with()` (workspace `base_url` override), files prefixed with the site path via `Diagnostic::under()`. Workspace `build` loads each site with `workspace::load_site_in_workspace_with_diagnostics()` (unknown-key warnings, prepended to that site's `BuildResult.diagnostics`) and, with `--strict`, builds every site then fails via `cli::build::strict_failure()` with workspace-relative link diagnostics. `--file <path>...` runs `check_files()`: config + per-file frontmatter/shortcodes (content), Tera syntax (templates), data-file syntax, no render. `--hook <claude|codex|cursor|opencode|gemini>` runs it as a coding agent's hook (`src/cli/harness_hooks.rs`): reads hook JSON on stdin, prints that harness's protocol JSON only when there are errors (capped at 40 lines), honors loop guards, and always exits 0 (silent outside a site; internal failures only note on stderr). `--post-edit` checks only the edited file(s) named in the hook input (`edited_paths()`: `tool_input.file_path`, Codex `apply_patch` headers, top-level `file_path`).

## Agent System
`seite agent [--with claude|codex|opencode|cursor|gemini] [--once] [prompt]` (default `SEITE_AGENT`, else claude) is a thin launcher: the prompt is passed as-is (no injected context, no `--allowedTools`/`--mcp-config`); the agent reads AGENTS.md and the `/seite` skill. Interactive by default (`claude "<prompt>"`, `gemini -i`, `opencode --prompt`), `--once` runs one-shot (`claude -p`, `codex exec --sandbox workspace-write`, `opencode run`, `cursor-agent -p`, `gemini -p`); `--yes` adds the harness's auto-approve flag. Per-harness argv lives in the `Harness` enum in `src/cli/agent.rs` (`run_once` is reused by `seite theme create`, which runs `/seite theme <desc>` or points to `/impeccable` when installed).

## Dev Server
`seite serve` starts HTTP server + file watcher. Returns `ServerHandle`. Shows Vite-style local + network URLs. `--open` launches browser, `--build` builds first. Interactive REPL (new, agent, theme, build, status, stop) reads stdin unless `--no-repl` is passed; the prompt is only printed to a real TTY, and if stdin closes (e.g. `</dev/null`, a background job) the server keeps serving instead of exiting. Live reload via `/__livereload` polling.

## Workspace System
Multi-site via `seite-workspace.toml`. `workspace::resolve_context()` returns `Standalone` or `Workspace`. `--site` flag filters. Unified serving routes `/<site-name>/...`.

## Deploy
GitHub Pages (git push), Cloudflare (wrangler), Netlify. `auto_commit = true` by default. Non-main branches auto-use preview. `--dry-run` for preview.

## Skill Pack System
`seite skill install|list|remove|update`. Known: `seomachine` (11 agents, 22 commands, 25 skills). Manifest in `.claude/.seite-skill-packs.json`. SEOMachine AGENTS.md section managed by HTML comment markers.

## Coding-Agent Harness Files
`seite init --agents [--mcp]` / `seite upgrade --agents [--mcp]` (in a workspace root or with `--site`, upgrade plans every site and confirms once) (claude, codex, opencode, cursor, gemini; default all, stored in `.seite/config.json`). All rules, skills, hooks, and (opt-in) MCP entries come from `src/cli/harness.rs` + `harness_hooks.rs`; upgrade merges via `check_agent_harness()` in `src/cli/upgrade.rs`. `seite hooks on|off|status` (`src/cli/hooks.rs`) manages the check hooks.

## Built-in Skills
One routed skill, `/seite` (SKILL.md + `reference/{check,new,preview,build,deploy,theme,brand,landing,collection,upgrade,hooks}.md`; OpenCode gets `.opencode/commands/seite.md`, Gemini `.gemini/commands/seite.toml`). Scaffolded by init into `.claude/skills/seite/` (Claude) and `.agents/skills/seite/` (Codex/Cursor/OpenCode/Gemini), upgraded with version tracking (`# seite-skill-version: N`, references replaced with SKILL.md). Per-agent paths/formats come from `harness::PROVIDERS`.
