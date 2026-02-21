---
title: Releases
description: Release history and changelog for page.
weight: 11
---

## v0.1.0

Initial release.

**Build pipeline:**
- 13-step build pipeline producing HTML, markdown, RSS, sitemap, search index, and LLM discovery files
- Image processing: auto-resize, WebP conversion, srcset, lazy loading
- Asset pipeline with CSS/JS minification and fingerprinted filenames
- Internal link validation at build time

**Content:**
- 6 collection presets: posts, docs, pages, changelog, roadmap, trust center
- 5 built-in shortcodes: youtube, vimeo, gist, callout, figure — plus user-defined shortcodes
- Data files (YAML/JSON/TOML) injected into template context
- Multi-language (i18n) with per-language URLs, RSS feeds, sitemaps, and hreflang tags
- Pagination, tag pages, table of contents, excerpts, reading time and word count

**Themes:**
- 6 bundled themes: default, minimal, dark, docs, brutalist, bento
- AI-generated custom themes via `seite theme create`
- Theme install/export for community sharing

**AI integration:**
- `seite agent` spawns Claude Code with full site context
- MCP server (`seite mcp`) with 5 tools and 6+ resources for AI tool integration
- Every site ships with `llms.txt`, `llms-full.txt`, and raw markdown

**Deploy:**
- GitHub Pages, Cloudflare Pages, and Netlify with guided setup
- Pre-flight checks, `--dry-run` preview, custom domain management
- Post-deploy verification, preview/staging deploys

**Developer experience:**
- Interactive dev server with REPL and live reload
- Analytics support (Google, GTM, Plausible, Fathom, Umami) with cookie consent
- Multi-site workspaces
- `seite collection add` for adding collections to existing sites
- Self-update from GitHub Releases with checksum verification
- Shell installer for macOS/Linux, PowerShell installer for Windows
