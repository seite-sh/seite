---
title: "Templates and Themes"
description: "Customize your seite static site with Tera templates, overridable blocks, seite-managed SEO partials, bundled starter and gallery themes, and data-driven navigation."
weight: 4
---

seite uses Tera templates to give you full control over your site's HTML output. Themes own layout and CSS; seite's partials own the SEO and GEO head, search and the language switcher, so those stay correct in every theme, including the ones you design.

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
| `{{ site.og_image }}` | Absolute URL of the language's generated site card (`[og]`), the `og:image` fallback; unset when cards are off |

### Page variables

| Variable | Description |
|----------|-------------|
| `{{ page.title }}` | Page title |
| `{{ page.content }}` | Rendered HTML content (use with `\| safe`) |
| `{{ page.date }}` | Publication date (posts) |
| `{{ page.updated }}` | Last modified date |
| `{{ page.description }}` | Page description |
| `{{ page.image }}` | Social preview image URL |
| `{{ page.collection_url }}` | Absolute URL of the page's collection index (with language prefix and `url_prefix`); unset for root-mounted collections such as `pages` |
| `{{ page.og_image }}` | Absolute URL of the page's generated social card (`[og]`); unset when the page has `image:` or no card of its own |
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

All bundled themes render `data.nav` and `data.footer` automatically when present. See [Configuration](/docs/configuration#data-files) for supported formats and directory structure.

## seite partials

seite ships five partials under the reserved `seite/` template namespace. Every bundled theme includes them, and so should yours:

| Partial | Include it | What it renders |
|---------|------------|-----------------|
| `seite/head.html` | inside `<head>`, right after `<title>` | description, canonical URL, Open Graph and Twitter tags (absolute `og:image`: `page.image`, else the generated card `page.og_image`, else the site card `site.og_image`), `article:*` times, JSON-LD (`BlogPosting`/`Article`/`WebSite` + `BreadcrumbList`), robots, RSS/Atom/`llms.txt` links, the markdown alternate (`page.markdown_url`), hreflang, the KaTeX stylesheet when `math` is on |
| `seite/search.html` | where the search box goes | the `role="search"` form (labelled by `t.search_label`) and the `#search-results` live region |
| `seite/scripts.html` | right before `</body>` | seite's end-of-body scripts (today: `seite/search-script.html`, which loads `search-index.json` on first input) |
| `seite/search-script.html` | (via `seite/scripts.html`) | the search script on its own |
| `seite/lang-switcher.html` | where the language links go | links to the page's translations; renders nothing on single-language pages |

The partials escape what they print: attribute values go through the `escape_attr` filter (`"`, `<`, `>` and a bare `&` become entities) and JSON-LD values through `json_script` (JSON with every `<` written as `\u003c`). Both filters are available to your own templates.

`<title>{% block title %}…{% endblock %}</title>` stays in the theme: Tera blocks cannot live in an included file, and collection templates override the title through that block.

The partials are embedded in the binary and **updated with seite**: SEO, GEO and search fixes reach every site on the next build, without touching `templates/base.html`. `seite check` warns (`template-missing-seite-head`) when a custom `base.html` does not include `seite/head.html`, directly or through templates it includes or extends.

**Overriding a partial.** A file at `templates/seite/<name>.html` replaces the embedded one, e.g. `templates/seite/search.html` for a different search box. An overridden partial is yours: it no longer receives seite's fixes, so override the smallest one that does what you need, and prefer CSS or the theme's own markup around the include.

## Bundled Themes

seite ships 11 themes inside the binary. They are examples and starting points; for your own look, see [Design your own theme](/docs/custom-themes).

**Starters** (offered by `seite init`, which picks `docs` for a docs-only site and `default` otherwise; `--theme` chooses):

- `default`: clean baseline. 720px column, system-ui font, blue links.
- `docs`: documentation layout with a fixed sidebar and auto-generated navigation.
- `landing`: marketing page with hero sections and calls to action.
- `blank`: bare semantic layout with its CSS in `static/css/site.css`, made for designing your own (`seite theme new`).

**Gallery** (examples, applied with `seite theme apply <name>`): `minimal`, `dark`, `brutalist`, `bento`, `terminal`, `magazine`, `academic`.

## Applying Themes

```bash
seite theme list              # Starters, gallery, installed themes
seite theme apply dark        # Apply a bundled theme
seite theme apply my-custom   # Apply an installed theme
seite theme new               # Start your own design from the blank starter
```

Applying writes `templates/base.html` (a customized one is backed up to `base.html.bak` first). `seite upgrade` replaces a bundled theme you never edited with its newest version. See the [Theme Gallery](/docs/theme-gallery) for previews.

## Installing & Sharing Themes

Download community themes from a URL, or export your own:

```bash
seite theme install https://example.com/themes/aurora.tera
seite theme install https://example.com/themes/aurora.tera --name my-aurora
seite theme export my-theme --description "Dark theme with green accents"
```

Installed themes are saved to `templates/themes/<name>.tera` and appear in `seite theme list`.

## Custom Themes with AI

```bash
seite theme new                                             # blank starter
seite theme create "minimal serif with warm earth tones"   # run your agent once
```

`seite theme create` runs your coding agent with `/seite theme <description>` (or points you to `/impeccable` when the impeccable design skill is installed). When the site is still on a bundled example it scaffolds `blank` first. The agent edits layout and CSS and keeps the `seite/*` includes. See [Design your own theme](/docs/custom-themes).

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
- [Theme Gallery](/docs/theme-gallery): the starters and the gallery of example themes
- [Shortcodes](/docs/shortcodes): add videos, callouts, and figures to your content
- [Configuration](/docs/configuration): data file setup and all `seite.toml` options
