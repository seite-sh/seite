---
title: "seite CLI Reference"
description: "Complete reference for every seite command, flag, and option, from init and build to deploy, agent, and self-update. Includes workspace and MCP commands."
weight: 11
---

Every feature of the seite static site generator is accessible through its CLI. This page documents every command, flag, and option.

{{% callout(type="tip") %}}
Run `seite <command> --help` for quick inline help on any command.
{{% end %}}

## Overview

`seite` has twenty subcommands. Running `seite` with no subcommand shows a context-aware welcome screen with the most useful commands for your situation.

| Command | Description |
|---------|-------------|
| `init` | Create a new site |
| `build` | Build the site |
| `check` | Validate the site without building it |
| `audit` | Free SEO, AI-visibility, accessibility and performance audit |
| `serve` | Development server with live reload |
| `new` | Create content files |
| `agent` | Open a coding agent in the site |
| `theme` | Manage themes |
| `hooks` | Turn the coding-agent check hooks on or off |
| `deploy`| Deploy to hosting platforms |
| `collection` | Add or list collections |
| `contact` | Set up contact forms |
| `access` | Manage Cloudflare Pages password groups |
| `skill` | Manage skill packs |
| `workspace` | Manage multi-site workspaces |
| `mcp` | MCP server for AI tool integration |
| `upgrade` | Update project config to match current binary |
| `self-update` | Update the seite binary to the latest release |
| `completions` | Generate shell completion scripts |
| `perf` | Audit site performance via PageSpeed Insights |
| `telemetry` | Manage anonymous usage telemetry |

### Global Flags

These flags work with any command:

| Flag | Description |
|------|-------------|
| `--site <name>` | Target a specific site in a workspace |
| `--config <path>` | Path to the project's `seite.toml` (must be that exact filename) |
| `--dir <path>` | Project directory |
| `--verbose` | Enable verbose logging (also shows per-step build timings) |
| `--json` | Print exactly one JSON document on stdout — `{"ok":true,"command":...,"data":...,"warnings":[...]}` or `{"ok":false,"command":...,"error":{"message":...,"chain":[...]}}` (plus `data` when the command has results to report, such as `upgrade --check`'s pending changes) — with all human-readable output on stderr. Not supported by `serve`, `agent`, `mcp`, `completions`, or `self-update`, which stream output or take over the terminal |
| `-y`, `--yes` | Never prompt: accept defaults and answer "yes" to confirmations (also `SEITE_YES=1`). Without a terminal, prompts fall back to their defaults, and any value with no default must be passed as a flag or the command errors naming it |

## seite init

Create a new site directory with scaffolded structure. See [Getting Started](/docs/getting-started) for a guided walkthrough.

```bash
seite init <name> [options]
```

| Flag | Description |
|------|-------------|
| `--title` | Site title |
| `--description` | Site description |
| `--deploy-target` | `github-pages`, `cloudflare`, or `netlify` |
| `--collections` | Comma-separated list: `posts,docs,pages,changelog,roadmap` |
| `--theme` | Starter theme: `default`, `docs`, `landing`, or `blank` (for designing your own). Default: `docs` for a docs-only site, `default` otherwise. Gallery themes are applied later with `seite theme apply <name>` |
| `--agents` | Coding agents to set up: `claude,codex,opencode,cursor,gemini` or `all` (default: all; the interactive picker preselects the agents installed on your machine) |
| `--mcp` | Also add the seite MCP server to each agent's config (off by default: coding agents use the CLI) |

`seite.toml` gets `[site] timezone` set to the system's IANA time zone (from `TZ` — empty means UTC; a POSIX rule such as `CET-1CEST` is not an IANA name — else the OS setting; `seite workspace add` does the same), so "today" for scheduled posts is the same on every machine and in CI; when no zone can be detected, the line is written commented out.

If flags are omitted, `seite init` prompts interactively. Without a terminal (or with `-y`/`--yes`), it uses defaults for everything except `--deploy-target`, which has none and is required in that case.

```bash
# Non-interactive
seite init mysite --title "My Blog" --deploy-target github-pages --collections posts,pages

# Only set the site up for Claude Code and Codex
seite init mysite --deploy-target github-pages --agents claude,codex

# Interactive
seite init mysite
```

Every site gets an `AGENTS.md` (read by every agent) and the docs for the installed seite version in `.seite/docs/`. `--agents` decides which agent-specific files are generated from the same bundled content:

| Agent | Files |
|-------|-------|
| `claude` (Claude Code) | `CLAUDE.md` (`@AGENTS.md` import), `.claude/settings.json` (permissions + hooks), `.claude/launch.json` (desktop preview), `.claude/rules/*.md`, `.claude/skills/seite/` |
| `cursor` (Cursor editor + `cursor-agent`) | `.cursor/cli.json`, `.cursor/hooks.json`, `.cursor/rules/*.mdc` (same guides, `globs` frontmatter) |
| `codex` (Codex CLI) | `.codex/hooks.json` |
| `opencode` (OpenCode) | `opencode.json` (permission defaults), `.opencode/commands/seite.md`, `.opencode/plugins/seite-check.js` |
| `gemini` (Gemini CLI) | `GEMINI.md` (`@AGENTS.md` import), `.gemini/settings.json` (hooks), `.gemini/commands/seite.toml` |

Codex, Cursor, OpenCode, and Gemini CLI read the `seite` skill from `.agents/skills/seite/`. With `--mcp`, each agent also gets its MCP config (`.mcp.json`, `.cursor/mcp.json`, `.codex/config.toml`, `opencode.json` `mcp`, `.gemini/settings.json` `mcpServers`). Every agent gets the `/seite` skill (`$seite …` in Codex): `check`, `audit`, `new`, `preview`, `build`, `deploy`, `theme`, `brand`, `landing`, `collection`, `upgrade`, `hooks` (see [Coding Agents](/docs/agent)). The selection is stored in `.seite/config.json` so `seite upgrade` keeps the same set of files current.

## seite build

Build the site from `seite.toml` in the current directory.

```bash
seite build [options]
```

| Flag | Description |
|------|-------------|
| `--drafts` | Include draft content in the build |
| `--strict` | Treat broken internal links and missing assets as build errors |
| `--future` | Include future-dated (scheduled) content, like `[build] future = true` (see [Scheduled publishing](/docs/configuration#scheduled-publishing)) |

The build pipeline cleans the output directory, loads templates, processes each collection, renders pages, generates RSS/sitemap/discovery files (`llms.txt`, `robots.txt`), writes markdown alongside the HTML, builds the search index, copies static files, processes images, and post-processes the generated HTML (srcset, lazy-loading, analytics injection, link validation, ...). Per-step timing is shown with `--verbose` (always included in `--json` output).

A full build never writes into `dist/` directly: it renders into a temporary staging directory next to it and only swaps it into place once every step (including any subdomain builds) succeeds. If the build fails partway through, or the process is killed, the previous `dist/` is left exactly as it was — you never end up with a half-written site. Leftover staging directories from a crashed or interrupted build are cleaned up automatically on the next build.

After building, `seite build` validates all internal links and asset references (`img`, `srcset`, `script`, `link rel=stylesheet`, `video`, `audio`, `track`) in the generated HTML. Broken links and missing assets (e.g., links pointing to `/posts/missing-slug`, an `<img>` with no matching file) are reported as warnings by default; `--strict` turns them into errors. Each one is attributed to where it was written — the markdown source file, a template, or a data file, with a line number when it can be found — falling back to naming the generated page when the source can't be traced (e.g. a listing page). A "did you mean" suggestion is included when a close match exists among the site's valid URLs. With `--json`, the result's `data.broken_links` and `data.missing_assets` (each grouped by target, with `source`/`line`/`locations`) and `data.warnings` give the same information as structured JSON instead of terminal text.

Relative links between markdown source files (`[intro](../docs/intro.md)`, or root-relative `/content/docs/intro.md`) are rewritten to the target page's published URL, keeping any `#fragment` or `?query`, preferring a translation in the current page's language when one exists, and respecting `base_path`. A link like this that matches no content file becomes a broken-link warning (or error, with `--strict`) instead of shipping a dead `.md` link. Root-relative links outside the content directory (e.g. `/docs/intro.md`) are left alone: they point at the raw markdown copy published alongside every page.

Problems are reported all at once, one per line, in compiler style (`file:line:col: severity[code]: message`, plus a `hint:` line). A bad shortcode in one post no longer hides a broken frontmatter in another. Unknown keys in `seite.toml` (e.g. `minfy = true`) are warnings with a did-you-mean hint; the build still succeeds. With `--json`, successful builds list warnings in `data.diagnostics`, and failed builds list every problem in `error.diagnostics`.

## seite check

Validate everything without touching the output directory: config (syntax and unknown keys), templates, data files, every content file (frontmatter and shortcodes), a full render, and internal links. The render happens in a temporary directory that is discarded, so `dist/` is never created or modified.

```bash
seite check [options]
```

| Flag | Description |
|------|-------------|
| `--strict` | Fail on warnings too (unknown config keys, broken links, ...) |
| `--drafts` | Include draft content |
| `--file <path>...` | Check only these files: frontmatter and shortcodes for content, Tera syntax for templates, syntax for data files, unknown keys for `seite.toml`. No render and no link check, so it's fast enough to run after every edit |
| `--hook <agent>` | Run as a coding agent's hook (`claude`, `codex`, `cursor`, `opencode`, `gemini`): reads the hook input on stdin, answers in that agent's hook protocol only when there are errors, and always exits 0. `seite init` wires it up; see [Check hooks](/docs/agent#check-hooks) |
| `--post-edit` | With `--hook`: check only the file(s) the agent just edited, named in the hook input, instead of the whole site |
| `--future` | Treat future-dated (scheduled) posts as published, like `seite build --future` (links to them are then fine) |
| `--external` | Also check external `http(s)` links in the rendered site over the network: 8 at a time (2 per host), 10 s timeout, `HEAD` then `GET`, up to 5 redirects. Working links are cached for 7 days in `.seite/cache/links.json`; failures are re-checked every run. Skips hosts that block bots and `[check] external_skip`. Never runs by default or in hooks |

Exits 0 when there are no errors (in `--strict` mode: no diagnostics at all) and 1 otherwise. Each diagnostic has a stable `code` that agents and CI can match on:

| Code | Severity | Meaning |
|------|----------|---------|
| `config-invalid` | error | `seite.toml` does not parse or has a wrong value type |
| `config-unknown-key` | warning | Key the config schema does not know (typo) |
| `frontmatter-missing` | error | Content file has no `---` frontmatter block |
| `frontmatter-parse` | error | Frontmatter YAML is invalid (line points into the file) |
| `frontmatter-schema` | error | Frontmatter breaks the collection's `[collections.fields.*]` schema (missing required field, wrong type, enum, bounds, pattern, custom field both at the top level and under `extra:`) |
| `frontmatter-schema-unknown` | warning | Key not declared in the schema (only with `strict_fields = true`) |
| `content-invalid` | error | Content file cannot be read |
| `shortcode-syntax` | error | Malformed shortcode (Hugo-style syntax gets a corrected example) |
| `shortcode-unknown` | error | Shortcode name is not registered (did-you-mean hint) |
| `shortcode-render` | error | Shortcode template failed to render |
| `data-file-parse` | error | YAML/JSON/TOML data file is invalid |
| `data-conflict` | error | Two data files map to the same `data.*` key |
| `url-collision` | error | Two pages resolve to the same URL |
| `template-parse` | error | Template has a syntax error (file, line, column) |
| `template-render` | error | Page failed to render (names the content file and the template) |
| `i18n-partial` | warning | A data language map is missing some configured languages |
| `broken-link` | warning (error with `--strict`) | Internal link, or relative `.md` link between content files, with no matching target (did-you-mean hint when a close match exists) |
| `missing-asset` | warning (error with `--strict`) | Referenced image, script, stylesheet, or media file doesn't exist in the build output |
| `link-to-scheduled` | warning (error with `--strict`) | Link from a published page to a post hidden by scheduling (future `date` or past `expires`); names the go-live date |
| `external-link-broken` | warning (error with `--strict`) | `--external`: link returns 404/410 (or another 4xx) or its host does not exist |
| `external-link-unreachable` | warning (error with `--strict`) | `--external`: timeout, connection failure, 5xx, 401/403/429 or a redirect loop — often temporary or bot blocking |
| `build-failed` | error | Any other build failure |

With `--json`, `data` is `{"diagnostics": [...], "summary": {"errors": n, "warnings": n}}`; on failure the same list is in `error.diagnostics`. With `--external`, `data.external` adds `{found, checked, cached, skipped, broken, unreachable}`.

With `[build] schedule = true`, `seite check` resolves links like `seite build` (scheduled posts are hidden unless `--future`) but still renders scheduled posts, so one that will go live on its own is validated before its date; it also prints how many posts are hidden and when the next goes live (an info note, not a diagnostic).

## seite audit

Free, offline SEO, GEO (AI visibility), accessibility, content and performance audit of the built site. Like `seite check`, it builds into a temporary directory (`dist/` is untouched). Every finding has a stable code (`seo-…`, `geo-…`, `a11y-…`, `content-…`, `perf-…`; `seite explain <code>`), the page, and the source file and line to edit. See [Audit](/docs/audit) for every check, its threshold and the score formula.

```bash
seite audit [options]
```

| Flag | Description |
|------|-------------|
| `--category <c>,...` | Only these categories: `seo`, `geo`, `a11y`, `content`, `perf` (repeatable or comma-separated) |
| `--fail-on <severity>` | Exit 1 when there is a finding of this severity or worse: `error` or `warning` (default: report only, exit 0) |
| `--dist <dir>` | Audit an existing output directory instead of building (findings point at the generated files; `content` checks are skipped) |
| `--min-severity <s>` | List only findings of this severity or worse: `error`, `warning`, `notice` (default; also filters `--json`, the summary counts everything) |
| `--top <n>` | Findings listed per category in the human report (default 10; `--json` has all) |

With `--json`, `data` is `{"summary": {"score", "errors", "warnings", "notices", "pages", "by_category"}, "omitted", "findings": [{"code", "severity", "category", "page_url", "file", "line", "message", "hint", "docs_url"}]}`, sorted deterministically. The MCP tool `seite_audit` returns the same document.

## seite serve

Start a development server with live reload.

```bash
seite serve [options]
```

| Flag | Description |
|------|-------------|
| `--host` | Host to bind to (default: `127.0.0.1`, use `0.0.0.0` for network access) |
| `--port` | Port to serve on (auto-finds an available port if the default is taken; an explicitly passed `--port` that's busy is an error instead) |
| `--build` | Build the site before serving |
| `--open` | Open the site in the default browser after starting |
| `--no-repl` | Don't read commands from stdin; just serve (with live reload) until interrupted |
| `--force-new` | Start a server even when `.seite/dev.json` records one already running here |

The server displays local and network URLs (Vite-style) and injects a small script into every HTML page it serves:

- **Live reload** over Server-Sent Events (`/__seite/events`): the page reloads after a build that changed the output. Browsers without `EventSource`, or when the stream keeps failing, poll `/__livereload` instead.
- **Build-error overlay**: when a rebuild fails (or the page is opened while the last build is broken), the page shows each diagnostic as `file:line:col severity[code] message` with its hint, in a dismissable panel. The next good build clears it.
- **Browser errors**: uncaught errors, unhandled promise rejections and `console.error` calls are sent to the server, printed in its terminal as `browser: <page> <kind>: <message>`, and kept (the last 50, repeats counted) for `seite preview`.

A failed build doesn't stop the server, including the first one: fix the file, save, and the page reloads.

Unless `--no-repl` is passed, an interactive REPL accepts commands:

- `new <collection> "Title"`: create content
- `agent [prompt]`: launch AI agent
- `theme apply <name>`: apply theme and rebuild
- `build`: rebuild the site
- `status`: show server info
- `stop`: stop the server

While it runs, the server records `{pid, port, url, started_at}` in `.seite/dev.json` (the workspace root in a workspace; gitignored) and removes it on `stop`, Ctrl-C, or SIGTERM. `seite context` reports it, and a second `seite serve` in the same directory prints the running server's URL and exits instead of starting another. A recorded server only counts when its pid is alive and its port accepts connections on 127.0.0.1; the URL is rebuilt from the port, never taken from the file, and a symlinked `dev.json` is ignored.

The server also answers a few endpoints for tools. `GET /__seite/status` returns `{pid, port, url, version, build, root, workspace, sites, mcp, last_build: {ok, at, duration_ms, errors, warnings, diagnostics, message}, browser_errors: [{kind, message, source, line, col, page, count}]}`; `POST /__seite/mcp` is the site's [MCP server over HTTP](/docs/mcp-server#over-http-from-the-dev-server). They only answer requests addressed to the server itself (`Host` is a loopback name or an IP address with the server's port), never send CORS headers, and the endpoints that accept data reject other origins and non-JSON bodies.

The REPL prompt is only printed to a real terminal. If stdin isn't a TTY (piped, redirected from `/dev/null`, or a background job) the server keeps serving instead of exiting when stdin closes.

## seite preview

What the running dev server knows, for agents and scripts: its URL, the last build outcome (diagnostics printed like `seite check`), and errors reported by open preview pages.

```bash
seite preview                          # URL, last build, browser errors
seite preview --json                   # the /__seite/status document plus "running": true
seite preview --clear-browser-errors   # print, then forget the browser errors
```

It finds the server through `.seite/dev.json` in the site (or the workspace root), like `seite context`. Without a running server it prints "no dev server running; start it with `seite serve`" and exits 1 (`--json`: `data` is `{"running": false}`). It exits 0 whenever a server answered, even if its last build failed: read `last_build.ok`.

## seite new

Create a new content file with frontmatter.

```bash
seite new <collection> "Title" [options]
```

| Flag | Description |
|------|-------------|
| `--tags` | Comma-separated tags |
| `--draft` | Mark as draft (excluded from builds unless `seite build --drafts`) |
| `--lang` | Language code for translations (e.g., `es`, `fr`) |

`seite new` refuses to overwrite a file that already exists at the target path.

```bash
seite new post "My Post" --tags rust,web
seite new doc "API Guide"
seite new page "About"
seite new post "Mi Post" --lang es    # Spanish translation
seite new changelog "v1.0.0" --tags new,improvement
seite new roadmap "Dark Mode" --tags planned
```

## seite agent

Open a coding agent in the site. The prompt is passed through unchanged; the agent reads `AGENTS.md` and the `/seite` skill and runs with the project's own permissions.

```bash
seite agent [prompt] [--once] [--with <harness>] [--auto-approve]
```

| Flag | Description |
|------|-------------|
| `--once` | Run the prompt once and exit (`claude -p`, `codex exec`, `opencode run`, `cursor-agent -p`, `gemini -p`) |
| `--with` | `claude` (default), `codex`, `opencode`, `cursor`, or `gemini`; `SEITE_AGENT` sets the default |
| `--auto-approve` | With `--once`, pass the harness's own auto-approve flag (`opencode run --auto`, `cursor-agent --force --approve-mcps`, `gemini --approval-mode=yolo`). Only honored from an interactive terminal outside an agent session; `--yes` never implies it |

Without `--once`, the agent starts an interactive session, seeded with the prompt when one is given. Run inside a coding agent's session, `seite agent` (and `seite theme create`) refuses to start a nested agent. If the harness isn't installed, the error names the install command. See [Coding Agents](/docs/agent).

## seite collection

Manage site collections: add presets to an existing site or list current collections.

```bash
seite collection <subcommand>
```

| Subcommand | Description |
|------------|-------------|
| `add <preset>` | Add a collection preset to the current site (updates `seite.toml`, creates content directory) |
| `list` | List all collections in the current site with their configuration |

Available presets: `posts`, `docs`, `pages`, `changelog`, `roadmap`, `trust`.

```bash
seite collection add changelog    # Add changelog collection
seite collection add roadmap      # Add roadmap collection
seite collection list             # Show all configured collections
```

## seite access

Inspect password-protected scopes and securely upload their secrets to Cloudflare Pages.

```bash
seite access <subcommand>
```

| Subcommand | Description |
|------------|-------------|
| `groups` | List password groups, protected paths/subdomains, and Pages projects |
| `set-password [GROUP]` | Prompt for and stage a group's password for production and preview; the group is inferred when only one exists |

`set-password` sends the password to Wrangler over stdin. It also creates a random session-signing secret; neither secret is stored in `seite.toml`, printed, or passed as a process argument. Cloudflare applies the staged secrets to the next deployment, so deploy after setting or rotating a password. That deployment invalidates sessions signed with the previous secret.

```bash
seite access groups
seite access set-password staff
# Run this from the Cloudflare Pages production branch
seite deploy

# The same staged password is available to preview deployments
seite deploy --preview
```

See [Private collections](/docs/configuration#private-collections) for path, whole-domain, subdomain, and protected-asset configuration.

## seite theme

Manage site themes: list, apply, install, export, and generate.

```bash
seite theme <subcommand>
```

| Subcommand | Description |
|------------|-------------|
| `list` | Show the starters, the gallery of example themes, and installed themes |
| `apply <name>` | Apply a bundled or installed theme (`blank` also writes `static/css/site.css` if missing) |
| `new [name] [--force]` | Start your own design: scaffold the `blank` starter (`templates/base.html` + `static/css/site.css`) and print how to design it with impeccable or `/seite theme`. Refuses to replace a customized `base.html` without `--force` (which backs it up first) |
| `create "<description>"` | Design a custom theme: scaffolds `blank` first when the site is on a bundled example; then, with impeccable installed, prints how to run `/impeccable`, otherwise runs your coding agent once with `/seite theme <description>` |
| `install <url>` | Download and install a theme from a URL |
| `export <name>` | Export the current theme as a shareable `.tera` file |

```bash
seite theme list
seite theme apply dark
seite theme new
seite theme create "brutalist with neon green accents"
seite theme install https://example.com/themes/aurora.tera
seite theme install https://example.com/themes/aurora.tera --name my-aurora
seite theme export my-theme --description "My custom dark theme"
```

Bundled themes: the starters `default`, `docs`, `landing`, `blank`, and the gallery `minimal`, `dark`, `brutalist`, `bento`, `terminal`, `magazine`, `academic`. They are examples and starting points; every theme includes seite's [partials](/docs/templates#seite-partials) for the SEO head, search and language switcher. Installed themes are stored in `templates/themes/` and listed alongside bundled themes. See the [Theme Gallery](/docs/theme-gallery) for visual previews.

## seite deploy

Deploy the built site.

```bash
seite deploy [options]
```

| Flag | Description |
|------|-------------|
| `--target` | Override deploy target (`github-pages`, `cloudflare`, `netlify`) |
| `--dry-run` | Preview what would be deployed without deploying |
| `--domain` | Set up a custom domain (prints DNS records, updates config, attaches to platform) |
| `--setup` | Run guided deploy setup |
| `--skip-checks` | Skip pre-flight checks |
| `--base-url` | Override base URL for this deploy |
| `--no-commit` | Skip auto-commit and push (overrides `deploy.auto_commit`) |

```bash
seite deploy                          # Commit, push, build, and deploy
seite deploy --no-commit              # Deploy without auto-commit/push
seite deploy --dry-run                # Preview changes
seite deploy --target netlify         # Override target
seite deploy --target cloudflare --dry-run
seite deploy --domain example.com     # Set up custom domain
seite deploy --setup                  # Guided setup wizard
```

## seite workspace

Manage multi-site workspaces. See the [Workspaces](/docs/workspace) guide for full details.

```bash
seite workspace <subcommand>
```

| Subcommand | Description |
|------------|-------------|
| `init [name]` | Initialize a new workspace in the current directory |
| `list` | List all sites in the workspace |
| `add <name>` | Add a new site to the workspace |
| `status` | Show detailed workspace status |

### workspace add flags

| Flag | Description |
|------|-------------|
| `--path` | Site directory path (default: `sites/<name>`) |
| `--title` | Site title |
| `--collections` | Comma-separated collections (default: `posts,pages`) |

```bash
seite workspace init my-workspace
seite workspace add blog --collections posts,pages --title "Blog"
seite workspace add docs --collections docs --path sites/documentation
seite workspace list
seite workspace status
```

When inside a workspace, `build`, `serve`, and `deploy` operate on all sites by default. Use `--site` to target one:

```bash
seite build --site blog               # Build only the blog
seite serve --site docs               # Serve only the docs
seite deploy --site blog --dry-run    # Preview blog deploy
seite check --site blog               # Check only the blog
```

Workspace builds report each site's unknown `seite.toml` keys like a single-site build (`sites/blog/seite.toml:12:1: warning[config-unknown-key]`; in `--json`, under `data.sites.<name>.diagnostics`). `seite build --strict` builds every site, then fails once with every site's broken links and missing assets; with `--json` they are in `error.diagnostics`, with file paths relative to the workspace root (`sites/blog/content/...`). `seite check` run from the workspace root (or with `--site`) checks the workspace's sites the same way, with workspace-relative paths.

## seite hooks

Turn the coding-agent `seite check` hooks on or off, or show their status. Each agent gets an after-edit hook (`seite check --hook <agent> --post-edit`) and a turn-end hook (`seite check --hook <agent>`); see [Check hooks](/docs/agent#check-hooks).

```bash
seite hooks status [--agents <list>]
seite hooks on [--agents <list>] [--tier stop|post-edit|all]
seite hooks off [--agents <list>] [--tier stop|post-edit|all]
```

`--agents` defaults to the project's agents, `--tier` to `all` (OpenCode's plugin holds both tiers, so it only takes `all`). `on` installs the hooks and records each tier in `.seite/config.json` (`hooks_installed`: `<agent>` for the turn-end tier, `<agent>:post-edit` for the after-edit tier). `off` removes only hook entries whose command is one seite generated (your other hooks stay; an edited OpenCode plugin is kept with a warning) and keeps the record, so `seite upgrade` doesn't add them back. `status` reads the config files and reports installed, missing, or partial per agent; with `--json`, `data.agents` lists `{agent, file, stop, post_edit}`.

## seite skill

Install, list, update, or remove skill packs and individual skills.

```bash
seite skill list
seite skill install <pack|url> [--name <skill>]
seite skill update [<name>]
seite skill remove <name>
```

Known packs:

| Pack | What it adds |
|------|--------------|
| `seomachine` | SEO research, writing, and optimization agents, commands, and skills, downloaded into `.claude/` |
| `impeccable` | The [impeccable](https://github.com/pbakaus/impeccable) design skill (`/impeccable`, `$impeccable` in Codex) |

`impeccable` is installed by its own npm installer, pinned to a version seite has reviewed: seite fetches `impeccable@<version>` with `npm install` into a fresh temporary directory (so a project `.npmrc` or `node_modules` can't substitute it), then runs `impeccable install --providers=<agents> --scope=project -y` in the site root, where `<agents>` are the coding agents in `.seite/config.json` (all five when none are recorded). `seite skill update impeccable` runs `impeccable update --scope=project -y` the same way. Both need Node.js (for `npm`) and ask before running the third-party package; `--yes` (or `SEITE_YES=1`) skips the question. `seite skill list` shows impeccable as installed when seite recorded it or when an `impeccable` skill folder exists in `.claude/skills/` or `.agents/skills/`. `seite skill remove impeccable` deletes those skill folders (only when their `SKILL.md` is impeccable's) and leaves any other files its installer added.

Installed packs are recorded in `.claude/.seite-skill-packs.json`. With `--json`, `skill list` returns `data.bundled`, `data.packs` (`{name, description, installed, source}`), and `data.custom_skills`.

## seite mcp

Start the MCP (Model Context Protocol) server. Communicates over stdio using JSON-RPC.

```bash
seite mcp [--dir /path/to/site]
```

It is meant for MCP hosts without a shell, such as the Claude Desktop chat: add `seite mcp --dir /path/to/site` to the host's MCP config. Coding agents use the `seite` CLI instead; `seite init --mcp` / `seite upgrade --mcp` also declare the server in each agent's project config. The server exposes **resources** (documentation, site config, content, themes) and **tools** (build, check, create content, search, apply theme, lookup docs, plus dev-server runtime tools). While `seite serve` runs, the same server is also served over HTTP at `http://localhost:<port>/__seite/mcp` (loopback only). See the [MCP Server](/docs/mcp-server) guide.

## seite upgrade

Update project configuration files to match the current binary version. When you upgrade the `seite` binary, your existing project may lack new config entries (e.g. new permissions or hooks). This command detects what's outdated and applies additive, non-destructive changes.

```bash
seite upgrade [options]
```

| Flag | Description |
|------|-------------|
| `--force` | Apply all upgrades without confirmation |
| `--check` | Check for needed upgrades without applying (exits with code 1 if outdated) |
| `--agents` | Change the coding agents the project is set up for (`claude,codex,opencode,cursor,gemini` or `all`) |
| `--mcp` | Also add the seite MCP server to the selected agents' configs. Without it, existing MCP config is left untouched |
| `--agent` | After upgrading, build a migration brief and hand it to a coding agent (see below). Cannot be combined with `--check` |
| `--with` | With `--agent`: the agent to start (`claude`, `codex`, `opencode`, `cursor`, `gemini`). Defaults to `SEITE_AGENT`, else `claude` |

```bash
seite upgrade                # Interactive: shows changes, asks for confirmation
seite upgrade --force        # Apply all changes without prompting
seite upgrade --check        # CI mode: exit 1 if upgrades needed, 0 if current
seite upgrade --agents claude,cursor   # Add Cursor files; stop maintaining Codex/OpenCode files
```

### Migration brief (`--agent`)

`seite upgrade --agent` runs the normal upgrade, then builds a markdown brief for a coding agent: what the upgrade changed automatically, the release notes for every version after the one the project recorded (up to the installed `seite`), and instructions to apply any manual migration steps the notes describe, keep the site's customizations, run `seite check` and `seite build` until clean, and report what changed. If the project has no readable recorded version, it is treated as the oldest: all release notes are included (newest kept, capped at about 48 KB), and the brief says so. The brief is also saved to `.seite/upgrade-brief.md` (gitignored). If the project was already current and no release notes apply, nothing is printed or launched.

- **Inside an agent session** (Claude Code, Codex, and the other supported agents): the brief is printed to stdout and nothing is launched; the running agent follows it. With `--json` it is in `data.brief` (`data.agent_delivery` is `printed`, or `none` when there was nothing to migrate).
- **From a terminal**: the agent from `--with` (else `SEITE_AGENT`, else Claude Code) starts interactively with a one-line prompt telling it to read `.seite/upgrade-brief.md` (the brief is never passed on the command line). No auto-approve flag is ever added, not even with `--yes`. If that agent isn't installed, or fails to start, the brief file path and an install hint are printed instead.
- **Without a terminal** (a script or CI), or with `--json`: the brief is printed.

In a workspace, the brief has one section per upgraded site and a single agent is started at the directory you ran the command from.

In a [workspace](/docs/workspace), run `seite upgrade` from the workspace root to upgrade every site at once (one confirmation for all of them), or pass `--site <name>` to upgrade a single site. `--check` fails if any site needs upgrading; `--json` lists each site's changes under `data.sites`.

Upgrade is **additive and non-destructive**:
- Creates or merges `.claude/settings.json` (permissions and hooks) and `.claude/launch.json`, adding new entries and never removing yours. MCP config (`.mcp.json`, `enabledMcpjsonServers`, the other agents' MCP entries) is only added with `--mcp`; with it, a legacy `mcpServers` block in `settings.json` is moved into `.mcp.json`
- Migrates project guidance to `AGENTS.md` while preserving existing instructions
- Keeps `CLAUDE.md` as a compatibility import of `AGENTS.md`
- Adds missing files for the selected coding agents (see [`seite init`](#seite-init)); projects created before agent selection existed are treated as `all`, and the selection is recorded. Existing configs are merged, never replaced. Rules files are only created when missing; the `seite` skill and its `reference/` files are refreshed when the bundled `seite-skill-version` is newer
- Removes the pre-0.21 `theme-builder`, `brand-identity`, and `landing-page` skills and the `design-prompts` rule when they are unmodified (content-hash match against every released version); edited copies stay, with a notice
- Writes `.seite/docs/` (missing pages always; every page after a seite update, dropping pages the new version doesn't ship) and gitignores it
- Adds hooks for agents that never had them, and the after-edit hook to agents whose turn-end hook is still in place (hooks you removed stay removed)
- Refreshes the seite-owned blocks in `AGENTS.md` (per-agent setup table, rules index) to match the selection
- Deselecting an agent never deletes its files — they are left in place and no longer maintained
- Creates `.seite/config.json` if missing: tracks the project's config version and agent selection
- Version-specific steps are gated, and the agent-file checks are idempotent, so running it on a current project is a fast no-op

{{% callout(type="tip") %}}
`seite build` will nudge you with a one-liner when your project config is outdated: *"Run `seite upgrade` for new features."* The build still succeeds. The nudge is informational only.
{{% end %}}

## seite completions

Generate shell completion scripts for tab-completion of commands, flags, and arguments.

```bash
seite completions <shell>
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

```bash
# Bash (add to ~/.bashrc)
seite completions bash >> ~/.bashrc

# Zsh (add to ~/.zshrc)
seite completions zsh >> ~/.zshrc

# Fish
seite completions fish > ~/.config/fish/completions/seite.fish

# PowerShell (add to $PROFILE)
seite completions powershell >> $PROFILE
```

## seite self-update

Update the `seite` binary itself to the latest release (or a specific version).

```bash
seite self-update [options]
```

| Flag | Description |
|------|-------------|
| `--check` | Check for updates without installing |
| `--target-version` | Update to a specific version (e.g., `0.2.0` or `v0.2.0`) |

```bash
seite self-update                          # Update to latest release
seite self-update --check                  # Just check, don't install
seite self-update --target-version 0.2.0   # Pin a specific version
```

The command downloads the appropriate binary for your platform from GitHub Releases, verifies the SHA256 checksum, and replaces the running binary atomically.

{{% callout(type="info") %}}
After updating the binary, run `seite upgrade` in each of your projects to bring their config files up to date.
{{% end %}}

### Automatic update checks

Seite checks for available updates in the background (at most once every 24 hours). When a newer version is available you'll see a one-liner after your command output:

```
ℹ A new version of seite is available: 0.1.8 → 0.2.0 (run `seite self-update`)
```

The check is non-blocking and silently skipped when offline. It never runs with `CI`, `DO_NOT_TRACK`, or `SEITE_NO_UPDATE_CHECK` set, when stdout/stderr aren't both a terminal (scripts, agents, piped output), or under `--json`; that also covers `seite self-update` (which already checks) and `seite mcp` (JSON-RPC over stdio).

## Agent-facing commands

These commands give coding agents (and scripts) everything the [MCP server](/docs/mcp-server) offers, from a shell. Each one runs the same code as the matching MCP tool, so the results are identical. All of them accept `--json` (one `{"ok","command","data"}` document on stdout, `data` being the tool's structured result) and the global `--dir`, `--config` and `--site` flags. They find the site by walking up from the current directory, so they also work from a subdirectory.

| Command | Same as MCP tool | What it does |
|---------|------------------|--------------|
| `seite context` | (none) | Once-per-session orientation: version, site, collections with item and draft counts, active theme, local docs, skill packs, agents and stop hooks, running dev server, and directives |
| `seite docs [query] [--topic <slug>]` | `seite_lookup_docs` | List the embedded docs, read one topic, or search them |
| `seite page <path-or-url> [--html]` | `seite_get_page` | One content file as the build sees it: resolved frontmatter, URL, output file, rendered HTML size (`--html` prints it) |
| `seite search <query> [--collection <name>] [--limit <n>]` | `seite_search` | Keyword search over content, drafts included (limit 1-100, default 20) |
| `seite stats [--collection <name>]` | `seite_content_stats` | Per-collection counts, drafts, items missing a description or tags, future-dated items, parse errors, missing translations |
| `seite templates` | `seite_list_templates` | User templates and the defaults they override, base-template blocks, context variables per template kind, shortcodes, data keys |
| `seite frontmatter set <path> key=value...` | `seite_update_frontmatter` | Add or replace top-level frontmatter keys; the body and untouched keys stay byte-identical |
| `seite frontmatter unset <path> key...` | `seite_update_frontmatter` | Remove top-level frontmatter keys (`title` cannot be removed) |
| `seite explain <code>` | (none) | Explain a diagnostic code from `seite check` or `seite build` |
| `seite preview [--clear-browser-errors]` | `seite_get_errors` | The running dev server's URL, last build outcome and browser errors |

### seite context

```bash
seite context          # plain text
seite context --json   # structured, including a "directives" array of {name, message}
```

The output ends with a `DIRECTIVES` list, one `NAME: message` line each:

| Directive | When |
|-----------|------|
| `UPGRADE_AVAILABLE` | `.seite/config.json` records an older seite version than the binary (or is missing): run `seite upgrade` |
| `NO_STOP_HOOK` | No turn-end `seite check` hook is installed: run `seite check` before finishing |
| `ADDON_SUGGESTION` | The site uses the default or a custom theme and no `impeccable` skill is installed: optionally suggest `seite skill install impeccable` for design work |
| `CONFIG_INVALID` | `seite.toml` exists but does not load |
| `DEV_BUILD_FAILED` | The running dev server's last build failed: run `seite preview` to read the errors |
| `BROWSER_ERRORS` | Pages open on the dev server reported JavaScript errors: run `seite preview` |
| `TIMEZONE_UNSET` | `[build] schedule = true` but no `[site] timezone`: what is live depends on each machine's clock; add `timezone = "<IANA name>"` |

Outside a seite project, `seite context` says so and exits successfully, so an agent can run it unconditionally.

### seite page

The argument is a source path when it exists on disk or ends in `.md` (relative to the current directory, the site root, or `content/`), and a published URL when it starts with `/` or `http(s)://`.

```bash
seite page content/posts/2026-01-02-hello.md
seite page /posts/hello --json
seite page /docs/install --html
```

### seite frontmatter

Values are parsed like YAML scalars: `true`/`false`, numbers and `null` keep their type; `[a, b]` and `{k: v}` are lists and maps; quoted values (`'"42"'`) are strings. Anything else is taken verbatim as a string, so `description=Note: this` sets exactly that text.

```bash
seite frontmatter set content/posts/hello.md draft=false "tags=[rust, web]" weight=2
seite frontmatter unset content/posts/hello.md image
```

### seite explain

```bash
seite explain broken-link
seite explain 'error[frontmatter-parse]'   # as printed by seite check
```

Every code is documented in the [error reference](/docs/errors). With `--json`, each diagnostic from `seite check` and `seite build` carries a `docs_url` linking to its section, and human `seite check` output ends with a pointer to `seite explain` whenever it reports a problem.
