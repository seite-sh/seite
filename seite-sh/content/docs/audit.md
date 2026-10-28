---
title: "Audit"
description: "seite audit is a free, offline SEO, AI-visibility, accessibility, content and performance audit of your built site, with the source file to fix per finding."
weight: 11
---

`seite audit` checks the built site the way a paid SEO crawler would, for free and without network access: titles and descriptions, headings, thin and orphan pages, broken `#anchors`, the sitemap, hreflang, how visible the site is to AI assistants (llms.txt, markdown copies, JSON-LD, robots.txt), accessibility basics, links to drafts and scheduled posts, and page weight. Every finding has a stable code, the page it is on, and the file to edit (the markdown source or the template), so a coding agent can fix it on its own.

```bash
seite audit                       # human report: score, counts per category, top findings
seite audit --json                # everything, for agents and scripts
seite audit --category seo,geo    # only some categories (repeatable or comma-separated)
seite audit --fail-on error       # exit 1 on any error (CI); --fail-on warning is stricter
seite audit --dist dist           # audit an existing output dir instead of building
seite audit --min-severity warning  # leave notices out of the list (still counted)
```

Like `seite check`, the audit builds the site into a temporary directory: `dist/` is never created or changed. Drafts are not built, and scheduled (future-dated) posts are hidden exactly as in `seite build`. With `--dist <dir>`, it reads that directory instead; findings then point at the generated files (`dist/posts/a.html:42`) because the link back to the sources is only known during a build, and the `content` checks (which need the sources) are skipped.

The audit reports; it never fails unless you pass `--fail-on`. Fix what it finds, then run it once more to confirm.

## Output

The human report starts with the score and counts, then lists the top findings per category (errors first; `--top N` changes how many, default 10):

```text
Audit: score 97/100 — 0 errors, 1 warning, 2 notices (6 pages)

SEO — 0 errors, 1 warning, 2 notices
  warning[seo-title-duplicate] /: <title> "My Site" is also used by /posts/ (and 1 more page)
      --> templates/base.html:7
      fix: make each page's title unique; search engines show one result per title
```

`--json` (and the MCP tool `seite_audit`) return one document with a stable shape:

```json
{
  "summary": {
    "score": 97, "errors": 0, "warnings": 1, "notices": 2, "pages": 6,
    "by_category": { "seo": { "errors": 0, "warnings": 1, "notices": 2 }, "geo": {...}, "a11y": {...}, "content": {...}, "perf": {...} }
  },
  "findings": [
    {
      "code": "seo-thin-content", "severity": "warning", "category": "seo",
      "page_url": "/posts/hello", "file": "content/posts/hello.md", "line": 1,
      "message": "42 words (under 150)",
      "hint": "expand the page to at least 150 words, merge it into a related page, or mark it a draft",
      "docs_url": "https://seite.sh/docs/errors#seo-thin-content"
    }
  ],
  "omitted": 0
}
```

Every key is always present: `page_url` is `null` for site-wide findings (robots.txt, sitemap, llms.txt), `file` and `line` are `null` when unknown. Findings are sorted by severity (errors first), then category, code, page, file and line, so two runs on the same site give the same output. `--min-severity error|warning|notice` (default `notice`) lists only findings of that severity or worse; the `summary` still counts everything, and `omitted` says how many were left out. A problem that comes from a template and repeats on many pages is reported once, on the first page, with "(and N more pages)".

`seite explain <code>` prints the explanation of any code; the [error reference](/docs/errors) lists them all.

## Score

`score = 100 − Σ penalty`, clamped to 0–100. Each finding costs its severity's weight — **error 10, warning 3, notice 0** (notices are suggestions: reported, never scored) — at most **3 findings per code** count, and one category costs at most **30**. One kind of problem on every page can't sink the score on its own, and fixing a kind of problem always raises it. A page without a description gets `seo-description-missing` only, not also `geo-llms-no-summary`. With `--category`, the score covers the selected categories only.

Starter content passes every check above notice level. On a fresh `seite init` site the bundled themes still title the posts index like the homepage (`seo-title-duplicate`), and tag pages and the trust index have notices and an ambiguous "Learn more" link; the themes are being fixed.

## Checks

| Code | Severity | What it checks |
|---|---|---|
| **SEO** | | |
| `seo-title-missing` | error | `<title>` present and not empty |
| `seo-title-duplicate` | warning | titles unique across indexable pages |
| `seo-title-long` | warning | `<title>` at most 60 characters |
| `seo-description-missing` | warning | content pages have `description:` |
| `seo-description-duplicate` | warning | descriptions unique |
| `seo-description-long` | warning | description at most 160 characters |
| `seo-description-short` | notice | description at least 50 characters |
| `seo-canonical-missing` | warning | `<link rel="canonical">` |
| `seo-h1-missing` / `seo-h1-multiple` | warning | exactly one `<h1>` in `<main>` (a site-title `<h1>` in the header is not counted) |
| `seo-heading-skip` | notice | no skipped heading levels in `<main>` |
| `seo-thin-content` | warning | content pages have at least 150 words (changelog and roadmap entries exempt) |
| `seo-orphan-page` | warning | some other page links to each content page |
| `seo-broken-anchor` | warning | `#fragment` links match an `id` on the target page |
| `seo-noindex-in-sitemap` | error | no `noindex` page in `sitemap.xml` |
| `seo-img-dimensions` | warning | `<img>` has `width` and `height` (layout shift) |
| `seo-og-image-missing` | notice | content pages have an `og:image` (generated `[og]` cards count) |
| `seo-hreflang` | warning | hreflang targets exist and link back |
| `seo-sitemap-missing` / `seo-sitemap-stale-url` | error | `sitemap.xml` exists and every URL in it was built |
| `seo-page-not-in-sitemap` | warning | every indexable content page is in the sitemap |
| **GEO (AI visibility)** | | |
| `geo-llms-txt-missing` | error | `llms.txt` exists |
| `geo-llms-txt-unlisted` | warning | every content page (default language, not private) is in `llms.txt` |
| `geo-llms-no-summary` | notice | `llms.txt` entries have a summary (from `description:`) |
| `geo-markdown-alternate-missing` | warning | content pages link their `.md` copy |
| `geo-jsonld-missing` / `geo-jsonld-invalid` / `geo-jsonld-no-type` | warning / error / warning | JSON-LD present, parses, and has `@type` |
| `geo-robots-missing` | warning | `robots.txt` exists |
| `geo-robots-blocks-ai-search` | warning | OAI-SearchBot, ChatGPT-User, PerplexityBot and Claude-SearchBot may fetch `/` |
| `geo-content-signal-missing` | notice | `robots.txt` has a `Content-Signal:` line |
| **Accessibility** | | |
| `a11y-html-lang` | error | `<html lang>` set |
| `a11y-img-alt` | error | every `<img>` has `alt` (`alt=""` is fine for decorative images) |
| `a11y-link-empty` | error | links have text or `aria-label` |
| `a11y-link-ambiguous` | warning | no bare "click here" / "read more" / "learn more" links (unless the same target is also linked descriptively) |
| `a11y-input-label` | error | form controls have a label |
| `a11y-button-name` | error | buttons have text or `aria-label` |
| `a11y-skip-link` | notice | a skip-to-content link |
| `a11y-duplicate-id` | warning | ids unique per page |
| **Content** (needs a build, not `--dist`) | | |
| `content-link-to-draft` | error | no links to drafts |
| `content-link-to-scheduled` | warning | no links to scheduled or expired pages |
| `content-missing-date` | warning | pages in dated collections have a date |
| **Performance** | | |
| `perf-image-large` | warning | images at most 300 KB (with `srcset`, the smallest candidate counts) |
| `perf-page-weight` | warning | HTML plus local CSS, JS and images at most 1.5 MB |
| `perf-render-blocking-script` | warning | `<script src>` in `<head>` has `defer` or `async` |
| `perf-img-lazy` | notice | images in `<main>` after the first have `loading="lazy"` (header images are above the fold) |

"Content pages" are pages rendered from a markdown file (not listings, tag pages, the homepage or alias redirects); with `--dist` they are the non-listing pages that link a `.md` copy. Alias redirect pages and `404.html` are not audited.

Not checked: broken links and missing files (that is `seite check`; `seite check --external` checks outside links), remote assets (nothing is downloaded), subdomain collections' separate outputs, and FAQ-style content without FAQ schema (too noisy to guess offline).

## In CI

```yaml
- run: seite audit --fail-on error
```

`--fail-on error` fails on any error-level finding, `--fail-on warning` on warnings too. In `--json` mode the report is still printed (with `"ok": false`).

## For agents

The `/seite` skill routes `/seite audit` to a short procedure: run `seite audit --json`, fix each finding in the file it names (content first, then templates), and run the audit once more. MCP clients call `seite_audit` (read-only) with optional `categories`, `top` (default 50) and `min_severity` (default `warning`); the summary always counts every finding.
