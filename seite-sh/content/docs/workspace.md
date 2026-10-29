---
title: "Multi-Site Workspaces"
description: "Manage multiple seite static sites in a single repository with one build command, a unified dev server, and coordinated multi-target deploys."
weight: 12
---

seite's workspace system lets you manage multiple static site generator projects from a single repository with one build command, a unified dev server, and coordinated deploys.

## Overview

A workspace lets you manage multiple `seite` sites from a single directory. Each site has its own `seite.toml`, content, templates, data and static files; the workspace builds, serves, upgrades and deploys them together.

Use cases:
- **Company site + blog + docs** as separate sites in one repo
- **Multi-brand** sites deployed from one repository
- **Monorepo** with a marketing site and product documentation

## Quick Start

```bash
# Initialize a workspace in the current directory
mkdir my-workspace && cd my-workspace
seite workspace init my-workspace

# Add sites
seite workspace add blog --collections posts,pages --title "Blog"
seite workspace add docs --collections docs --title "Documentation"

# Build all sites
seite build

# Serve all sites with unified dev server
seite serve

# Build/serve a specific site
seite build --site blog
seite serve --site docs
```

## Creating a Workspace

```bash
seite workspace init [name]
```

This initializes the **current directory** as the workspace root (it does not create a `[name]` directory; the name is only written into the config) and refuses to run where a `seite-workspace.toml` already exists:

```
./
├── seite-workspace.toml   # Workspace configuration
├── sites/                 # Default directory for sites
├── data/                  # Reserved (not read by builds yet)
├── static/                # Reserved (not read by builds yet)
└── templates/             # Reserved (not read by builds yet)
```

If you omit the name, an interactive prompt asks for it.

## Adding Sites

```bash
seite workspace add <name> [options]
```

| Flag | Description |
|------|-------------|
| `--path` | Directory for the site (default: `sites/<name>`) |
| `--title` | Site title |
| `--collections` | Comma-separated collections (default: `posts,pages`) |
| `--agents` | Coding agents to set the site up for, as `seite init --agents` (default: all) |
| `--mcp` | Also add the seite MCP server to each agent's config |

This creates a full site scaffold inside the workspace and registers it in `seite-workspace.toml`. Like `seite init`, it writes the site's coding-agent files (AGENTS.md, rules, the `/seite` skill, check hooks) in the site directory, so start your agent there:

```bash
seite workspace add blog --collections posts,pages --title "My Blog"
seite workspace add docs --collections docs --title "Documentation" --path sites/documentation
```

## Configuration

The workspace is configured through `seite-workspace.toml` at the workspace root:

```toml
[workspace]
name = "my-workspace"

# Reserved, not implemented yet: builds ignore these
# shared_data = "data"
# shared_static = "static"
# shared_templates = "templates"

[[sites]]
name = "blog"
path = "sites/blog"
# base_url = "https://blog.example.com"   # Override site's base_url
# output_dir = "dist/blog"                # Override output location

[[sites]]
name = "docs"
path = "sites/docs"

# Reserved, not implemented yet: builds ignore this table
# [cross_site]
# unified_sitemap = true
# cross_site_links = true
# unified_search = false
```

### Workspace Fields

| Field | Type | Description |
|-------|------|-------------|
| `workspace.name` | string | Workspace name |
| `workspace.shared_data` | string | Reserved. Must stay inside the workspace, but no build reads it yet |
| `workspace.shared_static` | string | Reserved. Must stay inside the workspace, but no build reads it yet |
| `workspace.shared_templates` | string | Reserved. Must stay inside the workspace, but no build reads it yet |

Shared data, static files and templates, and the `[cross_site]` options (`unified_sitemap`, `cross_site_links`, `unified_search`), are accepted in the file but not implemented: every site builds from its own `data/`, `static/` and `templates/`, with its own sitemap and search index. Copy shared files into each site for now.

### Site Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Site identifier (used in `--site` flag and URL routing) |
| `path` | string | Path to the site directory (relative to workspace root) |
| `base_url` | string | Override the site's `base_url` from its `seite.toml` |
| `output_dir` | string | Override output directory (relative to workspace root) |

Each site must have its own `seite.toml` inside its directory.

## The --site Flag

When inside a workspace, `build`, `serve`, and `deploy` operate on **all sites** by default; `check` and `upgrade` do too when run from the workspace root (inside a site's directory they cover just that site). Use the global `--site` flag to target a specific site:

```bash
seite build --site blog       # Build only the blog
seite deploy --site docs      # Deploy only the docs
seite serve --site blog       # Serve only the blog
```

The `--site` flag is available on all commands when a `seite-workspace.toml` is detected.

## Building

```bash
seite build                   # Build all sites
seite build --site blog       # Build one site
seite build --strict          # Treat broken links as errors (per-site)
```

Each site is built independently with its own config and templates. Build output shows per-site progress:

```
[1/2] Building site 'blog'
✓ 12 posts, 3 pages (0.45s)
[2/2] Building site 'docs'
✓ 8 docs (0.32s)
── Workspace build complete ──
✓ blog: 12 posts, 3 pages | docs: 8 docs
```

Internal link validation runs per-site after each build.

## Development Server

```bash
seite serve                   # Serve all sites
seite serve --site blog       # Serve one site
```

The workspace dev server routes requests by site name:

- `http://localhost:3000/`: workspace index listing all sites
- `http://localhost:3000/blog/`: blog site
- `http://localhost:3000/docs/`: docs site

File watching is per-site: when you edit a blog post, only the blog rebuilds. Live reload, the build-error overlay and browser-error reporting work across all sites, and `seite preview` reports every site's last build. The workspace REPL has only `status` and `stop`, and the workspace server has no [MCP endpoint](/docs/mcp-server#over-http-from-the-dev-server): use `seite serve --site <name>` for that.

The agent-facing commands (`seite page`, `seite search`, `seite stats`, `seite templates`, `seite frontmatter`, `seite og preview`) work on one site: at the workspace root, pass `--site <name>`.

## Deploying

```bash
seite deploy                  # Deploy all sites
seite deploy --site blog      # Deploy one site
seite deploy --dry-run        # Preview all deploys
```

Each site deploys according to its own `seite.toml` deploy config. Sites can use different deploy targets: one on GitHub Pages, another on Cloudflare.

{{% callout(type="tip") %}}
Use `seite deploy --site blog --dry-run` to preview a single site's deploy before pushing to production.
{{% end %}}

## Upgrading

```bash
seite upgrade                 # Upgrade every site (one confirmation)
seite upgrade --site blog     # Upgrade one site
seite upgrade --check         # CI: exit 1 if any site needs upgrading
```

After installing a new `seite` binary, run `seite upgrade` from the workspace root to bring every site's agent files and config up to date. Changes are listed per site before anything is written. Running it inside a site's directory upgrades only that site.

## Checking Status

```bash
seite workspace list          # List all sites with config status
seite workspace status        # Detailed workspace status (config, build state, titles)
```

## Workspace Detection

`seite` automatically detects workspaces by walking up the directory tree looking for `seite-workspace.toml`. This means you can run commands from anywhere inside the workspace:

```bash
cd my-workspace/sites/blog
seite build                   # Detects workspace, builds all sites
seite build --site blog       # Builds just the blog
```

If no `seite-workspace.toml` is found, `seite` operates in standalone mode using the local `seite.toml`.

## Next Steps

- [Configuration](/docs/configuration): site-level `seite.toml` settings that each workspace site uses
- [Deployment](/docs/deployment): deploy targets and options for each site
- [CLI Reference](/docs/cli-reference): complete list of commands and flags
