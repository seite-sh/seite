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
| `page.slug` | string | URL slug (e.g., `hello-world`) |
| `page.collection` | string | Collection name (e.g., `posts`) — empty string on homepage |
| `page.lang` | string | Language code for this page |
| `page.robots` | string? | Per-page robots directive (from `robots:` frontmatter) |
| `page.word_count` | number | Word count |
| `page.reading_time` | number | Estimated reading time in minutes (238 WPM) |
| `page.excerpt` | string? | Auto-extracted excerpt (from `<!-- more -->` marker or first paragraph) |
| `page.toc` | string | Auto-generated table of contents HTML from heading hierarchy |
| `page.extra` | object | Arbitrary data from `extra:` frontmatter — access via `{{ page.extra.field }}` |
| `nav` | array | Sidebar nav sections `[{name, label, items: [{title, url, active}]}]` |

Index template also gets:

| Variable | Type | Description |
|----------|------|-------------|
| `collections` | array | Listed collections `[{name, label, items}]` |
| `page` | object? | Homepage content (if `content/pages/index.md` exists) |
| `pagination` | object? | Pagination context (when collection has `paginate = N` — see Key Conventions) |

### Customizing Templates

Edit files in `templates/` to customize. Key rules:

- `base.html` is the root layout — all other templates extend it via `{% extends "base.html" %}`
- Content goes in `{% block content %}...{% endblock %}`
- Title goes in `{% block title %}...{% endblock %}`

### Overridable Blocks

All bundled themes provide these blocks for extension without copying the full `base.html`:

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

