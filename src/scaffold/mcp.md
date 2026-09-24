## MCP Server

This project includes a built-in MCP (Model Context Protocol) server (`seite mcp`), declared in `.mcp.json` and pre-approved via `enabledMcpjsonServers` in `.claude/settings.json`. Claude Code starts it when it opens this project; the first time, it may ask you to approve the project's MCP server (check with `/mcp`). No API keys required.

Use this file (AGENTS.md) for commands, config options, template syntax, and patterns. Use the MCP server to check **current site state** before creating or modifying content:

- `seite://content/{collection}` — see what content already exists (avoid duplicates)
- `seite://config` — read current `seite.toml` (may differ from defaults documented here)
- `seite://themes` — check current theme and available options
- `seite_search` — find content by title, tags, or description
- `seite_content_stats` — drafts, missing descriptions/tags/translations, future-dated posts
- `seite_get_page` — one page as the build sees it (resolved frontmatter, URL, rendered HTML)
- `seite_create_content` — create content files with proper frontmatter
- `seite_update_frontmatter` — change a page's frontmatter without touching its body
- `seite_list_templates` — template overrides, base blocks, context variables, shortcodes, data keys (call before editing templates)
- `seite_build` — build the site (preferred over shelling out to `seite build`)
- `seite_apply_theme` — apply a theme
- `seite_create_collection` — add a preset collection
- `seite_lookup_docs` — search page's embedded documentation for edge cases not covered here

Additional resources: `seite://docs/*` (full page documentation), `seite://mcp-config` (MCP settings)
