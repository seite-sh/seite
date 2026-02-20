---
title: "Configuration"
description: "Complete seite.toml reference — site settings, collections, build options, deployment, languages, and images."
weight: 2
---

## Overview

All configuration lives in `seite.toml` at the project root. Here's a complete example:

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

[analytics]
provider = "google"
id = "G-XXXXXXXXXX"
cookie_consent = true
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
| `auto_commit` | bool | `true` | Auto-commit and push before deploying. On non-main branches, auto-uses preview mode |

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

Optional. When this section is present, `page` automatically processes images in `static/`. When omitted, images are copied as-is with no resizing or rewriting. New projects created with `seite init` include this section by default.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `widths` | array | `[480, 800, 1200]` | Target widths in pixels for resized copies |
| `quality` | int | `80` | JPEG/WebP quality (1-100) |
| `webp` | bool | `true` | Generate WebP variants |
| `lazy_loading` | bool | `true` | Add `loading="lazy"` to `<img>` tags |

When configured, images in `static/` are resized to each width, optionally converted to WebP, and `<img>` tags in HTML are rewritten with `srcset` and `<picture>` elements. To disable image processing, remove the `[images]` section entirely.

## [analytics]

Optional. When present, analytics scripts are automatically injected into every HTML page during build. Supports Google Analytics 4, Google Tag Manager, Plausible, Fathom, and Umami.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `provider` | string | required | Analytics provider: `google`, `gtm`, `plausible`, `fathom`, `umami` |
| `id` | string | required | Measurement/tracking ID (e.g., `G-XXXXXXX`, `GTM-XXXXX`, domain, or site ID) |
| `cookie_consent` | bool | `false` | Show a cookie consent banner and gate analytics on user acceptance |
| `script_url` | string | varies | Custom script URL (required for self-hosted Umami, optional for others) |

### Examples

**Google Analytics 4 (direct):**

```toml
[analytics]
provider = "google"
id = "G-XXXXXXXXXX"
```

**Google Analytics 4 with cookie consent banner:**

```toml
[analytics]
provider = "google"
id = "G-XXXXXXXXXX"
cookie_consent = true
```

**Google Tag Manager:**

```toml
[analytics]
provider = "gtm"
id = "GTM-XXXXXXX"
cookie_consent = true
```

**Plausible Analytics (privacy-friendly, no cookies):**

```toml
[analytics]
provider = "plausible"
id = "example.com"
```

**Fathom Analytics:**

```toml
[analytics]
provider = "fathom"
id = "ABCDEF"
```

**Self-hosted Umami:**

```toml
[analytics]
provider = "umami"
id = "abc-def-123"
script_url = "https://stats.example.com/script.js"
```

{{% callout(type="tip") %}}
Privacy-respecting analytics like Plausible, Fathom, and Umami don't use cookies. You can typically use them without a consent banner (`cookie_consent = false`). Google Analytics and GTM set cookies and may require consent under GDPR/ePrivacy — set `cookie_consent = true` for those.
{{% end %}}

### Cookie consent banner

When `cookie_consent = true`, a fixed-position banner appears at the bottom of the page on the visitor's first visit. Analytics scripts only load after the visitor clicks "Accept". The choice is stored in `localStorage` so it persists across visits. The banner is fully accessible with `role="dialog"`, keyboard-navigable buttons, and responsive design.

### How it works

Analytics injection happens as a post-processing step after the main build. Every `.html` file in the output directory is rewritten:

- **Without consent:** the analytics `<script>` tag is injected before `</head>`. GTM also gets a `<noscript>` fallback after `<body>`.
- **With consent:** a consent banner with Accept/Decline buttons is injected before `</body>`. Analytics scripts are loaded dynamically only after the user accepts.

To remove analytics, delete the `[analytics]` section from `seite.toml`.

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

## seite-workspace.toml

For multi-site setups, a `seite-workspace.toml` at the workspace root configures all sites. Each site still has its own `seite.toml`.

```toml
[workspace]
name = "my-workspace"
shared_data = "data"           # Shared data directory (optional)
shared_static = "static"       # Shared static assets (optional)
shared_templates = "templates" # Shared templates (optional)

[[sites]]
name = "blog"
path = "sites/blog"
# base_url = "https://blog.example.com"  # Override site base_url
# output_dir = "dist/blog"               # Override output location

[[sites]]
name = "docs"
path = "sites/docs"

[cross_site]
unified_sitemap = false    # Combine all sites into one sitemap
cross_site_links = false   # Validate links across sites
unified_search = false     # Combined search index
```

See [Workspaces](/docs/workspace) for the full guide.

## Next Steps

- [Collections](/docs/collections) — configure how posts, docs, and pages behave
- [Templates & Themes](/docs/templates) — use config values and data files in your templates
- [Deployment](/docs/deployment) — deploy with the settings you've configured
- [Workspaces](/docs/workspace) — manage multiple sites in a single repository
