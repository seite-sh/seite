---
paths:
  - "src/config/**"
  - "src/content/**"
---
# Collections & Content

## Six Presets (`CollectionConfig::from_preset()`)

| Preset | has_date | has_rss | listed | nested | url_prefix | template |
|--------|----------|---------|--------|--------|------------|----------|
| posts  | true     | true    | true   | false  | /posts     | post.html |
| docs   | false    | false   | true   | true   | /docs      | doc.html |
| pages  | false    | false   | false  | false  | (empty)    | page.html |
| changelog | true  | true    | true   | false  | /changelog | changelog-entry.html |
| roadmap | false   | false   | true   | false  | /roadmap   | roadmap-item.html |
| trust  | false    | false   | true   | true   | /trust     | trust-item.html |

Optional `subdomain` deploys collection separately. Optional `deploy_project` for per-subdomain Cloudflare/Netlify.

## Adding a New Collection Preset
1. Add to `CollectionConfig::from_preset()` in `src/config/mod.rs`
2. Add default template in `src/templates/mod.rs` + `get_default_template()` match
3. Update `init.rs` template writing match
4. Add integration tests

## Singular→Plural Normalization
`find_collection()` normalizes "post" → "posts", "doc" → "docs", "seite" → "pages"

## Collection Index Pages
`content/{collection}/index.md` is extracted, injected as `{{ page.content }}` in index template. Supports `extra.redirect_to`. Paginated collections show on page 1 only. `{{ nav }}` variable available (cached from page rendering).

## Homepage
`content/pages/index.md` injects into index template. Translations (`index.es.md`) work.

## Changelog Collection
Date-based with RSS. Tag badges: `new` (green), `fix` (blue), `breaking` (red), `improvement` (purple), `deprecated` (gray). Dedicated `changelog-index.html`.

## Roadmap Collection
Weight-ordered, grouped by status tags (`planned`, `in-progress`, `done`, `cancelled`). Three layouts: grouped list, kanban, timeline.

## Multi-language (i18n)
Filename-based: `about.md` → `/about`, `about.es.md` → `/es/about`. Suffix must match `[languages.*]` config.

Template variables: `{{ lang }}` (current), `{{ default_language }}`, `{{ lang_prefix }}` (empty or `"/es"`), `{{ t }}` (UI strings), `{{ translations }}` (array of `{lang, url}`)

Key files: `src/content/mod.rs` (language detection), `src/build/mod.rs` (translation map, `ui_strings_for_lang()`)

Per-language outputs: index, RSS, llms.txt, search-index.json. Sitemap has xhtml:link alternates.

## Trust Center
Collection preset with data files (`data/trust/`), content pages, templates. Config: `[trust]` with `company` and `frameworks`. MCP: `seite://trust` resource.

## Contact Forms
`{{< contact_form() >}}` shortcode + `[contact]` config. HTML POST (Formspree, Web3Forms, Netlify) or JS embed (HubSpot, Typeform). CLI: `seite contact setup|status|remove`. Labels use `{{ t.contact_* }}` for i18n.
