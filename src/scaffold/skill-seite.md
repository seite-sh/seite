---
name: seite
description: Work on this seite static site — write or edit content (posts, docs, pages), check the site and fix every error, preview it locally, build it, deploy it, switch or tune the theme, and add collections. Use for any change to content/, templates/, data/, static/, or seite.toml, and whenever the user asks to check, fix, preview, build, publish, or deploy the site.
# seite-skill-version: 1
---

# seite site workflow

One entry point for everyday work on this site. `AGENTS.md` is the project reference (commands, content format, collections, conventions) and the context guides it indexes cover templates, shortcodes, data files, and config — read those for details; this skill only decides *what to run, in what order*.

**Dispatch:** `/seite <command> <args>` (Codex: `$seite <command> <args>`). Run the matching command below. With no command, infer it from the request (e.g. "write a post about X" → `new`, "is the site OK?" → `check`, "publish" → `deploy`), and ask once if two commands fit equally.

Prefer the seite MCP tools (`seite_*`) when they are available; the `seite` CLI does the same thing otherwise.

## Commands

| Command | What it does | Reference |
|---|---|---|
| `check` | Validate the whole site and fix every problem | [check](#check) |
| `new <type> "<title>"` | Create a post, doc, page, … and write it | [new](#new) |
| `preview` | Start the dev server in the background, report the URL | [preview](#preview) |
| `build` | Build `dist/` and report problems | [build](#build) |
| `deploy` | Dry-run, then deploy only after the user confirms | [deploy](#deploy) |
| `theme [name \| description]` | List, apply, or design a theme | [theme](#theme) |
| `collection [preset]` | List collections or add a preset | [collection](#collection) |

## check

1. Run MCP `seite_check` (or `seite check`). Output is `file:line:col: severity[code]: message` plus a `hint:`.
2. Fix every error at the reported source file and line — the `.md`, template, data file, or `seite.toml`, never `dist/`.
3. Re-run until it reports no errors. Then fix warnings (broken links, missing assets, unknown config keys) unless the user only asked about errors; `seite check --strict` fails on those too.
4. Summarize what you changed.

## new

1. Create the file with MCP `seite_create_content` (or `seite new <type> "<title>"`, with `--tags a,b`, `--draft`, or `--lang xx` as asked). `<type>` is a collection's singular name (`post`, `doc`, `page`, …).
2. Write the body the user asked for; keep the frontmatter format described in `AGENTS.md`. Link other pages with relative `.md` paths.
3. Run `check` on the result.

## preview

1. Start `seite serve --no-repl` as a background process (add `--build` if `dist/` is missing or stale). It keeps serving with live reload while you edit.
2. Report the local URL it prints (default `http://localhost:3000`; a free port is picked if that is taken).
3. Stop the process when the user is done, if your harness tracks background jobs.

## build

1. Run MCP `seite_build` (or `seite build --json`; add `--drafts` to include drafts).
2. Report `broken_links`, `missing_assets`, and `diagnostics` from the result. If there are errors, switch to `check`.

## deploy

1. Run `check` first; do not deploy a site with errors.
2. Run `seite deploy --dry-run` and show the user what it would do (target, branch, preview vs production).
3. **Ask the user before running a real `seite deploy`** — it commits, pushes, and publishes. Pass `--preview` for a staging URL, `--no-commit` if the user wants to commit themselves.

## theme

- No argument: `seite theme list`.
- A bundled theme name: MCP `seite_apply_theme` (or `seite theme apply <name>`), then `build`.
- A design description or "custom": use the `theme-builder` skill (or `seite theme create "<description>"` with Claude Code). Themes replace only `templates/base.html`.

## collection

- No argument: `seite collection list`.
- A preset (`posts`, `docs`, `pages`, `changelog`, `roadmap`, `trust`): MCP `seite_create_collection` (or `seite collection add <preset>`), then `check`.
