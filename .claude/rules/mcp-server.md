---
paths:
  - "src/mcp/**"
---
# MCP Server

`seite mcp` runs JSON-RPC 2.0 over stdio. Spawned by Claude Code via `.claude/settings.json`.

## Architecture
Synchronous read loop on stdin, dispatches methods, writes to stdout. All logging to stderr (never stdout). No async runtime.

## Resources (read-only)
- `seite://docs` / `seite://docs/{slug}` — 15 embedded docs (include_str!)
- `seite://config` — `seite.toml` as JSON
- `seite://content` / `seite://content/{collection}` — content inventory
- `seite://themes` — bundled + installed themes
- `seite://trust` — trust center state (when configured)
- `seite://mcp-config` — `.claude/settings.json`

## Tools (executable)
- `seite_build` — runs build pipeline, returns stats
- `seite_create_content` — creates content files with frontmatter
- `seite_search` — searches by title/description/tags
- `seite_apply_theme` — applies bundled or installed theme
- `seite_lookup_docs` — searches embedded docs

## Files
`src/mcp/mod.rs` (protocol), `src/mcp/resources.rs`, `src/mcp/tools.rs`, `src/docs.rs` + `seite-sh/content/docs/` (embedded docs)
