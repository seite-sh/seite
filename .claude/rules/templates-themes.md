---
paths:
  - "src/themes/**"
  - "src/themes.rs"
  - "src/templates/**"
---
# Templates & Themes

## Tera Templates
- Jinja2-compatible, all extend `base.html`
- Page variables: `{{ site.title }}`, `{{ page.title }}`, `{{ page.content | safe }}`, `{{ page.description }}`, `{{ page.date }}`, `{{ page.updated }}`, `{{ page.image }}`, `{{ page.slug }}`, `{{ page.tags }}`, `{{ page.url }}`, `{{ page.markdown_url }}` (published `.md` path, unset when none), `{{ page.collection }}`, `{{ page.robots }}`, `{{ page.word_count }}`, `{{ page.reading_time }}`, `{{ page.excerpt }}`, `{{ page.toc }}`, `{{ page.extra }}`
- Adjacent posts: `{{ prev_post }}` (older), `{{ next_post }}` (newer) — each has `.title`, `.url`, `.date`, `.description`
- Content pages also get `{{ page.taxonomies }}` (`{tags: [{name, slug, url}], categories: [...]}`), `{{ page.related }}` (`[{title, url, date, description, score}]`), `{{ page.series }}` (`{name, slug, url, parts, index, total, prev, next}` or null)
- Listing templates: `tags.html`/`taxonomy.html` (`taxonomy`, `terms`), `tag.html`/`term.html` (`taxonomy`, `term`, `terms`, `items`, `pagination`; tag pages keep `tags`, `tag_name`, `tags_url`), `series.html` (`series`, `items`)
- Post-page markup for categories/series/related lives in `DEFAULT_POST` and reuses the themes' `.tags` and `.article-nav` classes (no theme CSS needed)
- Global: `{{ collections }}`, `{{ lang }}`, `{{ default_language }}`, `{{ lang_prefix }}`, `{{ t }}`, `{{ translations }}`, `{{ nav }}`, `{{ data }}`
- Blocks: `{% block title %}`, `{% block content %}`, `{% block head %}`, `{% block extra_css %}`, `{% block extra_js %}`, `{% block header %}`, `{% block footer %}`, optional `{% block site_title %}` (blank's header site name; `DEFAULT_INDEX` overrides it with an `<h1>`)
- User `templates/` overrides embedded defaults

## seite partials (`src/templates/seite/*.html`, `SEITE_PARTIALS` in `src/templates/mod.rs`)
seite owns correctness, themes own layout + CSS. Registered under the reserved `seite/` namespace unless the site has `templates/seite/<name>` (override = no more automatic fixes). Each starts with `{# seite partial: <name> — managed by seite, updated on upgrade … #}` (comment and content on one line: Tera's `-#}` does not trim, and the partials are byte-tuned so themes render identically).
- `seite/head.html` — everything SEO/GEO after `<title>`: description, canonical, OG/Twitter, JSON-LD, BreadcrumbList, robots, RSS/Atom/llms.txt, markdown alternate, hreflang, KaTeX css. Generated OG cards: `page.og_image` / `site.og_image` branch next to the `og:image` block.
- `seite/search.html` (form + `#search-results`), `seite/search-script.html` (search JS), `seite/scripts.html` (end-of-body aggregator: add new seite-owned scripts as one more include), `seite/lang-switcher.html`
- `<title>{% block title %}` stays in the theme: Tera blocks cannot live in includes
- First line has no indent and the file has no trailing newline: the include site supplies both
- `template-missing-seite-head` warning (`templates::seite_head_diagnostics`) when a user `base.html` lacks the head include; "legacy theme" when it has its own `og:title`/JSON-LD

## Bundled themes (`src/themes/*.tera`, `src/themes.rs`)
- Starters (`ThemeKind::Starter`, offered by `seite init --theme`): `default`, `docs`, `landing`, `blank` (semantic skeleton, CSS in `src/themes/blank.css` → `static/css/site.css`, written by apply/init/`theme new` only when missing)
- Gallery (`ThemeKind::Gallery`, `seite theme apply`): `minimal`, `dark`, `brutalist`, `bento`, `terminal`, `magazine`, `academic`
- Every theme includes the four partials exactly once and contains no `og:title`/JSON-LD/canonical/search-index of its own (unit test)
- **Editing a theme changes its hash: append it to `LEGACY_THEME_HASHES` in `src/cli/upgrade.rs`** (test `current_bundled_themes_are_in_legacy_table` prints it). `seite upgrade` replaces an unedited copy of any historical version with the current theme
- `tests/fixtures/themes-pre-partials/` holds the 0.20.x themes; `tests/integration/theme_partials.rs` asserts rendered output stayed equivalent (normalization documented there)

## Theme Gallery / custom themes
- `seite theme install <url>` → `templates/themes/<name>.tera`; `seite theme export <name>` packages current theme
- Metadata: `{#- theme-description: ... -#}` Tera comment in first 10 lines
- `seite theme new [name] [--force]` scaffolds `blank` (refuses a customized base.html without `--force`; backs up); with impeccable installed also writes `.impeccable/live/config.json` add-only
- `seite theme create "<description>"` scaffolds `blank` first when the active theme is a bundled example, then runs `/seite theme` (or points to `/impeccable`)

## Themes still carry (layout/CSS)
- Skip link to `#main`, `<html lang="{{ lang }}">`, blocks `title head extra_css header content footer extra_js`
- CSS for `.video-embed`, `.callout-*`, `figure`/`figcaption`, `.contact-form`, changelog/roadmap/trust classes, `prefers-reduced-motion`

## Frontmatter Serialization
- `serde_yaml_ng` for YAML parsing
- `skip_serializing_if` on all optional fields
- Draft field only serialized when `true`
