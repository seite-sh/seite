---
title: "Collections"
description: "Configure content collections — posts, docs, pages, changelog, roadmap, and trust center — with pagination, date handling, and RSS."
weight: 3
---

## Overview

Collections are groups of related content. Each collection has its own directory under `content/`, URL prefix, and template. Five presets are available:

| Preset | Directory | Dated | RSS | Listed | Nested | URL Pattern |
|--------|-----------|-------|-----|--------|--------|-------------|
| posts  | `content/posts/` | Yes | Yes | Yes | No | `/posts/slug` |
| docs   | `content/docs/`  | No  | No  | Yes | Yes | `/docs/slug` |
| pages  | `content/pages/` | No  | No  | No  | No | `/slug` |
| changelog | `content/changelog/` | Yes | Yes | Yes | No | `/changelog/slug` |
| roadmap | `content/roadmap/` | No | No | Yes | No | `/roadmap/slug` |
| trust  | `content/trust/`   | No | No | Yes | Yes | `/trust/slug` |

## Defining Collections

Collections are configured in `seite.toml`:

```toml
[[collections]]
name = "posts"

[[collections]]
name = "docs"

[[collections]]
name = "pages"
```

Each preset comes with sensible defaults. You can override any field:

```toml
[[collections]]
name = "posts"
label = "Blog"
paginate = 10
```

## Collection Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | required | Collection identifier |
| `label` | string | capitalized name | Display name in templates |
| `directory` | string | name | Content directory under `content/` |
| `url_prefix` | string | `/name` | URL prefix for items |
| `default_template` | string | preset-based | Template file for rendering |
| `has_date` | bool | preset-based | Items have dates |
| `has_rss` | bool | preset-based | Include in RSS feed |
| `listed` | bool | preset-based | Show on index page |
| `nested` | bool | preset-based | Support subdirectories as groups |
| `paginate` | int | none | Items per page (enables pagination) |
| `subdomain` | string | none | Deploy to `{subdomain}.{base_domain}` |
| `subdomain_base_url` | string | none | Explicit URL override for subdomain (e.g., `https://docs.example.com`) |
| `deploy_project` | string | none | Cloudflare/Netlify project for subdomain |

## Posts

Posts are date-based content. Filenames should include the date:

```
content/posts/
├── 2026-01-15-hello-world.md
├── 2026-02-01-rust-tips.md
└── 2026-02-18-new-feature.md
```

The date is parsed from the filename (`YYYY-MM-DD-slug.md`) or from frontmatter. Posts are sorted by date (newest first) and included in the RSS feed.

{{% callout(type="tip") %}}
Use `seite serve --drafts` to preview draft posts during development. Drafts are excluded from production builds by default.
{{% end %}}

## Docs

Docs support nested directories for grouped navigation:

```
content/docs/
├── getting-started.md
├── guides/
│   ├── setup.md
│   └── advanced.md
└── reference/
    ├── api.md
    └── config.md
```

Nested docs get URLs like `/docs/guides/setup`. The docs theme shows a sidebar with sections grouped by directory.

{{% callout(type="info") %}}
Subdirectories automatically become sidebar sections. Create `content/docs/guides/` and every markdown file inside it appears under a "Guides" heading in the sidebar navigation.
{{% end %}}

### Sidebar Ordering

By default, docs are sorted alphabetically by title. Use `weight` in frontmatter to control the order:

```yaml
---
title: "Getting Started"
weight: 1
---
```

Lower values appear first. Items without `weight` sort alphabetically after all weighted items. This lets you create a guided learning path instead of a plain alphabetical list.

## Pages

Pages are standalone content at root URLs. They're unlisted (not shown on the index page):

```
content/pages/
├── about.md        → /about
├── contact.md      → /contact
└── index.md        → / (homepage content)
```

The special file `content/pages/index.md` injects its content into the homepage template as `{{ page.content }}`.

## Changelog

The changelog collection is for release notes and version history. Entries are date-based with RSS support, so users can subscribe to updates.

```
content/changelog/
├── 2026-01-15-v0-1-0.md
├── 2026-02-01-v0-2-0.md
└── 2026-02-18-v1-0-0.md
```

Use tags to categorize changes. Tags render as colored badges in the changelog templates:

- `new` — new features (green)
- `fix` — bug fixes (blue)
- `breaking` — breaking changes (red)
- `improvement` — enhancements (purple)
- `deprecated` — deprecations (gray)

```bash
seite new changelog "v1.0.0" --tags new,improvement
```

## Roadmap

The roadmap collection is for sharing your project's public roadmap. Items are ordered by `weight` (not date) and grouped by status tags.

```
content/roadmap/
├── dark-mode.md
├── api-v2.md
└── initial-release.md
```

Use tags for status and `weight` for priority ordering within each group:

- `planned` — upcoming work
- `in-progress` — actively being worked on
- `done` — completed
- `cancelled` — no longer planned

```yaml
---
title: "Dark Mode"
tags:
  - planned
weight: 1
---
```

Three index layouts are available. The default groups items by status. To switch layouts, create `templates/roadmap-index.html`:

```html
{# Kanban board (3-column CSS grid): #}
{% extends "roadmap-kanban.html" %}

{# Or timeline (vertical milestones): #}
{% extends "roadmap-timeline.html" %}
```

```bash
seite new roadmap "Feature Name" --tags planned
```

## Subdomains

Any collection can be deployed to its own subdomain. Set `subdomain` on the collection:

```toml
[[collections]]
name = "docs"
subdomain = "docs"           # → docs.example.com
deploy_project = "my-docs"   # optional: Cloudflare/Netlify project name
```

When `subdomain` is set:

- The collection gets its own output directory: `dist-subdomains/docs/`
- Its base URL becomes `https://docs.{base_domain}`
- Content is served at the subdomain root (no URL prefix)
- It gets its own sitemap, RSS, robots.txt, llms.txt, and search index
- It's excluded from the main site's build output

If your `base_url` contains `www` (e.g., `https://www.example.com`), the auto-derived URL would be `docs.www.example.com`. Use `subdomain_base_url` to set an explicit override:

```toml
[[collections]]
name = "docs"
subdomain = "docs"
subdomain_base_url = "https://docs.example.com"  # explicit override
deploy_project = "my-docs"
```

The dev server previews subdomain content at `/{name}-preview/` (e.g., `localhost:3000/docs-preview/`).

Deploy with `seite deploy` — subdomain collections are deployed automatically after the main site. GitHub Pages does not support per-collection subdomains; use Cloudflare Pages or Netlify.

## Pagination

Enable pagination on any listed collection:

```toml
[[collections]]
name = "posts"
paginate = 10
```

This generates:
- `/posts/` — page 1
- `/posts/page/2/` — page 2
- `/posts/page/3/` — page 3, etc.

Templates receive pagination context:

```
{{ pagination.current_page }}
{{ pagination.total_pages }}
{{ pagination.prev_url }}
{{ pagination.next_url }}
```

## Singular/Plural

Both `seite new post` and `seite new posts` work — the CLI normalizes singular to plural automatically.

## Tag Pages

Tags are collected from all posts and generate archive pages:

- `/tags/` — tag index with all tags and counts
- `/tags/rust/` — all posts tagged "rust"

Tag pages are i18n-aware and included in the sitemap.

## Next Steps

- [Configuration](/docs/configuration) — full reference for all collection fields and site settings
- [Templates & Themes](/docs/templates) — customize how collections are rendered
- [Shortcodes](/docs/shortcodes) — add rich content like videos, callouts, and figures to your pages
