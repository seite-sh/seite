---
paths:
  - "src/build/**"
---
# Build Pipeline

Steps run in this order inside `build_site_inner` (`src/build/mod.rs`; step numbers below match the `// Step N` comments in that file, kept even where a later step was inserted with a letter suffix):

1. Clean `dist/` (skipped for incremental builds)
1b. Copy `public/` → `dist/` root (no prefix, no minification)
2. Load Tera templates (user + embedded defaults, collection-aware)
2b. Load shortcode registry (built-in + user `templates/shortcodes/`)
2.5. Load data files (`data/` → `{{ data.filename }}` in templates)
3. Process each collection: walk content, parse frontmatter, expand shortcodes, render markdown, detect language, resolve slugs/URLs, compute word count/reading time/excerpt/ToC, build translation map, sort; then build the `ContentModel` once (taxonomy terms, `page.related` via a term → pages index, series) and render item pages with `page.taxonomies` / `page.related` / `page.series` added
4. Render index page(s) per language
4b. Paginated collection index pages
4b-extra. Non-paginated collection index pages
4b. Generate 404 page (per-language for multilingual sites)
4c. Generate taxonomy pages (tags + `[[taxonomies]]`: term index, term pages, optional pagination and per-term RSS/Atom) and series index pages — `ContentModel::render_pages` in `src/build/content_model.rs` (taxonomy.rs, series.rs); their URLs go to the sitemap
5. Generate RSS and Atom feeds
6. Generate sitemap (all non-private items, all languages, with `xhtml:link` alternates)
7. Generate discovery files: `robots.txt`, `llms.txt`, `llms-full.txt`
8. Output raw markdown alongside HTML for each page
9. Generate search index JSON
9b. Generate redirect pages from `aliases:` frontmatter
10. Copy static files (with optional minification and fingerprinting)
11. Process images (resize, WebP/AVIF variants, srcset)
12. Post-process all HTML files in a single pass: image srcset/`<picture>`, `loading="lazy"` (skip first image per page for LCP), code-copy buttons, subdomain/base-path link rewriting, analytics + cookie-consent injection, Mermaid client script injection, and internal-link validation
13. Generate the Cloudflare Pages advanced-mode Worker for any password-protected routes in this output (see `src/cli/access.rs`)

Collections with `subdomain` set are removed from the main run and built again as independent root-mounted sites into `dist-subdomains/{name}/` (own sitemap, RSS, robots.txt, search index) — see `build_subdomain_sites` in `src/build/mod.rs`.

## Atomic Output (Staging)

A full build (not an incremental dev-server rebuild) never writes into `dist/` directly: it renders into a sibling `{output}.staging-<pid>` directory (`StagedOutputs::stage` in `src/build/mod.rs`) and only swaps it into place with `StagedOutputs::commit()` after every step — including subdomain builds — succeeds. A failed or killed build leaves the previous `dist/` untouched; `StagedOutputs::drop` removes an uncommitted staging dir. `clean_stale_build_dirs` sweeps `{output}.staging-<pid>` / `{output}.old-<pid>` directories left behind by dead processes on the next build (a liveness check by pid skips ones still running). Incremental rebuilds from the dev server (content-only changes, nothing deleted) update `dist/` in place instead, and the file watcher ignores the staging/old directory names so it doesn't trigger rebuild loops on itself.

## Link and Asset Attribution

`src/build/links.rs` traces every broken link (`<a href>`, `<link rel=alternate>`) and missing asset (`img`/`srcset`/`script`/`stylesheet`/`video`/`audio`/`track`) back to the file it was authored in — the markdown source, a template, or a data file — with a 1-based line number when it can find one; `attribute_sources()` does the tracing, `BrokenLink::location()` formats it, and links that can only be traced to the generated page fall back to `"{page} (from template/listing)"`. `fill_suggestions()` adds a "did you mean" hint using Levenshtein distance against the site's valid URLs. `link_diagnostics()` converts these into `broken-link` / `missing-asset` diagnostics (warnings by default, errors with `--strict`).

## Markdown-to-Markdown Link Rewriting

`SourceLinkMap` (`src/build/markdown.rs`) rewrites relative links between content files — `[x](../docs/intro.md)`, root-relative `/content/docs/intro.md` — into the target page's published URL, preserving `#fragment`/`?query`, preferring a same-language translation file (`intro.es.md`) when the linking page isn't in the default language, and applying `base_path`. Root-relative links outside the content dir (e.g. `/docs/intro.md`) are left alone — they point at the published markdown copy the build writes alongside each page. A `.md` link that looks like a content-file reference but matches nothing becomes a `broken-link` diagnostic (kind `Markdown`) via `rewrite_md_links`'s unresolved list.

## Diagnostics Collection

`src/diagnostics.rs` defines `Diagnostic` (severity, stable `code`, message, optional file/line/column, optional hint) and `Diagnostics`, shared by `seite build`, `seite check`, and the MCP tools. A build (or check) collects every problem across every content/template/data file into one `Diagnostics` list instead of stopping at the first error — a bad shortcode in one post doesn't hide a broken frontmatter in another. `Diagnostic`'s `Display` renders compiler-style `file:line:col: severity[code]: message` plus an indented `hint:` line.

## Output Pattern

URLs are clean: `/posts/hello-world` → `dist/posts/hello-world.html` + `.md`
Dev server resolves clean URLs to `.html` files.

## Content Model

```rust
struct Frontmatter {
    title: String,
    date: Option<NaiveDate>,       // required for posts
    updated: Option<NaiveDate>,    // → JSON-LD dateModified, sitemap lastmod
    description: Option<String>,
    image: Option<String>,         // → og:image / twitter:image
    slug: Option<String>,          // override auto-generated slug
    tags: Vec<String>,
    draft: bool,                   // excluded unless --drafts
    template: Option<String>,      // override collection default
    robots: Option<String>,        // per-page <meta name="robots">
    weight: Option<i32>,           // ordering for non-date collections
    extra: HashMap<String, Value>, // → {{ page.extra.field }}
    expires: Option<NaiveDate>,    // last published day (content::schedule)
    series: Option<String>,        // → page.series, /series/<slug>/
    series_order: Option<i32>,
    other: BTreeMap<String, Value>, // #[serde(flatten)]: every other top-level key;
                                    // [[taxonomies]] terms (`categories: [..]`) via taxonomy_terms()
}

struct ContentItem {
    frontmatter, raw_body, html_body, source_path,
    slug, collection, url, lang,
}
```

## Taxonomies, Related, Series

- `taxonomy.rs`: `resolve_taxonomies()` = implicit `tags` (no feeds, legacy `tags.html`/`tag.html`, titles `Tags` / `Tag: x`) + `[[taxonomies]]`. Terms group by slug; tags display lowercased, others the most common spelling. Configured taxonomies default to per-term feeds. Implicit-tags output must stay byte-identical.
- `related.rs`: score = shared tag 1, shared other-taxonomy term 2; tie-break date distance (undated last), title, URL. Same collection + language, no drafts, no `index` pages. Limit: collection `related` → `[build] related` → 3. Dense term ids + reusable score vector, rayon, top-k via `select_nth_unstable_by`; in groups > 200 pages a term on > 20% of them is "common" (adds score, never seeds candidates) so it stays linear. `test_related_common_terms_stay_linear` bounds postings visited.
- `taxonomy::config_diagnostics`: `taxonomy-url-collision` (error, from `SiteConfig::load_with_diagnostics` and `build_site_inner`) and `taxonomy-reserved-name` (warning); `series.rs` emits `series-url-collision`.
- Frontmatter `other` is `skip_serializing`: published `.md` copies use `generate_published_frontmatter` (known fields + configured taxonomy keys); source edits go through YAML mappings, so unknown keys survive there. `series`/`series_order` deserialize leniently.
- Incremental builds re-render every collection holding a part of a changed page's series.
- `seite upgrade` (`check_default_templates`) replaces `templates/post.html`, `index.html` and `trust-index.html` when their hash matches a historical default (`DEFAULT_TEMPLATES` in `src/cli/upgrade.rs`, last hash = current); edited copies get a one-time notice. Append the new hash whenever `DEFAULT_POST`/`DEFAULT_INDEX`/`DEFAULT_TRUST_INDEX` changes (test `current_default_templates_are_in_hash_table`).
- `series.rs`: group by (language, slug of `series`) over public collections; order `series_order`, date asc, source path. `/series/<slug>/` only when used.
- The embedded `post.html` renders them with `{% if %}` blocks attached to existing lines, so pages without them render byte-identically. Sites keep their own `templates/post.html` from `init`.

## Schemas and Scheduling in Step 3

Per collection, `content::schema::Schema::for_site` validates each parsed file (`ParsedContent.frontmatter_text`/`frontmatter_line` give exact key lines): errors are returned like parse errors (→ `fatal`), `frontmatter-schema-unknown` warnings go to `diagnostics` through a mutex. Drafts are validated only with `--drafts`; future-dated files always. After the filename date is resolved, `content::schedule::Schedule::is_published(collection, fm)` — only with `[build] schedule = true` and `has_date` — drops future-dated (unless `config.build.future`) and expired items exactly like drafts, so they never reach feeds, sitemap, search, llms files, tags or listings.

## Data Files

`data/` holds YAML/JSON/TOML loaded at build time → `{{ data.filename }}` in templates. Nested dirs create nested keys. Two files with same stem = build error.

All 6 themes render `data.nav` (navigation) and `data.footer` (links + copyright). Internal links auto-prefixed with `{{ lang_prefix }}`; external links (with `external: true`) get `target="_blank"`.

UI string translations: `data/i18n/{lang}.yaml` overrides English defaults, injected as `{{ t }}`.

## Shortcodes

- Inline `{{< name(args) >}}` and body `{{% name(args) %}} markdown {{% end %}}`
- Named args: `key="string"`, `key=42`, `key=true`
- Expanded before markdown rendering; not expanded inside code blocks
- Built-ins: `youtube`, `vimeo`, `gist`, `callout` (body), `figure`, `contact_form`
- User-defined: `templates/shortcodes/*.html` (override built-ins by name)
- To add a built-in: create template in `src/shortcodes/builtins/`, add entry in `builtins.rs`
- Diagnostics: `parser::parse_shortcodes_recovering()` reports a malformed tag, skips past it, and keeps scanning, so `ShortcodeRegistry::expand_diagnostic()` returns every `shortcode-syntax` / `shortcode-unknown` problem in a file (sorted by line/column); `expand()` still surfaces only the first

## Image Processing

When `[images]` is configured: resize to widths, generate WebP/AVIF variants, rewrite `<img>` tags with srcset and `<picture>`, add `loading="lazy"` (skip first image per page for LCP).
