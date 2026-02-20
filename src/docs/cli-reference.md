---
title: "CLI Reference"
description: "Complete reference for all page CLI commands, flags, and options."
weight: 10
---

{{% callout(type="tip") %}}
Run `page <command> --help` for quick inline help on any command.
{{% end %}}

## Overview

`page` has eleven subcommands:

| Command | Description |
|---------|-------------|
| `init`  | Create a new site |
| `build` | Build the site |
| `serve` | Development server with live reload |
| `new`   | Create content files |
| `agent` | AI assistant with site context |
| `theme` | Manage themes |
| `deploy`| Deploy to hosting platforms |
| `workspace` | Manage multi-site workspaces |
| `mcp`   | MCP server for AI tool integration |
| `upgrade` | Update project config to match current binary |
| `self-update` | Update the page binary to the latest release |

### Global Flags

These flags work with any command:

| Flag | Description |
|------|-------------|
| `--site <name>` | Target a specific site in a workspace |
| `--config <path>` | Path to config file |
| `--dir <path>` | Project directory |
| `--verbose` | Enable verbose logging |
| `--json` | Output results as JSON |

## page init

Create a new site directory with scaffolded structure.

```bash
page init <name> [options]
```

| Flag | Description |
|------|-------------|
| `--title` | Site title |
| `--description` | Site description |
| `--deploy-target` | `github-pages`, `cloudflare`, or `netlify` |
| `--collections` | Comma-separated list: `posts,docs,pages` |

If flags are omitted, `page init` prompts interactively.

```bash
# Non-interactive
page init mysite --title "My Blog" --deploy-target github-pages --collections posts,pages

# Interactive
page init mysite
```

## page build

Build the site from `page.toml` in the current directory.

```bash
page build [options]
```

| Flag | Description |
|------|-------------|
| `--drafts` | Include draft content in the build |
| `--strict` | Treat broken internal links as build errors |

The build pipeline runs 12 steps: clean output, load templates, process collections, render pages, generate RSS, sitemap, discovery files, markdown output, search index, copy static files, process images, and post-process HTML. Per-step timing is shown in the output.

After building, `page build` validates all internal links in the generated HTML. Broken links (e.g., `<a href="/posts/nonexistent">`) are reported as warnings by default. Use `--strict` to fail the build when broken links are found — useful in CI pipelines.

## page serve

Start a development server with live reload.

```bash
page serve [options]
```

| Flag | Description |
|------|-------------|
| `--port` | Starting port (default: 3000, auto-increments if taken) |
| `--drafts` | Include drafts |

The server injects a live-reload script that polls for changes. An interactive REPL accepts commands:

- `new <collection> "Title"` — create content
- `agent [prompt]` — launch AI agent
- `theme apply <name>` — apply theme and rebuild
- `build` — rebuild the site
- `status` — show server info
- `stop` — stop the server

## page new

Create a new content file with frontmatter.

```bash
page new <collection> "Title" [options]
```

| Flag | Description |
|------|-------------|
| `--tags` | Comma-separated tags (posts only) |
| `--lang` | Language code for translations (e.g., `es`, `fr`) |

```bash
page new post "My Post" --tags rust,web
page new doc "API Guide"
page new page "About"
page new post "Mi Post" --lang es    # Spanish translation
```

## page agent

Launch an AI assistant with full site context.

```bash
page agent [prompt]
```

Two modes:
- **Interactive**: `page agent` — opens a Claude Code session
- **One-shot**: `page agent "write a blog post about Rust"` — runs and exits

The agent receives your site config, content inventory, template list, and available CLI commands. It can read, write, and edit files. Requires Claude Code: `npm install -g @anthropic-ai/claude-code`.

## page theme

Manage site themes — list, apply, install, export, and generate.

```bash
page theme <subcommand>
```

| Subcommand | Description |
|------------|-------------|
| `list` | Show all available themes (bundled + installed) |
| `apply <name>` | Apply a bundled or installed theme |
| `create "<description>"` | Generate a custom theme with AI |
| `install <url>` | Download and install a theme from a URL |
| `export <name>` | Export the current theme as a shareable `.tera` file |

```bash
page theme list
page theme apply dark
page theme create "brutalist with neon green accents"
page theme install https://example.com/themes/aurora.tera
page theme install https://example.com/themes/aurora.tera --name my-aurora
page theme export my-theme --description "My custom dark theme"
```

Six bundled themes: `default`, `minimal`, `dark`, `docs`, `brutalist`, `bento`. Installed themes are stored in `templates/themes/` and listed alongside bundled themes. See the [Theme Gallery](/docs/theme-gallery) for visual previews.

## page deploy

Deploy the built site.

```bash
page deploy [options]
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
page deploy                          # Commit, push, build, and deploy
page deploy --no-commit              # Deploy without auto-commit/push
page deploy --dry-run                # Preview changes
page deploy --target netlify         # Override target
page deploy --target cloudflare --dry-run
page deploy --domain example.com     # Set up custom domain
page deploy --setup                  # Guided setup wizard
```

## page workspace

Manage multi-site workspaces. See the [Workspaces](/docs/workspace) guide for full details.

```bash
page workspace <subcommand>
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
page workspace init my-workspace
page workspace add blog --collections posts,pages --title "Blog"
page workspace add docs --collections docs --path sites/documentation
page workspace list
page workspace status
```

When inside a workspace, `build`, `serve`, and `deploy` operate on all sites by default. Use `--site` to target one:

```bash
page build --site blog               # Build only the blog
page serve --site docs               # Serve only the docs
page deploy --site blog --dry-run    # Preview blog deploy
```

## page mcp

Start the MCP (Model Context Protocol) server for AI tool integration. Communicates over stdio using JSON-RPC.

```bash
page mcp
```

This command is designed to be spawned automatically by Claude Code (or other MCP clients) as a subprocess. It is configured in `.claude/settings.json` during `page init` and requires no manual invocation.

The server exposes **resources** (documentation, site config, content, themes) and **tools** (build, create content, search, apply theme, lookup docs). See the [MCP Server](/docs/mcp-server) guide for full details.

{{% callout(type="info") %}}
You don't need to run this command manually. Claude Code starts it automatically when you open a page project. Use `page upgrade` to add the MCP configuration to existing projects.
{{% end %}}

## page upgrade

Update project configuration files to match the current binary version. When you upgrade the `page` binary, your existing project may lack new config entries (e.g., MCP server settings). This command detects what's outdated and applies additive, non-destructive changes.

```bash
page upgrade [options]
```

| Flag | Description |
|------|-------------|
| `--force` | Apply all upgrades without confirmation |
| `--check` | Check for needed upgrades without applying (exits with code 1 if outdated) |

```bash
page upgrade                # Interactive: shows changes, asks for confirmation
page upgrade --force        # Apply all changes without prompting
page upgrade --check        # CI mode: exit 1 if upgrades needed, 0 if current
```

Upgrade is **additive and non-destructive**:
- Merges into `.claude/settings.json` — adds new entries, never removes yours
- Appends to `CLAUDE.md` — adds new sections, never modifies existing content
- Creates `.page/config.json` if missing — tracks the project's config version
- Each upgrade step is version-gated, so running it on a current project is a fast no-op

{{% callout(type="tip") %}}
`page build` will nudge you with a one-liner when your project config is outdated: *"Run `page upgrade` for new features."* The build still succeeds — the nudge is informational only.
{{% end %}}

## page self-update

Update the `page` binary itself to the latest release (or a specific version).

```bash
page self-update [options]
```

| Flag | Description |
|------|-------------|
| `--check` | Check for updates without installing |
| `--target-version` | Update to a specific version (e.g., `0.2.0` or `v0.2.0`) |

```bash
page self-update                          # Update to latest release
page self-update --check                  # Just check, don't install
page self-update --target-version 0.2.0   # Pin a specific version
```

The command downloads the appropriate binary for your platform from GitHub Releases, verifies the SHA256 checksum, and replaces the running binary atomically.

{{% callout(type="info") %}}
After updating the binary, run `page upgrade` in each of your projects to bring their config files up to date.
{{% end %}}
