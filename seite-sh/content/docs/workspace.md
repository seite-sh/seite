---
title: "Workspaces"
description: "Manage multiple seite sites in a single repository with shared resources, unified dev server, and coordinated deploys."
weight: 9
---

## Overview

A workspace lets you manage multiple `seite` sites from a single directory. Each site has its own `seite.toml`, content, and templates, but they share a root and can share data, static files, and templates.

Use cases:
- **Company site + blog + docs** as separate sites in one repo
- **Multi-brand** sites that share templates or data
- **Monorepo** with a marketing site and product documentation

## Quick Start

```bash
# Initialize a workspace
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

This creates:

```
my-workspace/
├── seite-workspace.toml   # Workspace configuration
├── sites/                # Default directory for sites
├── data/                 # Shared data files
├── static/               # Shared static assets
└── templates/            # Shared templates
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

This creates a full site scaffold inside the workspace and registers it in `seite-workspace.toml`:

```bash
seite workspace add blog --collections posts,pages --title "My Blog"
seite workspace add docs --collections docs --title "Documentation" --path sites/documentation
```

## Configuration

The workspace is configured through `seite-workspace.toml` at the workspace root:

```toml
[workspace]
name = "my-workspace"

# Shared resources available to all sites (optional)
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

# Cross-site features (optional)
# [cross_site]
# unified_sitemap = true
# cross_site_links = true
# unified_search = false
```

### Workspace Fields

| Field | Type | Description |
|-------|------|-------------|
| `workspace.name` | string | Workspace name |
| `workspace.shared_data` | string | Path to shared data directory |
| `workspace.shared_static` | string | Path to shared static assets |
| `workspace.shared_templates` | string | Path to shared templates |

### Site Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Site identifier (used in `--site` flag and URL routing) |
| `path` | string | Path to the site directory (relative to workspace root) |
| `base_url` | string | Override the site's `base_url` from its `seite.toml` |
| `output_dir` | string | Override output directory (relative to workspace root) |

Each site must have its own `seite.toml` inside its directory.

## The --site Flag

When inside a workspace, `build`, `serve`, and `deploy` operate on **all sites** by default. Use the global `--site` flag to target a specific site:

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

- `http://localhost:3000/` — workspace index listing all sites
- `http://localhost:3000/blog/` — blog site
- `http://localhost:3000/docs/` — docs site

File watching is per-site — when you edit a blog post, only the blog rebuilds. Live reload works across all sites.

## Deploying

```bash
seite deploy                  # Deploy all sites
seite deploy --site blog      # Deploy one site
seite deploy --dry-run        # Preview all deploys
```

Each site deploys according to its own `seite.toml` deploy config. Sites can use different deploy targets — one on GitHub Pages, another on Cloudflare.

{{% callout(type="tip") %}}
Use `seite deploy --site blog --dry-run` to preview a single site's deploy before pushing to production.
{{% end %}}

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

- [Configuration](/docs/configuration) — site-level `seite.toml` settings that each workspace site uses
- [Deployment](/docs/deployment) — deploy targets and options for each site
- [CLI Reference](/docs/cli-reference) — complete list of commands and flags
