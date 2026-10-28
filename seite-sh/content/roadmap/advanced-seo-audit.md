---
title: Advanced SEO Audit
description: "seite audit command for alt text, duplicate titles, heading levels, thin pages, llms.txt and more"
tags:
- done
weight: 10
---

Shipped in v0.21.0 as `seite audit`: a free, offline SEO, GEO, accessibility, content and performance audit. It builds into a scratch directory (or reads `--dist <dir>`) and reports each finding with a stable code, the page, and the markdown or template line to fix, plus a score. See the [audit docs](/docs/audit).

## Checks

- Missing image alt text
- Duplicate page titles and descriptions, titles over 60 characters, descriptions outside 50 to 160
- Orphaned pages (no internal links pointing to them)
- Missing Open Graph image
- Heading hierarchy issues (skipped levels, more than one `<h1>`)
- Content length warnings (thin pages)
- llms.txt completeness, JSON-LD validity, robots.txt rules for AI search crawlers
- Large images and pages, render-blocking scripts

Broken external links are checked by `seite check --external`, also new in v0.21.0.

## With a coding agent

The audit itself makes no AI calls. Your coding agent runs it through `/seite audit` (or the read-only MCP tool `seite_audit`) and fixes what it reports: writing missing descriptions, alt text and headings from the page content.

`--json`, `--category`, `--min-severity` and `--fail-on error|warning` make it usable in CI.
