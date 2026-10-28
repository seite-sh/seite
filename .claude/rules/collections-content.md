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

## Frontmatter Schemas
`[collections.fields.<name>]` (`FieldSchema` in `src/content/schema.rs`) + `strict_fields`. Presets get `schema::preset_fields(preset)` (never `required`), written by `seite init` / `seite collection add`; `seite upgrade` never adds schemas. Custom (non-top-level: not built-in, not a `[[taxonomies]]` name) fields may be written at the top level or under `extra:` (both = error); `schema::page_extra` exposes either as `page.extra.<name>`, and published `.md` copies add declared top-level custom fields (`custom_field_names`).

## Scheduling
Opt-in `[build] schedule = true` (init writes it), dated collections only: future `date` → excluded like drafts until due; `expires:` → excluded after that day; "today" = `[site] timezone` else local date (`schedule::today`, also used to date new files); `[build] future` / `--future` / serve include future (`src/content/schedule.rs`).

## Adding a New Collection Preset
1. Add to `CollectionConfig::from_preset()` in `src/config/mod.rs`
2. Add default template in `src/templates/mod.rs` + `get_default_template()` match
3. Update `init.rs` template writing match
4. Add integration tests

## Taxonomies, Related Pages, Series
`[[taxonomies]]` (name, label, url_prefix, feed, paginate, template, term_template) adds term pages beyond the implicit `tags`; frontmatter assigns terms with a top-level key of the taxonomy's name (captured by `Frontmatter::other`). `series:` / `series_order:` frontmatter drive `page.series` and `/series/<slug>/`. `[build] related` (default 3) and per-collection `related` size `page.related`. See `.claude/rules/build-pipeline.md`.

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

Per-language outputs: index, RSS, llms.txt, search-index.json, `search/` (full-text; English stemming only for `en*` languages). Sitemap has xhtml:link alternates.

## Trust Center
Collection preset with data files (`data/trust/`), content pages, templates. Config: `[trust]` with `company` and `frameworks`. MCP: `seite://trust` resource.

## Contact Forms
`{{< contact_form() >}}` shortcode + `[contact]` config. HTML POST (Formspree, Web3Forms, Netlify) or JS embed (HubSpot, Typeform). CLI: `seite contact setup|status|remove`. Labels use `{{ t.contact_* }}` for i18n. The `newsletter()` form reuses the `contact-form*` classes so themes style it.

## Comments & Newsletter
`[comments]` (giscus | utterances) and `[newsletter]` (buttondown | kit) in `src/config/embeds.rs`: types, `validate_config` (chained in `SiteConfig::validate_content_model`, located `config-invalid` via `[comments]key` locators), `comments_context`/`newsletter_context` (defaults filled; `SiteContext` + `shortcode_site_context` → `site.comments`/`site.newsletter`). Shortcodes `{{< comments() >}}`, `{{< newsletter(title, description, button, id) >}}` = `src/templates/seite/{comments,newsletter}.html`. Missing section → renders nothing + `comments-not-configured`/`newsletter-not-configured` warning (`ShortcodeRegistry::config_warnings`, via `RenderedBody.warnings` into build Step 3 warnings; skipped when the user overrides the shortcode). UI strings `t.newsletter_email`/`t.newsletter_submit`. Newsletter = plain POST, no JS (Buttondown forbids fetch).

## Git-based CMS
`seite cms setup <pages-cms|decap> [--force] [--branch]` (`src/cli/cms.rs`): `.pages.yml` or `static/admin/{index.html,config.yml}` from collections + built-in frontmatter + `[collections.fields.*]` (object fields skipped). YAML built from `serde_yaml_ng::Mapping`. Decap pinned `DECAP_VERSION` + `DECAP_SRI` (bump together; recompute sha384 of `dist/decap-cms.js`). Refuses to overwrite without `--force` (then `<file>.bak`), writes via `safe_fs`. `seite check` → `cms-config-stale` (`stale_config_diagnostics`). `seite audit` skips HTML under `static/`/`private-assets/` (pass-through files).
