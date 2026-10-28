---
title: "Import"
description: "seite import: move a WordPress, Ghost, Jekyll or Hugo site into seite in one command, with images, old-URL redirects and a report your coding agent can finish."
weight: 12
---

`seite import` moves an existing site into a seite site in one command. It writes Markdown posts and pages with mapped frontmatter, downloads or copies their images, keeps every old URL working through `aliases:` redirects, and leaves a report of everything that still needs a human or a coding agent. It needs no account or paid service, and importing the same source twice gives byte-identical files.

```bash
seite init myblog --collections posts,pages
cd myblog
seite import wordpress ~/Downloads/oldblog.WordPress.2026-10-01.xml
seite import ghost ~/Downloads/my-ghost.ghost.2026-10-01.json --site-url https://blog.example.com
seite import jekyll ../old-jekyll-site
seite import hugo ../old-hugo-site
seite check            # then `/seite check` in your coding agent
```

Run it inside a seite site. Posts go to the `posts` collection and pages to `pages`. Nothing is created in `seite.toml`: when a target collection is missing, the import stops and names the `seite collection add` command to run.

## Sources

| Source | What to pass | How to get it |
|---|---|---|
| WordPress | the WXR `.xml` export | Tools → Export → All content |
| Ghost | the JSON export | Settings → Migrate → Export content |
| Jekyll | the site directory (with `_config.yml`) | your repository |
| Hugo | the site directory (with `hugo.toml`/`config.toml`, or a `content/` directory) | your repository |

Export files are read up to 200 MB. Malformed input stops the import with the file and line (and column, for JSON) before anything is written.

## Options

| Flag | Effect |
|---|---|
| `--collection <name>` | collection for posts (default `posts`) |
| `--pages-collection <name>` | collection for pages (default `pages`) |
| `--dry-run` | report what would be imported; write nothing (no files, no images, no report) |
| `--force` | overwrite existing content files (default: skip them and list them as skipped) |
| `--drafts` | also import drafts, pending, private, scheduled and members-only items, as `draft: true` |
| `--no-images` | don't download or copy images; references keep their old URLs |
| `--allow-host <host>` | also download images from this host (`cdn.example.com`, `127.0.0.1:8080`); repeatable |
| `--site-url <url>` | the old site's URL when the export doesn't name it (Ghost writes `__GHOST_URL__`) |
| `--max-download-mb <n>` | total image download budget (default 500 MB) |

Without `--drafts`, drafts and other unpublished items are skipped and listed in the report. Trashed items, auto-drafts, revisions and menu items are never imported.

## What is mapped

| seite frontmatter | WordPress | Ghost | Jekyll | Hugo |
|---|---|---|---|---|
| `title` | title | title | `title` (else from the filename) | `title` |
| `date` | post date as written on the site | `published_at` in `[site] timezone` | `date` / filename date as written | `date` as written |
| `updated` | `post_modified` | `updated_at` | `last_modified_at` | `lastmod` |
| `description` | excerpt | custom excerpt / meta description | `description` / `excerpt` | `description` / `summary` |
| `tags` | tags | public tags (internal `#tags` dropped) | `tags` | `tags` |
| `categories` | categories (not "Uncategorized") | | `categories` + `<cat>/_posts` dirs | `categories` |
| `image` | featured image | `feature_image` | `image`, `header.image` | `images[0]`, `cover.image`, bundle `cover.*` |
| `draft: true` | draft/pending/private/scheduled, password-protected | draft/scheduled, members-only | `_drafts/`, `published: false` | `draft = true` |
| `aliases` | permalink, `_wp_old_slug` | `/<slug>/`, same-site canonical URL | permalink style or `permalink:`, `redirect_from` | `url`, `[permalinks]`, `aliases` |
| `extra.author` | author display name | primary author | `author` | `author` / `authors` |

Hugo also maps `series`, `expiryDate` (→ `expires`) and page `weight`.

- **Categories** go into a `categories` taxonomy only when `seite.toml` has `[[taxonomies]] name = "categories"`; otherwise they are added to `tags` and the report says so.
- **Authors** are written to `extra.author` unless the collection has `strict_fields = true` without an `author` field; then they are counted in the report.
- **Slugs** come from the source slug (percent-decoded and transliterated), files are named like `seite new` names them (`YYYY-MM-DD-slug.md` in dated collections). When two items, or an item and existing content, would get the same URL, the later one becomes `slug-2`, `slug-3`, … in a fixed order and the report lists it.
- **Old URLs** become `aliases:` only when they differ from the new URL, so `seite build` writes a redirect page at each one (and a `_redirects` entry). `.html` endings are dropped from aliases: `/2020/01/02/hello.html` becomes the redirect file `dist/2020/01/02/hello.html`.
- **Schema.** The frontmatter is checked against the collection's schema (`[collections.fields]`); problems are written anyway and reported with the file and line.

## Bodies

- **WordPress and Ghost HTML** is converted to Markdown with [htmd](https://crates.io/crates/htmd). WordPress block comments (`<!-- wp:… -->`) are removed and classic-editor paragraphs are rebuilt. `[caption]` and `<figure>` with an image become `{{< figure(...) >}}`; `[embed]`, YouTube/Vimeo iframes and video URLs alone on a line become `{{< youtube(...) >}}` / `{{< vimeo(...) >}}`. `[gallery]` and other WordPress shortcodes stay in place and are reported with their line in the export. Other iframes, `<video>` and `<audio>` are kept as HTML.
- **Jekyll Liquid:** `{% highlight lang %}` becomes a fenced code block, `{% raw %}` is unwrapped, `{{ site.baseurl }}` and `| relative_url` are dropped, `{% link %}` and `{% post_url %}` become the imported page's URL. `{% include %}` and any other Liquid stay in place and are reported.
- **Hugo shortcodes:** `youtube`, `vimeo`, `gist`, `figure`, `highlight` and `ref`/`relref` are converted. Any other shortcode is kept inside inline code (`` `{{< tweet … >}}` ``) so the build still passes, and reported. Hugo's `_index.md` list pages, headless bundles and translations (`about.fr.md`) are skipped and listed.
- Text that seite would read as a shortcode (a literal `{{<` in a WordPress post) is escaped as `{{\<`, which renders the same.

## Images

Images referenced by a body (`![](…)`, `<img src>`, `figure`, links to image files) and the featured image are stored under `static/images/imported/<slug>/` and the references rewritten to `/static/images/imported/<slug>/<name>`. Identical images are stored once.

- **Downloads** (WordPress, Ghost) use only `http`/`https`, and only from the export's own site host (WXR `<link>`/`wp:base_site_url`, Ghost's `url` setting or `--site-url`) or an `--allow-host` (host and port must match). Redirects are followed only to allowed hosts. Because the export file names its own site, a host taken from it must resolve to a public address (no loopback, private, link-local/cloud-metadata or CGNAT ranges), checked before every request and redirect; hosts you pass with `--allow-host` or `--site-url` are trusted as given. Each file must be served as `image/*` and be at most 20 MB, the whole import stays within `--max-download-mb`, and each request times out after 30 seconds. Anything refused or failed keeps its old URL and is listed in the report.
- **Copies** (Jekyll, Hugo) resolve root-relative paths against the site root (Jekyll) or `static/` and `assets/` (Hugo), and relative paths against the page's folder (Hugo page bundles). A file whose real path (after symlinks) is outside the source directory is never copied. External images on Jekyll and Hugo sites are left as they are.

## The report

The import prints a summary (imported, skipped and failed posts and pages; images downloaded, copied, deduplicated, failed and not imported), then the warnings, each with its source location (`export.xml:45`) and the file it concerns. `--json` returns the full report: `summary`, `images`, every item with its `source_id`, `status`, `path`, `url` and `aliases`, and every warning with `source`, `code`, `message` and `path`.

The same report is written to `.seite/cache/import-report.md` (gitignored) as a checklist, so a coding agent can work through it: run `seite check`, then ask your agent to finish the import (`/seite check` in Claude Code, `$seite check` in Codex).

Warning codes: `wp-shortcode`, `liquid`, `hugo-shortcode`, `hugo-section`, `unresolved-link`, `slug-collision`, `no-date`, `schema`, `categories-as-tags`, `author-dropped`, `members-only`, `canonical-elsewhere`, `ghost-url`, `empty-body`, `frontmatter`, `image-not-imported`, `image-failed`.
