---
paths:
  - "src/themes/*.tera"
  - "src/themes/**"
---
# SEO and GEO Guardrails

Every bundled theme `<head>` must include:

## Meta tags
- `<link rel="icon" href="/favicon.ico">`
- `<link rel="canonical">` — always `{{ site.base_url }}{{ page.url | default(value='/') }}`
- `<meta name="description">` — `{{ page.description | default(value=site.description) }}`
- `og:type` — `"article"` when `page.collection` set, `"website"` for index
- `og:url`, `og:title`, `og:description`, `og:site_name`, `og:locale`
- `og:image` (conditional on `page.image`, absolutized): use `{% set _abs_image = page.image %}{% if not page.image is starting_with("http") %}{% set _abs_image = site.base_url ~ page.image %}{% endif %}`
- `og:image:width` (1200), `og:image:height` (630) alongside `og:image`
- `article:published_time` (when `page.collection` + `page.date`)
- `article:modified_time` (when `page.collection` + `page.updated`)
- `twitter:card` — `"summary_large_image"` with image, `"summary"` without
- `twitter:title`, `twitter:description`, `twitter:image`
- `<meta name="robots">` (only when `page.robots` is set)

## JSON-LD Structured Data
- Posts (`page.collection == 'posts'`): `BlogPosting` with headline, description, datePublished, dateModified, author, publisher, url
- Docs/pages (collection set but not posts): `Article` with same fields
- Index/homepage: `WebSite` with name, description, url
- `BreadcrumbList` on all collection pages (Home → Collection → Page)

## Discovery Links
- `<link rel="alternate" type="application/rss+xml">` — RSS
- `<link rel="alternate" type="text/plain" title="LLM Summary" href="/llms.txt">`
- `<link rel="alternate" type="text/markdown" title="Markdown" href="{{ site.base_url }}{{ page.markdown_url }}">` (only when `page.markdown_url` is set — never `page.url ~ ".md"`; include `title` attr)

## robots.txt
- `User-agent: *` group carries `Content-Signal: search=yes, ai-input=yes, ai-train=no` (https://contentsignals.org)
- Allow AI search / user-fetch crawlers: ChatGPT-User, OAI-SearchBot, PerplexityBot, Perplexity-User, Claude-SearchBot, Claude-User
- Disallow AI training crawlers: GPTBot, ClaudeBot, Google-Extended, Applebot-Extended, meta-externalagent, CCBot, Bytespider
- Lists live in `AI_SEARCH_CRAWLERS` / `AI_TRAINING_CRAWLERS` in `src/build/discovery.rs`

## i18n in Themes
- Use `{{ lang }}` (not `site.language`) for current page language
- Use `{{ lang_prefix }}` to prefix internal links
- Use `{{ t.key }}` for all UI strings — never hardcode English
- `<html lang="{{ lang }}">`, `<meta property="og:locale" content="{{ lang }}">`
