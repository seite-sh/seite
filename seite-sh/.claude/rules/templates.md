---
paths:
  - "templates/**"
---
## Templates and Themes

Templates use [Tera](https://keats.github.io/tera/) syntax (Jinja2-compatible).

Hard rules:

- `base.html` is the theme; every other template starts with `{% extends "base.html" %}`.
- Render content with `{{ page.content | safe }}` (the `safe` filter is required).
- A theme owns layout and CSS only. seite owns SEO, search and i18n through partials that `base.html` includes: keep `{% include "seite/head.html" %}` (right after `<title>`), `{% include "seite/search.html" %}`, `{% include "seite/lang-switcher.html" %}` and `{% include "seite/scripts.html" %}` (before `</body>`). Move them within the layout freely; never copy a partial's contents into `base.html` and never delete an include when redesigning.
- Keep `<title>{% block title %}...{% endblock %}</title>` in `base.html` (blocks cannot live in a partial).
- Designers and agents (impeccable included) edit the layout in `base.html` and the look in CSS (`static/css/site.css` for themes started with `seite theme new`, the `<style>` element in the bundled examples).
- Build designs directly in `templates/` and `content/` and preview with `seite serve`; never write standalone HTML mockups.
- `seite theme apply <name>` overwrites `base.html` (a customized one is backed up first); `seite theme new` starts a custom design from the `blank` starter; `seite theme list` shows the starters and the gallery of examples.
- `templates/seite/<name>.html` overrides a seite partial and stops its automatic updates: only when nothing else works.

### Template Variables

Available in all templates:

| Variable | Type | Description |
|----------|------|-------------|
| `site.title` | string | Site title (language-specific if multilingual) |
| `site.description` | string | Site description |
| `site.base_url` | string | Base URL (e.g., `https://example.com`) |
| `site.language` | string | Default language code (configured in `seite.toml`) |
| `site.author` | string | Author name |
| `lang` | string | Current page language code |
| `default_language` | string | Default language code (same as `site.language`) |
| `lang_prefix` | string | URL prefix for current language (empty for default, `"/es"` for Spanish, etc.) |
| `t` | object | UI translation strings (override via `data/i18n/{lang}.yaml`) |
| `translations` | array | Translation links `[{lang, url}]` (empty if no translations) |
| `data` | object | Data files — `data/nav.yaml` → `{{ data.nav }}` (see Data Files section) |
| `page.title` | string | Page title |
| `page.content` | string | Rendered HTML (use `{{ page.content \| safe }}`) |
| `page.date` | string? | Publish date (if set) |
| `page.updated` | string? | Last-modified date (from `updated:` frontmatter) |
| `page.description` | string? | Page description |
| `page.image` | string? | Social-preview image URL (from `image:` frontmatter) |
| `page.tags` | array | Tags |
| `page.url` | string | URL path |
| `page.markdown_url` | string? | Path of the published markdown copy (e.g. `/posts/hello.md`, `/posts/index.md`); unset when no `.md` is written for the page |
| `page.slug` | string | URL slug (e.g., `hello-world`) |
| `page.collection` | string | Collection name (e.g., `posts`) — empty string on homepage |
| `page.lang` | string | Language code for this page |
| `page.robots` | string? | Per-page robots directive (from `robots:` frontmatter) |
| `page.word_count` | number | Word count |
| `page.reading_time` | number | Estimated reading time in minutes (238 WPM) |
| `page.excerpt` | string? | Auto-extracted excerpt (from `<!-- more -->` marker or first paragraph) |
| `page.toc` | string | Auto-generated table of contents HTML from heading hierarchy |
| `page.extra` | object | Arbitrary data from `extra:` frontmatter — access via `{{ page.extra.field }}` |
| `page.taxonomies` | object | Content pages: terms per taxonomy, e.g. `page.taxonomies.categories` = `[{name, slug, url}]` |
| `page.related` | array | Content pages: related pages `[{title, url, date, description, score}]` (same collection and language, by shared tags/categories) |
| `page.series` | object? | Content pages in a series: `{name, url, parts: [{title, url, current}], index, total, prev, next}` |
| `nav` | array | Sidebar nav sections `[{name, label, items: [{title, url, active}]}]` |

Index template also gets:

| Variable | Type | Description |
|----------|------|-------------|
| `collections` | array | Listed collections `[{name, label, items}]` |
| `page` | object? | Homepage content (if `content/pages/index.md` exists) |
| `pagination` | object? | Pagination context (when collection has `paginate = N` — see Key Conventions) |

Link tags with `page.taxonomies.tags[].url` (it follows the tags `url_prefix`), not a hard-coded `/tags/` path.

Taxonomy pages (`taxonomy.html`, `term.html`; tags use `tags.html`, `tag.html`) get `taxonomy` `{name, label, url}`, `terms` `[{name, slug, url, count, feed_url}]`, and on term pages `term` plus `items` and `pagination`. Series pages (`series.html`) get `series` and `items` (the parts in order).

### Customizing Templates

Edit files in `templates/` to customize. Key rules:

- `base.html` is the root layout — all other templates extend it via `{% extends "base.html" %}`
- Content goes in `{% block content %}...{% endblock %}`
- Title goes in `{% block title %}...{% endblock %}`

### Overridable Blocks

All bundled themes provide these blocks for extension without copying the full `base.html` (a custom theme must keep them too):

| Block | Purpose |
|-------|---------|
| `{% block title %}` | Page `<title>` tag |
| `{% block head %}` | Extra tags inside `<head>` (stylesheets, meta) |
| `{% block extra_css %}` | Additional CSS (inside `<style>` or `<link>`) |
| `{% block extra_js %}` | Additional JavaScript (before `</body>`) |
| `{% block header %}` | Site header / navigation |
| `{% block content %}` | Main page content |
| `{% block footer %}` | Site footer |

Example — adding a custom stylesheet without overriding the full theme:

```html
{% extends "base.html" %}
{% block extra_css %}<link rel="stylesheet" href="/static/custom.css">{% endblock %}
{% block content %}
  <h1>{{ page.title }}</h1>
  {{ page.content | safe }}
{% endblock %}
```
