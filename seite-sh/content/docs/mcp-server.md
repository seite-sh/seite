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

The MCP server runs as a subprocess (`seite mcp`) communicating over stdio using JSON-RPC. Claude Code only reads project MCP servers from `.mcp.json`:

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

`.claude/settings.json` pre-approves that server so Claude Code can start it without asking each time:

```json
{
  "enabledMcpjsonServers": ["seite"]
}
```

Both files are scaffolded by `seite init`; `seite upgrade` adds them to existing projects and migrates any older `mcpServers` block out of `settings.json` (Claude Code never reads MCP servers from there). The first time Claude Code opens the project it may still ask you to approve the server once.

Other MCP clients (Codex CLI, OpenCode, Cursor, ...) run the same `seite mcp` command from the site directory.

### Protocol versions

The server negotiates the MCP revision with each client:

- **`initialize` handshake** — revisions `2025-11-25`, `2025-06-18`, `2025-03-26`, and `2024-11-05`. The server answers with the version the client asks for, or `2025-11-25` if it asks for one it doesn't know.
- **Stateless requests** — revision `2026-07-28`: `server/discover` lists the supported versions, and each request declares its version in `_meta`. An unsupported version gets an `UnsupportedProtocolVersionError` (`-32022`) listing the supported ones.

Newer fields are only sent to clients that negotiated a revision defining them: tool `annotations` (read-only/destructive/idempotent hints) from `2025-03-26`, tool `title`, `outputSchema`, and `structuredContent` from `2025-06-18`. The `initialize`/`server/discover` result includes `instructions` telling the agent when to use the server instead of reading files.

## Resources

Resources are read-only data that AI tools can query. Each resource has a URI.

| Resource | URI | Description |
|----------|-----|-------------|
| Documentation index | `seite://docs` | List of all documentation pages with titles and descriptions |
| Documentation page | `seite://docs/{slug}` | Full markdown content of a specific doc page |
| Site configuration | `seite://config` | Current `seite.toml` serialized as JSON |
| Content overview | `seite://content` | All collections with item counts |
| Collection items | `seite://content/{collection}` | Items in a collection with `title`, `slug`, `url`, `path` (source file, relative to the site root), `lang`, `draft`, `date`, `tags`, `description`, `weight`. A file that fails to parse appears as `{path, parse_error}` instead of being silently skipped |
| Themes | `seite://themes` | Available bundled and installed themes |
| MCP configuration | `seite://mcp-config` | `.mcp.json` (server declaration) and `.claude/settings.json` (approval + permissions) |

Documentation resources are always available (they're embedded in the binary). Site-specific resources (`seite://config`, `seite://content/*`, `seite://themes`, `seite://mcp-config`) are only available when running inside a page project directory.

`resources/templates/list` advertises the parameterized URIs `seite://content/{collection}` and `seite://docs/{slug}`.

## Tools

Tools are actions that AI tools can execute. A tool-execution failure (bad arguments, no site found, a failed build, an unknown theme, an existing file without `overwrite`, ...) comes back as a normal result with `isError: true` and an actionable message — only a missing/unknown tool name is a protocol-level error.

A successful result is compact JSON text; clients on `2025-06-18` or newer also get the same object as `structuredContent`, and every tool except `seite_lookup_docs` declares an `outputSchema` for it.

| Tool | Changes files? |
|------|----------------|
| `seite_search`, `seite_get_page`, `seite_content_stats`, `seite_list_templates`, `seite_lookup_docs` | No (read-only) |
| `seite_build` | Writes the output directory |
| `seite_create_content` | Creates a content file (replaces one only with `overwrite: true`) |
| `seite_update_frontmatter` | Rewrites one file's frontmatter |
| `seite_apply_theme` | Writes `templates/base.html` (backs up a customized one) |
| `seite_create_collection` | Rewrites `seite.toml`, creates a content directory |

### seite_build

Build the site to the output directory.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `drafts` | boolean | No | Include draft content in the build (default: false) |
| `strict` | boolean | No | Fail (`isError`) if the build produced warnings or broken internal links (default: false) |

Returns build statistics, `warnings` (e.g. a custom template that failed to parse and fell back to the built-in default), and `broken_links` (`[{target, sources}]` — internal links pointing at pages that don't exist).

### seite_create_content

Create a new content file with frontmatter (same rules as `seite new`).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `collection` | string | Yes | Collection name, e.g. `posts`, `docs`, `pages`, `changelog`, `roadmap` (singular aliases like `post` work) |
| `title` | string | Yes | Title of the content |
| `slug` | string | No | Filename slug (lowercase letters, digits, `-`, `_`); defaults to a slug of the title |
| `description` | string | No | Frontmatter description (meta description and listings) |
| `tags` | string[] | No | Tags for the content |
| `body` | string | No | Markdown body content; omit for frontmatter only |
| `draft` | boolean | No | Create as draft (default: false) |
| `weight` | integer | No | Ordering weight for non-date collections (lower sorts first) |
| `extra` | object | No | Arbitrary frontmatter data exposed to templates as `page.extra` |
| `subdir` | string | No | Sub-directory inside a nested collection, e.g. `guides` (docs only) |
| `lang` | string | No | Language code for a translation (must be configured under `[languages]`); adds a `.{lang}.md` suffix |
| `overwrite` | boolean | No | Replace the file if it already exists (default: false — otherwise the call fails) |

Returns the source `path` (relative to the site root) and the `url` it will be published at.

### seite_get_page

Inspect one content file as the build sees it. Pass exactly one of:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | One of | Source file, relative to the site root (`content/docs/guides/intro.md`) or to the content directory |
| `url` | string | One of | Published URL (`/docs/guides/intro`; a full URL on the site's `base_url` also works) |

Returns `path`, `collection`, `url`, `slug`, `lang`, `draft`, `date`, the resolved `frontmatter` (e.g. a date taken from the filename), `word_count`, `reading_time`, the markdown `body`, the rendered body `html` (shortcodes + markdown, without the page template; `null` with a `render_error` if rendering fails), and `output_path` when the page has been built.

### seite_update_frontmatter

Edit only the frontmatter of a content file. The markdown body is kept byte-for-byte.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Content file (`.md`) inside the content directory |
| `set` | object | No | Top-level keys to add or replace (e.g. `{"tags": ["rust"], "draft": false}`) |
| `unset` | string[] | No | Top-level keys to remove (`title` can't be removed) |

The result must still parse and keep its required fields (`title`; a date for dated collections, from `date` or the filename). Paths outside the content directory are refused. Unknown keys and key order are preserved; YAML comments inside the frontmatter are not. Repeating the same update writes nothing (`changed: false`).

### seite_search

Search site content (including drafts) by keyword.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search keywords (case-insensitive substring match) |
| `collection` | string | No | Limit search to a specific collection (singular aliases work) |
| `limit` | integer | No | Maximum results to return, 1-100 (default: 20) |

Matches titles, descriptions, tags, and body text, ranked title > description/tags > body. Returns `total` matches and the `returned` subset, each with source `path` and published `url`.

### seite_content_stats

Content health overview, optionally for one `collection`: item and draft counts, items missing a `description`, dated items without tags, future-dated items, files that fail to parse, and — on multilingual sites — default-language items missing a translation, per configured language. Lists are capped at 50 per category; the `totals` counts are exact.

### seite_list_templates

No parameters. Returns what an agent needs before editing templates: the user templates in `templates/` (and which embedded defaults they override), the blocks defined in the active `base.html`, the Tera context variables available to each template kind (item pages, homepage, collection indexes, 404, tag pages, shortcodes), built-in and custom shortcodes with their parameters and usage, and the keys of loaded data files.

### seite_create_collection

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `preset` | string | Yes | `posts`, `docs`, `pages`, `changelog`, `roadmap`, or `trust` |

Adds the collection to `seite.toml` and creates its content directory — the same as `seite collection add`. Fails if the collection already exists.

### seite_apply_theme

Apply a bundled or installed theme to the site.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Theme name (`default`, `minimal`, `dark`, `docs`, `brutalist`, `bento`, `landing`, `terminal`, `magazine`, `academic`, or an installed theme) |

If the current `base.html` has been customized (it matches no known theme), it's backed up to `base.html.bak` (or `base.html.bak.N`) before being replaced, and the backup path is returned.

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
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}' | seite mcp
```

A full session requires the initialization handshake first, then queries:

```bash
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n' | seite mcp
```

## Next Steps

- [AI Agent](/docs/agent): interactive AI sessions with `seite agent`
- [CLI Reference](/docs/cli-reference): the `seite mcp` command reference
- [Configuration](/docs/configuration): the full `seite.toml` reference
- [AI Static Site Generator: What It Means and Why It Matters](/blog/ai-static-site-generator): why the MCP server is a key part of what makes a static site generator AI-native
