## MCP Server

This project includes a built-in MCP (Model Context Protocol) server (`seite mcp`) — no API keys required. Each coding agent reads it from its own project config and may need a one-time approval:

{mcp_setup}
Use this file for commands, config options, template syntax, and patterns. Use the MCP server to check **current site state** before creating or modifying content:

- `seite://content/{collection}` — what content exists, with each item's source `path` and real `url` (avoid duplicates)
- `seite://config` — current `seite.toml` (may differ from defaults documented here)
- `seite://themes` — active theme and available options
- `seite_check` — every problem in the site (config, frontmatter, shortcodes, templates, links, assets) with file and line; run after changes
- `seite_search` — find content by title, description, tags, or body text (ranked)
- `seite_content_stats` — drafts, missing descriptions/tags/translations, future-dated posts
- `seite_get_page` — one page as the build sees it (resolved frontmatter, URL, rendered HTML)
- `seite_create_content` — create content with valid frontmatter (refuses to overwrite)
- `seite_update_frontmatter` — change a page's frontmatter without touching its body
- `seite_list_templates` — template overrides, base blocks, context variables, shortcodes, data keys (call before editing templates)
- `seite_build` — build the site and get warnings, broken links, and missing assets back as data
- `seite_apply_theme` — apply a theme (backs up a customized `base.html`)
- `seite_create_collection` — add a preset collection
- `seite_lookup_docs` — search seite's embedded documentation for edge cases not covered here

Additional resources: `seite://docs/*` (full seite documentation), `seite://mcp-config` (MCP settings){trust_resource}

