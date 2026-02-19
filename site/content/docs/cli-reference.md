---
title: "CLI Reference"
description: "Complete reference for all page CLI commands, flags, and options."
weight: 10
---

{{% callout(type="tip") %}}
Run `page <command> --help` for quick inline help on any command.
{{% end %}}

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
| `--strict` | Treat broken internal links as build errors |

The build pipeline runs 12 steps: clean output, load templates, process collections, render pages, generate RSS, sitemap, discovery files, markdown output, search index, copy static files, process images, and post-process HTML. Per-step timing is shown in the output.

After building, `page build` validates all internal links in the generated HTML. Broken links (e.g., `<a href="/posts/nonexistent">`) are reported as warnings by default. Use `--strict` to fail the build when broken links are found — useful in CI pipelines.

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

Manage site themes — list, apply, install, export, and generate.

```bash
page theme <subcommand>
```

| Subcommand | Description |
|------------|-------------|
| `list` | Show all available themes (bundled + installed) |
| `apply <name>` | Apply a bundled or installed theme |
| `create "<description>"` | Generate a custom theme with AI |
| `install <url>` | Download and install a theme from a URL |
| `export <name>` | Export the current theme as a shareable `.tera` file |

```bash
page theme list
page theme apply dark
page theme create "brutalist with neon green accents"
page theme install https://example.com/themes/aurora.tera
page theme install https://example.com/themes/aurora.tera --name my-aurora
page theme export my-theme --description "My custom dark theme"
```

Six bundled themes: `default`, `minimal`, `dark`, `docs`, `brutalist`, `bento`. Installed themes are stored in `templates/themes/` and listed alongside bundled themes. See the [Theme Gallery](/docs/theme-gallery) for visual previews.

## page deploy

Deploy the built site.

```bash
page deploy [options]
```

| Flag | Description |
|------|-------------|
| `--target` | Override deploy target (`github-pages`, `cloudflare`, `netlify`) |
| `--dry-run` | Preview what would be deployed without deploying |
| `--domain` | Set up a custom domain (prints DNS records, updates config, attaches to platform) |
| `--setup` | Run guided deploy setup |
| `--skip-checks` | Skip pre-flight checks |
| `--base-url` | Override base URL for this deploy |

```bash
page deploy                          # Use target from page.toml
page deploy --dry-run                # Preview changes
page deploy --target netlify         # Override target
page deploy --target cloudflare --dry-run
page deploy --domain example.com     # Set up custom domain
page deploy --setup                  # Guided setup wizard
```
