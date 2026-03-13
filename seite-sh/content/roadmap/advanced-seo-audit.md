---
title: Advanced SEO Audit
description: "seite audit command for missing alt text, broken links, duplicate titles, and AI-generated meta"
tags:
- planned
weight: 10
---

A standalone `seite audit` command that goes beyond the existing build-time link checking. Modeled on the expectations set by WordPress Yoast but tailored for static sites.

### Checks

- Missing image alt text
- Duplicate page titles and descriptions
- Orphaned pages (no internal links pointing to them)
- Missing Open Graph or Twitter Card meta
- Heading hierarchy issues (skipped levels)
- Content length warnings (thin pages)
- Broken external links (async HTTP HEAD checks)

### AI-powered (via seite agent)

- Auto-generate missing meta descriptions from content
- Suggest alt text for images
- Keyword density and readability scoring

seite already has deep SEO built into all themes (JSON-LD, Open Graph, canonical URLs, hreflang, robots.txt). The SEOMachine skill pack covers some of this. This feature surfaces it as a first-class CLI command.
