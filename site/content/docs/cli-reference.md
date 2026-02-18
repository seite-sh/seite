---
title: "CLI Reference"
description: "Complete reference for all page CLI commands, flags, and options."
---

## Overview

`page` has seven subcommands:

| Command | Description |
|---------|-------------|
| `init`  | Create a new site |
| `build` | Build the site |
| `serve` | Development server with live reload |
| `new`   | Create content files |
| `agent` | AI assistant with site context |
| `theme` | Manage themes |
| `deploy`| Deploy to hosting platforms |

## page init

Create a new site directory with scaffolded structure.

```bash
page init <name> [options]
```

| Flag | Description |
|------|-------------|
| `--title` | Site title |
| `--description` | Site description |
| `--deploy-target` | `github-pages`, `cloudflare`, or `netlify` |
| `--collections` | Comma-separated list: `posts,docs,pages` |

If flags are omitted, `page init` prompts interactively.

```bash
# Non-interactive
page init mysite --title "My Blog" --deploy-target github-pages --collections posts,pages

# Interactive
page init mysite
```

## page build

Build the site from `page.toml` in the current directory.

```bash
page build [options]
```

| Flag | Description |
|------|-------------|
| `--drafts` | Include draft content in the build |

The build pipeline runs 12 steps: clean output, load templates, process collections, render pages, generate RSS, sitemap, discovery files, markdown output, search index, copy static files, process images, and post-process HTML. Per-step timing is shown in the output.

## page serve

Start a development server with live reload.

```bash
page serve [options]
```

| Flag | Description |
|------|-------------|
| `--port` | Starting port (default: 3000, auto-increments if taken) |
| `--drafts` | Include drafts |

The server injects a live-reload script that polls for changes. An interactive REPL accepts commands:

- `new <collection> "Title"` — create content
- `agent [prompt]` — launch AI agent
- `theme apply <name>` — apply theme and rebuild
- `build` — rebuild the site
- `status` — show server info
- `stop` — stop the server

## page new

Create a new content file with frontmatter.

```bash
page new <collection> "Title" [options]
```

| Flag | Description |
|------|-------------|
| `--tags` | Comma-separated tags (posts only) |
| `--lang` | Language code for translations (e.g., `es`, `fr`) |

```bash
page new post "My Post" --tags rust,web
page new doc "API Guide"
page new page "About"
page new post "Mi Post" --lang es    # Spanish translation
```

## page agent

Launch an AI assistant with full site context.

```bash
page agent [prompt]
```

Two modes:
- **Interactive**: `page agent` — opens a Claude Code session
- **One-shot**: `page agent "write a blog post about Rust"` — runs and exits

The agent receives your site config, content inventory, template list, and available CLI commands. It can read, write, and edit files. Requires Claude Code: `npm install -g @anthropic-ai/claude-code`.

## page theme

Manage site themes.

```bash
page theme <subcommand>
```

| Subcommand | Description |
|------------|-------------|
| `list` | Show available bundled themes |
| `apply <name>` | Apply a bundled theme |
| `create "<description>"` | Generate a custom theme with AI |

```bash
page theme list
page theme apply dark
page theme create "brutalist with neon green accents"
```

Six bundled themes: `default`, `minimal`, `dark`, `docs`, `brutalist`, `bento`.

## page deploy

Deploy the built site.

```bash
page deploy [options]
```

| Flag | Description |
|------|-------------|
| `--target` | Override deploy target (`github-pages`, `cloudflare`, `netlify`) |
| `--dry-run` | Preview what would be deployed without deploying |

```bash
page deploy                          # Use target from page.toml
page deploy --dry-run                # Preview changes
page deploy --target netlify         # Override target
page deploy --target cloudflare --dry-run
```
