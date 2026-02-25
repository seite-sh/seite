---
title: "Building Custom Themes"
description: "Step-by-step guide to creating a custom theme from scratch — template structure, CSS, SEO requirements, and testing."
weight: 5
---

## Overview

Every seite theme is a single Tera template file that serves as `base.html`. It contains the full HTML structure, all CSS (inline), SEO meta tags, search, pagination, and accessibility features. No external stylesheets, no build tools, no preprocessors — one file, completely self-contained.

This guide walks you through creating a theme from scratch. If you'd rather start from an existing theme and modify it, see [Copying a Bundled Theme](#copying-a-bundled-theme).

## Quick Start: Copy and Modify

The fastest path to a custom theme:

```bash
# Apply a bundled theme as your starting point
seite theme apply default

# The theme is now at templates/base.html — edit it directly
```

Open `templates/base.html` and start changing CSS values, colors, fonts, and layout. Run `seite serve` and changes reload instantly.

## Starting from Scratch

Create `templates/base.html` with this minimal skeleton:

```html
<!DOCTYPE html>
<html lang="{{ lang }}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}{{ page.title | default(value=site.title) }} — {{ site.title }}{% endblock %}</title>

    <!-- SEO meta tags (required — see SEO section below) -->
    <meta name="description" content="{{ page.description | default(value=site.description) }}">
    <link rel="canonical" href="{{ site.base_url }}{{ page.url | default(value='/') }}">

    {% block head %}{% endblock %}

    <style>
        {% block extra_css %}{% endblock %}

        /* Your CSS here */
        body {
            max-width: 720px;
            margin: 2rem auto;
            padding: 0 1rem;
            font-family: system-ui, sans-serif;
            line-height: 1.6;
        }
    </style>
</head>
<body>
    <a href="#main" class="skip-link">{{ t.skip_to_content | default(value="Skip to main content") }}</a>

    {% block header %}
    <header>
        <h1><a href="{{ lang_prefix }}/">{{ site.title }}</a></h1>
    </header>
    {% endblock %}

    <main id="main">
        {% block content %}
        {{ page.content | safe }}
        {% endblock %}
    </main>

    {% block footer %}
    <footer>
        <p>&copy; {{ site.title }}</p>
    </footer>
    {% endblock %}

    {% block extra_js %}{% endblock %}
</body>
</html>
```

This is a valid (if minimal) theme. Build it and verify:

```bash
seite build
seite serve
```

## Required Template Blocks

Every theme must define these 7 blocks so that collection-specific templates (`post.html`, `doc.html`, etc.) can extend and override them:

| Block | Purpose |
|-------|---------|
| `{% block title %}` | Page `<title>` tag |
| `{% block head %}` | Extra content in `<head>` (meta tags, preloads) |
| `{% block extra_css %}` | Page-specific CSS injected inside `<style>` |
| `{% block header %}` | Site header and navigation |
| `{% block content %}` | Main page content |
| `{% block footer %}` | Site footer |
| `{% block extra_js %}` | Page-specific JavaScript before `</body>` |

## Adding SEO Meta Tags

Search engines and social platforms need these tags. Add them to `<head>`:

```html
<!-- Core SEO -->
<meta name="description" content="{{ page.description | default(value=site.description) }}">
<link rel="canonical" href="{{ site.base_url }}{{ page.url | default(value='/') }}">
{% if page.robots %}<meta name="robots" content="{{ page.robots }}">{% endif %}

<!-- Open Graph -->
<meta property="og:type" content="{% if page.collection %}article{% else %}website{% endif %}">
<meta property="og:url" content="{{ site.base_url }}{{ page.url | default(value='/') }}">
<meta property="og:title" content="{{ page.title | default(value=site.title) }}">
<meta property="og:description" content="{{ page.description | default(value=site.description) }}">
{% if page.image %}{% set _abs_image = page.image %}{% if not page.image is starting_with("http") %}{% set _abs_image = site.base_url ~ page.image %}{% endif %}
<meta property="og:image" content="{{ _abs_image }}">
<meta property="og:image:width" content="1200"><meta property="og:image:height" content="630">{% endif %}
<meta property="og:site_name" content="{{ site.title }}">
<meta property="og:locale" content="{{ lang }}">
{% if page.collection and page.date %}<meta property="article:published_time" content="{{ page.date }}">{% endif %}
{% if page.collection and page.updated %}<meta property="article:modified_time" content="{{ page.updated }}">{% endif %}

<!-- Twitter Card -->
<meta name="twitter:card" content="{% if page.image %}summary_large_image{% else %}summary{% endif %}">
<meta name="twitter:title" content="{{ page.title | default(value=site.title) }}">
<meta name="twitter:description" content="{{ page.description | default(value=site.description) }}">
{% if page.image %}<meta name="twitter:image" content="{{ _abs_image }}">{% endif %}

<!-- Discovery links -->
<link rel="alternate" type="application/rss+xml" title="{{ site.title }}" href="{{ lang_prefix }}/feed.xml">
<link rel="alternate" type="text/plain" title="LLM Summary" href="{{ lang_prefix }}/llms.txt">
{% if page.url %}<link rel="alternate" type="text/markdown" href="{{ site.base_url }}{{ page.url }}.md">{% endif %}

<!-- Multi-language alternates -->
{% if translations %}{% for t in translations %}
<link rel="alternate" hreflang="{{ t.lang }}" href="{{ site.base_url }}{{ t.url }}">
{% endfor %}
<link rel="alternate" hreflang="x-default" href="{{ site.base_url }}{{ page.url | default(value='/') }}">
{% endif %}
```

## Adding JSON-LD Structured Data

Add this before `</head>` for rich search results:

```html
<script type="application/ld+json">
{% set _url = site.base_url ~ page.url | default(value='/') %}
{% set _title = page.title | default(value=site.title) %}
{% set _desc = page.description | default(value=site.description) %}
{% if page.collection == 'posts' %}
{"@context":"https://schema.org","@type":"BlogPosting",
 "headline":{{ _title | json_encode() }},
 "description":{{ _desc | json_encode() }},
 "datePublished":{{ page.date | default(value='') | json_encode() }},
 {% if page.updated %}"dateModified":{{ page.updated | json_encode() }},{% endif %}
 "author":{"@type":"Person","name":{{ site.author | json_encode() }}},
 "url":{{ _url | json_encode() }}}
{% elif page.collection %}
{"@context":"https://schema.org","@type":"Article",
 "headline":{{ _title | json_encode() }},
 "description":{{ _desc | json_encode() }},
 "url":{{ _url | json_encode() }}}
{% else %}
{"@context":"https://schema.org","@type":"WebSite",
 "name":{{ site.title | json_encode() }},
 "description":{{ site.description | json_encode() }},
 "url":{{ site.base_url | json_encode() }}}
{% endif %}
</script>
```

All bundled themes also emit a **BreadcrumbList** on collection pages (Home → Collection → Page):

```html
{% if page.collection %}{% set _bc_col_url = site.base_url ~ lang_prefix ~ "/" ~ page.collection %}
<script type="application/ld+json">
{"@context":"https://schema.org","@type":"BreadcrumbList",
 "itemListElement":[
   {"@type":"ListItem","position":1,"name":{{ site.title | json_encode() }},"item":{{ site.base_url | json_encode() }}},
   {"@type":"ListItem","position":2,"name":{{ page.collection | title | json_encode() }},"item":{{ _bc_col_url | json_encode() }}},
   {"@type":"ListItem","position":3,"name":{{ _title | json_encode() }}}
 ]}
</script>
{% endif %}
```

## Adding Navigation

Render `data.nav` for header links (all bundled themes do this):

```html
{% if data.nav %}
<nav class="site-nav" aria-label="Main navigation">
    {% for item in data.nav %}
    {% if item.external %}
    <a href="{{ item.url }}" target="_blank" rel="noopener">{{ item.title }}</a>
    {% else %}
    <a href="{{ lang_prefix }}{{ item.url }}">{{ item.title }}</a>
    {% endif %}
    {% endfor %}
</nav>
{% endif %}
```

Note the `{{ lang_prefix }}` on internal links — this ensures URLs are correct for multilingual sites.

## Adding Search

Search uses a JSON index generated at build time. Add the search UI and JavaScript:

```html
<!-- In your header or sidebar -->
<form class="search-form" role="search" aria-label="{{ t.search_label | default(value='Search site content') }}">
    <input type="search" id="search-input"
           placeholder="{{ t.search_placeholder | default(value='Search…') }}"
           autocomplete="off">
</form>
<div id="search-results" aria-live="polite"></div>

<!-- Before </body> -->
<script>
(function(){
    var input = document.getElementById('search-input');
    var results = document.getElementById('search-results');
    if (!input) return;
    var index = null;
    input.addEventListener('focus', function() {
        if (index) return;
        fetch('{{ lang_prefix }}/search-index.json')
            .then(function(r) { return r.json(); })
            .then(function(data) { index = data; });
    });
    input.addEventListener('input', function() {
        var q = input.value.toLowerCase().trim();
        if (!q || !index) { results.innerHTML = ''; return; }
        var matches = index.filter(function(item) {
            return item.title.toLowerCase().includes(q)
                || (item.description || '').toLowerCase().includes(q)
                || (item.tags || []).some(function(t) { return t.toLowerCase().includes(q); });
        }).slice(0, 8);
        if (!matches.length) {
            results.innerHTML = '<div class="no-results">{{ t.no_results | default(value="No results") }}</div>';
            return;
        }
        results.innerHTML = matches.map(function(m) {
            return '<a href="' + m.url + '">' + m.title +
                   (m.description ? '<div class="result-meta">' + m.description + '</div>' : '') +
                   '</a>';
        }).join('');
    });
})();
</script>
```

## Adding Pagination

When a collection has `paginate = N` in config, the `{{ pagination }}` context is available:

```html
{% if pagination %}
<nav class="pagination" aria-label="Pagination">
    {% if pagination.previous_url %}
    <a href="{{ pagination.previous_url }}">{{ t.newer | default(value="Newer") }}</a>
    {% endif %}
    <span>{{ t.page_n_of_total | default(value="Page") | replace(from="{n}", to=pagination.current_page) | replace(from="{total}", to=pagination.total_pages) }}</span>
    {% if pagination.next_url %}
    <a href="{{ pagination.next_url }}">{{ t.older | default(value="Older") }}</a>
    {% endif %}
</nav>
{% endif %}
```

## Adding a Language Switcher

For multilingual sites, show available translations:

```html
{% if translations | length > 0 %}
<div class="lang-switcher">
    <strong>{{ lang | upper }}</strong>
    {% for tr in translations %}
    <a href="{{ tr.url }}">{{ tr.lang | upper }}</a>
    {% endfor %}
</div>
{% endif %}
```

## Accessibility Checklist

Every theme should include:

- **Skip-to-main link** — `<a href="#main" class="skip-link">Skip to main content</a>` as the first element in `<body>`
- **Landmark roles** — `role="search"` on search forms, `aria-label` on navigation
- **Live regions** — `aria-live="polite"` on search results for screen reader announcements
- **Focus rings** — visible focus indicators on all interactive elements (don't remove `outline`)
- **Reduced motion** — `@media (prefers-reduced-motion: reduce)` to disable animations

```css
.skip-link {
    position: absolute;
    left: -9999px;
    top: 0;
    z-index: 100;
    padding: 0.5rem 1rem;
    background: #fff;
}
.skip-link:focus {
    left: 1rem;
}
@media (prefers-reduced-motion: reduce) {
    *, *::before, *::after {
        animation-duration: 0.01ms !important;
        transition-duration: 0.01ms !important;
    }
}
```

## CSS Approach: Why Inline Styles

seite themes use inline CSS (inside `<style>` tags in the template) rather than external stylesheets or preprocessors like Sass. This is a deliberate design choice:

- **Single-file themes** — one `.tera` file contains everything. Copy it, share it, install it from a URL. No asset dependencies to manage.
- **Zero build tools** — no Sass compiler, no PostCSS, no Node.js. The single Rust binary handles everything.
- **Instant portability** — themes work identically on any machine without toolchain setup.
- **AI-friendly** — when `seite theme create` generates a theme, it produces one complete file. No multi-file coordination needed.

If you need advanced CSS features, modern CSS covers most use cases that previously required preprocessors:

| Sass feature | CSS equivalent |
|-------------|----------------|
| Variables (`$color`) | Custom properties (`--color`) |
| Nesting (`.a { .b {} }`) | Native CSS nesting (`.a { .b {} }`) |
| Color functions | `color-mix()`, `oklch()` |
| Math | `calc()`, `min()`, `max()`, `clamp()` |

For projects that genuinely need Sass, compile it externally (`sass style.scss static/style.css`) and reference the output in your template. seite copies everything in `static/` to the output directory.

## Copying a Bundled Theme

To customize a bundled theme without starting from scratch:

```bash
# Apply the theme you want to start from
seite theme apply dark

# Now edit templates/base.html directly
```

The applied theme becomes your `templates/base.html`. Modify colors, fonts, spacing, layout — anything you want. The dev server (`seite serve`) live-reloads your changes instantly.

## Shortcode CSS

If your content uses built-in shortcodes, include CSS for them:

```css
/* Video embeds (YouTube, Vimeo) */
.video-embed { position: relative; padding-bottom: 56.25%; height: 0; overflow: hidden; margin: 1.5rem 0; }
.video-embed iframe { position: absolute; top: 0; left: 0; width: 100%; height: 100%; border: 0; }

/* Callouts */
.callout { border-left: 4px solid #0057b7; background: #f0f4ff; padding: 1rem 1.25rem; margin: 1.5rem 0; border-radius: 0 4px 4px 0; }
.callout-title { font-weight: 600; margin-bottom: 0.5rem; font-size: 0.9rem; }
.callout-info { border-left-color: #0057b7; background: #f0f4ff; }
.callout-warning { border-left-color: #d97706; background: #fffbeb; }
.callout-tip { border-left-color: #059669; background: #ecfdf5; }

/* Figures */
figure { margin: 1.5rem 0; }
figure img { display: block; max-width: 100%; }
figcaption { font-size: 0.85rem; color: #666; margin-top: 0.5rem; }
```

## Testing Your Theme

After building your theme, verify these work:

1. **Build succeeds** — `seite build` with no errors
2. **Homepage renders** — check `/` in the dev server
3. **Posts render** — check a post page with tags, date, reading time
4. **Docs render** — check a doc page (sidebar navigation if using docs theme)
5. **Search works** — type in the search box, results appear
6. **Pagination works** — if you have enough posts, page navigation renders
7. **Tags work** — click a tag, `/tags/{tag}/` shows filtered results
8. **RSS link** — `/feed.xml` returns valid XML
9. **Mobile** — resize browser below 768px, layout adapts
10. **Accessibility** — Tab through the page, focus rings are visible

## Exporting and Sharing

Package your theme for others:

```bash
seite theme export my-theme --description "Dark theme with green accents"
```

This saves `templates/themes/my-theme.tera` with metadata. Host the file anywhere and others install it with:

```bash
seite theme install https://your-site.com/themes/my-theme.tera
```

## Next Steps

- [Templates & Themes](/docs/templates) — all template variables, blocks, and data file integration
- [Theme Gallery](/docs/theme-gallery) — visual previews of all 6 bundled themes
- [Shortcodes](/docs/shortcodes) — content components your theme CSS should support
