---
title: "MCP Server for AI Integration"
description: "Give AI tools structured access to your static site's content, configuration, and build tools via the Model Context Protocol. Auto-configured, no API keys."
weight: 10
---

AI tools are good at reading files, but they're better when they can work with concepts. The MCP (Model Context Protocol) server built into seite gives AI tools structured access to your site: collections, content items, themes, build actions; instead of making them parse raw markdown and TOML. It is what makes the [AI agent](/docs/agent) and other AI integrations understand your static site generator project as a project, not just a pile of files.

## Overview

`seite` includes a built-in [MCP](https://modelcontextprotocol.io/) (Model Context Protocol) server that gives AI tools structured access to your site. When you open a seite project in Claude Code, the MCP server starts automatically, no API keys or setup required.

The server exposes your site's documentation, configuration, content, and themes as **resources**, and provides **tools** for building, creating content, searching, and applying themes.

## Why MCP?

Without MCP, an AI tool working on your site has to:
1. Find and read `seite.toml` to understand your configuration
2. List directories to discover collections
3. Parse YAML frontmatter from each markdown file
4. Guess which themes are available by reading template files
5. Run `seite build` via shell and parse terminal output for errors

With MCP, the same tool makes a single structured request and gets back typed JSON:

```
Request:  resources/read  →  seite://content/posts
Response: [
  {"title": "Rust Error Handling", "date": "2026-03-01", "tags": ["rust", "tutorial"], "slug": "rust-error-handling", "url": "/posts/rust-error-handling", "draft": false},
  {"title": "Why Static Sites", "date": "2026-02-15", "tags": ["web"], "slug": "why-static-sites", "url": "/posts/why-static-sites", "draft": false}
]
```

No file parsing. No guessing. The AI tool gets clean data and can focus on what you actually asked it to do.

## How It Works

The MCP server runs as a subprocess (`seite mcp`) communicating over stdio using JSON-RPC. Claude Code launches it automatically based on the configuration in `.claude/settings.json` (created by `seite init` or added with `seite upgrade`: see the [configuration docs](/docs/configuration) for details on `seite.toml`):

```json
{
  "mcpServers": {
    "seite": {
      "command": "seite",
      "args": ["mcp"]
    }
  }
}
```

This is scaffolded by `seite init` and can be added to existing projects with `seite upgrade`.

## Resources

Resources are read-only data that AI tools can query. Each resource has a URI.

| Resource | URI | Description |
|----------|-----|-------------|
| Documentation index | `seite://docs` | List of all documentation pages with titles and descriptions |
| Documentation page | `seite://docs/{slug}` | Full markdown content of a specific doc page |
| Site configuration | `seite://config` | Current `seite.toml` serialized as JSON |
| Content overview | `seite://content` | All collections with item counts |
| Collection items | `seite://content/{collection}` | Items in a collection with metadata (title, date, tags, slug, url, draft status) |
| Themes | `seite://themes` | Available bundled and installed themes |
| MCP configuration | `seite://mcp-config` | The `.claude/settings.json` MCP server configuration |

Documentation resources are always available (they're embedded in the binary). Site-specific resources (`seite://config`, `seite://content/*`, `seite://themes`, `seite://mcp-config`) are only available when running inside a page project directory.

## Tools

Tools are actions that AI tools can execute.

### seite_build

Build the site to the output directory.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `drafts` | boolean | No | Include draft content in the build (default: false) |

Returns build statistics including pages built per collection, timing, and any errors.

### seite_create_content

Create a new content file with frontmatter.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `collection` | string | Yes | Collection name (`posts`, `docs`, or `pages`) |
| `title` | string | Yes | Title of the content |
| `tags` | string[] | No | Tags for the content |
| `body` | string | No | Markdown body content |
| `draft` | boolean | No | Create as draft |

Returns the file path, URL, and slug of the created content.

### seite_search

Search site content by keywords.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search keywords |
| `collection` | string | No | Limit search to a specific collection |

Matches against titles, descriptions, and tags. Returns up to 20 results with metadata.

### seite_apply_theme

Apply a bundled or installed theme to the site.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Theme name (`default`, `minimal`, `dark`, `docs`, `brutalist`, `bento`, or an installed theme) |

### seite_lookup_docs

Look up page documentation by topic or keyword.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | No | Search keywords to find in documentation |
| `topic` | string | No | Specific doc topic slug (e.g., `configuration`, `templates`, `deployment`) |

When `topic` matches a doc slug, returns the full page. When `query` is provided, searches across all documentation and returns matching sections with context.

## Practical Example

Say you're using Claude Code and ask: "Add a new blog post summarizing our three most recent posts."

**Without MCP**, the AI has to `glob` for markdown files, `read` each one, parse YAML frontmatter manually, sort by date, extract titles and descriptions, then write the summary post. Multiple tool calls, fragile parsing, easy to miss edge cases.

**With MCP**, the AI calls `seite://content/posts`, gets a sorted JSON array of all posts with metadata, picks the top three, and writes the summary. One resource call instead of a dozen file operations. The [AI agent](/docs/agent) uses this automatically. You don't need to think about it.

## Manual Testing

You can test the MCP server manually by sending JSON-RPC messages:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | seite mcp
```

A full session requires the initialization handshake first, then queries:

```bash
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}\n' | seite mcp
```

## Next Steps

- [AI Agent](/docs/agent): interactive AI sessions with `seite agent`
- [CLI Reference](/docs/cli-reference): the `seite mcp` command reference
- [Configuration](/docs/configuration): the full `seite.toml` reference
- [AI Static Site Generator: What It Means and Why It Matters](/blog/ai-static-site-generator): why the MCP server is a key part of what makes a static site generator AI-native
