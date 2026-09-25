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

`seite` has eighteen subcommands. Running `seite` with no subcommand shows a context-aware welcome screen with the most useful commands for your situation.

| Command | Description |
|---------|-------------|
| `init` | Create a new site |
| `build` | Build the site |
| `check` | Validate the site without building it |
| `serve` | Development server with live reload |
| `new` | Create content files |
| `agent` | AI assistant with site context |
| `theme` | Manage themes |
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
| `--json` | Print exactly one JSON document on stdout — `{"ok":true,"command":...,"data":...,"warnings":[...]}` or `{"ok":false,"command":...,"error":{"message":...,"chain":[...]}}` — with all human-readable output on stderr. Not supported by `serve`, `agent`, `mcp`, `completions`, or `self-update`, which stream output or take over the terminal |
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
| `--agents` | Coding agents to set up: `claude,codex,opencode,cursor` or `all` (default: all; the interactive picker preselects the agents installed on your machine) |

If flags are omitted, `seite init` prompts interactively. Without a terminal (or with `-y`/`--yes`), it uses defaults for everything except `--deploy-target`, which has none and is required in that case.

```bash
# Non-interactive
seite init mysite --title "My Blog" --deploy-target github-pages --collections posts,pages

# Only set the site up for Claude Code and Codex
seite init mysite --deploy-target github-pages --agents claude,codex

# Interactive
seite init mysite
```

Every site gets an `AGENTS.md` (read by all four agents). `--agents` decides which agent-specific files are generated from the same bundled content:

| Agent | Files |
|-------|-------|
| `claude` (Claude Code) | `CLAUDE.md` (`@AGENTS.md` import), `.mcp.json`, `.claude/settings.json`, `.claude/rules/*.md`, `.claude/skills/*/SKILL.md` |
| `cursor` (Cursor editor + `cursor-agent`) | `.cursor/mcp.json`, `.cursor/cli.json`, `.cursor/rules/*.mdc` (same guides, `globs` frontmatter) |
| `codex` (Codex CLI) | `.codex/config.toml` (`[mcp_servers.seite]`) |
| `opencode` (OpenCode) | `opencode.json` (`mcp.seite` + permission defaults), `.opencode/commands/seite.md` |

Codex, Cursor, and OpenCode also get the bundled skills in `.agents/skills/`. Every agent gets the `seite` workflow skill: `/seite check`, `/seite new post "Title"`, `/seite preview`, `/seite build`, `/seite deploy`, `/seite theme`, `/seite collection` (`$seite …` in Codex; see [AI Agent](/docs/agent)). The selection is stored in `.seite/config.json` so `seite upgrade` keeps the same set of files current.

## seite build

Build the site from `seite.toml` in the current directory.

```bash
seite build [options]
```

| Flag | Description |
|------|-------------|
| `--drafts` | Include draft content in the build |
| `--strict` | Treat broken internal links and missing assets as build errors |

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

Exits 0 when there are no errors (in `--strict` mode: no diagnostics at all) and 1 otherwise. Each diagnostic has a stable `code` that agents and CI can match on:

| Code | Severity | Meaning |
|------|----------|---------|
| `config-invalid` | error | `seite.toml` does not parse or has a wrong value type |
| `config-unknown-key` | warning | Key the config schema does not know (typo) |
| `frontmatter-missing` | error | Content file has no `---` frontmatter block |
| `frontmatter-parse` | error | Frontmatter YAML is invalid (line points into the file) |
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
| `build-failed` | error | Any other build failure |

With `--json`, `data` is `{"diagnostics": [...], "summary": {"errors": n, "warnings": n}}`; on failure the same list is in `error.diagnostics`.

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

The server displays local and network URLs (Vite-style) and injects a live-reload script that polls for changes. Unless `--no-repl` is passed, an interactive REPL accepts commands:

- `new <collection> "Title"`: create content
- `agent [prompt]`: launch AI agent
- `theme apply <name>`: apply theme and rebuild
- `build`: rebuild the site
- `status`: show server info
- `stop`: stop the server

The REPL prompt is only printed to a real terminal. If stdin isn't a TTY (piped, redirected from `/dev/null`, or a background job) the server keeps serving instead of exiting when stdin closes.

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

Launch an AI assistant with full site context.

```bash
seite agent [prompt]
```

Two modes:
- **Interactive**: `seite agent`: opens a Claude Code session
- **One-shot**: `seite agent "write a blog post about Rust"`: runs and exits

The agent receives your site config, content inventory, template list, and available CLI commands. It can read, write, and edit files. Requires Claude Code: `npm install -g @anthropic-ai/claude-code`.

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
| `list` | Show all available themes (bundled + installed) |
| `apply <name>` | Apply a bundled or installed theme |
| `create "<description>"` | Generate a custom theme with AI |
| `install <url>` | Download and install a theme from a URL |
| `export <name>` | Export the current theme as a shareable `.tera` file |

```bash
seite theme list
seite theme apply dark
seite theme create "brutalist with neon green accents"
seite theme install https://example.com/themes/aurora.tera
seite theme install https://example.com/themes/aurora.tera --name my-aurora
seite theme export my-theme --description "My custom dark theme"
```

10 bundled themes: `default`, `minimal`, `dark`, `docs`, `brutalist`, `bento`, `landing`, `terminal`, `magazine`, `academic`. Installed themes are stored in `templates/themes/` and listed alongside bundled themes. See the [Theme Gallery](/docs/theme-gallery) for visual previews.

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
```

## seite mcp

Start the MCP (Model Context Protocol) server for AI tool integration. Communicates over stdio using JSON-RPC.

```bash
seite mcp
```

This command is designed to be spawned automatically by your coding agent as a subprocess. `seite init` declares it in each selected agent's project config — `.mcp.json` (Claude Code, pre-approved in `.claude/settings.json`), `.cursor/mcp.json` (Cursor), `.codex/config.toml` (Codex), `opencode.json` (OpenCode) — so it requires no manual invocation.

The server exposes **resources** (documentation, site config, content, themes) and **tools** (build, create content, search, apply theme, lookup docs). See the [MCP Server](/docs/mcp-server) guide for full details.

{{% callout(type="info") %}}
You don't need to run this command manually. Agents start it when you open the project, after a one-time approval: Claude Code may ask the first time (`/mcp`), Codex loads project config only once you trust the project, Cursor needs `cursor-agent mcp enable seite` (or approval in its MCP settings), and OpenCode starts it automatically. Use `seite upgrade` to add the configuration to existing projects — it also migrates any older `mcpServers` block out of `.claude/settings.json`, which Claude Code no longer reads.
{{% end %}}

## seite upgrade

Update project configuration files to match the current binary version. When you upgrade the `seite` binary, your existing project may lack new config entries (e.g., MCP server settings). This command detects what's outdated and applies additive, non-destructive changes.

```bash
seite upgrade [options]
```

| Flag | Description |
|------|-------------|
| `--force` | Apply all upgrades without confirmation |
| `--check` | Check for needed upgrades without applying (exits with code 1 if outdated) |
| `--agents` | Change the coding agents the project is set up for (`claude,codex,opencode,cursor` or `all`) |

```bash
seite upgrade                # Interactive: shows changes, asks for confirmation
seite upgrade --force        # Apply all changes without prompting
seite upgrade --check        # CI mode: exit 1 if upgrades needed, 0 if current
seite upgrade --agents claude,cursor   # Add Cursor files; stop maintaining Codex/OpenCode files
```

Upgrade is **additive and non-destructive**:
- Creates or merges `.mcp.json` (the seite MCP server declaration) and `.claude/settings.json` (permissions + `enabledMcpjsonServers`), adding new entries and never removing yours; any legacy `mcpServers` block in `settings.json` is moved into `.mcp.json`, since Claude Code only reads project MCP servers from there
- Migrates project guidance to `AGENTS.md` while preserving existing instructions
- Keeps `CLAUDE.md` as a compatibility import of `AGENTS.md`
- Adds missing files for the selected coding agents (see [`seite init`](#seite-init)); projects created before agent selection existed are treated as `all`, and the selection is recorded. Existing configs are merged, never replaced: `.cursor/mcp.json` keeps your other servers, `opencode.json` gets `mcp.seite` only if missing and the permission defaults only if it has no `permission` key, and `.codex/config.toml` gets `[mcp_servers.seite]` appended with your comments and tables untouched. Rules files are only created when missing; skills are refreshed when the bundled `seite-skill-version` is newer
- Refreshes the seite-owned blocks in `AGENTS.md` (per-agent MCP table, rules index) to match the selection
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
