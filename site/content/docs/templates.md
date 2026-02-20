---
title: "Templates & Themes"
description: "Customize your site with Tera templates, overridable blocks, and six bundled themes."
weight: 4
---

## Template Engine

`page` uses Tera, a Jinja2-compatible template engine. All templates extend `base.html`. User templates in `templates/` override bundled defaults.

## Template Variables

### Site variables

| Variable | Description |
|----------|-------------|
| `{{ site.title }}` | Site title |
| `{{ site.description }}` | Site description |
| `{{ site.base_url }}` | Base URL (e.g., `https://example.com`) |
| `{{ site.language }}` | Default language code |
| `{{ site.author }}` | Site author |

### Page variables

| Variable | Description |
|----------|-------------|
| `{{ page.title }}` | Page title |
| `{{ page.content }}` | Rendered HTML content (use with `\| safe`) |
| `{{ page.date }}` | Publication date (posts) |
| `{{ page.updated }}` | Last modified date |
| `{{ page.description }}` | Page description |
| `{{ page.image }}` | Social preview image URL |
| `{{ page.slug }}` | URL slug |
| `{{ page.tags }}` | List of tags |
| `{{ page.url }}` | Page URL path |
| `{{ page.collection }}` | Collection name |
| `{{ page.robots }}` | Robots meta directive |
| `{{ page.word_count }}` | Word count |
| `{{ page.reading_time }}` | Reading time in minutes |
| `{{ page.excerpt }}` | Auto-extracted excerpt (HTML) |
| `{{ page.toc }}` | Table of contents entries |
| `{{ page.extra }}` | Custom frontmatter data |

### Context variables

| Variable | Description |
|----------|-------------|
| `{{ collections }}` | List of collections (index pages) |
| `{{ nav }}` | Navigation sections (doc pages) |
| `{{ data }}` | Data files from `data/` directory |
| `{{ lang }}` | Current language code |
| `{{ translations }}` | Available translations |
| `{{ default_language }}` | Default language code (from `seite.toml`) |
| `{{ lang_prefix }}` | URL prefix for current language (empty for default, `"/es"` for others) |
| `{{ t }}` | UI translation strings object (override via `data/i18n/{lang}.yaml`) |
| `{{ pagination }}` | Pagination context |

## Overridable Blocks

All bundled themes provide these blocks for customization:

```html
{% block title %}...{% endblock %}       <!-- Page title -->
{% block head %}{% endblock %}            <!-- Extra head content -->
{% block extra_css %}{% endblock %}       <!-- Page-specific CSS -->
{% block header %}...{% endblock %}       <!-- Header/nav area -->
{% block content %}...{% endblock %}      <!-- Main content -->
{% block footer %}{% endblock %}          <!-- Footer content -->
{% block extra_js %}{% endblock %}        <!-- Page-specific JS -->
```

To override a block, create a template that extends `base.html`:

```html
{% extends "base.html" %}

{% block extra_css %}
<style>
  .custom-class { color: red; }
</style>
{% endblock %}

{% block content %}
<div class="custom-class">
  {{ page.content | safe }}
</div>
{% endblock %}
```

Here's a more complete example — a custom `post.html` with reading time, tags, and a back link:

```html
{% extends "base.html" %}

{% block content %}
<article>
  <h1>{{ page.title }}</h1>
  <p>{{ page.date }} · {{ page.reading_time }} min read</p>
  {{ page.content | safe }}
  {% if page.tags %}
  <div class="tags">
    {% for tag in page.tags %}<span>{{ tag }}</span>{% endfor %}
  </div>
  {% endif %}
  <a href="/posts">Back to all posts</a>
</article>
{% endblock %}
```

{{% callout(type="info") %}}
Always use `| safe` with `{{ page.content }}`. Tera escapes HTML by default, so without `| safe` your rendered content would display as raw HTML tags.
{{% end %}}

## Extra Frontmatter

Pass arbitrary data to templates using the `extra` field:

```yaml
---
title: "My Page"
extra:
  hero_image: /static/hero.jpg
  show_sidebar: false
  custom_color: "#ff6600"
---
```

Access in templates:

```html
{% if page.extra.hero_image %}
<img src="{{ page.extra.hero_image }}" alt="Hero">
{% endif %}
```

{{% callout(type="tip") %}}
Create `data/nav.yaml` with your links and every bundled theme renders header navigation automatically — no template editing needed.
{{% end %}}

## Data Files in Templates

Place YAML, JSON, or TOML files in the `data/` directory to inject structured data into all templates. Files are accessible via `{{ data.filename }}`.

### Navigation example

Create `data/nav.yaml`:

```yaml
- title: Blog
  url: /posts
- title: About
  url: /about
```

Use in templates:

```html
{% if data.nav %}
<nav>
  {% for item in data.nav %}
  <a href="{{ item.url }}">{{ item.title }}</a>
  {% endfor %}
</nav>
{% endif %}
```

### Footer example

Create `data/footer.yaml`:

```yaml
links:
  - title: GitHub
    url: https://github.com/user/repo
copyright: "2026 My Company"
```

Use in templates:

```html
{% if data.footer %}
  {% if data.footer.links %}
  <nav>
    {% for link in data.footer.links %}
    <a href="{{ link.url }}">{{ link.title }}</a>
    {% endfor %}
  </nav>
  {% endif %}
  <p>{{ data.footer.copyright }}</p>
{% endif %}
```

All 6 bundled themes render `data.nav` and `data.footer` automatically when present. See [Configuration](/docs/configuration#data-files) for supported formats and directory structure.

## Bundled Themes

Six themes ship with the binary — no downloads needed:

### default
Clean baseline. 720px centered column, system-ui font, blue links. Good starting point.

### minimal
Literary/essay feel. 600px column, Georgia serif, bottom-border-only search input. Typography carries all personality.

### dark
True black (`#0a0a0a`) background, violet (`#8b5cf6`) accent. Styled focus rings, high-contrast text.

### docs
Documentation layout. Fixed 260px sidebar with auto-generated navigation, GitHub-style colors. Best for technical documentation.

### brutalist
Neo-brutalist design. Cream background, 3px black borders, hard non-blurred shadows, yellow (`#ffe600`) accent. No border-radius.

### bento
Card grid layout. 1000px column, CSS grid with mixed card sizes, border-radius 20px, soft shadows. Dark/indigo accent cards.

## Applying Themes

```bash
seite theme list              # Show all bundled + installed themes
seite theme apply dark        # Apply a bundled theme
seite theme apply my-custom   # Apply an installed theme
```

See the [Theme Gallery](/docs/theme-gallery) for visual previews of all bundled themes.

## Installing & Sharing Themes

Download community themes from a URL, or export your own:

```bash
seite theme install https://example.com/themes/aurora.tera
seite theme install https://example.com/themes/aurora.tera --name my-aurora
seite theme export my-theme --description "Dark theme with green accents"
```

Installed themes are saved to `templates/themes/<name>.tera` and appear in `seite theme list`.

## Custom Themes with AI

Generate a completely custom theme:

```bash
seite theme create "minimal serif with warm earth tones and generous whitespace"
```

This uses Claude Code to generate a `templates/base.html` with all required blocks, SEO tags, search, and accessibility features. Export AI-generated themes to share them:

```bash
seite theme export earth-tones --description "Warm serif theme with earth tones"
```

## Table of Contents

Docs and posts automatically get a table of contents. Headings receive `id` anchors, and `{{ page.toc }}` provides the structured data:

```html
{% if page.toc %}
<nav class="toc">
  {% for entry in page.toc %}
  <a href="#{{ entry.id }}" style="padding-left: {{ entry.level }}em">
    {{ entry.text }}
  </a>
  {% endfor %}
</nav>
{% endif %}
```

## Translatable UI Strings

All bundled themes and default templates use the `{{ t }}` object for UI text. This allows multilingual sites to translate interface strings without overriding entire themes.

### Default Keys

| Key | Default (English) |
|-----|-------------------|
| `t.search_placeholder` | Search… |
| `t.skip_to_content` | Skip to main content |
| `t.no_results` | No results |
| `t.newer` | Newer |
| `t.older` | Older |
| `t.page_n_of_total` | Page {n} of {total} |
| `t.search_label` | Search site content |
| `t.min_read` | min read |
| `t.contents` | Contents |
| `t.tags` | Tags |
| `t.all_tags` | All tags |
| `t.tagged` | Tagged |
| `t.changelog` | Changelog |
| `t.roadmap` | Roadmap |
| `t.not_found_title` | Page Not Found |
| `t.not_found_message` | The page you requested could not be found. |
| `t.go_home` | Go to the homepage |
| `t.in_progress` | In Progress |
| `t.planned` | Planned |
| `t.done` | Done |
| `t.other` | Other |

### Overriding Strings

Create `data/i18n/{lang}.yaml` to override any key for a specific language:

```yaml
# data/i18n/es.yaml
search_placeholder: "Buscar…"
skip_to_content: "Ir al contenido principal"
no_results: "Sin resultados"
newer: "Más recientes"
older: "Más antiguos"
```

## Next Steps

- [Theme Gallery](/docs/theme-gallery) — browse all six bundled themes with visual previews
- [Shortcodes](/docs/shortcodes) — add videos, callouts, and figures to your content
- [Configuration](/docs/configuration) — data file setup and all `seite.toml` options
