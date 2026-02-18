---
title: Configuration
description: Complete page.toml reference — site settings, collections, build options, deployment, languages, and images.
---

## Overview

All configuration lives in `page.toml` at the project root. Here's a complete example:

```toml
[site]
title = "My Site"
description = "A personal blog and documentation"
base_url = "https://example.com"
language = "en"
author = "Jane Doe"

[[collections]]
name = "posts"
paginate = 10

[[collections]]
name = "docs"

[[collections]]
name = "pages"

[build]
output_dir = "dist"
minify = true
fingerprint = true

[deploy]
target = "github-pages"

[languages.es]
title = "Mi Sitio"
description = "Un blog personal"

[images]
widths = [480, 800, 1200]
quality = 80
webp = true
lazy_loading = true
```

## [site]

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `title` | string | required | Site title, used in templates and meta tags |
| `description` | string | `""` | Site description for SEO |
| `base_url` | string | `"http://localhost:3000"` | Full base URL for canonical links, sitemap, RSS |
| `language` | string | `"en"` | Default language code |
| `author` | string | `""` | Author name for JSON-LD and RSS |

{{% callout(type="warning") %}}
Set `base_url` to your real domain before deploying. Leaving it as `localhost` will trigger a pre-flight warning and produce incorrect canonical URLs, sitemaps, and RSS feeds.
{{% end %}}

## [[collections]]

Each `[[collections]]` entry defines a content collection. See [Collections](/docs/collections) for full details.

```toml
[[collections]]
name = "posts"
label = "Blog"
directory = "posts"
url_prefix = "/posts"
default_template = "post.html"
has_date = true
has_rss = true
listed = true
nested = false
paginate = 10
```

## [build]

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `output_dir` | string | `"dist"` | Build output directory |
| `data_dir` | string | `"data"` | Directory for data files (YAML/JSON/TOML) |
| `minify` | bool | `false` | Strip CSS/JS comments and collapse whitespace |
| `fingerprint` | bool | `false` | Add content hash to asset filenames for cache busting |

{{% callout(type="tip") %}}
Enable `minify` for production builds — it strips CSS/JS comments and collapses whitespace for smaller files. Enable `fingerprint` when your CDN caches aggressively — content hashes in filenames ensure browsers always fetch the latest version.
{{% end %}}

When `fingerprint = true`, static files get hashed names (e.g., `style.a1b2c3d4.css`) and an `asset-manifest.json` is written to the output directory.

## Data Files

Place YAML, JSON, or TOML files in the `data/` directory to make structured data available in all templates as `{{ data.filename }}`.

### Supported formats

| Extension | Format |
|-----------|--------|
| `.yaml`, `.yml` | YAML |
| `.json` | JSON |
| `.toml` | TOML |

### Nested directories

Subdirectories create nested keys:

```
data/
  nav.yaml          → {{ data.nav }}
  authors.json      → {{ data.authors }}
  menus/
    main.yaml       → {{ data.menus.main }}
    footer.yaml     → {{ data.menus.footer }}
```

### Theme integration

All bundled themes conditionally render `data.nav` for header navigation and `data.footer` for footer links and copyright. Example `data/nav.yaml`:

```yaml
- title: Blog
  url: /posts
- title: About
  url: /about
- title: Docs
  url: /docs
```

Example `data/footer.yaml`:

```yaml
links:
  - title: GitHub
    url: https://github.com/user/repo
  - title: Twitter
    url: https://twitter.com/user
copyright: "2026 My Company"
```

### Conflict detection

The build will error if two data files share the same stem (e.g., `authors.yaml` and `authors.json`) or if a file and directory conflict (e.g., `nav.yaml` and `nav/main.yaml`). Unknown file extensions are skipped with a warning.

## [deploy]

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `target` | string | `"github-pages"` | Deploy target: `github-pages`, `cloudflare`, `netlify` |
| `repo` | string | auto-detected | Git repository URL (GitHub Pages) |
| `project` | string | auto-detected | Project name (Cloudflare Pages) |

## [languages.*]

Optional. Add language sections to enable multi-language support:

```toml
[languages.es]
title = "Mi Sitio"
description = "Un blog personal y documentaci&oacute;n"

[languages.fr]
title = "Mon Site"
```

Each language can override `title` and `description`. See [Multi-language](/docs/i18n) for details.

## [images]

Optional. When this section is present, `page` automatically processes images in `static/`. When omitted, images are copied as-is with no resizing or rewriting. New projects created with `page init` include this section by default.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `widths` | array | `[480, 800, 1200]` | Target widths in pixels for resized copies |
| `quality` | int | `80` | JPEG/WebP quality (1-100) |
| `webp` | bool | `true` | Generate WebP variants |
| `lazy_loading` | bool | `true` | Add `loading="lazy"` to `<img>` tags |

When configured, images in `static/` are resized to each width, optionally converted to WebP, and `<img>` tags in HTML are rewritten with `srcset` and `<picture>` elements. To disable image processing, remove the `[images]` section entirely.

## Frontmatter

Content files use YAML frontmatter between `---` delimiters:

```yaml
---
title: "Page Title"
date: 2026-02-18
updated: 2026-02-19
description: "Used in meta tags, OG, Twitter Cards, JSON-LD"
image: /static/preview.jpg
slug: custom-slug
tags:
  - rust
  - web
draft: true
template: custom.html
robots: noindex
extra:
  hero: true
  color: "#4361EE"
---
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | Yes | Page title |
| `date` | date | Posts only | Publication date (YYYY-MM-DD) |
| `updated` | date | No | Last modified date |
| `description` | string | No | SEO description |
| `image` | string | No | Social preview image |
| `slug` | string | No | Override auto-generated slug |
| `tags` | list | No | Content tags |
| `draft` | bool | No | Exclude from build unless `--drafts` |
| `template` | string | No | Override default template |
| `robots` | string | No | Per-page robots directive |
| `extra` | map | No | Arbitrary data for templates |

## Next Steps

- [Collections](/docs/collections) — configure how posts, docs, and pages behave
- [Templates & Themes](/docs/templates) — use config values and data files in your templates
- [Deployment](/docs/deployment) — deploy with the settings you've configured
