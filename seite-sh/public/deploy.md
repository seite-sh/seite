---
name: seite-deploy
version: 0.21.0
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

This builds the site, commits it as the `gh-pages` branch, and force-pushes it. Set the Pages
source to "Deploy from a branch: gh-pages" for this. Sites from `seite init` also get
`.github/workflows/deploy.yml`, which builds and publishes on every push to `main` with
`actions/deploy-pages` (Pages source: "GitHub Actions"); use one method or the other.
GitHub Pages has no preview deploys: `seite deploy` refuses `--preview` and non-main branches
for this target before it commits or pushes anything. Cloudflare Pages and Netlify deploy non-main branches as previews.

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

Requires `wrangler` CLI to be installed and authenticated (`wrangler login`). The generated
GitHub Actions workflow deploys to the configured (or detected) project name.

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

Requires the `netlify` CLI, installed and logged in (`netlify login`). seite writes `netlify.toml`
for Netlify's own builds; no GitHub Actions workflow is generated for this target.

---

## Common Options

```bash
seite deploy --dry-run    # print the plan (branch, production or preview, commit + push) without deploying
```

When `auto_commit = true` (default), seite commits and pushes changes before deploying. The
branch and production/preview decision is made before that commit, so `--dry-run` shows exactly
what a real run would do.

---

## Subdomain Deploys

Collections can deploy to subdomains:

```toml
[[collections]]
name = "docs"
# ...other collection fields as before...
subdomain = "docs"                    # deploys to docs.example.com
deploy_project = "my-site-docs"       # Cloudflare/Netlify project name
```

Run `seite deploy --setup` to auto-create projects for subdomain collections.

Note: GitHub Pages does not support subdomain deploys. Use Cloudflare Pages or Netlify.
