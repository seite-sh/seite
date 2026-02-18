---
title: "Deployment"
description: "Deploy your page site to GitHub Pages, Cloudflare Pages, or Netlify."
---

## Overview

`page` supports three deployment targets out of the box:

- **GitHub Pages** — git-based deployment with auto-generated GitHub Actions
- **Cloudflare Pages** — wrangler-based deployment
- **Netlify** — CLI-based deployment

## Configuration

Set the deploy target in `page.toml`:

```toml
[deploy]
target = "github-pages"
```

## GitHub Pages

The default deployment target. When you run `page init` with `--deploy-target github-pages`, a GitHub Actions workflow is generated at `.github/workflows/deploy.yml` that builds and deploys on every push to `main`.

### Manual deployment

```bash
page build
page deploy
```

This pushes the `dist/` directory to the `gh-pages` branch.

### Dry run

Preview what would be deployed:

```bash
page deploy --dry-run
```

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
page build
page deploy
```

### Dry run

```bash
page deploy --target cloudflare --dry-run
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
page build
page deploy
```

### Dry run

```bash
page deploy --target netlify --dry-run
```

## Custom Domains

Set up a custom domain for any deploy target:

```bash
page deploy --domain example.com
```

This will:
1. Print DNS records to add at your registrar
2. Update `base_url` and `deploy.domain` in `page.toml`
3. For Cloudflare: offer to attach the domain to your Pages project via the API
4. For Netlify: offer to add the domain via `netlify domains:add`
5. For GitHub Pages: auto-generate a `CNAME` file on the next deploy

You can also set the domain directly in `page.toml`:

```toml
[deploy]
target = "cloudflare"
project = "my-site"
domain = "example.com"
```

When `deploy.domain` is set, pre-flight checks will verify the domain is attached to your project. If not, the interactive recovery will offer to attach it automatically.

## Override Target

Override the configured target on the command line:

```bash
page deploy --target netlify     # Deploy to Netlify regardless of page.toml
page deploy --target cloudflare  # Deploy to Cloudflare
```

## Build Output

After `page build`, the `dist/` directory contains everything needed:

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
