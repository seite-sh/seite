# page

[![CI](https://github.com/sanchezomar/page/actions/workflows/rust.yml/badge.svg)](https://github.com/sanchezomar/page/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**AI-native static site generator** — every page ships as HTML for browsers, markdown for LLMs, and structured data for search engines. Single binary. Zero config. Built with Rust.

## Install

**macOS / Linux:**

```bash
curl -fsSL https://pagecli.dev/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://pagecli.dev/install.ps1 | iex
```

**From source:**

```bash
cargo install page
```

## Quickstart

```bash
page init mysite --title "My Site" --collections posts,docs,pages
cd mysite
page build
page serve
```

Open `http://localhost:3000`. Edit content in `content/`, templates in `templates/`. The dev server live-reloads on changes.

## What it does

`page build` runs a 13-step pipeline that produces:

- **HTML** pages with SEO metadata, Open Graph, Twitter Cards, and JSON-LD structured data
- **Markdown** copies of every page (for LLM consumption)
- **RSS feeds** per collection and per language
- **XML sitemap** with hreflang alternates
- **Search index** (JSON, per language)
- **LLM discovery files** — `llms.txt` and `llms-full.txt`
- **Processed images** — resized, WebP variants, srcset, lazy loading

All from markdown + YAML frontmatter. No JavaScript runtime. No build dependencies.

## Features

- **6 bundled themes** — default, minimal, dark, docs, brutalist, bento. Or generate a custom one: `page theme create "coral brutalist with lime accents"`
- **AI agent** — `page agent` spawns Claude Code with full site context. Create posts, apply themes, and manage content conversationally
- **MCP server** — `page mcp` exposes site data to AI tools via the Model Context Protocol
- **Multi-language (i18n)** — filename-based translations, per-language URLs, RSS, sitemaps, and hreflang tags
- **6 collection presets** — posts, docs, pages, changelog, roadmap, trust center
- **Shortcodes** — `youtube`, `vimeo`, `gist`, `callout`, `figure` built-in, plus user-defined
- **Deploy anywhere** — GitHub Pages, Cloudflare Pages, Netlify with guided setup, pre-flight checks, and `--dry-run`
- **Image pipeline** — auto-resize, WebP conversion, srcset/`<picture>`, configurable quality
- **Analytics** — Google Analytics, GTM, Plausible, Fathom, Umami with optional cookie consent banner
- **Multi-site workspaces** — manage multiple sites from one directory
- **Self-update** — `page self-update` fetches the latest release with checksum verification

## Deploy

```bash
page deploy              # Commit, push, build, deploy
page deploy --dry-run    # Preview what would happen
page deploy --setup      # Guided first-time setup
```

Supports GitHub Pages, Cloudflare Pages, and Netlify. Configure in `page.toml`:

```toml
[deploy]
target = "cloudflare"    # or "github-pages" or "netlify"
```

## Documentation

Full docs at **[pagecli.dev/docs](https://pagecli.dev/docs/getting-started)**

- [Getting Started](https://pagecli.dev/docs/getting-started)
- [Configuration](https://pagecli.dev/docs/configuration)
- [Collections](https://pagecli.dev/docs/collections)
- [Templates](https://pagecli.dev/docs/templates)
- [Shortcodes](https://pagecli.dev/docs/shortcodes)
- [i18n](https://pagecli.dev/docs/i18n)
- [Deployment](https://pagecli.dev/docs/deployment)
- [CLI Reference](https://pagecli.dev/docs/cli-reference)

## License

MIT
