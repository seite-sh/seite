---
title: Getting Started
description: Install page and build your first static site in under a minute.
---

## Installation

### Quick install (macOS and Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/sanchezomar/page/main/install.sh | sh
```

This downloads a prebuilt binary for your platform and installs it to `~/.local/bin`.

To install a specific version:

```bash
VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/sanchezomar/page/main/install.sh | sh
```

### Install from source

If you have the Rust toolchain installed:

```bash
cargo install page
```

### Platform support

| Platform | Architecture | Install method |
|----------|-------------|----------------|
| macOS | Apple Silicon (M1/M2/M3/M4) | Shell installer or cargo |
| macOS | Intel x86_64 | Shell installer or cargo |
| Linux | x86_64 | Shell installer or cargo |
| Linux | aarch64/arm64 | Shell installer or cargo |
| Windows | x86_64 | `cargo install page` |

### Verify

```bash
page --version
```

## Create Your First Site

Scaffold a new site with posts, docs, and pages collections:

```bash
page init mysite --title "My Site" --description "A personal blog" --collections posts,docs,pages
cd mysite
```

This creates the following structure:

```
mysite/
├── content/
│   ├── posts/     # Date-based blog posts with RSS
│   ├── docs/      # Documentation with sidebar navigation
│   └── pages/     # Standalone pages (about, contact, etc.)
├── templates/     # Tera templates (override bundled themes)
├── static/        # Static assets (CSS, JS, images)
├── page.toml      # Site configuration
└── .claude/       # Claude Code agent configuration
```

## Create Content

Add a blog post:

```bash
page new post "Hello World" --tags intro,welcome
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
page new doc "Getting Started"
```

Add a standalone page:

```bash
page new page "About"
```

## Build Your Site

Build the site to the `dist/` directory:

```bash
page build
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
page serve
```

The server starts at `http://localhost:3000` (auto-increments if the port is taken) and watches for file changes. An interactive REPL lets you run commands without restarting:

```
page> new post "Another Post"
page> theme apply dark
page> build
page> status
page> stop
```

## Themes

List available themes:

```bash
page theme list
```

Apply a bundled theme:

```bash
page theme apply dark
```

Generate a custom theme with AI:

```bash
page theme create "minimal serif with warm colors"
```

## Next Steps

- [CLI Reference](/docs/cli-reference) — All commands and flags
- [Collections](/docs/collections) — Configure posts, docs, and pages
- [Configuration](/docs/configuration) — Full `page.toml` reference
- [Templates & Themes](/docs/templates) — Customize the look
