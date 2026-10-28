---
name: seite
description: Work on this seite static site — write or edit content, check and fix errors, audit SEO and accessibility, preview, build, deploy, design the theme or brand, build landing pages, add collections, upgrade seite, and manage the check hooks. Use for any change to content/, templates/, data/, static/, public/, or seite.toml.
argument-hint: "[check | audit | new <type> \"<title>\" | preview | build | deploy | theme [name|description] | brand | landing [page] | collection [preset] | upgrade | hooks [on|off|status]]"
# seite-skill-version: 7
---

# seite

Run `/seite <command> [args]` (Codex: `$seite <command> [args]`). Read the command's reference file, then follow its steps.

| Command | Use it to | Steps |
|---|---|---|
| `check` | find and fix every error | [reference/check.md](reference/check.md) |
| `audit` | fix SEO, AI-visibility and accessibility findings | [reference/audit.md](reference/audit.md) |
| `new <type> "<title>"` | create a post, doc, page, … | [reference/new.md](reference/new.md) |
| `preview` | show the site in a browser | [reference/preview.md](reference/preview.md) |
| `build` | build `dist/` and report problems | [reference/build.md](reference/build.md) |
| `deploy` | publish, after the user confirms | [reference/deploy.md](reference/deploy.md) |
| `theme [name \| description]` | apply or design the theme | [reference/theme.md](reference/theme.md) |
| `brand` | logo, colors, favicon | [reference/brand.md](reference/brand.md) |
| `landing [page]` | homepage or a landing page | [reference/landing.md](reference/landing.md) |
| `collection [preset]` | list or add collections | [reference/collection.md](reference/collection.md) |
| `upgrade` | update this project to the installed seite | [reference/upgrade.md](reference/upgrade.md) |
| `hooks [on \| off \| status]` | manage the automatic checks | [reference/hooks.md](reference/hooks.md) |

## Routing

- Command given: open its reference file and follow it.
- No command: choose one from the request ("write a post about X" → `new`, "is the site OK?" → `check`, "improve SEO" → `audit`, "publish" → `deploy`, "make it look like X" → `theme`). If two fit equally well, ask the user once.

## Principles

- This skill drives the `seite` CLI, version 0.21 or newer. If `seite --version` fails, ask the user before installing it (`curl -fsSL https://seite.sh/install.sh | sh`; Windows: `irm https://seite.sh/install.ps1 | iex`); if it prints an older version, suggest `seite self-update`. If no `seite.toml` exists in this directory or its parents, this is not a seite site: say so, and run `seite init` only if the user wants a new site.
- Run `seite context` once at the start of a session. If it fails, read `seite.toml` and list `content/` instead.
- Prefer seite commands to hand edits: `seite new`, `seite frontmatter set`, `seite theme apply`, `seite collection add`.
- Edit sources only (`content/`, `templates/`, `data/`, `static/`, `public/`, `seite.toml`). Never edit `dist/`.
- Look up seite features in `.seite/docs/` or with `seite docs <query>`; `seite explain <code>` explains a diagnostic code.
- Verify once: `seite check`, fix every error, run `seite check` again, then stop. Report errors that remain instead of looping.
