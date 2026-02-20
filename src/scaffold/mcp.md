## MCP Server

This project includes a built-in MCP (Model Context Protocol) server that starts automatically when Claude Code opens this project. No API keys or setup required. It is configured in `.claude/settings.json`.

Use this file (CLAUDE.md) for commands, config options, template syntax, and patterns. Use the MCP server to check **current site state** before creating or modifying content:

- `page://content/{collection}` — see what content already exists (avoid duplicates)
- `page://config` — read current `page.toml` (may differ from defaults documented here)
- `page://themes` — check current theme and available options
- `page_search` — find content by title, tags, or description
- `page_build` — build the site (preferred over shelling out to `page build`)
- `page_create_content` — create content files with proper frontmatter
- `page_apply_theme` — apply a theme
- `page_lookup_docs` — search page's embedded documentation for edge cases not covered here

Additional resources: `page://docs/*` (full page documentation), `page://mcp-config` (MCP settings)

