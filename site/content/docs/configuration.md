---
title: "Configuration"
description: "Complete page.toml reference — site settings, collections, build options, deployment, languages, and images."
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
| `minify` | bool | `false` | Strip CSS/JS comments and collapse whitespace |
| `fingerprint` | bool | `false` | Add content hash to asset filenames for cache busting |

When `fingerprint = true`, static files get hashed names (e.g., `style.a1b2c3d4.css`) and an `asset-manifest.json` is written to the output directory.

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

Optional. Configure automatic image processing:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `widths` | array | `[]` | Target widths in pixels for resized copies |
| `quality` | int | `80` | JPEG/WebP quality (1-100) |
| `webp` | bool | `false` | Generate WebP variants |
| `lazy_loading` | bool | `false` | Add `loading="lazy"` to `<img>` tags |

When configured, images in `static/` are resized to each width, optionally converted to WebP, and `<img>` tags in HTML are rewritten with `srcset` and `<picture>` elements.

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
