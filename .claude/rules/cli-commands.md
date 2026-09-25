---
paths:
  - "src/cli/**"
---
# CLI Commands

- clap 4.5 with derive macros
- Each subcommand: `src/cli/{name}.rs` with `{Command}Args` + `pub fn run(args) -> anyhow::Result<()>`
- Interactive prompts go through `src/cli/prompt.rs` (wraps `dialoguer`), never called directly — it degrades safely under `--yes`/`-y`/`SEITE_YES=1` or a non-TTY: defaults are used where one exists, otherwise the command errors naming the missing flag
- Global `--json` prints exactly one document on stdout (`{"ok","command","data","warnings"}` or `{"ok":false,"error":{"message","chain"}}`); set a command's payload with `output::json::set_data()`. Rejected for `serve`, `agent`, `mcp`, `completions`, `self-update`

## Subcommands
init, new, build, check, serve, deploy, agent, theme, mcp, workspace, upgrade, contact, collection, access, skill, self-update, completions, perf, telemetry

## seite check
`src/cli/check.rs` runs config, frontmatter, shortcode, data-file, template, render, link, and asset validation via `check_site()` — the real build pipeline (`build::build_site`), but rendered into a `tempfile` scratch dir that's always discarded, so `dist/`/`dist-subdomains/` are never touched. Diagnostics come back as one sorted, deduplicated `Diagnostics` list (see `src/diagnostics.rs`); paths are relativized back to the site root (`relativize()` strips the scratch prefix from both `file` and any absolute paths quoted in the message). `--strict` also fails on warnings (e.g. `config-unknown-key`, `broken-link`); `--drafts` includes draft content. Exit 0 (no errors, or in `--strict` no diagnostics at all) / 1 otherwise; `--json` shape is `{"diagnostics": [...], "summary": {"errors", "warnings"}}` on success and `error.diagnostics` on failure. Output is compiler-style: `file:line:col: severity[code]: message` + an indented `hint:` line.

## Agent System
`seite agent` spawns Claude Code with system prompt containing site config, content inventory, template list, frontmatter format. Two modes: `seite agent "prompt"` (non-interactive) and `seite agent` (interactive).

## Dev Server
`seite serve` starts HTTP server + file watcher. Returns `ServerHandle`. Shows Vite-style local + network URLs. `--open` launches browser, `--build` builds first. Interactive REPL (new, agent, theme, build, status, stop) reads stdin unless `--no-repl` is passed; the prompt is only printed to a real TTY, and if stdin closes (e.g. `</dev/null`, a background job) the server keeps serving instead of exiting. Live reload via `/__livereload` polling.

## Workspace System
Multi-site via `seite-workspace.toml`. `workspace::resolve_context()` returns `Standalone` or `Workspace`. `--site` flag filters. Unified serving routes `/<site-name>/...`.

## Deploy
GitHub Pages (git push), Cloudflare (wrangler), Netlify. `auto_commit = true` by default. Non-main branches auto-use preview. `--dry-run` for preview.

## Skill Pack System
`seite skill install|list|remove|update`. Known: `seomachine` (11 agents, 22 commands, 25 skills). Manifest in `.claude/.seite-skill-packs.json`. SEOMachine AGENTS.md section managed by HTML comment markers.

## Coding-Agent Harness Files
`seite init --agents` / `seite upgrade --agents` (claude, codex, opencode, cursor; default all, stored in `.seite/config.json`). All rules, skills, and MCP entries come from `src/cli/harness.rs`; upgrade merges via `check_agent_harness()` in `src/cli/upgrade.rs`. `seite agent` stays Claude-only.

## Built-in Skills
`/seite` (verb dispatcher: check, new, preview, build, deploy, theme, collection; OpenCode gets a `.opencode/commands/seite.md` wrapper), `/theme-builder` (4-phase theme creation), `/brand-identity` (5-phase visual identity), `/landing-page` (conditional on pages collection). Scaffolded by init into `.claude/skills/` (Claude) and `.agents/skills/` (Codex/Cursor/OpenCode), upgraded with version tracking (`# seite-skill-version: N`). Per-agent paths/formats come from `harness::PROVIDERS`.
