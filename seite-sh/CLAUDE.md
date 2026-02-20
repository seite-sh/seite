# page

AI-native static site generator for the modern web

This is a static site built with the `seite` CLI tool.

## SEO and GEO Requirements

> **These are non-negotiable rules for every page on this site.**
> They apply when writing content, creating templates, or asking the AI agent to build or redesign anything.

### Every page `<head>` MUST include

1. **Canonical URL** — `<link rel="canonical" href="{{ site.base_url }}{{ page.url | default(value='/') }}">` (deduplicates indexed URLs)
2. **Open Graph tags** — `og:type`, `og:url`, `og:title`, `og:description`, `og:site_name`, `og:locale`
   - `og:type = article` when `page.collection` is set; `website` for the homepage
   - `og:image` only when `page.image` is set
3. **Twitter Card tags** — `twitter:card`, `twitter:title`, `twitter:description`
   - `twitter:card = summary_large_image` when `page.image` is set; `summary` otherwise
4. **JSON-LD structured data** — `<script type="application/ld+json">` block:
   - `BlogPosting` for posts (include `datePublished`, `dateModified` if `page.updated` is set)
   - `Article` for docs and other collection pages
   - `WebSite` for the homepage/index
5. **Markdown alternate link** — `<link rel="alternate" type="text/markdown" href="{{ site.base_url }}{{ page.url }}.md">` (LLM-native differentiator)
6. **llms.txt discovery** — `<link rel="alternate" type="text/plain" title="LLM Summary" href="/llms.txt">`
7. **RSS autodiscovery** — `<link rel="alternate" type="application/rss+xml" ...>`
8. **Language attribute** — `<html lang="{{ site.language }}">` (already in bundled themes)

### Per-page frontmatter best practices

- **Always set `description:`** — used verbatim in `<meta name="description">`, `og:description`, `twitter:description`, and JSON-LD. Without it, `site.description` is used as a fallback but that is generic.
- **Set `image:`** for posts with a visual — unlocks `og:image`, `twitter:image`, and the `summary_large_image` card type
- **Set `updated:`** when you revise existing content — populates `dateModified` in JSON-LD
- **Set `robots: noindex`** on draft-like or utility pages (tag pages, test pages) that should not appear in search results

### What NOT to do

- Do not remove canonical, OG, Twitter Card, or JSON-LD blocks when customizing `base.html`
- Do not use `site.description` directly for meta tags — always use `page.description | default(value=site.description)`
- Do not hardcode URLs — always compose from `site.base_url ~ page.url`

## Commands

```bash
seite build                              # Build the site
seite build --drafts                     # Build including draft content
seite serve                              # Dev server with live reload + REPL
seite serve --port 8080                  # Use a specific port
seite new post "Title"                  # Create new post
seite new doc "Title"                  # Create new doc
seite new page "Title"                  # Create new page
seite new post "Title" --tags rust,web   # Create with tags
seite new post "Title" --draft           # Create as draft
seite new post "Title" --lang es         # Create translation (needs [languages.es] in config)
seite theme list                         # List available themes
seite theme apply <name>                 # Apply a bundled theme (default, minimal, dark, docs, brutalist, bento)
seite theme create "coral brutalist"     # Generate a custom theme with AI (requires Claude Code)
seite agent                              # Interactive AI agent session
seite agent "write about Rust"           # One-shot AI agent prompt
seite deploy                             # Deploy to configured target
```

### Dev Server REPL

`seite serve` starts a dev server with live reload and an interactive REPL:

```
new <collection> <title> [--lang <code>]  Create new content
agent [prompt]                           Start AI agent or run one-shot
theme <name>                             Apply a theme
build [--drafts]                         Rebuild the site
status                                   Show server info
stop                                     Stop and exit
```

## Project Structure

```
content/posts/    # Posts content (markdown + YAML frontmatter)
content/docs/    # Documentation content (markdown + YAML frontmatter)
content/pages/    # Pages content (markdown + YAML frontmatter)
templates/       # Tera (Jinja2-compatible) HTML templates
static/          # Static assets (copied as-is to dist/)
dist/            # Build output (generated, do not edit)
seite.toml        # Site configuration
```

## Collections

### Posts
- Directory: `content/posts/`
- URL prefix: `/posts`
- Template: `post.html`
- Date-based: yes (filename format: `YYYY-MM-DD-slug.md`)
- Included in RSS feed (`/feed.xml`)

### Documentation
- Directory: `content/docs/`
- URL prefix: `/docs`
- Template: `doc.html`
- Date-based: no (filename format: `slug.md`)
- Supports nested directories (e.g., `section/slug.md` → `/docs/section/slug`)

### Pages
- Directory: `content/pages/`
- URL prefix: `(root)`
- Template: `page.html`
- Date-based: no (filename format: `slug.md`)

## Content Format

Content files are markdown with YAML frontmatter:

```yaml
---
title: "Post Title"
date: 2025-01-15        # required for dated collections
description: "Optional"  # page description — used in meta/OG/Twitter/JSON-LD
image: /static/og.png    # optional social-preview image (og:image / twitter:image)
updated: 2025-06-01      # optional last-modified date → JSON-LD dateModified
tags:                     # optional
  - tag1
  - tag2
draft: true              # optional, hides from default build
slug: custom-slug        # optional, overrides auto-generated slug
template: custom.html    # optional, overrides collection default template
robots: noindex          # optional, per-page <meta name="robots">
---

Markdown content here.
```

### Homepage

To add custom content to the homepage, create `content/pages/index.md`. Its rendered content will appear above the collection listings on the index page. The homepage is injected as `{{ page.content }}` in the index template.

## Multi-language Support

Add translations by configuring languages in `seite.toml` and creating translated content files:

```toml
# seite.toml
[languages.es]
title = "Mi Sitio"              # optional title override
description = "Un sitio web"     # optional description override
```

Then create translated files with a language suffix before `.md`:

```
content/pages/about.md       # English (default) → /about
content/pages/about.es.md    # Spanish            → /es/about
content/posts/2025-01-15-hello.es.md  # Spanish post → /es/posts/hello
```

- Default language content lives at the root URL (`/about`)
- Other languages get a `/{lang}/` prefix (`/es/about`)
- Items with the same slug across languages are automatically linked as translations
- Per-language RSS feeds, sitemaps with hreflang alternates, and discovery files are generated automatically
- Without `[languages.*]` config, the site is single-language and works as normal

## Templates and Themes

Templates use [Tera](https://keats.github.io/tera/) syntax (Jinja2-compatible). All templates extend `base.html`.

### Available Themes

| Theme | Description |
|-------|-------------|
| `default` | Clean, readable with system fonts |
| `minimal` | Typography-first, serif |
| `dark` | Dark mode (true black, violet accent) |
| `docs` | Sidebar layout for documentation |
| `brutalist` | Neo-brutalist: thick borders, hard shadows, yellow accent |
| `bento` | Card grid layout with rounded corners and soft shadows |

Apply with `seite theme apply <name>`. This overwrites `templates/base.html`.

### Template Variables

Available in all templates:

| Variable | Type | Description |
|----------|------|-------------|
| `site.title` | string | Site title (language-specific if multilingual) |
| `site.description` | string | Site description |
| `site.base_url` | string | Base URL (e.g., `https://example.com`) |
| `site.language` | string | Language code (e.g., `en`) |
| `site.author` | string | Author name |
| `lang` | string | Current page language code |
| `translations` | array | Translation links `[{lang, url}]` (empty if no translations) |
| `page.title` | string | Page title |
| `page.content` | string | Rendered HTML (use `{{ page.content \| safe }}`) |
| `page.date` | string? | Publish date (if set) |
| `page.updated` | string? | Last-modified date (from `updated:` frontmatter) |
| `page.description` | string? | Page description |
| `page.image` | string? | Social-preview image URL (from `image:` frontmatter) |
| `page.tags` | array | Tags |
| `page.url` | string | URL path |
| `page.collection` | string | Collection name (e.g., `posts`) — empty string on homepage |
| `page.robots` | string? | Per-page robots directive (from `robots:` frontmatter) |
| `nav` | array | Sidebar nav sections `[{name, label, items: [{title, url, active}]}]` |

Index template also gets:

| Variable | Type | Description |
|----------|------|-------------|
| `collections` | array | Listed collections `[{name, label, items}]` |
| `page` | object? | Homepage content (if `content/pages/index.md` exists) |

### Customizing Templates

Edit files in `templates/` to customize. Key rules:

- `base.html` is the root layout — all other templates extend it via `{% extends "base.html" %}`
- Content goes in `{% block content %}...{% endblock %}`
- Title goes in `{% block title %}...{% endblock %}`
- When editing `base.html`, preserve these for full functionality:
  - `<html lang="{{ site.language }}">` — language attribute
  - `<link rel="canonical">` — canonical URL (required for SEO)
  - Open Graph tags: `og:type`, `og:url`, `og:title`, `og:description`, `og:site_name`, `og:locale`
  - Twitter Card tags: `twitter:card`, `twitter:title`, `twitter:description`
  - JSON-LD `<script type="application/ld+json">` — structured data for search engines and LLMs
  - `<meta name="robots">` — only emitted when `page.robots` is set in frontmatter
  - `<link rel="alternate" type="text/markdown">` — markdown version for LLM consumption
  - `<link rel="alternate" type="text/plain" href="/llms.txt">` — LLM summary discovery
  - RSS link: `<link rel="alternate" type="application/rss+xml" ...>`
  - hreflang links for i18n: `{% if translations %}...{% endif %}`
  - Language switcher: `{% if translations | length > 1 %}...{% endif %}`
  - Content block: `{% block content %}{% endblock %}`

### SEO and GEO Guardrails

All bundled themes already emit the full SEO+GEO head block (see **SEO and GEO Requirements** at the top of this file). When writing a custom `base.html` or modifying an existing one, you **must** preserve all of the following:

- **Always** include `<link rel="canonical">` pointing to `{{ site.base_url }}{{ page.url | default(value='/') }}`
- **Always** use `{{ page.description | default(value=site.description) }}` for description meta — not `site.description` alone
- **Always** include Open Graph (`og:*`) and Twitter Card (`twitter:*`) tags for social sharing
- **Always** include JSON-LD structured data: `BlogPosting` for posts, `Article` for docs/pages, `WebSite` for index
- **Use** `og:type = article` when `page.collection` is set; `website` for the homepage
- **Use** `twitter:card = summary_large_image` when `page.image` is set; `summary` otherwise
- **Include** `<link rel="alternate" type="text/markdown">` — this is your LLM-native differentiator
- **Include** `<link rel="alternate" type="text/plain" href="/llms.txt">` — LLM discovery
- **Add** `description:`, `image:`, and `updated:` to frontmatter for best SEO/GEO coverage
- **Use** `robots: noindex` in frontmatter for pages that should not appear in search results

## Features

- **Syntax highlighting** — Fenced code blocks with language annotations are automatically highlighted
- **Docs sidebar navigation** — Doc pages get a sidebar nav listing all docs, grouped by directory. Use the `docs` theme: `seite theme apply docs`
- **Homepage content** — Create `content/pages/index.md` for custom homepage hero/landing content above collection listings
- **Multi-language** — Filename-based translations with per-language URLs, RSS, sitemap, and discovery files
- **SEO+GEO optimized** — Every page gets canonical URL, Open Graph, Twitter Card, JSON-LD structured data (`BlogPosting`/`Article`/`WebSite`), and per-page robots meta. No plugins needed.
- **LLM discoverability** — Generates `llms.txt` and `llms-full.txt` for LLM consumption; `<link rel="alternate" type="text/markdown">` in every page's `<head>`
- **RSS feed** — Auto-generated at `/feed.xml` (per-language feeds at `/{lang}/feed.xml`)
- **Sitemap** — Auto-generated at `/sitemap.xml` with hreflang alternates
- **Search** — `dist/search-index.json` is auto-generated every build; the default theme includes a client-side search input that queries it. No config needed.
- **Asset pipeline** — Add `minify = true` and/or `fingerprint = true` to `[build]` in `seite.toml` to minify CSS/JS and add content-hash suffixes (`main.a1b2c3d4.css`) with a `dist/asset-manifest.json`
- **Markdown output** — Every page gets a `.md` file alongside `.html` in `dist/`
- **Clean URLs** — `/posts/hello-world` (no `.html` extension)
- **Draft exclusion** — `draft: true` in frontmatter hides from builds (use `--drafts` to include)

## Design Prompts

When asked to redesign or create a theme, use one of these directions as a starting point.
Edit `templates/base.html` directly — or apply a bundled theme first with `seite theme apply <name>` then edit.

**Minimal / Editorial** — Single column max 620px, Georgia serif body, geometric sans for UI elements.
No decorative elements. Bottom-border-only search input. White/off-white (`#FAF9F6`) background,
near-black (`#1A1A1A`) text, one muted link accent. Typography carries all personality.

**Bold / Neo-Brutalist** — Thick black borders (3px solid `#000000`), hard non-blurred box shadows
(`6px 6px 0 #000`). No border-radius. Saturated fill: yellow `#FFE600`, lime `#AAFF00`, or coral `#FF4D00`.
Cream (`#FFFEF0`) background. Font-weight 900. Headlines 4rem+. Buttons shift their shadow on hover to press in.

**Bento / Card Grid** — Responsive CSS grid, gap 16px, all cards border-radius 20px. Mixed card sizes
(1-, 2-, 3-col spans). Cards have independent background colors. Floating shadow:
`box-shadow: 0 4px 24px rgba(0,0,0,0.08)`. Warm neutral palette (`#F5F0EB`) with one dark-accent card per row.

**Dark / Expressive** — True black (`#000000` or `#0A0A0A`) surfaces. One neon accent:
green `#00FF87`, blue `#0066FF`, or violet `#8B5CF6`. Off-white text (`#E8E8E8`).
Translucent nav with `backdrop-filter: blur(12px)`. Visible, styled focus rings.

**Glass / Aurora** — Gradient mesh background (violet `#7B2FBE` → teal `#00C9A7`, or
indigo `#1A1040` → electric blue `#4361EE`). Floating panels: `backdrop-filter: blur(16px)`,
`rgba(255,255,255,0.10)` fill, `1px solid rgba(255,255,255,0.2)` border. Use for cards/nav only.

**Accessible / High-Contrast** — WCAG AAA ratios. Min 16px body. 3px colored focus rings
(design feature, not afterthought). Min 44px click targets. One semantic accent. No color-only
information. Full `prefers-reduced-motion: reduce` support.

## Key Conventions

- Run `seite build` after creating or editing content to regenerate the site
- URLs are clean (no extension): `/posts/hello-world` on disk is `dist/posts/hello-world.html`
- Templates use Tera syntax and extend `base.html`
- Use `{{ page.content | safe }}` to render HTML content (the `safe` filter is required)
- Themes only replace `base.html` — collection templates (`post.html`, `doc.html`, `page.html`) are separate
- The `static/` directory is copied as-is to `dist/static/` during build
- Pagination: add `paginate = 10` to a `[[collections]]` block in `seite.toml` to generate `/posts/`, `/posts/page/2/`, etc.
  Use `{% if pagination %}<nav>...</nav>{% endif %}` in templates; variables: `pagination.current_page`, `pagination.total_pages`, `pagination.prev_url`, `pagination.next_url`
- Search is always enabled: `dist/search-index.json` is generated every build. All bundled themes include a search box wired to it. No config needed.
- Asset pipeline: set `minify = true` and/or `fingerprint = true` under `[build]` in `seite.toml`
  - `minify` strips CSS/JS comments and collapses whitespace
  - `fingerprint` writes `file.<hash8>.ext` copies of each static asset and a `dist/asset-manifest.json` mapping original names to fingerprinted names
- Custom theme: `seite theme create "your design description"` generates `templates/base.html` with Claude (requires Claude Code)
