---
title: "Templates & Themes"
description: "Customize your site with Tera templates, overridable blocks, and six bundled themes."
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
| `{{ lang }}` | Current language code |
| `{{ translations }}` | Available translations |
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
page theme apply dark      # Apply bundled theme
page theme list            # List all themes
```

## Custom Themes with AI

Generate a completely custom theme:

```bash
page theme create "minimal serif with warm earth tones and generous whitespace"
```

This uses Claude Code to generate a `templates/base.html` with all required blocks, SEO tags, search, and accessibility features.

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
