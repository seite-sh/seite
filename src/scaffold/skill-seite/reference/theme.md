# /seite theme [name | description]

A theme is `templates/base.html`. Collection templates (`post.html`, `doc.html`, …) extend it and stay as they are.

## List or apply

- No argument: `seite theme list`.
- A bundled theme name: `seite theme apply <name>` (a customized `base.html` is backed up), then verify as in [check.md](check.md).

## Design a custom theme (`/seite theme <description>`)

1. Design decisions: if impeccable is installed (`.claude/skills/impeccable/` or `.agents/skills/impeccable/` exists), use `/impeccable` (Codex: `$impeccable`) for direction, type, color, and layout, and apply its result within the contract below. Otherwise decide from the description, and mention once that `seite skill install impeccable` adds design tooling.
2. Start from the closest bundled theme (`seite theme apply <name>`) and restyle it. It already has the SEO head, search, and i18n wiring.
3. Build directly in `templates/base.html` (assets in `static/`, colors from `data/brand.yaml` if it exists). Never make standalone HTML mockups to port later.
4. Preview as in [preview.md](preview.md), once after the first visible change.
5. Verify as in [check.md](check.md). If impeccable is installed, run `seite build` and then `impeccable detect dist/` once, and fix what it reports.
6. Optional: `seite theme export <name>` saves the theme as `templates/themes/<name>.tera`.

## Contract for base.html

- A full HTML document that extends nothing: `<html lang="{{ lang }}">`.
- Blocks: `title`, `head`, `extra_css`, `header`, `content` (inside `<main id="main-content">`), `footer`, `extra_js`.
- Head: everything in the `seo-requirements` context guide (canonical URL, `page.description | default(value=site.description)`, Open Graph and Twitter tags with an absolute image URL, JSON-LD, RSS/Atom links, the `/llms.txt` link, the markdown alternate inside `{% if page.markdown_url %}`, hreflang links for `translations`, `page.robots`).
- Body: `{{ page.content | safe }}` on pages; loop over `collections` on index pages; render `pagination` when it is set.
- i18n: UI text from `{{ t.<key> }}`, internal links prefixed with `{{ lang_prefix }}`, never hardcoded English.
- Search: a `role="search"` form with `<input type="search" id="search-input">` and `<div id="search-results" aria-live="polite">`. The script loads `{{ site.base_path }}{{ lang_prefix }}/search-index.json` on first input: an array of `{title, description, url, collection, tags, date, lang}`. It filters on title, description, and tags, and prefixes result URLs with `site.base_path`.
- Accessibility: a skip link to `#main-content`, visible focus styles, and `prefers-reduced-motion` respected.
