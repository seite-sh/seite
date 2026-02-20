---
title: "MCP Server"
description: "Structured AI access to site content, configuration, themes, and build tools via the Model Context Protocol."
weight: 9
---

## Overview

`page` includes a built-in [MCP](https://modelcontextprotocol.io/) (Model Context Protocol) server that gives AI tools structured access to your site. When you open a page project in Claude Code, the MCP server starts automatically — no API keys or setup required.

The server exposes your site's documentation, configuration, content, and themes as **resources**, and provides **tools** for building, creating content, searching, and applying themes.

## How It Works

The MCP server runs as a subprocess (`page mcp`) communicating over stdio using JSON-RPC. Claude Code launches it automatically based on the configuration in `.claude/settings.json`:

```json
{
  "mcpServers": {
    "page": {
      "command": "page",
      "args": ["mcp"]
    }
  }
}
```

This is scaffolded by `page init` and can be added to existing projects with `page upgrade`.

## Resources

Resources are read-only data that AI tools can query. Each resource has a URI.

| Resource | URI | Description |
|----------|-----|-------------|
| Documentation index | `page://docs` | List of all documentation pages with titles and descriptions |
| Documentation page | `page://docs/{slug}` | Full markdown content of a specific doc page |
| Site configuration | `page://config` | Current `page.toml` serialized as JSON |
| Content overview | `page://content` | All collections with item counts |
| Collection items | `page://content/{collection}` | Items in a collection with metadata (title, date, tags, slug, url, draft status) |
| Themes | `page://themes` | Available bundled and installed themes |
| MCP configuration | `page://mcp-config` | The `.claude/settings.json` MCP server configuration |

Documentation resources are always available (they're embedded in the binary). Site-specific resources (`page://config`, `page://content/*`, `page://themes`, `page://mcp-config`) are only available when running inside a page project directory.

## Tools

Tools are actions that AI tools can execute.

### page_build

Build the site to the output directory.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `drafts` | boolean | No | Include draft content in the build (default: false) |

Returns build statistics including pages built per collection, timing, and any errors.

### page_create_content

Create a new content file with frontmatter.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `collection` | string | Yes | Collection name (`posts`, `docs`, or `pages`) |
| `title` | string | Yes | Title of the content |
| `tags` | string[] | No | Tags for the content |
| `body` | string | No | Markdown body content |
| `draft` | boolean | No | Create as draft |

Returns the file path, URL, and slug of the created content.

### page_search

Search site content by keywords.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search keywords |
| `collection` | string | No | Limit search to a specific collection |

Matches against titles, descriptions, and tags. Returns up to 20 results with metadata.

### page_apply_theme

Apply a bundled or installed theme to the site.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Theme name (`default`, `minimal`, `dark`, `docs`, `brutalist`, `bento`, or an installed theme) |

### page_lookup_docs

Look up page documentation by topic or keyword.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | No | Search keywords to find in documentation |
| `topic` | string | No | Specific doc topic slug (e.g., `configuration`, `templates`, `deployment`) |

When `topic` matches a doc slug, returns the full page. When `query` is provided, searches across all documentation and returns matching sections with context.

## Manual Testing

You can test the MCP server manually by sending JSON-RPC messages:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | page mcp
```

A full session requires the initialization handshake first, then queries:

```bash
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}\n' | page mcp
```

## Next Steps

- [AI Agent](/docs/agent) — interactive AI sessions with `page agent`
- [CLI Reference](/docs/cli-reference) — the `page mcp` command reference
- [Configuration](/docs/configuration) — the full `page.toml` reference
