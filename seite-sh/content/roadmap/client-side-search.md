---
title: Client-Side Search
description: JSON search index with instant client-side search built into all themes
tags:
- done
weight: 18
---

Build step generates `search-index.json` with title, description, URL, collection, tags, date, and language for every page. Per-language indices at `/{lang}/search-index.json`. All bundled themes include a search box that fetches the index and filters client-side with instant results. v0.21.0 added an optional sharded full-text index (`[build] search = "full"`); see [search improvements](/roadmap/search-improvements).
