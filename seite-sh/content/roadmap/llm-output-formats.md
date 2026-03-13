---
title: LLM-Ready Output Formats
description: "llms.txt, llms-full.txt, markdown alternates, and structured discovery files alongside HTML"
tags:
- done
weight: 24
---

Every page generates a `.md` file alongside its `.html` for LLM consumption. Site-wide `llms.txt` (summary per llmstxt.org spec) and `llms-full.txt` (full content in markdown) are generated automatically. Also ships: `robots.txt` with AI crawler directives, `sitemap.xml` with hreflang alternates, `search-index.json`, and `asset-manifest.json` when fingerprinting is enabled.
