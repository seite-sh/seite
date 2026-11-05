---
title: Search Improvements
description: "Full-text, per-language search on any static host, with collection filtering and no external service"
tags:
- done
weight: 8
---

Shipped in v0.21.0. With `[build] search = "full"` (the default for new sites) the build writes a sharded full-text index, and the bundled search box downloads only the shards for the typed words. No service, API key or Node.

## Improvements

- **Full-text search**: page bodies are indexed, not just titles and descriptions; a query on a 1,000-page site downloads well under 100 KB
- **Ranking**: all words must match, the last as a prefix while typing; titles rank above headings, tags and descriptions, those above body text
- **i18n-aware search**: one index per language; English matches word forms, CJK is matched by character pairs
- **Collection filtering**: `{% set search_collections = "docs" %}` limits a search box to some collections
- **Accessible results**: highlighted snippets, keyboard navigation, result count announced to screen readers

Not planned for now: tag facets, Pagefind integration and search analytics.
