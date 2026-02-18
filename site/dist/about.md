---
title: About
description: page is an AI-native static site generator built with Rust. Zero runtime JavaScript, six bundled themes, single binary.
---

## About page

**page** is a static site generator built with Rust, designed from the ground up for the AI era.

### Design Philosophy

**AI-native output.** Every page generates HTML for browsers, markdown for LLMs, and structured data for search engines. Your site ships with `llms.txt` and `llms-full.txt` discovery files so AI systems can understand your content.

**Zero runtime JavaScript.** Search, navigation, and theme switching work without JavaScript frameworks. The only JS is a lightweight search script that lazy-loads when the search input is focused.

**Single binary.** All six themes are compiled into the binary. No downloads, no node_modules, no build dependencies. `cargo install page` and you're ready.

**Convention over configuration.** Three collection presets (posts, docs, pages) cover most use cases. Frontmatter is YAML. Templates are Tera (Jinja2-compatible). Dates are parsed from filenames. Tags generate archive pages. It just works.

### Built With

- **Rust** — fast builds, reliable deploys, single binary distribution
- **Tera** — Jinja2-compatible templates
- **pulldown-cmark** — CommonMark markdown parsing
- **syntect** — syntax highlighting (base16-ocean.dark theme)
- **image** — image resizing and WebP conversion
- **Claude Code** — AI agent integration for content creation and theme generation

### Open Source

page is open source. Contributions, issues, and feedback are welcome.

[View on GitHub](https://github.com/sanchezomar/page)
