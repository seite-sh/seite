---
name: seite-deploy
version: 1.0.0
description: Deploy guide for seite sites. Covers GitHub Pages, Cloudflare Pages, and Netlify.
---

# Seite Deploy Guide

Seite supports three deploy targets. Configure in `seite.toml`, then run `seite deploy`.

---

## GitHub Pages

```toml
# seite.toml
[deploy]
target = "github-pages"
auto_commit = true
```

```bash
seite deploy
```

This builds the site, commits to the `gh-pages` branch, and pushes. GitHub Pages serves from
that branch automatically. Non-main branches create preview deploys.

---

## Cloudflare Pages

```toml
# seite.toml
[deploy]
target = "cloudflare"
project = "my-project"       # Cloudflare Pages project name
domain = "example.com"       # optional custom domain
auto_commit = true
```

```bash
seite deploy --setup   # first time: creates Cloudflare project
seite deploy           # subsequent deploys
```

Requires `wrangler` CLI to be installed and authenticated (`wrangler login`).

---

## Netlify

```toml
# seite.toml
[deploy]
target = "netlify"
auto_commit = true
```

```bash
seite deploy
```

---

## Common Options

```bash
seite deploy --dry-run    # preview what would happen without deploying
```

When `auto_commit = true` (default), seite commits and pushes changes before deploying.

---

## Subdomain Deploys

Collections can deploy to subdomains:

```toml
[[collections]]
name = "docs"
subdomain = "docs"                    # deploys to docs.example.com
deploy_project = "my-site-docs"       # Cloudflare/Netlify project name
```

Run `seite deploy --setup` to auto-create projects for subdomain collections.

Note: GitHub Pages does not support subdomain deploys. Use Cloudflare Pages or Netlify.
