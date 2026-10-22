---
title: "Templates and Themes"
description: "Customize your seite static site with Tera templates, overridable blocks, 10 bundled themes, and data-driven navigation. No JavaScript framework needed."
weight: 4
---

seite uses Tera templates to give you full control over your site's HTML output. The static site generator ships 10 bundled themes and supports AI-generated custom themes, all as single self-contained files with no external dependencies.

## Template Engine

`seite` uses Tera, a Jinja2-compatible template engine. All templates extend `base.html`. User templates in `templates/` override bundled defaults.

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
| `{{ page.markdown_url }}` | Path of the page's published markdown copy (e.g. `/posts/hello.md`, `/posts/index.md`); unset when no `.md` file is written (empty homepage, paginated pages after the first, tag pages, 404) |
| `{{ page.collection }}` | Collection name |
| `{{ page.robots }}` | Robots meta directive |
| `{{ page.word_count }}` | Word count |
| `{{ page.reading_time }}` | Reading time in minutes |
| `{{ page.excerpt }}` | Auto-extracted excerpt (HTML) |
| `{{ page.toc }}` | Table of contents entries |
| `{{ page.extra }}` | Custom frontmatter data (`extra:`). Custom schema fields appear here wherever they are written (top level or under `extra:`), e.g. `{{ page.extra.author }}` |
| `{{ page.taxonomies }}` | Content pages: terms per taxonomy, tags first: `page.taxonomies.categories` is a list of `{name, slug, url}` |
| `{{ page.related }}` | Content pages: related pages of the same collection and language, `[{title, url, date, description, score}]` (empty when none; see [Related pages](/docs/collections#related-pages)) |
| `{{ page.series }}` | Content pages in a series: `{name, slug, url, parts: [{title, url, current}], index, total, prev, next}` (`index` is 1-based; `prev`/`next` are `{title, url}` or unset); unset otherwise |
| `{{ prev_post }}` | Previous (older) post link: `.title`, `.url`, `.date`, `.description` |
| `{{ next_post }}` | Next (newer) post link: `.title`, `.url`, `.date`, `.description` |

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
| `{{ taxonomy }}` | Taxonomy index and term pages: `{name, label, url}` |
| `{{ terms }}` | Taxonomy index and term pages: every term `{name, slug, url, count, feed_url}` (`feed_url` only when the term has feeds) |
| `{{ term }}` | Term pages: the listed term `{name, slug, url, count, feed_url}` |
| `{{ series }}` | Series index pages (`series.html`): `{name, slug, url, parts, total}`; `items` lists the parts in order |

Tag pages (`tags.html`, `tag.html`) also keep their original variables: `tags` (same as `terms`), `tag_name`, `tags_url`, and `items`.

### Taxonomy, term and series templates

| Template | Renders | Default |
|----------|---------|---------|
| `tags.html` / `tag.html` | `/tags/` and `/tags/<slug>/` | embedded |
| `taxonomy.html` | Index of any other taxonomy (`/categories/`) | embedded; override per taxonomy with `template` |
| `term.html` | Term pages (`/categories/rust/`, `/categories/rust/page/2/`) | embedded; override per taxonomy with `term_template` |
| `series.html` | Series index pages (`/series/<slug>/`) | embedded |

The default `post.html` renders categories (and any other taxonomy) next to the tags, a series box ("Part 2 of 5 · next/previous part") after the content, and a "Related" list after the post navigation. Tag links come from `page.taxonomies.tags[].url`, so they follow the tags taxonomy's `url_prefix`. Sites created with an earlier seite have their own copy of `templates/post.html`: `seite upgrade` replaces it with the new default when it is exactly a default seite wrote. To add the features to an edited copy, use for example:

```html
{% if page.series %}<nav class="article-nav series-nav" aria-label="{{ t.series }}">
  <span>{{ t.series_part | replace(from="{n}", to=page.series.index ~ "") | replace(from="{total}", to=page.series.total ~ "") }} · <a href="{{ page.series.url }}">{{ page.series.name }}</a></span>
</nav>{% endif %}
{% if page.related %}<aside class="related"><h2>{{ t.related }}</h2><ul>
  {% for r in page.related %}<li><a href="{{ r.url }}">{{ r.title }}</a></li>{% endfor %}
</ul></aside>{% endif %}
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
| `t.prev_post` | Previous |
| `t.next_post` | Next |
| `t.in_progress` | In Progress |
| `t.planned` | Planned |
| `t.done` | Done |
| `t.other` | Other |
| `t.categories` | Categories |
| `t.related` | Related |
| `t.series` | Series |
| `t.series_part` | Part {n} of {total} |

A taxonomy without a configured `label` takes its display name from the UI string named after it (`t.tags`, `t.categories`, or a key you add in `data/i18n/{lang}.yaml`), so taxonomy labels translate like every other string.

### Overriding Strings

Create `data/i18n/{lang}.yaml` to override any key:

```yaml
# data/i18n/es.yaml
search_placeholder: "Buscar…"
skip_to_content: "Ir al contenido principal"
no_results: "Sin resultados"
newer: "Más recientes"
older: "Más antiguos"
```

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

Here's a more complete example: a custom `post.html` with reading time, tags, and a back link:

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
Always use `| safe` with `{{ page.content }}`. Tera escapes HTML by default, so without `| safe` your rendered content would display as raw HTML tags. Content can include rich elements via [shortcodes](/docs/shortcodes): videos, callouts, figures, and contact forms.
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
Create `data/nav.yaml` with your links and every bundled theme renders header navigation automatically, no template editing needed.
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

All 10 bundled themes render `data.nav` and `data.footer` automatically when present. See [Configuration](/docs/configuration#data-files) for supported formats and directory structure.

## Bundled Themes

10 themes ship with the binary, no downloads needed:

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

This runs your coding agent with `/seite theme <description>` (or points you to `/impeccable` when the impeccable design skill is installed) to build a `templates/base.html` with all required blocks, SEO tags, search, and accessibility features. Export AI-generated themes to share them:

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

## Next Steps

- [Building Custom Themes](/docs/custom-themes): step-by-step guide to creating a theme from scratch
- [Theme Gallery](/docs/theme-gallery): browse all 10 bundled themes with visual previews
- [Shortcodes](/docs/shortcodes): add videos, callouts, and figures to your content
- [Configuration](/docs/configuration): data file setup and all `seite.toml` options
