---
title: Visual Editing Mode
description: "Browser-based content editor — edit frontmatter and markdown with live preview, no terminal required"
tags:
- shipped
weight: 5
---

The #1 pain point across all SSG communities: editing content without touching a terminal, YAML frontmatter, or git. This is the largest addressable audience gap between SSGs and WordPress/AI app builders.

**Status: shipped in v0.10.1.** Run `seite edit` to open a browser-based visual editor with:

- **Collection file browser** — sidebar listing all content files with draft/published indicators
- **Frontmatter form** — edit title, date, description, tags, image, slug, template, weight, and draft status via form fields
- **Markdown editor** — monospace textarea with tab support and live word count
- **Live preview** — side-by-side iframe showing the built site, auto-refreshes on save
- **Full CRUD** — create new content in any collection, save edits, delete files

We chose the **hybrid approach**: a lightweight web UI for frontmatter and content editing. The AI-native editing story continues through `seite agent` and MCP — the visual editor complements rather than replaces those workflows.
