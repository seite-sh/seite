---
title: "Design Your Own Theme"
description: "Design a custom seite theme with impeccable or your coding agent: start from the blank starter, keep seite's SEO partials, put the look in static/css/site.css, verify with seite check."
weight: 5
---

The bundled themes are examples. A site that looks like yours starts from the `blank` starter and gets designed by your coding agent, ideally with the impeccable design skill. seite keeps the parts that must stay correct (SEO and GEO head, search, language switcher) in its own partials, so a redesign cannot drop them.

## Design your own theme

```bash
seite theme new                  # blank starter: templates/base.html + static/css/site.css
```

`seite theme new [name]` writes a bare, semantic layout (header, navigation, main, footer, docs sidebar, skip link) to `templates/base.html` and a neutral stylesheet to `static/css/site.css`. It refuses to replace a customized `base.html` unless you pass `--force`, and then backs it up first (`base.html.bak`), like `seite theme apply`.

### With impeccable (recommended)

[impeccable](https://github.com/pbakaus/impeccable) is a design skill for coding agents: direction, typography, color, layout, and a detector for common design problems.

1. **Install**: `seite skill install impeccable` (asks before running its npm installer). It also points impeccable's live mode at `templates/base.html`.
2. **Brief**: in your agent, `/impeccable init`. It writes `PRODUCT.md`, the site brief.
3. **Design**: `/impeccable <describe your design>` (Codex: `$impeccable …`). It edits `templates/base.html` (layout) and `static/css/site.css` (the look).
4. **Verify**: `seite check`, then `seite build` and one `impeccable detect dist/`. Fix what they report; don't loop on taste.
5. **Preview**: `seite serve` and open the URL in your agent's built-in browser (or give it to `impeccable live`).

### Without impeccable

In your agent, run `/seite theme <description>` (Codex: `$seite theme <description>`), or from the shell `seite theme create "<description>"`, which runs your agent once. When the site is still on a bundled example, `theme create` scaffolds `blank` first, so the design starts clean instead of being painted over another look.

## The theme contract

A theme owns **layout and CSS**. seite owns everything that has to be right on every page, in partials you include:

```html
<!DOCTYPE html>
<html lang="{{ lang }}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" href="/favicon.ico">
    <title>{% block title %}{{ site.title }}{% endblock %}</title>
    {% include "seite/head.html" %}
    <link rel="stylesheet" href="/static/css/site.css">
    {% block extra_css %}{% endblock %}
    {% block head %}{% endblock %}
</head>
<body>
    <a href="#main" class="skip-link">{{ t.skip_to_content }}</a>
    <header>
    {% block header %}
        <a href="{{ lang_prefix }}/">{{ site.title }}</a>
        {% include "seite/lang-switcher.html" %}
        {% include "seite/search.html" %}
    {% endblock %}
    </header>
    <main id="main">
        {% block content %}{% endblock %}
    </main>
    <footer>{% block footer %}{% endblock %}</footer>
    {% include "seite/scripts.html" %}
    {% block extra_js %}{% endblock %}
</body>
</html>
```

Rules for designers and agents:

- **Keep the `{% include "seite/…" %}` lines.** Move them around the layout as you like, but never copy a partial's contents into `base.html`: the copy stops receiving fixes, and `seite check` reports `template-missing-seite-head`.
- **Keep `<title>{% block title %}…{% endblock %}</title>` in the theme**, right before `seite/head.html`. Collection templates override the title through that block.
- **Keep the blocks** `title`, `head`, `extra_css`, `header`, `content`, `footer`, `extra_js`. `post.html`, `doc.html` and the other templates extend `base.html` and fill them.
- **Build in `templates/` and `static/`**, preview with `seite serve`. No standalone HTML mockups to port later.
- Put the look in `static/css/site.css`. Links are root-relative (`/static/…`); seite rewrites them for a `base_path` and keeps the plain file next to fingerprinted copies, so they work with `minify` and `fingerprint`.
- Prefer utility classes? `seite theme new --tailwind` starts the blank starter on [Tailwind CSS](/docs/configuration#tailwind-css): `base.html` links `/static/css/tailwind.css`, compiled at build time from `static/css/tailwind.css` by the standalone Tailwind CLI (no Node).

What each partial does, and how to override one, is in [Templates → seite partials](/docs/templates#seite-partials).

### Template blocks

| Block | Purpose |
|-------|---------|
| `{% block title %}` | Page `<title>` |
| `{% block head %}` | Extra tags in `<head>` (preloads, page-specific meta) |
| `{% block extra_css %}` | Extra CSS (`<style>` or `<link>`) |
| `{% block header %}` | Site header and navigation |
| `{% block site_title %}` | Optional: the site name inside the header. The default `index.html` overrides it with an `<h1>`, so listing pages without a page title still have one (the `blank` starter uses it) |
| `{% block content %}` | Main page content |
| `{% block footer %}` | Extra markup after the footer |
| `{% block extra_js %}` | Page-specific JavaScript before `</body>` |

## Navigation

Render `data.nav` (from `data/nav.yaml`) for header links:

```html
{% if data.nav %}
<nav class="site-nav" aria-label="Main navigation">
    {% for item in data.nav %}{% if item.external %}<a href="{{ item.url }}" target="_blank" rel="noopener">{{ item.title }}</a>{% else %}<a href="{{ lang_prefix }}{{ item.url }}">{{ item.title }}</a>{% endif %}{% endfor %}
</nav>
{% endif %}
```

Prefix internal links with `{{ lang_prefix }}` so they work on translated pages. Docs pages also get `nav` (sidebar sections with `items[].active`); the `blank` starter renders it as a sidebar.

## Pagination

When a collection has `paginate = N`, index templates get `pagination`. The default `index.html` renders it; a theme only styles `.pagination`:

```html
{% if pagination %}
<nav class="pagination">
    {% if pagination.prev_url %}<a href="{{ pagination.prev_url }}">&larr; {{ t.newer }}</a>{% endif %}
    <span>{{ t.page_n_of_total | replace(from="{n}", to=pagination.current_page ~ "") | replace(from="{total}", to=pagination.total_pages ~ "") }}</span>
    {% if pagination.next_url %}<a href="{{ pagination.next_url }}">{{ t.older }} &rarr;</a>{% endif %}
</nav>
{% endif %}
```

## Accessibility checklist

- **Skip link**: `<a href="#main" class="skip-link">{{ t.skip_to_content }}</a>` first in `<body>`, pointing at `<main id="main">`.
- **One `<h1>` per page**: collection templates write the page title as `<h1>`; keep the site name in the header out of `<h1>`.
- **Focus**: visible focus styles on every interactive element.
- **Reduced motion**: `@media (prefers-reduced-motion: reduce)` disables animations.
- **Language**: `<html lang="{{ lang }}">` and UI text from `{{ t.<key> }}`, never hardcoded English.

The search partial already has `role="search"`, labels from `t.search_label`, and an `aria-live` results region.

## CSS

The `blank` starter keeps its CSS in `static/css/site.css`: agents and design tools edit CSS files well, and impeccable's detector scans them. The bundled example themes keep theirs inline in `<style>`, so a single `.tera` file can be shared and installed from a URL.

### Page transitions

Every bundled theme, `blank`'s `static/css/site.css` included, cross-fades between pages with the browser's cross-document view transitions, only for visitors who haven't asked for reduced motion:

```css
@media (prefers-reduced-motion: no-preference) { @view-transition { navigation: auto; } }
```

It needs no JavaScript and applies to same-origin navigations; browsers without support simply load the next page. Keep the rule (or remove it for instant page changes); keep it inside the `prefers-reduced-motion: no-preference` query.

Modern CSS covers what preprocessors were used for: custom properties, native nesting, `color-mix()`, `clamp()`. If you need Sass, compile it into `static/` and link the output.

### Classes the default templates and shortcodes use

Style these if your content uses them (the `blank` stylesheet has neutral versions): `.toc`, `.pagination`, `.article-nav`, `.tags`, `.tag-link`, `.related`, `.excerpt`, `.homepage-content`, `.docs-section-list`, `.callout` (`-info`, `-warning`, `-danger`, `-tip`, `-note`), `.video-embed`, `figure`/`figcaption`, `.contact-form*`, `.katex-display`, `#search-results` (`.result-meta`, `.no-results`), `.lang-switcher`.

## Testing your theme

1. `seite check` (add `--strict` to treat warnings as errors): templates, links, the SEO head.
2. With impeccable: `seite build`, then one `impeccable detect dist/`.
3. In `seite serve`: the homepage, a post (tags, date), a doc (table of contents, sidebar), search, a tag page, a translated page if you have languages.
4. Narrow the window below 768px; tab through the page and check focus.

## Starting from a bundled example instead

```bash
seite theme apply dark      # writes templates/base.html
```

Edit its layout and CSS; keep the includes. A bundled theme you never edit is replaced with its newest version by `seite upgrade`; once you edit it, it is yours.

## Exporting and sharing

```bash
seite theme export my-theme --description "Dark theme with green accents"
```

This saves `templates/themes/my-theme.tera` (your `base.html`, includes and all). Stylesheets it links from `static/` (such as `static/css/site.css`) are inlined as a `<style>` element, so the exported file works on its own. Others install it with `seite theme install <url>`.

## Next steps

- [Templates](/docs/templates): variables, blocks, seite partials, data files
- [Theme Gallery](/docs/theme-gallery): the starters and the example looks
- [Shortcodes](/docs/shortcodes): content components your CSS should support
