---
paths:
  - "src/themes/*.tera"
  - "src/themes/**"
  - "src/templates/seite/**"
---
# SEO and GEO Guardrails

The SEO/GEO head lives in ONE place: `src/templates/seite/head.html`. Themes include it right after `<title>` and must never carry their own copy. Fix or extend SEO there, never in `src/themes/*.tera`. Themes keep only `<meta charset>`, viewport, `<link rel="icon" href="/favicon.ico">` and `<title>{% block title %}…{% endblock %}</title>`.

`seite/head.html` must render:

## Meta tags
- `<meta name="description">` — `{{ page.description | default(value=site.description) }}`
- `<link rel="canonical">` — always `{{ site.base_url }}{{ page.url | default(value='/') }}`
- `og:type` — `"article"` when `page.collection` set, `"website"` for index
- `og:url`, `og:title`, `og:description`, `og:site_name`, `og:locale`
- `og:image` (conditional on `page.image`, absolutized): use `{% set _abs_image = page.image %}{% if not page.image is starting_with("http") %}{% set _abs_image = site.base_url ~ page.image %}{% endif %}`
- `og:image:width` (1200), `og:image:height` (630) alongside `og:image`
- Generated cards (`[og]`): `{% elif page.og_image or site.og_image %}` branch after the `page.image` one sets `_abs_image` to `page.og_image | default(value=site.og_image)` (already absolute) and adds `og:image:alt` (page title); `twitter:card`/`twitter:image` test `_abs_image`. Without `[og]` both are unset (skipped in serialization), so output is byte-identical to before. Never generate a card for private-collection pages (titles would leak)
- `article:published_time` (when `page.collection` + `page.date`)
- `article:modified_time` (when `page.collection` + `page.updated`)
- `twitter:card` — `"summary_large_image"` with an image (own or generated card), `"summary"` without
- `twitter:title`, `twitter:description`, `twitter:image`
- `<meta name="robots">` (only when `page.robots` is set)
- KaTeX stylesheet when `math_enabled`

## JSON-LD Structured Data
- Posts (`page.collection == 'posts'`): `BlogPosting` with headline, description, datePublished, dateModified, author, publisher, url
- Docs/pages (collection set but not posts): `Article` with same fields
- Index/homepage: `WebSite` with name, description, url
- `BreadcrumbList` on all collection pages except the homepage: Home (`site.base_url ~ lang_prefix`) → Collection (`page.collection_url`: base URL + language prefix + `url_prefix`, set by the build) → Page; root-mounted collections (empty `url_prefix`, e.g. `pages`) get Home → Page. Never build crumb URLs from the collection name (`og::test_breadcrumb_urls_resolve_to_built_pages` checks every crumb is a built page)

## Discovery Links
- `<link rel="alternate" type="application/rss+xml">` and Atom — RSS/Atom
- `<link rel="alternate" type="text/plain" title="LLM Summary" href="/llms.txt">`
- `<link rel="alternate" type="text/markdown" title="Markdown" href="{{ site.base_url }}{{ page.markdown_url }}">` (only when `page.markdown_url` is set — never `page.url ~ ".md"`; include `title` attr)
- hreflang for each of `translations` plus `x-default`

## Changing the partial
- Every `{{ }}` inside an attribute value uses `| escape_attr`; JSON-LD and inline-JS values use `| json_script` (never `json_encode()`: it lets `</script>` through). Filters live in `templates::register_filters`
- Byte-level output of every theme is covered by `tests/integration/theme_partials.rs`; an intentional change updates that test's expectations, the docs (`seite-sh/content/docs/templates.md` "seite partials") and the changelog
- Sites get the change on their next build (no `seite theme apply` needed) unless they override `templates/seite/head.html`

## robots.txt
- `User-agent: *` group carries `Content-Signal: search=yes, ai-input=yes, ai-train=no` (https://contentsignals.org)
- Allow AI search / user-fetch crawlers: ChatGPT-User, OAI-SearchBot, PerplexityBot, Perplexity-User, Claude-SearchBot, Claude-User
- Disallow AI training crawlers: GPTBot, ClaudeBot, Google-Extended, Applebot-Extended, meta-externalagent, CCBot, Bytespider
- Lists live in `AI_SEARCH_CRAWLERS` / `AI_TRAINING_CRAWLERS` in `src/build/discovery.rs`

## i18n in Themes
- Use `{{ lang }}` (not `site.language`) for current page language
- Use `{{ lang_prefix }}` to prefix internal links
- Use `{{ t.key }}` for all UI strings — never hardcode English
- `<html lang="{{ lang }}">` in the theme; `og:locale` comes from the partial
