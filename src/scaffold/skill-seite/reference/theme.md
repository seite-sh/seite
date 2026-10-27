# /seite theme [name | description]

A theme is `templates/base.html` (layout) plus its CSS. Collection templates (`post.html`, `doc.html`, …) extend it and stay as they are. The bundled themes are examples and starting points.

## List or apply

- No argument: `seite theme list` (starters, the gallery of examples, installed themes).
- A bundled theme name: `seite theme apply <name>` (a customized `base.html` is backed up), then verify as in [check.md](check.md).

## Design a custom theme (`/seite theme <description>`)

1. Start from the blank starter: if `seite context` reports a bundled theme other than `blank` (or no `templates/base.html`), run `seite theme new` (it writes `templates/base.html` and `static/css/site.css`). Never paint a new design over a bundled example. A customized `base.html` is the user's work: restyle it in place instead.
2. Design decisions: if impeccable is installed (`.claude/skills/impeccable/` or `.agents/skills/impeccable/` exists), use it: `/impeccable init` once if `PRODUCT.md` is missing, then `/impeccable <description>` (Codex: `$impeccable`), applied within the contract below. Otherwise decide from the description, and mention once that `seite skill install impeccable` adds design tooling.
3. Edit layout in `templates/base.html` and the look in `static/css/site.css` (colors from `data/brand.yaml` if it exists). Never make standalone HTML mockups to port later.
   Tailwind (the user asks for it, or `seite context` shows `Tailwind:`): start with `seite theme new --tailwind` instead; style with utility classes in templates and content, design tokens in the `@theme` block of `static/css/tailwind.css`, and keep base.html's link to `/static/css/tailwind.css` (the build compiles it; no Node or npm).
4. Preview as in [preview.md](preview.md), once after the first visible change.
5. Verify, bounded: `seite check`, fix what it reports, `seite check` once more, stop. If impeccable is installed, run `seite build` and then one `impeccable detect dist/`, fix what it reports, and stop.
6. Optional: `seite theme export <name>` saves the theme as `templates/themes/<name>.tera` (share `static/css/site.css` with it).

## Contract for base.html

- A full HTML document that extends nothing: `<html lang="{{ lang }}">`, `<meta charset>`, viewport meta, `<title>{% block title %}{{ site.title }}{% endblock %}</title>`.
- seite's partials, included once each and never copied or removed:
  - `{% include "seite/head.html" %}` right after `<title>`: canonical, description, Open Graph/Twitter, JSON-LD, hreflang, feeds, `llms.txt`, markdown alternate, robots, KaTeX. Do not add these tags yourself.
  - `{% include "seite/search.html" %}` where the search box goes, `{% include "seite/lang-switcher.html" %}` where the language links go.
  - `{% include "seite/scripts.html" %}` right before `</body>` (the search script).
- Blocks: `title`, `head`, `extra_css`, `header`, `content` (inside `<main id="main">`), `footer`, `extra_js`.
- CSS: `<link rel="stylesheet" href="/static/css/site.css">` (root-relative; seite handles `base_path` and fingerprinting).
- Body: `{{ page.content | safe }}` on pages; loop over `collections` on index pages; render `pagination` when it is set. Navigation from `data.nav`, docs sidebar from `nav`.
- i18n: UI text from `{{ t.<key> }}`, internal links prefixed with `{{ lang_prefix }}`, never hardcoded English.
- Accessibility: a skip link to `#main` first in `<body>`, one `<h1>` per page (the page title; the site name in the header is not an `<h1>`), visible focus styles, and `prefers-reduced-motion` respected.
