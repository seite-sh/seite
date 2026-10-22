---
title: "Error Reference"
description: "Every diagnostic code reported by seite check and seite build: what it means, common causes, and how to fix it."
weight: 12
---

`seite check` and `seite build` report problems as compiler-style lines such as `content/posts/a.md:3:1: error[frontmatter-parse]: ...`. The part in brackets is a stable **diagnostic code**. Each code has a section below; run `seite explain <code>` to print it in the terminal. In `--json` output every diagnostic carries a `docs_url` pointing at its section here.

Errors fail `seite check` and `seite build`. Warnings fail only with `--strict`.

## broken-link

**What it means:** an internal link points at a page (or, for relative `.md` links, a content file) that does not exist in the built site. Reported as a warning, or as an error with `--strict`.

**Common causes:** a typo in the URL, a page that was renamed or deleted, a missing collection prefix (`/hello` instead of `/posts/hello`), a link to a draft page in a build without `--drafts`, or a relative `.md` link whose path is not relative to the linking file.

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

## i18n-partial

**What it means:** a per-language value in a data file (a "language map" such as `description: {en: ..., de: ...}`) is missing one of the site's configured languages. Pages in that language fall back to the default language's text. Reported as a warning.

**Common causes:** a language was added to `seite.toml` (`[languages.xx]`) without translating the data files, or a typo in a language code.

**How to fix:** add the missing language key to the value named in the message, or remove the language from `seite.toml` if the site is not translated yet. See [multi-language sites](/docs/i18n).

## missing-asset

**What it means:** a page references a file (image, stylesheet, script, download) that does not exist in the build output. Reported as a warning, or as an error with `--strict`.

**Common causes:** the file was never added to `static/` or `public/`, a typo in the path, a path relative to the wrong directory, or a wrong case (`Logo.png` vs `logo.png`) that works on macOS but breaks on Linux hosts.

**How to fix:** add the file under `static/` (served from `/static/...`) or `public/` (served from the site root), or fix the path in the content or template.

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
