---
paths:
  - "src/mcp/**"
---
# MCP Server

`seite mcp` runs JSON-RPC 2.0 over stdio. Declared in `.mcp.json` (the only place Claude Code reads project MCP servers); `.claude/settings.json` pre-approves it via `enabledMcpjsonServers` so it starts without a prompt. `seite upgrade` migrates any legacy `mcpServers` block out of `settings.json`.

## Architecture
Synchronous read loop on stdin, dispatches methods, writes to stdout. All logging to stderr (never stdout). No async runtime.

## Resources (read-only)
- `seite://docs` / `seite://docs/{slug}` — 15 embedded docs (include_str!)
- `seite://config` — `seite.toml` as JSON
- `seite://content` / `seite://content/{collection}` — content inventory
- `seite://themes` — bundled + installed themes
- `seite://trust` — trust center state (when configured)
- `seite://mcp-config` — `.mcp.json` + `.claude/settings.json`

## Tools (executable)
- `seite_build` — runs build pipeline; returns stats, `warnings`, `broken_links`; `strict=true` makes either an `isError`
- `seite_create_content` — creates content files with frontmatter (slug, description, tags, weight, extra, subdir, lang); refuses to replace an existing file unless `overwrite=true`
- `seite_search` — ranked search (title > description/tags > body) with `limit`; returns `total`/`returned`
- `seite_apply_theme` — applies bundled (10) or installed theme; backs up a customized `base.html` to `base.html.bak`
- `seite_lookup_docs` — searches embedded docs

All five are the complete tool set (`TOOL_NAMES` in `src/mcp/tools.rs`). A tool-execution failure (bad args, no site, build/theme error) is an ordinary result with `isError: true`, not a JSON-RPC error.

## Files
`src/mcp/mod.rs` (protocol), `src/mcp/resources.rs`, `src/mcp/tools.rs`, `src/docs.rs` + `seite-sh/content/docs/` (embedded docs)
