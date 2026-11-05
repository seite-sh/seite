---
title: Git-based CMS
description: "Give editors a web UI for your seite site with a free Git-based CMS, Pages CMS or Decap CMS, configured from your collections and schema in one command."
weight: 15
---

seite sites are markdown files in a Git repository, so any Git-based CMS can edit them: the CMS reads and commits files through the GitHub API, and your normal deploy publishes the change. `seite cms setup` writes the CMS configuration from `seite.toml`: one CMS collection per seite collection, with fields from the built-in frontmatter and your [frontmatter schema](/docs/configuration#frontmatter-schemas).

Both supported CMSs are free and need no paid service.

```bash
seite cms setup pages-cms          # writes .pages.yml
seite cms setup decap              # writes static/admin/index.html + static/admin/config.yml
seite cms setup decap --branch main
seite cms setup pages-cms --force  # regenerate after changing collections or fields
```

`seite cms setup` never overwrites an existing file unless you pass `--force`; with `--force` the previous file is first copied to `.seite/cache/backups/<path>.bak` (then `<path>.1.bak`, … so earlier backups are kept). That directory is gitignored and never deployed. `--force` refuses to replace a symlink. `--json` reports the files written, backups, skipped fields and next steps.

## Sites in a subdirectory of the repository

In a monorepo or a [workspace](/docs/workspace), the CMS works with repository paths. `seite cms setup` asks git for the site's path inside the repository (`git rev-parse --show-prefix`, hooks disabled) and prefixes every folder with it (`sites/blog/content/posts`, `sites/blog/static/images`). `.pages.yml` is written at the repository root, because that is where Pages CMS reads it; it then describes this site only, so for several sites in one repository merge their `content` entries by hand. Decap's files stay in the site's `static/admin/`. A site reached through a symlink, whose path does not end with the repository prefix, is refused with an explanation.

## Pages CMS

[Pages CMS](https://pagescms.org) is a hosted, open-source CMS for GitHub repositories. There is nothing to deploy: commit `.pages.yml`, open [app.pagescms.org](https://app.pagescms.org), sign in with GitHub and pick the repository.

The generated `.pages.yml` (see the [Pages CMS configuration reference](https://pagescms.org/docs/configuration/)) contains:

- `media`: uploads go to `static/images/` and are linked as `/static/images/…` (prefixed with the base path of a project site).
- one `content` entry per collection: `type: collection`, `path: content/<directory>`, `format: yaml-frontmatter`, and a `filename` that matches seite's naming — `{year}-{month}-{day}-{primary}.md` for dated collections (posts, changelog), `{primary}.md` otherwise. Nested collections (docs) get `subfolders: true`.
- fields: `title`, `date` (dated collections), `description`, `tags`, `image`, `updated` (dated collections), `draft`, your schema fields, and the markdown `body`.

## Decap CMS

[Decap CMS](https://decapcms.org) is a single-page app served from your own site at `/static/admin/`. `seite cms setup decap` writes:

- `static/admin/index.html`: loads Decap CMS 3.16.3 from jsDelivr with a pinned version and a Subresource Integrity hash (the browser refuses a modified file), and carries `<meta name="robots" content="noindex, nofollow">`.
- `static/admin/config.yml`: `backend: github` with `repo` read from your `origin` remote (when it is on GitHub; otherwise `OWNER/REPO` to fill in), `branch` (`--branch`, else the current branch, else `main`), `media_folder: static/images`, `public_folder: /static/images`, and one folder collection per seite collection with `create: true`, a slug matching seite's file naming, and widgets for every field.

The admin page is copied to `dist/static/admin/` like any static file. It is not a content page, so it never appears in the sitemap, the search index, `llms.txt`/`llms-full.txt` or `seite audit`.

### Authentication

Decap's GitHub backend logs editors in with GitHub OAuth, which needs a small server-side helper (an "OAuth proxy") because a static site cannot keep the OAuth client secret. Free options:

- **On Netlify**: create a GitHub OAuth app and add it under *Site configuration → Access & security → OAuth*. No `base_url` needed.
- **Anywhere else** (GitHub Pages, Cloudflare Pages): deploy a Cloudflare Worker such as [sveltia-cms-auth](https://github.com/sveltia/sveltia-cms-auth) or [decap-proxy](https://github.com/sterlingwes/decap-proxy) on the free Workers plan, then add its URL to `config.yml`:

  ```yaml
  backend:
    name: github
    repo: owner/repo
    branch: main
    base_url: https://cms-auth.<you>.workers.dev
  ```

Netlify Identity with Git Gateway (which lets editors log in without a GitHub account) was deprecated by Netlify in 2025 and gets no maintenance; check its current status before relying on it, and prefer one of the options above.

Rerunning `seite cms setup decap --force` regenerates `config.yml`, so add `base_url` again afterwards (the backup in `.seite/cache/backups/static/admin/config.yml.bak` has it).

## Field mapping

| seite type | Pages CMS | Decap CMS |
|---|---|---|
| `string`, `url` | `string` | `string` |
| `text` | `text` | `text` |
| `number` / `integer` | `number` | `number` (`value_type: float` / `int`) |
| `bool` | `boolean` | `boolean` |
| `date` | `date` (`yyyy-MM-dd`) | `datetime` (`YYYY-MM-DD`, no time) |
| `datetime` | `date` with `time: true` | `datetime` |
| `image` | `image` | `image` |
| `enum` | `select` | `select` |
| `list` (of `enum`) | `select` with `multiple: true` | `select` with `multiple: true` |
| `list` (other items) | the item type with `list: true` | `list` |
| `object` | skipped | skipped |

A schema field's `required`, `description` and `default` carry over. A schema field named like a built-in (for example `description`) replaces the built-in one. `object` fields have no nested schema in seite, so they are skipped and listed in the command's output — edit them in the file.

## Keeping it in sync

After you rename or remove a collection, `seite check` warns ([`cms-config-stale`](/docs/errors#cms-config-stale)) when `.pages.yml` or `static/admin/config.yml` still points at the old folder. Rerun `seite cms setup <cms> --force`.

## Next Steps

- [Configuration](/docs/configuration#frontmatter-schemas) — frontmatter schemas the CMS fields come from
- [Deployment](/docs/deployment) — publish what editors commit
