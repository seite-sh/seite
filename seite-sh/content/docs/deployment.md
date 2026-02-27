---
title: "Deployment"
description: "Deploy your seite site to GitHub Pages, Cloudflare Pages, or Netlify."
weight: 8
---

## Overview

`seite` supports three deployment targets out of the box:

- **GitHub Pages** — git-based deployment with auto-generated GitHub Actions
- **Cloudflare Pages** — wrangler-based deployment
- **Netlify** — CLI-based deployment

## Which Target Should I Choose?

| Target | Best for | Highlights |
|--------|----------|------------|
| **GitHub Pages** | Open-source projects | Simplest setup, free, auto-generated CI workflow |
| **Cloudflare Pages** | Production sites | Fastest global CDN, custom domain API, preview deploys |
| **Netlify** | Team workflows | Familiar DX, draft deploys, easy rollbacks |

All three are free for static sites. You can switch targets anytime by changing one line in `seite.toml`.

## Configuration

Set the deploy target in `seite.toml`:

```toml
[deploy]
target = "github-pages"
```

{{% callout(type="warning") %}}
Make sure `base_url` in `seite.toml` is set to your real domain before deploying. The default `localhost` URL will produce incorrect canonical links, sitemaps, and RSS feeds. Use `--base-url` to override at deploy time without modifying the config file.
{{% end %}}

{{% callout(type="info") %}}
`seite` runs pre-flight checks before every deploy — verifying the output directory exists, `base_url` isn't localhost, required CLI tools are installed, and platform config is valid. If a check fails, interactive recovery will offer to fix it.
{{% end %}}

## Auto-Commit and Push

By default, `seite deploy` automatically commits all changes and pushes to the remote before building and deploying. This makes deploy a true one-step workflow.

**Branch-based preview:** when you deploy from a branch other than `main` or `master`, `seite` automatically deploys as a preview instead of production. No need to pass `--preview` manually.

To disable auto-commit for a project, set `auto_commit = false` in `seite.toml`:

```toml
[deploy]
target = "github-pages"
auto_commit = false
```

Or skip it for a single deploy with `--no-commit`:

```bash
seite deploy --no-commit
```

## GitHub Pages

The default deployment target. When you run `seite init` with `--deploy-target github-pages`, a GitHub Actions workflow is generated at `.github/workflows/deploy.yml` that builds and deploys on every push to `main`.

### Manual deployment

```bash
seite build
seite deploy
```

This pushes the `dist/` directory to the `gh-pages` branch.

### Dry run

Preview what would be deployed:

```bash
seite deploy --dry-run
```

{{% callout(type="tip") %}}
Always run `--dry-run` before your first deploy to verify everything looks right. It shows exactly what would be pushed without making any changes.
{{% end %}}

### Custom repository

```toml
[deploy]
target = "github-pages"
repo = "https://github.com/user/repo.git"
```

## Cloudflare Pages

### Setup

1. Install wrangler: `npm install -g wrangler`
2. Authenticate: `wrangler login`
3. Configure:

```toml
[deploy]
target = "cloudflare"
project = "my-site"
```

The project name is auto-detected from `wrangler.toml` if present.

### Deploy

```bash
seite build
seite deploy
```

### Dry run

```bash
seite deploy --target cloudflare --dry-run
```

## Netlify

### Setup

1. Install Netlify CLI: `npm install -g netlify-cli`
2. Authenticate: `netlify login`
3. Configure:

```toml
[deploy]
target = "netlify"
```

### Deploy

```bash
seite build
seite deploy
```

### Dry run

```bash
seite deploy --target netlify --dry-run
```

## Custom Domains

Set up a custom domain for any deploy target:

```bash
seite deploy --domain example.com
```

This will:
1. Print DNS records to add at your registrar
2. Update `base_url` and `deploy.domain` in `seite.toml`
3. For Cloudflare: offer to attach the domain to your Pages project via the API
4. For Netlify: offer to add the domain via `netlify domains:add`
5. For GitHub Pages: auto-generate a `CNAME` file on the next deploy

You can also set the domain directly in `seite.toml`:

```toml
[deploy]
target = "cloudflare"
project = "my-site"
domain = "example.com"
```

When `deploy.domain` is set, pre-flight checks will verify the domain is attached to your project. If not, the interactive recovery will offer to attach it automatically.

## Subdomain Deploys

Collections with `subdomain` set are deployed as separate sites. Each subdomain collection gets its own output directory (`dist-subdomains/{name}/`) and is deployed independently after the main site.

```toml
[[collections]]
name = "docs"
subdomain = "docs"           # → docs.example.com
subdomain_base_url = "https://docs.example.com"  # optional: explicit URL override
deploy_project = "my-docs"   # Cloudflare/Netlify project for this subdomain
```

`subdomain_base_url` overrides the auto-derived subdomain URL. This is useful when `base_url` contains `www` (e.g., `https://www.example.com` would otherwise produce `docs.www.example.com`).

`deploy_project` specifies which Cloudflare Pages or Netlify project to deploy the subdomain to. If omitted, the global `deploy.project` is used as a fallback.

`seite deploy --dry-run` shows the full subdomain deploy plan. In a workspace, cross-site domain conflicts are detected and warned about at deploy time.

{{% callout(type="warning") %}}
GitHub Pages does not support per-collection subdomains. Use Cloudflare Pages or Netlify for subdomain deploys.
{{% end %}}

## Override Target

Override the configured target on the command line:

```bash
seite deploy --target netlify     # Deploy to Netlify regardless of seite.toml
seite deploy --target cloudflare  # Deploy to Cloudflare
```

## Build Output

After `seite build`, the `dist/` directory contains everything needed:

```
dist/
├── index.html           # Homepage
├── index.md             # Homepage markdown
├── feed.xml             # RSS feed
├── sitemap.xml          # XML sitemap
├── robots.txt           # Robots directives
├── llms.txt             # LLM summary
├── llms-full.txt        # Full LLM content
├── search-index.json    # Client-side search data
├── 404.html             # Error page
├── posts/
│   ├── hello-world.html
│   └── hello-world.md
├── docs/
│   ├── getting-started.html
│   └── getting-started.md
└── static/
    └── ...
```

All three platforms serve `404.html` automatically for missing routes.

## Next Steps

- [Configuration](/docs/configuration) — all deploy-related settings in `seite.toml`
- [CLI Reference](/docs/cli-reference) — complete list of `seite deploy` flags
