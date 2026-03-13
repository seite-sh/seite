---
title: "Deploy a Static Site to Cloudflare Pages in One Command"
date: 2026-03-13
description: "Deploy your static site to Cloudflare Pages with one CLI command. Free global CDN, preview deploys, custom domains, and zero dashboard clicking. Step-by-step."
tags:
  - cloudflare
  - deployment
  - static-site-generator
  - devops
draft: false
extra:
  primary_keyword: "deploy static site cloudflare pages"
---

The average Cloudflare Pages tutorial has 14 steps and 8 screenshots. Create a project. Connect GitHub. Select a repository. Configure build settings. Set environment variables. Click deploy. Wait. Check the dashboard. Set up a custom domain. Configure DNS. Wait again.

You just want to deploy a static site to Cloudflare Pages. You should not need a tutorial for this.

Here is what most guides also skip: Cloudflare [deprecated Pages in April 2025](https://developers.cloudflare.com/workers/static-assets/migration-guides/migrate-from-pages/), pushing everyone toward Workers with Static Assets. The Pages product still works, it's in maintenance mode, but most tutorials haven't caught up. So you're following outdated steps for a product that's on life support.

This guide shows you how to deploy a static site to Cloudflare Pages with one command, covers the Pages-to-Workers situation honestly, and explains when Cloudflare is the right choice versus GitHub Pages and Netlify. No screenshots. No dashboard clicking. Just CLI commands that work regardless of what Cloudflare renames the product next.

## Why Deploy to Cloudflare?

Before you deploy anywhere, you should know what you're getting.

Cloudflare operates [330+ edge locations across 120+ countries](https://www.cloudflare.com/network/). Your static site gets served from whichever data center is closest to the visitor. That translates to sub-50ms response times for 95% of internet-connected users globally, according to Cloudflare's own network benchmarks.

Cloudflare Pages free hosting is generous. Unlimited bandwidth, unlimited requests, unlimited sites, 500 builds per month, and 100 custom domains per project. For a static site, you will never pay Cloudflare a dollar.

Compare that to the alternatives:

| | Cloudflare | GitHub Pages | Netlify |
|---|---|---|---|
| **Bandwidth** | Unlimited | 100 GB/month | 100 GB/month |
| **Build minutes** | 500/month | 10 min/build (Actions) | 100 min/month |
| **Custom domains** | 100 per project | 1 per repo | Unlimited |
| **Preview deploys** | Yes (branch-based) | No | Yes |
| **Global CDN** | 330+ cities | GitHub's CDN | CDN included |
| **CLI deploy** | wrangler | git push only | netlify-cli |
| **Free SSL** | Yes | Yes | Yes |
| **Price** | Free | Free | Free (reduced from 300 min in 2025) |

Netlify [reduced their free build minutes from 300 to 100 in 2025](https://www.netlify.com/pricing/), pushing cost-conscious developers toward Cloudflare. GitHub Pages works well for open-source projects but lacks preview deploys and has tighter bandwidth limits.

For production sites that need global performance, custom domains, and preview deploys, Cloudflare is hard to beat on the free tier.

**Want a static site generator with [one-command deploy](/docs/deployment) to Cloudflare, GitHub Pages, and Netlify?** [Get started with seite](/docs/getting-started) and run `seite deploy`.

## The Cloudflare Pages Situation in 2026

This is the part most tutorials skip entirely.

### What Actually Happened

In April 2025, Cloudflare announced that Pages is deprecated in favor of [Workers with Static Assets](https://developers.cloudflare.com/workers/static-assets/). That sounds alarming, but here's what it actually means:

**Pages still works.** Your existing deployments keep running. The `wrangler pages deploy` command still works. Your `*.pages.dev` URLs still resolve. Cloudflare is maintaining the product but not adding new features.

**Workers with Static Assets is the replacement.** Instead of a separate "Pages" product, static site hosting is now part of Cloudflare Workers. The deployment mechanism is similar (wrangler CLI), the CDN is the same (330+ edge locations), and static asset requests are still free.

**The practical difference is small.** For static sites, the migration from Pages to Workers involves changing a config file, not rewriting your site. Cloudflare's official migration documentation walks through the handful of changes needed.

### What This Means for You

If you're deploying a static site today, you have two options:

1. **Use Pages.** It still works. Your deploy workflow is unchanged. When Cloudflare eventually sunsets Pages, the migration is mechanical.
2. **Use Workers with Static Assets.** Start on the new platform. Same CDN, same performance, future-proof.

Either way, the underlying infrastructure is identical. Your site gets served from the same edge network, with the same performance characteristics, at the same price (free).

The smarter approach: use a deploy tool that abstracts the Cloudflare API. When Cloudflare renamed their product, developers who hard-coded dashboard workflows had to rewrite their process. Developers who ran `seite deploy` didn't change a thing.

When Tomas, a backend developer, built a documentation site for his team's internal API, he followed a Cloudflare Pages tutorial from 2023. Twenty minutes of dashboard clicking, GitHub integration setup, and build configuration. It worked. Six months later, he saw the deprecation notice and spent an afternoon researching the migration. His colleague Maya, who'd used seite, pointed at the one line in her `seite.toml` that said `target = "cloudflare"` and shrugged. She'd deploy to Workers when seite updates the target. Same command, same workflow.

## The 14-Step Tutorial (What Most Guides Tell You)

Here's the typical Cloudflare Pages deploy workflow from every other tutorial on the internet:

1. Sign up for a Cloudflare account
2. Navigate to Workers & Pages in the dashboard
3. Click "Create application"
4. Select the "Pages" tab
5. Click "Connect to Git"
6. Authorize Cloudflare with your GitHub account
7. Select your repository
8. Configure the build settings (framework preset, build command, output directory)
9. Set environment variables (if needed)
10. Click "Save and Deploy"
11. Wait for the build to complete
12. Check the deployment URL
13. Go to Custom Domains settings
14. Configure DNS records for your domain

That's the happy path. If something goes wrong, like your build command failing, or your output directory being wrong, or your environment variables missing, you're back in the dashboard hunting through settings.

There's a faster way.

## Deploy in One Command with seite

### Initial Setup (One Time)

Install seite:

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

Install wrangler (Cloudflare's CLI) and authenticate:

```bash
npm install -g wrangler
wrangler login
```

Set your deploy target in [`seite.toml`](/docs/configuration):

```toml
[deploy]
target = "cloudflare"
project = "my-site"
```

That's the setup. You do it once.

### Deploy

```bash
seite deploy
```

One command. Here's what happens under the hood:

1. **Auto-commits** your changes (unless you opt out with `--no-commit`)
2. **Pushes** to your remote repository
3. **Builds** the site (HTML, markdown, sitemap, RSS, [llms.txt](/blog/what-is-llms-txt), search index, robots.txt)
4. **Deploys** to Cloudflare via wrangler
5. **Prints** the live URL

No dashboard. No clicking. No build configuration in the Cloudflare UI. See the full [`seite deploy` reference](/docs/cli-reference) for all available flags.

### What Gets Deployed

When you deploy a static site to Cloudflare Pages with seite, you're not just deploying HTML files. The `dist/` directory reflects your site's [collections](/docs/collections) and contains a complete, [AI-readable](/blog/ai-static-site-generator), SEO-optimized site:

```
dist/
  index.html              # Homepage
  index.md                # Homepage (markdown for AI)
  sitemap.xml             # Search engine sitemap
  feed.xml                # RSS feed
  robots.txt              # Crawler directives (AI-aware)
  llms.txt                # AI discovery summary
  llms-full.txt           # Complete content for AI
  search-index.json       # Client-side search data
  404.html                # Error page
  posts/
    my-post.html          # Blog post (HTML)
    my-post.md            # Blog post (markdown)
  docs/
    getting-started.html  # Docs page
    getting-started.md    # Docs page (markdown)
  static/
    ...                   # Assets
```

Every page ships as HTML and markdown. Every build generates discovery files for search engines and AI models. This is what differentiates deploying with a tool that understands [generative engine optimization](/blog/generative-engine-optimization) from manually uploading files through a dashboard. seite's themes ship pre-compiled into the binary too, so there's no npm install step in your CI pipeline. See [why seite compiles themes into the binary](/blog/compiled-themes-binary) for the full rationale.

### Preview Deploys from Branches

Push from any branch that isn't `main` or `master`, and seite automatically deploys as a preview instead of production. No flags needed.

```bash
git checkout -b redesign
# make changes
seite deploy
# → Deployed preview: https://abc123.my-site.pages.dev
```

Share the preview URL with your team. When the branch merges to main, the next `seite deploy` pushes to production. This is how preview deploys should work: zero configuration, just branch-based routing.

### Custom Domains

Setting up a Cloudflare Pages custom domain takes one CLI command with seite:

```bash
seite deploy --domain example.com
```

This prints the DNS records you need to add at your registrar, then offers to attach the domain to your Cloudflare project via the API. SSL is automatic. No dashboard navigation required.

You can also set the domain permanently in [`seite.toml`](/docs/configuration):

```toml
[deploy]
target = "cloudflare"
project = "my-site"
domain = "example.com"
```

On subsequent deploys, seite verifies the domain is attached and warns you if the DNS records are missing.

### Dry Run

Not sure what will happen? Preview the deploy without pushing anything:

```bash
seite deploy --dry-run
```

This runs the build, shows you what would be deployed, and stops. Always worth running before your first deploy to a new target.

## Deploy with Wrangler Only (No SSG Required)

If you already have a `dist/` directory with your static files and don't use seite, you can deploy directly with wrangler:

```bash
# Install and authenticate
npm install -g wrangler
wrangler login

# Deploy your build output
wrangler pages deploy ./dist --project-name my-site
```

That's it. Wrangler uploads your files to Cloudflare's CDN and returns a live URL. No GitHub integration, no build configuration, no dashboard.

For the Workers path (future-proof), create a `wrangler.toml`:

```toml
name = "my-site"
compatibility_date = "2025-08-23"

[assets]
directory = "./dist"
```

Then deploy with:

```bash
wrangler deploy
```

The key difference: `wrangler pages deploy` uploads to Pages (deprecated but functional). `wrangler deploy` with an `[assets]` block uploads to Workers with Static Assets (the current path). Both serve your files from the same global CDN.

If you're choosing between manual wrangler commands and a tool like seite, consider what else you need. Wrangler handles the upload. seite handles the build, commit, push, upload, preview routing, and [custom domain attachment](/docs/deployment), plus generates the [complete set of output files](/docs/cli-reference) (sitemap, RSS, llms.txt, search index) that make your site discoverable.

## Deploy to GitHub Pages and Netlify Too

Cloudflare isn't the only option. seite supports all three major static hosting platforms with the same one-command workflow. Switch targets by changing one line in [`seite.toml`](/docs/configuration):

```toml
# GitHub Pages
[deploy]
target = "github-pages"

# Netlify
[deploy]
target = "netlify"

# Cloudflare
[deploy]
target = "cloudflare"
project = "my-site"
```

The deploy command is always the same: `seite deploy`. The build output is identical. Only the upload mechanism changes.

**When to use each:**

- **GitHub Pages**: open-source projects, personal sites, anything already on GitHub. Simplest setup with auto-generated CI workflow.
- **Cloudflare**: production sites that need global CDN performance, custom domains, and preview deploys. Best free tier.
- **Netlify**: team workflows, familiar developer experience, easy rollbacks.

When Nina, a startup founder, launched her SaaS landing page, she started with GitHub Pages because the repo was already on GitHub. Two months later, she needed preview deploys for her marketing team to review changes before they went live. She changed `target = "github-pages"` to `target = "cloudflare"`, added a `project = "acme-landing"` line, and ran `seite deploy`. Same command, different platform, five-minute migration.

If you're still evaluating static site generators, see our [Hugo alternative comparison](/blog/hugo-alternative) for SSG-to-SSG comparison or the [WordPress alternative for developers](/blog/wordpress-alternative-for-developers) guide if you're migrating from a CMS. Detailed setup guides for all three deploy targets are in the [deployment documentation](/docs/deployment).

## Common Deploy Issues and Fixes

### "wrangler: command not found"

Install wrangler globally:

```bash
npm install -g wrangler
```

Then authenticate:

```bash
wrangler login
```

### "Project not found"

Your `project` name in `seite.toml` must match a Cloudflare Pages project. If the project doesn't exist yet, `seite deploy` will prompt you to create it. Or create it manually:

```bash
wrangler pages project create my-site
```

### Base URL Still Shows localhost

The most common deploy mistake. If `base_url` in `seite.toml` is still `http://localhost:3000`, your canonical URLs, sitemaps, RSS feeds, and [llms.txt](/blog/what-is-llms-txt) will all point to localhost. Set it to your actual domain before deploying:

```toml
[site]
base_url = "https://example.com"
```

seite's pre-flight checks will catch this and warn you, but fix it in the config rather than relying on the safety net.

### 404 on Clean URLs

seite generates clean URLs by default: `/posts/hello-world` instead of `/posts/hello-world.html`. All three deploy targets (Cloudflare, GitHub Pages, Netlify) handle this correctly out of the box. If you're getting 404s, make sure your `dist/` directory contains `posts/hello-world.html`, not a directory with an `index.html` inside it.

### Build Fails with Missing Template

If `seite build` fails before deploy, you're probably missing a template file. Run `seite theme apply default` to restore the base template, then customize from there. See the [templates documentation](/docs/templates) for the full list of available themes and template variables.

## Frequently Asked Questions

### Is Cloudflare Pages free?

Yes. The free tier includes unlimited bandwidth, unlimited requests, unlimited sites, 500 builds per month, and 100 custom domains per project. Static asset requests incur no charges. You will not pay Cloudflare for hosting a static site.

### Is Cloudflare Pages deprecated?

Yes, partially. Cloudflare deprecated Pages in April 2025 in favor of Workers with Static Assets. Pages continues to work and is maintained, but no new features are being added. New projects should consider using Workers with Static Assets. For static sites, the practical difference is minimal, and migration is straightforward. Tools like seite that abstract the deploy mechanism will handle the transition automatically.

### What is the difference between Cloudflare Pages and Workers?

Cloudflare Pages is (was) a dedicated static site hosting product with GitHub integration, build pipelines, and preview deploys. Cloudflare Workers is a serverless compute platform that now also supports serving static assets. Workers with Static Assets combines server-side code and static file hosting in one product. For pure static sites, the end result is the same: your files get served from Cloudflare's global CDN.

### Cloudflare Pages vs GitHub Pages: which should I use?

GitHub Pages is simpler to set up for projects already on GitHub. Cloudflare Pages offers better performance (330+ edge locations vs GitHub's CDN), preview deploys, and a more generous free tier (unlimited bandwidth vs 100 GB/month). Choose GitHub Pages for open-source simplicity. Choose Cloudflare for production sites that need global performance and custom domains. With seite, you can switch between them by changing one line in your config.

### Can I use a custom domain with Cloudflare Pages for free?

Yes. The free tier supports up to 100 custom domains per project. SSL certificates are provisioned automatically. With seite, run `seite deploy --domain example.com` to set up DNS and attach the domain via the Cloudflare API.

### How many sites can I deploy on Cloudflare Pages?

Unlimited on the free tier. There is no cap on the number of projects or sites you can create. The only limit is 500 builds per month across all projects.

### Cloudflare Pages vs Netlify: which is better for free hosting?

Both offer generous free tiers, but Cloudflare Pages gives you unlimited bandwidth while Netlify caps at 100 GB/month. Netlify reduced their free build minutes from 300 to 100 in 2025, making Cloudflare's 500 builds/month more attractive for active projects. Netlify offers a more polished team collaboration dashboard and easier rollbacks. For static sites with significant traffic, Cloudflare's free tier is more generous. For team workflows where the UI matters, Netlify has the edge.

### How do I migrate from Cloudflare Pages to Workers?

Cloudflare provides an [official migration guide](https://developers.cloudflare.com/workers/static-assets/migration-guides/migrate-from-pages/). For static sites, the migration involves creating a `wrangler.toml` (or `wrangler.jsonc`) with an `assets.directory` pointing to your build output. You then wrangler deploy static site files with `wrangler deploy` instead of `wrangler pages deploy`. Your site's content, URLs, and performance remain identical.

## Deploying Should Not Be the Hard Part

Three things to remember:

1. **One command is enough.** One command to deploy your website to a global CDN. `seite deploy` builds, commits, pushes, and publishes. No dashboard, no CI config, no manual steps.
2. **The platform doesn't matter as much as the workflow.** Whether Cloudflare calls it Pages or Workers, your deploy command stays the same. Use a tool that abstracts the platform so you don't rewrite your workflow every time a hosting provider renames a product.
3. **A deploy is more than HTML.** When you deploy with seite, you're shipping a complete site: HTML for browsers, markdown for AI, [llms.txt](/blog/what-is-llms-txt) for AI search engines, sitemap for Google, RSS for subscribers, and search index for your visitors. One build, every format.

If you want to deploy a static site to Cloudflare Pages (or GitHub Pages, or Netlify) with one command:

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

Build your site. Deploy it. [Ship it in an afternoon.](/docs/getting-started) If you are a startup that needs a full site — landing page, docs, blog, changelog — deployed in one afternoon before a demo or launch, see [static site generator for startups](/blog/static-site-generator-for-startups) for the complete timeline.
