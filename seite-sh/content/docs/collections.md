---
title: "Content Collections"
description: "Configure content collections in your seite static site: posts, docs, pages, changelog, roadmap, and trust center, with pagination, RSS, and subdomain support."
weight: 3
---

Collections are the core content model in seite. The static site generator ships six collection presets that handle common patterns, from date-ordered blog posts to nested documentation, so you can start writing content immediately.

## Overview

Collections are groups of related content. Each collection has its own directory under `content/`, URL prefix, and template. Six presets are available:

| Preset | Directory | Dated | RSS | Listed | Nested | URL Pattern |
|--------|-----------|-------|-----|--------|--------|-------------|
| posts | `content/posts/` | Yes | Yes | Yes | No | `/posts/slug` |
| docs | `content/docs/` | No | No | Yes | Yes | `/docs/slug` |
| pages | `content/pages/` | No | No | No | No | `/slug` |
| changelog | `content/changelog/` | Yes | Yes | Yes | No | `/changelog/slug` |
| roadmap | `content/roadmap/` | No | No | Yes | No | `/roadmap/slug` |
| trust | `content/trust/` | No | No | Yes | Yes | `/trust/slug` |

## Defining Collections

Collections are configured in `seite.toml` (created during [`seite init`](/docs/getting-started)):

```toml
[[collections]]
name = "posts"
label = "Posts"
directory = "posts"
has_date = true
has_rss = true
listed = true
url_prefix = "/posts"
nested = false
default_template = "post.html"

[[collections]]
name = "pages"
label = "Pages"
directory = "pages"
has_date = false
has_rss = false
listed = false
url_prefix = ""
nested = false
default_template = "page.html"
```

`name`, `label`, `directory` and `default_template` are required in every entry; a `[[collections]]` table with only `name` fails with `config-invalid: missing field`. `seite init --collections …` and `seite collection add <preset>` write each preset's full table, so you rarely type one by hand. Edit any field, or add one:

```toml
[[collections]]
name = "posts"
label = "Blog"
directory = "posts"
has_date = true
has_rss = true
listed = true
url_prefix = "/posts"
default_template = "post.html"
paginate = 10
```

## Collection Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | required | Collection identifier |
| `label` | string | required | Display name in templates |
| `directory` | string | required | Content directory under `content/` (relative, no `..`) |
| `url_prefix` | string | `""` | URL prefix for items (`""` serves them at the site root, like pages) |
| `default_template` | string | required | Template file for rendering |
| `has_date` | bool | `false` | Items have dates |
| `has_rss` | bool | `false` | Include in RSS feed |
| `listed` | bool | `false` | Show on index page |
| `private` | bool | `false` | Build the hub + pages but hide them from all discovery (sitemap, llms.txt, search, feeds, social cards). With [`[access]`](/docs/configuration#access) the pages are also password-protected on Cloudflare Pages. See [Private collections](/docs/configuration#private-collections) |
| `access_group` | string | collection name | Password group of a private collection when `[access]` is set; collections with the same group share a password |
| `nested` | bool | `false` | Support subdirectories as groups |
| `paginate` | int | none | Items per page (enables pagination) |
| `subdomain` | string | none | Deploy to `{subdomain}.{base_domain}` |
| `subdomain_base_url` | string | none | Explicit URL override for subdomain (e.g., `https://docs.example.com`) |
| `deploy_project` | string | none | Cloudflare/Netlify project for subdomain |
| `related` | int | `[build] related` (3) | Maximum related pages per item; `0` disables `page.related` for this collection |
| `fields` | tables | preset-based | Typed frontmatter schema: `[collections.fields.<name>]` with `type`, `required`, `enum`, `min`/`max`, `pattern`, `default`, `description`. See [Frontmatter schemas](/docs/configuration#frontmatter-schemas) |
| `strict_fields` | bool | `false` | Warn about frontmatter keys the schema does not declare |

`seite init` and `seite collection add` write a small schema for each preset (for example posts: `description` a string, `image`, `tags` a list of strings; docs: `weight` an integer). Nothing in a preset is required and presets set no length bounds. In a list of strings, numbers and booleans count as strings (`tags: [2024, rust]`), like the frontmatter parser. Tighten it in `seite.toml`, for example:

```toml
[collections.fields.description]
type = "string"
required = true
max = 160
description = "Summary for search results (50–160 characters)"
```

`seite check` then reports `content/posts/x.md:3: error[frontmatter-schema]: field `description` is too long: 212 characters, at most 160 allowed` with the description as the hint. `seite templates` lists every collection's fields.

## Posts

Posts are date-based content. Filenames should include the date:

```
content/posts/
├── 2026-01-15-hello-world.md
├── 2026-02-01-rust-tips.md
└── 2026-02-18-new-feature.md
```

The date is parsed from the filename (`YYYY-MM-DD-slug.md`) or from frontmatter. Posts are sorted by date (newest first) and included in the RSS feed.

With `[build] schedule = true` (set by `seite init`), a post dated in the future is scheduled: it stays out of the build until its date, and `expires: YYYY-MM-DD` takes a post down after that day. The generated GitHub Actions workflow rebuilds daily so scheduled posts go live on their own. See [Scheduled publishing](/docs/configuration#scheduled-publishing).

{{% callout(type="tip") %}}
`seite serve` always includes draft posts, so you can preview them during development. Drafts are excluded from `seite build` unless you pass `--drafts`.
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

Nested docs get URLs like `/docs/guides/setup`: the docs theme shows a sidebar with sections grouped by directory.

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

- `new`: new features (green)
- `fix`: bug fixes (blue)
- `breaking`: breaking changes (red)
- `improvement`: enhancements (purple)
- `deprecated`: deprecations (gray)

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

- `planned`: upcoming work
- `in-progress`: actively being worked on
- `done`: completed
- `cancelled`: no longer planned

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
# label, directory, default_template, … as written by the preset
subdomain = "docs"           # → docs.example.com
deploy_project = "my-docs"   # optional: Cloudflare/Netlify project name
```

When `subdomain` is set:

- The collection gets its own output directory: `dist-subdomains/docs/`
- Its base URL becomes `https://docs.{base_domain}`
- Content is served at the subdomain root (no URL prefix)
- It gets its own sitemap, RSS, robots.txt, llms.txt, and search index
- It's excluded from the main site's build output

### Subdomain Root Page

To customize what appears at the subdomain root (e.g., `docs.example.com/`), create `content/{collection}/index.md`:

```
content/docs/index.md    → docs.example.com/  (subdomain root)
```

The subdomain root renders with the collection's **own index template** and context — exactly like the collection's index page on the main domain (e.g. a `trust` subdomain root renders `trust-index.html` with its cert grid, subprocessor table, and FAQ; a `docs` subdomain root renders `docs-index.html`). Adding `content/{collection}/index.md` injects its content into that template as `{{ page.content }}`, exactly like `content/pages/index.md` works for the main site homepage — useful for a custom hero or intro above the listing.

If your `base_url` contains `www` (e.g., `https://www.example.com`), the auto-derived URL would be `docs.www.example.com`. Use `subdomain_base_url` to set an explicit override:

```toml
[[collections]]
name = "docs"
# label, directory, default_template, … as before
subdomain = "docs"
subdomain_base_url = "https://docs.example.com"  # explicit override
deploy_project = "my-docs"
```

The dev server previews subdomain content at `/{name}-preview/` (e.g., `localhost:3000/docs-preview/`).

Deploy with `seite deploy`: subdomain collections are deployed automatically after the main site. GitHub Pages does not support per-collection subdomains; use Cloudflare Pages or Netlify. See the [Deployment guide](/docs/deployment) for platform-specific setup.

## Collection Index Pages

Any collection can have a custom index page by creating `content/{collection}/index.md`. This content is injected into the collection's index template as `{{ page.content }}`:

```
content/docs/index.md        → /docs/     (collection index)
content/changelog/index.md   → /changelog/ (collection index)
```

This works identically to how `content/pages/index.md` provides content for the site homepage. The `index.md` is extracted from the collection. It won't appear as a regular item in listings. All frontmatter fields are available in the template context (`page.title`, `page.description`, `page.extra`, etc.).

For paginated collections, the `index.md` content appears only on page 1.

### Docs Index with Sidebar

The docs collection ships a dedicated `docs-index.html` template that renders the same sidebar navigation as individual doc pages. When using the docs theme:

- **With `index.md`**: Shows your custom content alongside the sidebar
- **Without `index.md`**: Auto-generates a section overview grouped by subdirectory

The sidebar nav (`{{ nav }}`) is available in all collection index templates, so custom templates can also render it.

### Redirect to a Specific Page

Instead of showing custom content, you can redirect the collection index to an existing page using `redirect_to` in the `extra` field:

```yaml
---
title: Docs
extra:
  redirect_to: /docs/getting-started
---
```

This generates an instant redirect at `/docs/` → `/docs/getting-started`. This is the most common pattern for docs sites: route the docs landing page directly to the first doc.

Redirects work for both regular collection indexes and subdomain roots.

## Pagination

Enable pagination on any listed collection:

```toml
[[collections]]
name = "posts"
# label, directory, default_template, … as before
paginate = 10
```

This generates:
- `/posts/`: page 1
- `/posts/page/2/`: page 2
- `/posts/page/3/`: page 3, etc.

Templates receive pagination context:

```
{{ pagination.current_page }}
{{ pagination.total_pages }}
{{ pagination.prev_url }}
{{ pagination.next_url }}
```

## Singular/Plural

Both `seite new post` and `seite new posts` work. The CLI normalizes singular to plural automatically.

## Tag Pages

Tags are collected from all posts and generate archive pages:

- `/tags/`: tag index with all tags and counts
- `/tags/rust/`, all posts tagged "rust"

Tag pages are i18n-aware and included in the sitemap.

## Categories and Other Taxonomies

Add a `[[taxonomies]]` entry to `seite.toml` (see [Configuration](/docs/configuration#taxonomies)) and assign terms with a frontmatter key of the same name:

```yaml
---
title: "Borrowing explained"
tags: [rust, beginners]
categories: [Programming]
---
```

This generates `/categories/`, `/categories/programming/`, and per-term feeds at `/categories/programming/feed.xml` and `atom.xml`. The default post template lists the categories next to the tags.

## Related Pages

Every content page gets `page.related`: up to 3 other pages from the same collection and language that share terms with it. Each shared tag scores 1 and each shared term of any other taxonomy (such as a category) scores 2. Ties go to the page closest in date (undated pages last), then by title. Drafts and pages sharing no terms are never related. The default post template shows them as a "Related" list.

On large collections (more than 200 pages in one language), a term carried by more than 20% of the pages, such as a `blog` tag on every post, does not by itself make pages related; it still adds to the score of pages that share another term. This keeps builds fast.

Set `[build] related = 5` to change the size site-wide, or `related = 0` on a collection (or under `[build]`) to turn it off.

## Series

Give the parts of a multi-part article the same `series` name:

```yaml
---
title: "Ownership"
date: 2026-03-01
series: "Rust from scratch"
series_order: 2   # optional
---
```

`series` may also be a list (Hugo style); its first entry is used. `series_order` accepts a number or a numeric string. Other shapes are ignored.

Parts are ordered by `series_order` (parts that have one come first), then by date (oldest first), then by file path. Each part gets `page.series` (name, URL, all parts, its position, previous and next part), and the default post template shows a "Part 2 of 5" box with links to the neighboring parts. Each series gets an index page at `/series/rust-from-scratch/` listing its parts in order (per language, in the sitemap). Pages in private collections never join a series. Nothing is generated when no page uses `series`.

`/series/` URLs are reserved for series index pages: avoid a collection or page whose URL starts with `/series/`.

## Next Steps

- [Configuration](/docs/configuration): full reference for all collection fields and site settings
- [Templates & Themes](/docs/templates): customize how collections are rendered
- [Shortcodes](/docs/shortcodes): add rich content like videos, callouts, and figures to your pages
