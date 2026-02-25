## SEO and GEO Requirements

> **These are non-negotiable rules for every page on this site.**
> They apply when writing content, creating templates, or asking the AI agent to build or redesign anything.

### Every page `<head>` MUST include

1. **Favicon** — `<link rel="icon" href="/favicon.ico">` (user places `favicon.ico` in `public/`)
2. **Canonical URL** — `<link rel="canonical" href="{{ site.base_url }}{{ page.url | default(value='/') }}">`  (deduplicates indexed URLs)
3. **Open Graph tags** — `og:type`, `og:url`, `og:title`, `og:description`, `og:site_name`, `og:locale`
   - `og:type = article` when `page.collection` is set; `website` for the homepage
   - `og:image` only when `page.image` is set — must be an absolute URL. Use `{% set _abs_image = page.image %}{% if not page.image is starting_with("http") %}{% set _abs_image = site.base_url ~ page.image %}{% endif %}` then reference `{{ _abs_image }}`
   - `og:image:width` and `og:image:height` — include `1200` and `630` (standard social preview dimensions)
   - `article:published_time` — emit when `page.collection` and `page.date` are set
   - `article:modified_time` — emit when `page.collection` and `page.updated` are set
4. **Twitter Card tags** — `twitter:card`, `twitter:title`, `twitter:description`, `twitter:image`
   - `twitter:card = summary_large_image` when `page.image` is set; `summary` otherwise
   - `twitter:image` — same absolutization as `og:image` (use `_abs_image` variable)
5. **JSON-LD structured data** — `<script type="application/ld+json">` block:
   - `BlogPosting` for posts (include `datePublished`, `dateModified` if `page.updated` is set)
   - `Article` for docs and other collection pages
   - `WebSite` for the homepage/index
   - `BreadcrumbList` for all collection pages (Home → Collection → Page)
6. **Markdown alternate link** — `<link rel="alternate" type="text/markdown" title="Markdown" href="{{ site.base_url }}{{ page.url }}.md">` (LLM-native differentiator — must include `title` attribute)
7. **llms.txt discovery** — `<link rel="alternate" type="text/plain" title="LLM Summary" href="/llms.txt">`
8. **RSS autodiscovery** — `<link rel="alternate" type="application/rss+xml" ...>`
9. **Language attribute** — `<html lang="{{ lang }}">` (already in bundled themes)

### Per-page frontmatter best practices

- **Always set `description:`** — used verbatim in `<meta name="description">`, `og:description`, `twitter:description`, and JSON-LD. Without it, `site.description` is used as a fallback but that is generic.
- **Set `image:`** for posts with a visual — unlocks `og:image`, `twitter:image`, and the `summary_large_image` card type
- **Set `updated:`** when you revise existing content — populates `dateModified` in JSON-LD and `article:modified_time` OG tag; also used as sitemap `lastmod` (takes priority over `date`)
- **Set `robots: noindex`** on draft-like or utility pages (tag pages, test pages) that should not appear in search results

### What NOT to do

- Do not remove canonical, OG, Twitter Card, or JSON-LD blocks when customizing `base.html`
- Do not use `site.description` directly for meta tags — always use `page.description | default(value=site.description)`
- Do not hardcode URLs — always compose from `site.base_url ~ page.url`
- Do not use raw `page.image` in OG/Twitter tags — always absolutize relative paths first

