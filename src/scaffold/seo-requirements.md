## SEO and GEO Requirements

> **These are non-negotiable rules for every page on this site.**
> They apply when writing content, creating templates, or asking the AI agent to build or redesign anything.

### The head comes from seite

`templates/base.html` includes seite's head partial right after `<title>`:

```html
<title>{% block title %}{{ site.title }}{% endblock %}</title>
{% include "seite/head.html" %}
```

`seite/head.html` is managed by seite and updated with it. It renders: the description (`page.description`, else `site.description`), the canonical URL, Open Graph and Twitter Card tags (absolute `og:image` from `page.image`, `article:published_time`/`modified_time`), JSON-LD (`BlogPosting` for posts, `Article` for other collections, `WebSite` for the homepage, plus `BreadcrumbList`), `robots` from `page.robots`, RSS/Atom and `/llms.txt` links, the markdown alternate (only when `page.markdown_url` is set), hreflang links for translations, and the KaTeX stylesheet when math is on.

The theme itself keeps: `<html lang="{{ lang }}">`, `<meta charset>`, the viewport meta, the favicon link (`/favicon.ico`, the file goes in `public/`) and `<title>{% block title %}…{% endblock %}</title>`.

### Per-page frontmatter best practices

- **Always set `description:`** — used verbatim in `<meta name="description">`, `og:description`, `twitter:description`, and JSON-LD. Without it, `site.description` is used as a fallback but that is generic.
- **Set `image:`** for posts with a visual — unlocks `og:image`, `twitter:image`, and the `summary_large_image` card type
- **Set `updated:`** when you revise existing content — populates `dateModified` in JSON-LD and `article:modified_time` OG tag; also used as sitemap `lastmod` (takes priority over `date`)
- **Set `robots: noindex`** on draft-like or utility pages (tag pages, test pages) that should not appear in search results

### What NOT to do

- Do not remove `{% include "seite/head.html" %}` when customizing or redesigning `base.html`, and do not copy its contents into `base.html` (the copy stops getting fixes; `seite check` warns with `template-missing-seite-head`).
- Do not add a second canonical, Open Graph, Twitter Card, or JSON-LD block in a theme: the partial already writes them. Page-specific extra tags go in `{% block head %}`.
- Do not override `templates/seite/head.html` unless you accept maintaining the SEO head yourself.
- Do not hardcode URLs in templates — compose them from `site.base_url`, `lang_prefix` and `page.url`.
