---
paths:
  - "src/build/**"
---
# Build Pipeline (15 steps)

1. Clean `dist/`
1b. Copy `public/` → `dist/` (no prefix, no minification)
2. Load Tera templates (user + embedded defaults)
2b. Load shortcode registry (built-in + user `templates/shortcodes/`)
2.5. Load data files (`data/` → `{{ data.filename }}` in templates)
3. Process collections: walk content, parse frontmatter, expand shortcodes, render markdown, detect language, resolve slugs/URLs, compute word count/reading time/excerpt/ToC, build translation map, sort
3b. Inject i18n context (`lang_prefix`, `default_language`, `t` UI strings)
4. Render index pages (per-language), paginated/non-paginated collection indexes, 404, tag pages. Docs use `docs-index.html` with sidebar nav
5. Generate RSS feeds (default + per-language)
6. Generate sitemap with `xhtml:link` alternates for translations
7. Generate `llms.txt` and `llms-full.txt` (per-language)
8. Output raw markdown alongside HTML
9. Generate search index JSON (per-language)
10. Copy static files
11. Process images (resize, WebP, AVIF)
12. Post-process HTML (srcset, `<picture>`, `loading="lazy"` — skip first image for LCP)
13. Inject analytics + optional cookie consent
14. Build subdomain sites (collections with `subdomain` set get own pipeline into `dist-subdomains/`)

## Output Pattern

URLs are clean: `/posts/hello-world` → `dist/posts/hello-world.html` + `.md`
Dev server resolves clean URLs to `.html` files.

## Content Model

```rust
struct Frontmatter {
    title: String,
    date: Option<NaiveDate>,       // required for posts
    updated: Option<NaiveDate>,    // → JSON-LD dateModified, sitemap lastmod
    description: Option<String>,
    image: Option<String>,         // → og:image / twitter:image
    slug: Option<String>,          // override auto-generated slug
    tags: Vec<String>,
    draft: bool,                   // excluded unless --drafts
    template: Option<String>,      // override collection default
    robots: Option<String>,        // per-page <meta name="robots">
    weight: Option<i32>,           // ordering for non-date collections
    extra: HashMap<String, Value>, // → {{ page.extra.field }}
}

struct ContentItem {
    frontmatter, raw_body, html_body, source_path,
    slug, collection, url, lang,
}
```

## Data Files

`data/` holds YAML/JSON/TOML loaded at build time → `{{ data.filename }}` in templates. Nested dirs create nested keys. Two files with same stem = build error.

All 6 themes render `data.nav` (navigation) and `data.footer` (links + copyright). Internal links auto-prefixed with `{{ lang_prefix }}`; external links (with `external: true`) get `target="_blank"`.

UI string translations: `data/i18n/{lang}.yaml` overrides English defaults, injected as `{{ t }}`.

## Shortcodes

- Inline `{{< name(args) >}}` and body `{{% name(args) %}} markdown {{% end %}}`
- Named args: `key="string"`, `key=42`, `key=true`
- Expanded before markdown rendering; not expanded inside code blocks
- Built-ins: `youtube`, `vimeo`, `gist`, `callout` (body), `figure`, `contact_form`
- User-defined: `templates/shortcodes/*.html` (override built-ins by name)
- To add a built-in: create template in `src/shortcodes/builtins/`, add entry in `builtins.rs`

## Image Processing

When `[images]` is configured: resize to widths, generate WebP/AVIF variants, rewrite `<img>` tags with srcset and `<picture>`, add `loading="lazy"` (skip first image per page for LCP).
