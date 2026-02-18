---
title: "Introducing page"
date: 2026-02-18
description: "page is a new static site generator built with Rust, designed for the AI era. Ships with llms.txt, markdown output, and a built-in Claude Code agent."
tags:
  - release
  - rust
---

Today we're launching **page**, a static site generator designed for the AI era.

## What makes page different?

Most static site generators treat HTML as the primary output. page treats every page as multi-format: HTML for browsers, markdown for LLMs, and structured data for search engines.

Every site built with page ships with:

- **llms.txt** and **llms-full.txt** — discovery files for AI crawlers
- **Raw markdown** alongside every HTML file
- **JSON-LD** structured data on every page
- **XML sitemap** with hreflang alternates for translations

## Built-in AI agent

Run `page agent` to get an AI assistant that understands your entire site. It knows your config, content inventory, templates, and available commands. Ask it to write blog posts, create documentation, or generate custom themes.

```bash
page agent "write a blog post about Rust error handling"
```

Or start an interactive session:

```bash
page agent
```

## Six bundled themes

page ships with six themes compiled directly into the binary: default, minimal, dark, docs, brutalist, and bento. Apply one with a single command:

```bash
page theme apply dark
```

Or generate a completely custom theme with AI:

```bash
page theme create "coral brutalist with lime accents"
```

## Everything you need

- Multi-language support with filename-based translations
- Client-side search (no server required)
- Image processing with automatic WebP conversion and srcset
- Deploy to GitHub Pages, Cloudflare Pages, or Netlify
- CSS/JS minification and asset fingerprinting
- Pagination, tag pages, table of contents, reading time
- Full SEO: canonical URLs, Open Graph, Twitter Cards, RSS feeds

## Get started

```bash
cargo install page
page init mysite --collections posts,docs,pages
cd mysite
page serve
```

Read the [Getting Started guide](/docs/getting-started) for a complete walkthrough.
