---
paths:
  - "src/themes/**"
  - "src/templates/**"
---
# Templates & Themes

## Tera Templates
- Jinja2-compatible, all extend `base.html`
- Page variables: `{{ site.title }}`, `{{ page.title }}`, `{{ page.content | safe }}`, `{{ page.description }}`, `{{ page.date }}`, `{{ page.updated }}`, `{{ page.image }}`, `{{ page.slug }}`, `{{ page.tags }}`, `{{ page.url }}`, `{{ page.collection }}`, `{{ page.robots }}`, `{{ page.word_count }}`, `{{ page.reading_time }}`, `{{ page.excerpt }}`, `{{ page.toc }}`, `{{ page.extra }}`
- Global: `{{ collections }}`, `{{ lang }}`, `{{ default_language }}`, `{{ lang_prefix }}`, `{{ t }}`, `{{ translations }}`, `{{ nav }}`, `{{ data }}`
- Blocks: `{% block title %}`, `{% block content %}`, `{% block head %}`, `{% block extra_css %}`, `{% block extra_js %}`, `{% block header %}`, `{% block footer %}`
- User `templates/` overrides embedded defaults

## 10 Bundled Themes (`.tera` files in `src/themes/`)
- `default` — 720px column, system-ui, blue links
- `minimal` — 600px column, Georgia serif, literary feel
- `dark` — True black, violet accent
- `docs` — Fixed 260px sidebar with auto-scrolling nav
- `brutalist` — Cream, 3px borders, hard shadows, yellow accent
- `bento` — CSS grid, article cards with border-radius 20px
- `landing` — Marketing landing page with hero section
- `terminal` — Monospace hacker aesthetic, green-on-black
- `magazine` — Multi-column editorial layout
- `academic` — Clean scholarly style with serif typography

Each compiled via `include_str!` and registers as `base.html` when applied. Edit the `.tera` files directly.

## Theme Gallery
- `seite theme install <url>` → `templates/themes/<name>.tera`
- `seite theme export <name>` packages current theme
- Metadata: `{#- theme-description: ... -#}` Tera comment in first 10 lines
- `seite theme create "<description>"` spawns Claude to write `templates/base.html`

## All themes include
- hreflang tags + language switcher when `translations` non-empty
- canonical URL, Open Graph, Twitter Card, JSON-LD, `<meta name="robots">`, markdown alternate, llms.txt link
- Accessibility: skip-to-main, `role="search"`, `aria-label`, `aria-live="polite"`, `prefers-reduced-motion`
- CSS for `.video-embed`, `.callout-*`, `figure`/`figcaption`, `.contact-form`
- CSS for changelog badges, roadmap layouts (grouped/kanban/timeline), trust center

## Frontmatter Serialization
- `serde_yaml_ng` for YAML parsing
- `skip_serializing_if` on all optional fields
- Draft field only serialized when `true`
