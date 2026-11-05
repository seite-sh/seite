---
title: Theme Community Ecosystem
description: Curated theme registry, browse command, validation, and contributor guide
tags:
- in-progress
weight: 7
---

Build discoverability and a contributor on-ramp for community themes. Theme sharing infrastructure is partially complete: install from URL and export are shipped.

## Completed

- **`seite theme install <url>`**: download and install `.tera` themes from any URL
- **`seite theme export <name>`**: package the current theme as a shareable `.tera` file with metadata
- **Theme metadata**: `{#- theme-description: ... -#}` convention in theme files
- **AI-generated themes**: `/seite theme <description>` in any supported coding agent (or `seite theme create "<description>"`) designs a custom theme
- **Starters and gallery** (v0.21.0): 4 starter themes (default, docs, landing, blank) plus a 7-theme gallery, listed by `seite theme list`

## Remaining

- **Curated theme registry**: `themes.json` listing community themes with name, description, author, install URL, and preview
- **`seite theme browse`**: fetch and display available community themes with filtering
- **`seite theme validate`**: check a `.tera` file for required blocks, SEO meta, accessibility, and search JS before publishing
- **GitHub discovery**: `seite-theme` topic, template repo, naming conventions
- **Community showcase**: gallery on seite.sh with live previews and one-click install
- **Contributor guide**: docs page explaining how to create, test, validate, and submit themes
