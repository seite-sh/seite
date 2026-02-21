---
title: "Getting Started"
description: "Install seite and build your first static site in under a minute."
weight: 1
---

## Installation

### Quick install (macOS and Linux)

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

This downloads a prebuilt binary for your platform and installs it to `~/.local/bin`.

To install a specific version:

```bash
VERSION=v0.1.0 curl -fsSL https://seite.sh/install.sh | sh
```

### Quick install (Windows)

```powershell
irm https://seite.sh/install.ps1 | iex
```

### Install from source

If you have the Rust toolchain installed (all platforms):

```bash
cargo install seite
```

### Platform support

| Platform | Architecture | Install method |
|----------|-------------|----------------|
| macOS | Apple Silicon (M1/M2/M3/M4) | Shell installer or cargo |
| macOS | Intel x86_64 | Shell installer or cargo |
| Linux | x86_64 | Shell installer or cargo |
| Linux | aarch64/arm64 | Shell installer or cargo |
| Windows | x86_64 | PowerShell installer or cargo |

### Verify

```bash
seite --version
```

{{% callout(type="tip") %}}
Run `seite --help` to see all available commands at a glance.
{{% end %}}

## Create Your First Site

Scaffold a new site with posts, docs, and pages collections:

```bash
seite init mysite --title "My Site" --description "A personal blog" --collections posts,docs,pages
cd mysite
```

{{% callout(type="info") %}}
All flags are optional. Run `seite init mysite` and interactive prompts will guide you through each setting.
{{% end %}}

This creates the following structure:

```
mysite/
├── content/
│   ├── posts/     # Date-based blog posts with RSS
│   ├── docs/      # Documentation with sidebar navigation
│   └── pages/     # Standalone pages (about, contact, etc.)
├── templates/     # Tera templates (override bundled themes)
├── static/        # Static assets (CSS, JS, images)
├── seite.toml      # Site configuration
├── .claude/       # Claude Code agent configuration (includes MCP server)
└── .seite/         # Project metadata (version tracking)
```

## Create Content

Add a blog post:

```bash
seite new post "Hello World" --tags intro,welcome
```

This creates `content/posts/2026-02-18-hello-world.md` with frontmatter:

```yaml
---
title: "Hello World"
date: 2026-02-18
tags:
  - intro
  - welcome
---

Your content here...
```

Add a documentation page:

```bash
seite new doc "Getting Started"
```

Add a standalone page:

```bash
seite new page "About"
```

## Build Your Site

Build the site to the `dist/` directory:

```bash
seite build
```

The build generates:
- HTML pages with clean URLs (`/posts/hello-world`)
- Markdown copies alongside every HTML file
- RSS feed at `/feed.xml`
- XML sitemap at `/sitemap.xml`
- Search index at `/search-index.json`
- LLM discovery files at `/llms.txt` and `/llms-full.txt`

## Development Server

Start a dev server with live reload:

```bash
seite serve
```

The server starts at `http://localhost:3000` (auto-increments if the port is taken) and watches for file changes. An interactive REPL lets you run commands without restarting:

```
seite> new post "Another Post"
seite> theme apply dark
seite> build
seite> status
seite> stop
```

{{% callout(type="tip") %}}
The REPL is the fastest way to iterate. Create content, switch themes, and rebuild — all without leaving the dev server.
{{% end %}}

## Themes

List available themes:

```bash
seite theme list
```

Apply a bundled theme:

```bash
seite theme apply dark
```

Generate a custom theme with AI:

```bash
seite theme create "minimal serif with warm colors"
```

## Updating

Update the binary itself:

```bash
seite self-update
```

After updating, bring your project's config files up to date:

```bash
seite upgrade
```

This adds any new configuration that shipped with the new version (e.g., MCP server settings, new permission entries). It's additive and non-destructive — your existing settings are preserved.

## Next Steps

- [Collections](/docs/collections) — understand how posts, docs, and pages work and how to customize them
- [Configuration](/docs/configuration) — the full `seite.toml` reference when you need to tune settings
- [Templates & Themes](/docs/templates) — customize the look, override blocks, and browse the 6 bundled themes
- [Deployment](/docs/deployment) — ship your site to GitHub Pages, Cloudflare, or Netlify
- [Workspaces](/docs/workspace) — manage multiple sites in a single repository
- [AI Agent](/docs/agent) — let Claude write content, debug builds, and generate themes for you
- [MCP Server](/docs/mcp-server) — structured AI access to your site via the Model Context Protocol
