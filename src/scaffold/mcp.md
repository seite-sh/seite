## MCP Server

This project includes a built-in MCP (Model Context Protocol) server (`seite mcp`), declared in `.mcp.json` and pre-approved via `enabledMcpjsonServers` in `.claude/settings.json`. Claude Code starts it when it opens this project; the first time, it may ask you to approve the project's MCP server (check with `/mcp`). No API keys required.

Use this file for commands, config options, template syntax, and patterns. Use the MCP server to check **current site state** before creating or modifying content:

- `seite://content/{collection}` — what content exists, with each item's source `path` and real `url` (avoid duplicates)
- `seite://config` — current `seite.toml` (may differ from defaults documented here)
- `seite://themes` — active theme and available options
- `seite_search` — find content by title, description, tags, or body text (ranked)
- `seite_build` — build the site and get warnings and broken links back as data
- `seite_create_content` — create content with valid frontmatter (refuses to overwrite)
- `seite_apply_theme` — apply a theme (backs up a customized `base.html`)
- `seite_lookup_docs` — search seite's embedded documentation for edge cases not covered here

Additional resources: `seite://docs/*` (full seite documentation), `seite://mcp-config` (MCP settings){trust_resource}

