---
title: page
description: AI-native static site generator. Ships llms.txt, markdown alongside HTML, and a built-in Claude Code agent. Built with Rust.
---

# Build sites that speak to humans and machines

**page** is a static site generator designed for the AI era. Every page ships as HTML for browsers, markdown for LLMs, and structured data for search engines. No configuration required — it just works.

```bash
cargo install page
page init mysite --collections posts,docs,pages
cd mysite
page serve
```

---

## Why page?

### AI-Native Output

Every page generates `llms.txt`, `llms-full.txt`, and raw markdown files alongside HTML. Your content is ready for LLM consumption from day one.

### Built-in AI Agent

Run `page agent` to launch an interactive Claude Code session with full site context — your config, content inventory, templates, and CLI commands are all available. Or pass a prompt directly: `page agent "write a post about Rust error handling"`.

### Six Bundled Themes

Ship with **default**, **minimal**, **dark**, **docs**, **brutalist**, and **bento** themes. Apply with one command or generate a completely custom theme with AI: `page theme create "coral brutalist with lime accents"`.

### Complete SEO + GEO

Canonical URLs, Open Graph, Twitter Cards, JSON-LD structured data, XML sitemap, RSS feeds, and per-page robots directives — all generated automatically from your content.

### Multi-Language Support

Filename-based translations: `about.md` for English, `about.es.md` for Spanish. Per-language URLs, hreflang tags, RSS feeds, search indexes, and discovery files — all automatic.

### Deploy Anywhere

GitHub Pages, Cloudflare Pages, or Netlify. One command: `page deploy`. Includes `--dry-run` to preview changes and auto-generated GitHub Actions workflows.

---

## Quick Start

**1. Create a site**

```bash
page init mysite --title "My Site" --collections posts,docs,pages
cd mysite
```

**2. Write content**

```bash
page new post "Hello World" --tags intro,welcome
```

**3. Build and preview**

```bash
page build    # Generates dist/ with HTML, markdown, RSS, sitemap, llms.txt
page serve    # Dev server with live reload at localhost:3000
```

**4. Deploy**

```bash
page deploy   # Push to GitHub Pages, Cloudflare, or Netlify
```
