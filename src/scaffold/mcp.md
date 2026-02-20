## MCP Server

This project includes a built-in MCP (Model Context Protocol) server that starts automatically when Claude Code opens this project. No API keys or setup required. It is configured in `.claude/settings.json`.

Use this file (CLAUDE.md) for commands, config options, template syntax, and patterns. Use the MCP server to check **current site state** before creating or modifying content:

- `seite://content/{collection}` — see what content already exists (avoid duplicates)
- `seite://config` — read current `seite.toml` (may differ from defaults documented here)
- `seite://themes` — check current theme and available options
- `seite_search` — find content by title, tags, or description
- `seite_build` — build the site (preferred over shelling out to `seite build`)
- `seite_create_content` — create content files with proper frontmatter
- `seite_apply_theme` — apply a theme
- `seite_lookup_docs` — search page's embedded documentation for edge cases not covered here

Additional resources: `seite://docs/*` (full page documentation), `seite://mcp-config` (MCP settings)

