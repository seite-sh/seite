## Features

- **Syntax highlighting** — Fenced code blocks with language annotations are automatically highlighted
- **Homepage content** — Create `content/pages/index.md` for custom homepage hero/landing content above collection listings
- **Multi-language** — Filename-based translations with per-language URLs, RSS, sitemap, and discovery files
- **SEO+GEO optimized** — Every page gets canonical URL, Open Graph, Twitter Card, JSON-LD structured data (`BlogPosting`/`Article`/`WebSite`), and per-page robots meta. No plugins needed.
- **LLM discoverability** — Generates `llms.txt` and `llms-full.txt` for LLM consumption; `<link rel="alternate" type="text/markdown">` in every page's `<head>`
- **RSS feed** — Auto-generated at `/feed.xml` (per-language feeds at `/{lang}/feed.xml`)
- **Sitemap** — Auto-generated at `/sitemap.xml` with hreflang alternates
- **Search** — `dist/search-index.json` is auto-generated every build; the default theme includes a client-side search input that queries it. No config needed.
- **Image processing** — Add `[images]` to `seite.toml` to auto-resize images, generate WebP variants, inject `srcset`/`<picture>` elements, and add `loading="lazy"`. See Configuration section below.
- **Analytics** — Add `[analytics]` to `seite.toml` for Google Analytics, GTM, Plausible, Fathom, or Umami. Optional cookie consent banner. See Configuration section below.
- **Tag pages** — Auto-generated `/tags/` index and `/tags/{tag}/` archive pages, included in sitemap
- **404 page** — Auto-generated `dist/404.html` using the `404.html` template. Customize by creating `templates/404.html`. Dev server serves it on 404 responses.
- **Table of contents** — `{{ page.toc }}` is auto-generated from heading hierarchy; all headings get `id` anchors for deep linking
- **Reading time & word count** — `{{ page.reading_time }}` (minutes, 238 WPM) and `{{ page.word_count }}` available in all templates
- **Asset pipeline** — Add `minify = true` and/or `fingerprint = true` to `[build]` in `seite.toml` to minify CSS/JS and add content-hash suffixes (`main.a1b2c3d4.css`) with a `dist/asset-manifest.json`
- **Markdown output** — Every page gets a `.md` file alongside `.html` in `dist/`
- **Clean URLs** — `/posts/hello-world` (no `.html` extension)
- **Draft exclusion** — `draft: true` in frontmatter hides from builds (use `--drafts` to include)
- **Shortcodes** — Reusable content components in markdown. See Shortcodes section below.

