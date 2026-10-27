---
title: "Error Reference"
description: "Every diagnostic code reported by seite check and seite build, and every seite audit finding: what it means, common causes, and how to fix it."
weight: 12
---

`seite check` and `seite build` report problems as compiler-style lines such as `content/posts/a.md:3:1: error[frontmatter-parse]: ...`. The part in brackets is a stable **diagnostic code**. Each code has a section below; run `seite explain <code>` to print it in the terminal. In `--json` output every diagnostic carries a `docs_url` pointing at its section here.

Errors fail `seite check` and `seite build`. Warnings fail only with `--strict`.

`seite audit` findings use the same reference: their codes start with `seo-`, `geo-`, `a11y-`, `content-` or `perf-`, and their severity is `error`, `warning` or `notice` (they never fail a build; `seite audit --fail-on` turns them into an exit code). See [Audit](/docs/audit).

## a11y-button-name

**What it means:** A `seite audit` finding (error): a `<button>` has no text, image alt, `aria-label` or `title`.

**How to fix:** Give the button text, or an `aria-label` for icon buttons.

## a11y-duplicate-id

**What it means:** A `seite audit` finding (warning): the same `id` appears more than once on a page; links and labels pointing at it hit the first one only.

**How to fix:** Make ids unique. Repeated heading texts in markdown and copied template snippets are the usual causes.

## a11y-html-lang

**What it means:** A `seite audit` finding (error): `<html>` has no `lang` attribute, so screen readers guess the pronunciation language.

**How to fix:** Use `<html lang="{{ lang }}">` in the layout template.

## a11y-img-alt

**What it means:** A `seite audit` finding (error): an `<img>` has no `alt` attribute. Screen readers then read the file name. (Also hurts image search.)

**How to fix:** Describe the image: `![A diagram of the build pipeline](/static/pipeline.png)` in markdown, `alt="..."` in templates. Use `alt=""` only for purely decorative images.

## a11y-input-label

**What it means:** A `seite audit` finding (error): a form control (`input`, `select`, `textarea`) has no label: no `<label for>`, no wrapping `<label>`, no `aria-label`/`aria-labelledby`/`title`.

**How to fix:** Add `<label for="email">Email</label>` with a matching `id`, or an `aria-label`.

## a11y-link-ambiguous

**What it means:** A `seite audit` finding (warning): a link's whole text is generic ("click here", "read more", "learn more", "here", "more", "link", ...) and it has no `aria-label`. Out of context (screen-reader link lists) it says nothing. Not reported when the same target is also linked with a descriptive text on the page.

**How to fix:** Use text that names the target (`[Read the install guide](/docs/install)`), or add `aria-label="Read the install guide"` in templates.

## a11y-link-empty

**What it means:** A `seite audit` finding (error): a link has no text, image alt or `aria-label`, so assistive technology announces only "link".

**How to fix:** Give the link visible text, or an `aria-label` for icon-only links.

## a11y-skip-link

**What it means:** A `seite audit` finding (notice): the page has no skip link (an `<a href="#...">` whose text contains "skip"), so keyboard users tab through the whole header on every page.

**How to fix:** Add `<a href="#main" class="skip-link">Skip to main content</a>` as the first element in `<body>` and `id="main"` on `<main>` (the bundled themes have it).

## broken-link

**What it means:** an internal link points at a page (or, for relative `.md` links, a content file) that does not exist in the built site. Reported as a warning, or as an error with `--strict`.

**Common causes:** a typo in the URL, a page that was renamed or deleted, a missing collection prefix (`/hello` instead of `/posts/hello`), a link to a draft page in a build without `--drafts` or to a scheduled (future-dated) or expired page, or a relative `.md` link whose path is not relative to the linking file.

**How to fix:** point the link at an existing URL (the hint suggests the closest one), or create the missing page. `seite search <words>` finds a page's real URL; `seite page <path>` shows the URL a file publishes to.

```markdown
[Install guide](/docs/instal)        <!-- broken -->
[Install guide](/docs/install)       <!-- fixed -->
```

## build-failed

**What it means:** `seite check` ran the real build pipeline and it stopped with an error that has no more specific code (for example an I/O error writing the scratch output, or an image that cannot be processed).

**Common causes:** unreadable files or directories, a corrupt image in `static/`, a file system permission problem.

**How to fix:** read the message, which carries the underlying error, fix the named file, and run `seite check` again. `seite build --verbose` shows which step failed.

## check-file-skipped

**What it means:** `seite check --file <path>` was given a path it did not check, so a clean result does not cover it. Reported as a warning (an error with `--strict`). The message says why: the file was not found, it is outside the site, it is not a file, or it is not a content, template or data file.

**Common causes:** a typo in the path, a file that was deleted or renamed, a path outside the site root, or a file type `--file` does not cover (images, CSS, scripts under `static/`).

**How to fix:** correct the path. `--file` checks `seite.toml`, content `.md` files, templates `.html` and data `.yaml`/`.yml`/`.json`/`.toml` files; run `seite check` without `--file` to validate the whole site.

## config-invalid

**What it means:** `seite.toml` could not be loaded: a TOML syntax error, or a value of the wrong type. Nothing else can be checked until it loads.

**Common causes:** an unclosed quote or bracket, a string where a number or boolean is expected (`minify = "yes"`), a duplicated table or key.

**How to fix:** fix the line and column named in the diagnostic.

```toml
[build]
minify = "yes"   # error: expected a boolean
minify = true    # fixed
```

## config-unknown-key

**What it means:** `seite.toml` contains a key seite does not know. The key is ignored, so the setting has no effect. Reported as a warning.

**Common causes:** a typo (`minfy`), a key in the wrong table (`base_url` under `[build]` instead of `[site]`), or an option from another static site generator.

**How to fix:** rename the key as the `did you mean` hint suggests, move it to the right table, or delete it. The [configuration reference](/docs/configuration) lists every key.

## content-invalid

**What it means:** a content file could not be read at all (before its frontmatter was looked at).

**Common causes:** the file is not valid UTF-8 (for example a file saved in another encoding, or a binary file with a `.md` extension), or it is not readable.

**How to fix:** re-save the file as UTF-8, or move non-markdown files out of `content/` (images belong in `static/`).

## content-link-to-draft

**What it means:** A `seite audit` finding (error): a published page links to a page marked `draft: true`. Drafts are not built, so the link is broken on the live site.

**How to fix:** Publish the target (remove `draft: true`) or remove the link until it is ready.

## content-link-to-scheduled

**What it means:** A `seite audit` finding (warning): a published page links to a page that scheduling hides: its `date` is in the future (it goes live later) or its `expires:` date has passed. The message names the date. Same situation as the `link-to-scheduled` diagnostic of `seite check`.

**How to fix:** Remove the link until the target is live, or give the linking page the same or a later date.

## content-missing-date

**What it means:** A `seite audit` finding (warning): a page in a dated collection (`has_date = true`, e.g. posts, changelog) has no `date:` and no `YYYY-MM-DD-` filename prefix. Feeds, sorting and scheduling need a date.

**How to fix:** Add `date: 2026-01-31` to the frontmatter, or rename the file with a date prefix.

## data-conflict

**What it means:** two data files resolve to the same template key, so seite cannot decide which one `data.<key>` refers to.

**Common causes:** `data/nav.yaml` next to `data/nav.json`, or a file and a directory with the same name (`data/team.yaml` and `data/team/`).

**How to fix:** keep one source per key: delete or rename one of the files named in the message.

## data-file-parse

**What it means:** a file in `data/` (YAML, JSON or TOML) has a syntax error and was not loaded.

**Common causes:** wrong indentation in YAML, a trailing comma in JSON, an unquoted string that contains `: ` in YAML.

**How to fix:** fix the syntax at the line and column given.

```yaml
- name: Ada
   role: Founder     # error: bad indentation
- name: Ada
  role: Founder      # fixed
```

## external-link-broken

**What it means:** `seite check --external` requested an external `http(s)` link from the rendered site and it is gone: HTTP 404 or 410, another 4xx answer, or a host that does not exist (DNS lookup failed). Reported against the file and line the link was written in. A warning, or an error with `--strict`.

**Common causes:** the page moved or was deleted, a typo in the URL, a domain that expired.

**How to fix:** correct the URL (search the site you meant to link for the page's new address) or remove the link. Failures are re-checked on every `seite check --external`; working links are cached for 7 days in `.seite/cache/links.json`.

```markdown
[Spec](https://example.com/old-spec)   <!-- 404 -->
[Spec](https://example.com/spec)       <!-- fixed -->
```

## external-link-unreachable

**What it means:** `seite check --external` could not confirm an external link: the request timed out, the connection failed, the server answered 5xx, refused the check (401, 403, 429), or redirected more than 5 times. Unlike `external-link-broken`, this is often temporary or a site that blocks automated requests. A warning, or an error with `--strict`.

**Common causes:** a slow or overloaded server, a site behind bot protection, a redirect loop, a network problem on your side.

**How to fix:** open the URL in a browser. If it works, leave the link (run the check again later) or add the host to the skip list so it is never requested:

```toml
[check]
external_skip = ["example.com"]   # also skips its subdomains
external_timeout = 20             # seconds per request (default 10)
```

## frontmatter-missing

**What it means:** a content file does not start with a frontmatter block, so it has no title and cannot be built.

**Common causes:** the file starts with a heading or blank text instead of `---`, or the closing `---` is missing.

**How to fix:** start the file with a frontmatter block that has at least a title.

```markdown
---
title: "My Page"
---

Page content starts here.
```

## frontmatter-parse

**What it means:** the YAML frontmatter of a content file is invalid: a syntax error, a value of the wrong type, or a missing required field (`title`).

**Common causes:** an unquoted title containing `: `, a date that is not `YYYY-MM-DD`, `tags: rust, web` instead of a list, no `title:` line.

**How to fix:** fix the line named in the diagnostic. Quote strings that contain a colon. `seite frontmatter set <file> key=value` edits a key without touching the rest of the file.

```yaml
title: Rust: a tour          # error: mapping values are not allowed here
title: "Rust: a tour"        # fixed
tags: [rust, web]            # a list, not "rust, web"
```

## frontmatter-schema

**What it means:** a content file's frontmatter breaks its collection's schema (`[collections.fields.<name>]` in `seite.toml`): a required field is missing or empty, a value has the wrong type, is not one of the allowed `enum` values, is too long/short or out of range (`min`/`max`), does not match `pattern`, or a custom field is set both at the top level and under `extra:`. The message names the field, what was expected and what was found; the hint repeats the field's `description` from the schema. The line is the offending key (or the `extra:` / opening `---` line for a missing field).

**Common causes:** a new file without the fields the collection requires, a typo in an enum value, a quoted number (`rating: "4"` for an integer), a date not in `YYYY-MM-DD` form, the same custom field (`author:`) both at the top level and under `extra:`.

**How to fix:** edit the frontmatter to match the schema. `seite templates` (or `seite context`) prints each collection's fields; `seite new` writes defaults and empty placeholders for required fields. Custom (non-built-in) fields go either at the top level or under `extra:` (one of the two); templates read them as `{{ page.extra.<name> }}` either way:

```yaml
---
title: "Hello"
description: "One sentence for search results."   # built-in: top level
extra:
  author: "Jane Doe"                              # custom field: under extra (or top level)
  rating: 4                                       # integer, not "4"
---
```

## frontmatter-schema-unknown

**What it means:** the collection sets `strict_fields = true` and the file has a key its schema does not declare: a top-level key that is neither built-in, a `[[taxonomies]]` name, nor a declared field (the build ignores it), or an `extra.<name>` the schema does not list. A warning (fails only with `--strict`).

**Common causes:** a typo in a key (`descripton`), a field the schema does not declare yet.

**How to fix:** rename the key to a declared field (the hint suggests the closest one), declare it with `[collections.fields.<name>]` in `seite.toml`, or remove it. Built-in keys (`title`, `date`, `tags`, …) are always allowed.

## geo-content-signal-missing

**What it means:** A `seite audit` finding (notice): `robots.txt` has no `Content-Signal:` line saying how AI may use the content (see contentsignals.org).

**How to fix:** Add `Content-Signal: search=yes, ai-input=yes, ai-train=no` (seite's generated robots.txt has it).

## geo-jsonld-invalid

**What it means:** A `seite audit` finding (error): a JSON-LD block does not parse as JSON, so consumers ignore it entirely.

**How to fix:** Fix the template that writes it. Insert values with `| json_encode` so quotes and newlines are escaped: `"headline": {{ page.title | json_encode() | safe }}`.

## geo-jsonld-missing

**What it means:** A `seite audit` finding (warning): the page has no JSON-LD structured data (`<script type="application/ld+json">`), which search engines and AI assistants use to understand what the page is.

**How to fix:** Add a JSON-LD block to the layout (the bundled themes emit `BlogPosting`, `WebPage`, `BreadcrumbList`, ...).

## geo-jsonld-no-type

**What it means:** A `seite audit` finding (warning): a JSON-LD object (or an item of its array or `@graph`) has no `@type`, so it describes nothing.

**How to fix:** Add `"@type"` to every object, e.g. `"@type": "WebPage"`.

## geo-llms-no-summary

**What it means:** A `seite audit` finding (notice): `llms.txt` lists the page without a summary because the page has no `description:`. AI assistants choose pages by these summaries. When the SEO category is audited too, the page is reported once, as `seo-description-missing`.

**How to fix:** Add `description:` to the page's frontmatter.

## geo-llms-txt-missing

**What it means:** A `seite audit` finding (error): the output has no `llms.txt`, the index AI assistants read to find a site's content.

**How to fix:** Build with seite (it writes `llms.txt` and `llms-full.txt`). With `--dist`, point at a complete build output.

## geo-llms-txt-unlisted

**What it means:** A `seite audit` finding (warning): a content page in the default language is not listed in `llms.txt` (pages of `private` collections are expected to be missing).

**How to fix:** Rebuild; if a custom `llms.txt` in `static/` or `public/` replaces the generated one, add the page or remove the custom file.

## geo-markdown-alternate-missing

**What it means:** A `seite audit` finding (warning): a content page has no `<link rel="alternate" type="text/markdown">` pointing at its `.md` copy, which AI agents prefer over HTML.

**How to fix:** Add `<link rel="alternate" type="text/markdown" href="{{ page.markdown_url }}">` to the layout `<head>` (the bundled themes have it).

## geo-robots-blocks-ai-search

**What it means:** A `seite audit` finding (warning): `robots.txt` blocks at least one AI *search* crawler (OAI-SearchBot, ChatGPT-User, PerplexityBot, Claude-SearchBot) from `/`, so AI assistants can't fetch or cite the site. Blocking AI *training* crawlers (GPTBot, ClaudeBot, ...) is fine and not reported.

**How to fix:** Allow the search crawlers (`User-agent: OAI-SearchBot` / `Allow: /`) and block training crawlers by name instead. seite's generated robots.txt already does this; the finding usually means the audited output came from elsewhere (`--dist`) or a post-build step replaced the file.

## geo-robots-missing

**What it means:** A `seite audit` finding (warning): the output has no `robots.txt`.

**How to fix:** Build with seite (it writes one that allows search and AI search crawlers) or add `static/robots.txt`.

## i18n-partial

**What it means:** a per-language value in a data file (a "language map" such as `description: {en: ..., de: ...}`) is missing one of the site's configured languages. Pages in that language fall back to the default language's text. Reported as a warning.

**Common causes:** a language was added to `seite.toml` (`[languages.xx]`) without translating the data files, or a typo in a language code.

**How to fix:** add the missing language key to the value named in the message, or remove the language from `seite.toml` if the site is not translated yet. See [multi-language sites](/docs/i18n).

## link-to-scheduled

**What it means:** with `[build] schedule = true`, a published page links to a post the build leaves out because it is scheduled (its `date` is in the future) or expired (its `expires:` date has passed). The link is broken on the live site until the target is published. The message names the target's go-live (or expiry) date. A warning, or an error with `--strict`, in both `seite check` and `seite build`.

**Common causes:** a teaser linking to a post scheduled for later, a page still linking to an expired announcement.

**How to fix:** remove the link until the target goes live, give the linking page the same or a later `date:` so both publish together, or bring the target forward. To preview the site as it will look once everything is published, use `seite build --future` / `seite check --future`.

```markdown
Read the [launch post](/posts/launch)   <!-- scheduled for 2026-11-01 -->
```

## missing-asset

**What it means:** a page references a file (image, stylesheet, script, download) that does not exist in the build output. Reported as a warning, or as an error with `--strict`.

**Common causes:** the file was never added to `static/` or `public/`, a typo in the path, a path relative to the wrong directory, or a wrong case (`Logo.png` vs `logo.png`) that works on macOS but breaks on Linux hosts.

**How to fix:** add the file under `static/` (served from `/static/...`) or `public/` (served from the site root), or fix the path in the content or template.

## og-config-invalid

**What it means:** The `[og]` section of `seite.toml` (generated social cards) names something seite cannot use: a color that is not `#rgb`/`#rrggbb`, a `logo` that is missing, outside the site, a symlink, not `.svg`/`.png`, not a valid image, over 1 MB or over 4096 px per side, or a `font` that is missing, a symlink or not a TrueType/OpenType file. An error: `seite build` stops and `seite check` reports it at the offending line.

**How to fix:** Correct the value; paths are relative to the site root. `seite og preview /` renders a card to try a change.

```toml
[og]
accent = "#0057b7"          # not "blue"
logo = "static/logo.svg"    # SVG or PNG inside the site
font = "static/fonts/NotoSansJP-Bold.ttf"
```

## og-missing-glyphs

**What it means:** A build warning: some pages got no social card of their own because their title has characters the card font lacks (CJK, emoji, scripts other than Latin, Greek and Cyrillic). seite skips those cards rather than drawing empty boxes; the pages use the site card (`og/index.png`) instead. The file is the first such page.

**How to fix:** Set `[og] font` to a `.ttf`/`.otf` that has the glyphs (for example a Noto Sans CJK subset under `static/fonts/`), or give the pages their own `image:`.

## perf-image-large

**What it means:** A `seite audit` finding (warning): an image used by a page is larger than **300 KB**. For an `<img>` with `srcset`, the smallest candidate counts. Reported once per image.

**How to fix:** Compress or resize it. Local images under `static/` get resized WebP copies when `[images]` is configured; for very large photos, export a smaller original.

## perf-img-lazy

**What it means:** A `seite audit` finding (notice): an image in `<main>` after its first image has no `loading="lazy"`, so it loads before the reader scrolls to it. The first image in `<main>` and images in the header are skipped (they are usually above the fold).

**How to fix:** Add `loading="lazy"`. For markdown images, `[images] lazy_loading = true` does it.

## perf-page-weight

**What it means:** A `seite audit` finding (warning): the page's HTML plus its local stylesheets, scripts and images (same rule as `perf-image-large`) exceed **1.5 MB**. Remote assets are not counted.

**How to fix:** Remove or compress the heaviest images, drop unused scripts, and split very long pages.

## perf-render-blocking-script

**What it means:** A `seite audit` finding (warning): a `<script src>` in `<head>` has neither `defer` nor `async` (and is not `type="module"`), so the browser stops rendering until it downloads and runs.

**How to fix:** Add `defer` (order-preserving) or `async`, or move the script to the end of `<body>`.

## seo-broken-anchor

**What it means:** A `seite audit` finding (warning): an internal link has a `#fragment` that matches no `id` on the target page (the page exists; `seite check` reports missing pages).

**How to fix:** Use the heading's real id (seite builds ids from heading text: `## Install steps` → `#install-steps`; `seite page <path>` lists them), or drop the fragment.

## seo-canonical-missing

**What it means:** A `seite audit` finding (warning): the page has no `<link rel="canonical">`, so search engines may index duplicate URLs (with and without a trailing slash, or the `.html` file) separately.

**How to fix:** Add `<link rel="canonical" href="{{ page_url }}">` to the `<head>` of the layout template (the bundled themes have it).

## seo-description-duplicate

**What it means:** A `seite audit` finding (warning): two or more content pages have the same description.

**How to fix:** Write a description specific to each page.

## seo-description-long

**What it means:** A `seite audit` finding (warning): the description is longer than **160 characters**; search results truncate it.

**How to fix:** Shorten `description:` to at most 160 characters, with the key point first.

## seo-description-missing

**What it means:** A `seite audit` finding (warning): a content page has no `description:` in its frontmatter, so search engines and link previews pick their own snippet (the bundled themes fall back to the site description, which then repeats on every such page).

**How to fix:** Add `description:` with 50–160 characters that summarize the page.

## seo-description-short

**What it means:** A `seite audit` finding (notice): the description is shorter than **50 characters**, too short to be a useful search snippet.

**How to fix:** Expand `description:` to 50–160 characters that say what the reader gets from the page.

## seo-h1-missing

**What it means:** A `seite audit` finding (warning): the page has no `<h1>` at all.

**How to fix:** Give every page one `<h1>`, usually the page title in the template (`<h1>{{ page.title }}</h1>`).

## seo-h1-multiple

**What it means:** A `seite audit` finding (warning): there is more than one `<h1>` inside `<main>` (or in the whole page when it has no `<main>`). A site-title `<h1>` in the header does not count.

**How to fix:** Keep one `<h1>` per page. The usual cause is a markdown `# Heading` in a page whose template already prints the title as `<h1>`: use `##` for sections in the body.

## seo-heading-skip

**What it means:** A `seite audit` finding (notice): a heading skips a level, e.g. an `<h3>` right after an `<h1>`. Screen readers and search engines use heading levels as the page outline.

**How to fix:** Use `##` before `###` in markdown. In listing templates, use `<h2>` for item titles under the page's `<h1>`.

## seo-hreflang

**What it means:** A `seite audit` finding (warning): in a multilingual site, a page's `<link rel="alternate" hreflang>` points at a translation that was not built, or at a page that does not link back with its own hreflang. Search engines ignore one-way hreflang.

**How to fix:** Make sure the translation file exists (`about.es.md` next to `about.md`) and that the layout prints the hreflang links for every page (the bundled themes do).

## seo-img-dimensions

**What it means:** A `seite audit` finding (warning): an `<img>` has no `width` and `height`, so the page jumps when it loads (layout shift, a Core Web Vitals metric).

**How to fix:** Local images that seite processes get `width`/`height` automatically; for images in templates or raw HTML add both attributes with the intrinsic size.

## seo-noindex-in-sitemap

**What it means:** A `seite audit` finding (error): a page is listed in `sitemap.xml` but tells search engines not to index it (`robots: noindex`). The signals contradict each other.

**How to fix:** Remove `robots: noindex` if the page should be found, or keep it out of the sitemap (draft it, or move it to a `private` collection).

## seo-og-image-missing

**What it means:** A `seite audit` finding (notice): a content page has no `og:image`, so links shared on social networks and chat apps show no preview image.

**How to fix:** Turn on generated social cards (`[og]` with `generate = true` in `seite.toml`; new sites have it) so every page gets one, or set `image:` in the frontmatter (a path under `static/` or a URL).

## seo-orphan-page

**What it means:** A `seite audit` finding (warning): no other page of the site links to this content page, so readers and crawlers only find it through the sitemap. Listing pages, the homepage and `noindex` pages are not checked.

**How to fix:** Link to it from a related page, its collection index, or the navigation (`data/nav.yaml`).

## seo-page-not-in-sitemap

**What it means:** A `seite audit` finding (warning): an indexable content page is missing from `sitemap.xml` (pages of `private` collections are expected to be missing).

**How to fix:** Rebuild the site. If a custom `sitemap.xml` in `static/` or `public/` replaces the generated one, remove it or add the page.

## seo-sitemap-missing

**What it means:** A `seite audit` finding (error): the output has no `sitemap.xml`.

**How to fix:** Build with seite (it always writes one). With `--dist`, point at a complete build output.

## seo-sitemap-stale-url

**What it means:** A `seite audit` finding (error): `sitemap.xml` lists a URL that has no built page.

**How to fix:** Rebuild the site; if it persists, check the page's `slug`/URL and `base_url` in `seite.toml`.

## seo-thin-content

**What it means:** A `seite audit` finding (warning): a content page has fewer than **150 words** (body text, from the markdown). Thin pages rank poorly and are rarely cited. Changelog and roadmap entries are exempt; `robots: noindex` pages are skipped.

**How to fix:** Expand the page, merge it into a related page, or mark it `draft: true` until it is ready.

## seo-title-duplicate

**What it means:** A `seite audit` finding (warning): two or more indexable pages share the same `<title>`. Search engines show one result per title and treat the pages as competing. Pages with `robots: noindex` are ignored.

**How to fix:** Make each title unique. When the duplicates are generated pages (for example a collection index that reuses the site title), fix the template's `{% block title %}`. Each page gets its own finding (at its `title:` line, or the generated page), naming the pages it collides with.

## seo-title-long

**What it means:** A `seite audit` finding (warning): the `<title>` (page title plus the site name the theme appends) is longer than **60 characters**, so search results truncate it.

**How to fix:** Shorten `title:` in the frontmatter, or make the template's title suffix shorter.

## seo-title-missing

**What it means:** A `seite audit` finding (error): the page has no `<title>`, or it is empty. Search results and browser tabs have nothing to show.

**How to fix:** Give the page a `title:` in its frontmatter; the layout template must print it inside `<title>` (the bundled themes do). For a listing page, check the template's `{% block title %}`.

## series-url-collision

**What it means:** a series index page (`/series/<slug>/`) has the same URL as a content page, so the series page replaces it. Reported as a warning.

**Common causes:** a page whose URL starts with `/series/`, such as a `pages` entry with `slug: series/rust`, or a collection whose `url_prefix` is `/series`.

**How to fix:** rename the series (`series:` frontmatter) or give the page a different `slug:`. `/series/` is reserved for series index pages.

## shortcode-render

**What it means:** a shortcode was found and parsed, but rendering its template failed.

**Common causes:** a missing required parameter, a parameter of the wrong type, or an error inside a custom shortcode template in `templates/shortcodes/`.

**How to fix:** compare the call with the shortcode's parameters (`seite templates` lists every shortcode with its parameters and a usage example), and fix the custom template if the message points into one.

## shortcode-syntax

**What it means:** a shortcode call in a content file is malformed and cannot be parsed.

**Common causes:** Hugo-style syntax (`name key="v"` instead of `name(key="v")`, or a `/name` closing tag), an unclosed body shortcode (missing `end`), unbalanced quotes or parentheses in the arguments.

**How to fix:** use seite's syntax: inline shortcodes are `name(arg="value")` between `{{<` and `>}}`; body shortcodes open with `name(...)` between `{{%` and `%}}` and close with `end` between the same delimiters. See [shortcodes](/docs/shortcodes).

## shortcode-unknown

**What it means:** a content file calls a shortcode that is neither built in nor defined in `templates/shortcodes/`.

**Common causes:** a typo in the name, a shortcode from another generator, or a custom shortcode file that is missing or misnamed.

**How to fix:** use one of the available shortcodes listed in the message, or create `templates/shortcodes/<name>.html`.

## taxonomy-reserved-name

**What it means:** a `[[taxonomies]]` entry is named after a frontmatter field seite reads itself (`title`, `date`, `extra`, `series`, `draft`, …). Pages assign terms with a top-level key named after the taxonomy, and that key already means something else, so the taxonomy never gets terms. Reported as a warning.

**Common causes:** naming a taxonomy `series` or `extra`.

**How to fix:** rename the taxonomy, e.g. `topics`, and use that key in frontmatter.

## taxonomy-url-collision

**What it means:** a `[[taxonomies]]` entry's URL prefix is already used by a collection, another taxonomy (including `/tags`), the series index pages (`/series`), or a language (`/es`). Their pages would be written to the same paths and overwrite each other, so the build stops. Reported as an error.

**Common causes:** `url_prefix = "/posts"` on a taxonomy, or a taxonomy named after a collection or a language code.

**How to fix:** give the taxonomy its own `url_prefix`:

```toml
[[taxonomies]]
name = "categories"
url_prefix = "/categories"   # not "/posts"
```

## tailwind-download-failed

**What it means:** the site has `[build.tailwind]` (and no `SEITE_TAILWIND_BIN` in the environment), and `seite build`, `seite serve` or the MCP `seite_build` tool could not get the pinned Tailwind CSS standalone CLI: the download from the GitHub release failed (no network, a proxy, a timeout), its SHA-256 did not match the checksum built into seite (the file is discarded, never run), or Tailwind publishes no binary for this OS/CPU. An error: `seite build` stops. The file is `seite.toml`. `seite check`, `seite audit`, coding-agent hooks and the other MCP tools never run Tailwind, so they never report it.

**How to fix:** retry with a working connection; the binary is downloaded once and cached (`~/Library/Caches/seite/tailwind/` on macOS, `$XDG_CACHE_HOME/seite/tailwind/` or `~/.cache/seite/tailwind/` on Linux, `%LOCALAPPDATA%\seite\tailwind\` on Windows). Without a per-user cache directory (no `HOME`/`LOCALAPPDATA`) it fails too: seite never runs the CLI from inside a project. Offline, or on an unsupported platform, install `tailwindcss` yourself and point seite at it through the environment with an absolute path (`seite.toml` cannot choose a program to run):

```bash
export SEITE_TAILWIND_BIN=/usr/local/bin/tailwindcss
```

## tailwind-failed

**What it means:** `[build.tailwind]` is set and the Tailwind CLI did not produce the stylesheet: the `input` file is missing, a symlink or outside the site, `SEITE_TAILWIND_BIN` does not exist or cannot run, the CLI ran longer than two minutes, or `tailwindcss` exited with an error (the message quotes its output, for example `Cannot apply unknown utility class`). An error: `seite build` stops. The file is the Tailwind input (or `seite.toml` for a bad `SEITE_TAILWIND_BIN` or input path). `seite check`, `seite audit`, agent hooks and MCP check tools never run Tailwind, so they never report it.

**How to fix:** fix what Tailwind reports in the input CSS (an unknown class in `@apply`, a bad `@import` or `@source` path), or create the input with `seite theme new --tailwind`. To see the full output, run the same command from the site root: `tailwindcss -i static/css/tailwind.css -o /tmp/out.css`.

## tailwind-untrusted

**What it means:** the site has `[build.tailwind]`, and you have not trusted this project. Tailwind CSS runs the project's own code: its stylesheet can load plugins (`@plugin "./x.js"`, `@config`) that are JavaScript files from the repository, and can import any CSS file it reaches. seite only runs it for projects you trusted. An error: `seite build`, `seite serve` and the MCP `seite_build` tool stop. The file is `seite.toml`, at the `[build.tailwind]` line. `seite check` and `seite audit` never run Tailwind and never report it.

**How to fix:** if you trust the repository's code (as you would before running its npm scripts), run this once in the site root:

```bash
seite trust            # --revoke undoes it, --list shows trusted projects
```

The decision is stored per user (in seite's config directory, keyed by the site's path), never in the repository. Coding agents are not allowed to run `seite trust`. In CI, set `SEITE_TRUST_PROJECT=1` in the build step's environment (the workflows seite generates do this for Tailwind sites); see [Tailwind CSS](/docs/configuration#tailwind-css).

## template-markdown-link

**What it means:** a template in `templates/` builds the page's markdown URL as `{{ page.url }}.md`. That produces broken links such as `/.md`, `/posts/.md` or `/tags/.md` on pages that have no markdown copy (the homepage, collection indexes, tag pages, paginated pages, 404). Reported as a warning.

**Common causes:** a `base.html` created or customized before seite 0.20.2, or copied from an older theme.

**How to fix:** use `page.markdown_url`, which is set only when a `.md` copy is actually published, and guard the tag with it.

```html
<!-- before -->
<link rel="alternate" type="text/markdown" href="{{ site.base_url }}{{ page.url }}.md">
<!-- after -->
{% if page.markdown_url %}<link rel="alternate" type="text/markdown" href="{{ site.base_url }}{{ page.markdown_url }}">{% endif %}
```

## template-missing-seite-head

**What it means:** your `templates/base.html` does not include `seite/head.html` (directly, or through a template it includes or extends, such as `templates/partials/head.html`), the partial that writes the canonical URL, description, Open Graph and Twitter tags, JSON-LD, hreflang, feed, `llms.txt` and markdown links. Reported as a warning (it fails only `--strict` builds and checks). The message says which case applies:

- **legacy theme**: the file still has its own copy of those tags (a bundled theme from before seite 0.21, or a copy of one). The site works, but SEO and GEO fixes in new seite versions never reach it.
- **missing SEO head**: the file has neither the include nor its own tags, so pages ship without them.

**Common causes:** a theme applied or customized before 0.21; a redesign that rewrote `<head>` and dropped the include.

**How to fix:** run `seite upgrade` (it replaces a bundled theme you never edited), or `seite theme apply <name>`. To keep a customized design, replace the SEO tags after `<title>` with the include, and use the other partials the same way:

```html
<title>{% block title %}{{ site.title }}{% endblock %}</title>
{% include "seite/head.html" %}      {# inside <head>, right after <title> #}
{% include "seite/search.html" %}    {# where the search box goes #}
{% include "seite/scripts.html" %}   {# right before </body> #}
```

See [Templates → seite partials](/docs/templates#seite-partials).

## template-parse

**What it means:** a template has a Tera syntax error. In `seite check` this is an error; in `seite build` it is a warning because the build falls back to the built-in templates (so your customizations are silently missing from the output).

**Common causes:** an unclosed `{% block %}`, `{% if %}` or `{% for %}`, a misspelled tag (`{% endfor %}` vs `{% end for %}`), Jinja or Liquid syntax Tera does not support.

**How to fix:** fix the template at the line and column given. `seite templates` lists the blocks the base template defines.

## template-render

**What it means:** a template parsed but failed while rendering a page.

**Common causes:** a variable that does not exist in that context (`page.titel`), a filter applied to the wrong type, a missing `{% extends "base.html" %}` block, an `include` of a file that does not exist.

**How to fix:** the hint suggests the closest variable name. `seite templates` lists the variables available to each kind of template (page, collection index, homepage, tags, 404).

## url-collision

**What it means:** two content files would be published at the same URL, so one would overwrite the other.

**Common causes:** the same slug in two files (`hello.md` and `2026-01-02-hello.md` in a collection that strips dates), a `slug:` frontmatter override that matches another page, or a translation file with the wrong language suffix.

**How to fix:** give one of the files a different filename or `slug:`, or delete the duplicate. The message names both files.
