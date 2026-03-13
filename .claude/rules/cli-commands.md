---
paths:
  - "src/cli/**"
---
# CLI Commands

- clap 4.5 with derive macros
- Each subcommand: `src/cli/{name}.rs` with `{Command}Args` + `pub fn run(args) -> anyhow::Result<()>`
- Interactive prompts use `dialoguer` (only when CLI args not provided)

## Subcommands (15)
init, new, build, serve, deploy, agent, theme, mcp, workspace, upgrade, contact, collection, skill, self-update, completions

## Agent System
`seite agent` spawns Claude Code with system prompt containing site config, content inventory, template list, frontmatter format. Two modes: `seite agent "prompt"` (non-interactive) and `seite agent` (interactive).

## Dev Server
`seite serve` starts HTTP server + file watcher. Returns `ServerHandle`. Shows Vite-style local + network URLs. `--open` flag launches browser. Interactive REPL: new, agent, theme, build, status, stop. Live reload via `/__livereload` polling.

## Workspace System
Multi-site via `seite-workspace.toml`. `workspace::resolve_context()` returns `Standalone` or `Workspace`. `--site` flag filters. Unified serving routes `/<site-name>/...`.

## Deploy
GitHub Pages (git push), Cloudflare (wrangler), Netlify. `auto_commit = true` by default. Non-main branches auto-use preview. `--dry-run` for preview.

## Skill Pack System
`seite skill install|list|remove|update`. Known: `seomachine` (11 agents, 22 commands, 25 skills). Manifest in `.claude/.seite-skill-packs.json`. SEOMachine CLAUDE.md section managed by HTML comment markers.

## Built-in Skills
`/theme-builder` (4-phase theme creation), `/brand-identity` (5-phase visual identity), `/landing-page` (conditional on pages collection). Scaffolded by init, upgraded with version tracking (`# seite-skill-version: N`).
