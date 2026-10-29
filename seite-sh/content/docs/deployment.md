---
title: "Deploying Your Static Site"
description: "Deploy your seite static site to GitHub Pages, Cloudflare Pages, or Netlify with one command. Preview deploys, custom domains, and subdomain support built in."
weight: 8
---

Most static site generators leave deployment as an exercise for the reader. seite ships with first-class deploy support for three major platforms: one command handles the build, commit, push, and publish cycle. Preview deploys happen automatically when you push from a branch, and custom domains are wired up interactively.

## Overview

`seite` supports three deployment targets out of the box:

- **GitHub Pages**: git-based deployment with auto-generated GitHub Actions
- **Cloudflare Pages**: wrangler-based deployment
- **Netlify**: CLI-based deployment

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
`seite` runs pre-flight checks before every deploy: verifying the output directory exists, `base_url` isn't localhost, required CLI tools are installed, and platform config is valid. If a check fails, interactive recovery will offer to fix it.
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

The default deployment target. When you run [`seite init`](/docs/getting-started) with `--deploy-target github-pages`, a GitHub Actions workflow is generated at `.github/workflows/deploy.yml` that builds and deploys on every push to `main`.

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

1. Install wrangler: `npm install -g wrangler` (see [Cloudflare Pages docs](https://developers.cloudflare.com/pages/) for details)
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

1. Install Netlify CLI: `npm install -g netlify-cli` (see [Netlify docs](https://docs.netlify.com/) for details)
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

[Collections](/docs/collections) with `subdomain` set are deployed as separate sites. Each subdomain collection gets its own output directory (`dist-subdomains/{name}/`) and is deployed independently after the main site.

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

### Custom domain for a subdomain

`seite deploy --setup` auto-creates the Cloudflare Pages **project** for each subdomain collection, and `seite deploy` pushes to it (the project gets a `*.pages.dev` URL automatically). Attaching the **custom domain** (e.g. `docs.example.com`) to that project is a **one-time manual step** — seite only auto-attaches the *main* site's domain, not a subdomain's. In the Cloudflare dashboard, open the subdomain's Pages project → **Custom domains** → add the host. If the zone is on the same Cloudflare account, Cloudflare creates the DNS record for you.

### Password-protecting paths and subdomains

Cloudflare Pages can enforce seite's built-in password access. Configure `[access]`, mark collections `private = true`, and assign `access_group` values when scopes need different passwords. A main-site collection protects its `url_prefix`; a private subdomain collection protects its entire Pages project.

```toml
[access]
mode = "password"
session_hours = 168

[[collections]]
name = "docs"
private = true
access_group = "staff"
subdomain = "docs"
deploy_project = "my-docs"
```

After the Pages projects exist, upload each password:

```bash
seite access groups
seite access set-password staff
seite deploy
```

`set-password` stages the password and separate signing secrets for both production and preview. Deploy from the Pages project's configured production branch to activate production; projects created by seite use `main`. Use `seite deploy --preview` to activate preview. That deployment invalidates sessions signed with the environment's previous secret.

The generated `_worker.js` and `_routes.json` use Cloudflare secret bindings and signed, secure session cookies. The Worker also answers `Accept: text/markdown` requests (see [Markdown for agents](#markdown-for-agents)). Custom versions of either file conflict with password access and fail the build. Deployment pre-flight rejects password-enabled sites on GitHub Pages or Netlify.

## Markdown for agents

Every build writes a markdown copy of each page next to its HTML (`/posts/hello-world` → `/posts/hello-world.md`, a collection index → `/posts/index.md`, the homepage → `/index.md` when `content/pages/index.md` exists), linked from the page head with `<link rel="alternate" type="text/markdown">`. Each copy starts (right after its frontmatter) with one line pointing agents at the rest of the site:

```markdown
> Index of this site for agents: [llms.txt](https://example.com/llms.txt). Full text: [llms-full.txt](https://example.com/llms-full.txt). All pages: [sitemap.md](https://example.com/sitemap.md).
```

Pages in another language of a multilingual site point at that language's `/<lang>/llms.txt`. `/sitemap.md` lists every page with its markdown URL and description, per collection (and per language): the pages of `sitemap.xml`, without `noindex` pages and drafts. `llms.txt` links it.

### `Accept: text/markdown`

Agents that send `Accept: text/markdown` can also get the markdown at the page's normal URL. A request gets markdown when `text/markdown` has a higher quality than `text/html`, or the same quality and is named while HTML only matches a wildcard (`text/markdown, */*`). Browsers never ask for markdown, and `*/*` alone gets HTML. Negotiated HTML responses carry `Vary: Accept`, and a page without a markdown copy falls through to the normal response.

| Where | Negotiation |
|-------|-------------|
| `seite serve` | Always on (`.md` URLs are served as `text/markdown; charset=utf-8`). |
| Cloudflare Pages with `[access]` | Always on: the generated password Worker already runs on every request. A protected page's markdown is served only after the same password check. |
| Cloudflare Pages without `[access]` | Opt-in: `[deploy] markdown_negotiation = true` generates `_worker.js` (Pages advanced mode) and a `_routes.json` that skips `/static/*`, `/og/*` and `/search/*`. |
| Netlify | Not generated (see below). |
| GitHub Pages | Not possible (no request-time code). Agents use the `.md` URLs, `sitemap.md` and `llms.txt`. |

Cost on Cloudflare: static asset requests are free and unlimited, but with a Worker every page request is a Functions request, billed as a Workers request. The Workers Free plan allows 100,000 requests a day (shared with your other Workers, reset at midnight UTC). Past that, the project's *Settings → Runtime → Fail open / closed* setting decides whether Pages serves the static files (fail open) or an error page. That is why it is off by default; a site with `[access]` already pays this cost.

Cloudflare also does not apply [`_headers`](https://developers.cloudflare.com/pages/configuration/headers/) or [`_redirects`](https://developers.cloudflare.com/pages/configuration/redirects/) rules to requests a Pages Function (including `_worker.js`) serves. With negotiation on, that is every page request outside `/static/`, `/og/` and `/search/`: custom headers from `public/_headers` and custom redirects are dropped there, and `seite build` warns when either file is present. `aliases:` keep working through the HTML redirect page seite writes at each old URL (a meta refresh instead of a 301).

Only plain paths negotiate: a request path with `%` escapes, `//`, a backslash or a `.`/`..` segment always gets the normal response, and the Worker re-checks the exact `.md` path it fetches (for `[access]`, against the same password route). An extensionless path tries `<path>.md`, then `<path>/index.md`.

Netlify: `_redirects` and `netlify.toml` redirect conditions only match `Country`, `Language`, `Role` and `Cookie`, not `Accept`, so negotiation needs an Edge Function. Edge Functions count against your plan's usage on every matching request, and they are deployed only when Netlify builds the site (the `netlify.toml` seite generates) or by `netlify deploy` run from the site root; the generated GitHub Actions workflow uploads `dist/` only. seite does not generate one. If you want it anyway, a minimal version (untested example; HTML responses keep no `Vary` header):

```js
// netlify/edge-functions/markdown.js
export default async (request) => {
  const url = new URL(request.url);
  const accept = request.headers.get("accept") || "";
  const last = url.pathname.split("/").pop();
  if (!/text\/markdown/i.test(accept) || /text\/html/i.test(accept) || last.includes(".")) return;
  const md = url.pathname.endsWith("/") ? url.pathname + "index.md" : url.pathname + ".md";
  const response = await fetch(new URL(md, url));
  if (!response.ok) return;
  return new Response(response.body, {
    headers: { "Content-Type": "text/markdown; charset=utf-8", "Vary": "Accept" },
  });
};

export const config = { path: "/*", excludedPath: ["/static/*", "/og/*", "/search/*"] };
```

## Scheduled publishing

With `[build] schedule = true`, posts with a future `date` are left out of the build until their date, and `expires:` posts drop out after theirs (see [Scheduled publishing](/docs/configuration#scheduled-publishing)). A static site only changes when it is rebuilt, so something has to rebuild it daily. All options below are free.

- **GitHub Pages:** the workflow `seite init` generates (`.github/workflows/deploy.yml`) has a daily trigger, so scheduled posts go live on free GitHub Actions minutes:

  ```yaml
  on:
    push:
      branches: [main]
    workflow_dispatch:
    schedule:
      - cron: '17 5 * * *'   # daily, 05:17 UTC
  ```

  With `schedule = true`, `seite upgrade` adds it to a workflow that is still exactly as seite generated it; if you edited yours, it prints the lines to add instead.

  GitHub disables scheduled workflows in public repositories after 60 days without repository activity (and emails you first). Re-enable it under *Actions → Deploy → Enable workflow* (or `gh workflow enable deploy.yml`), or push any commit before then; manual runs (`workflow_dispatch`) keep working meanwhile.
- **Cloudflare Pages / Netlify via GitHub Actions:** the generated workflow for these targets has the same daily trigger (the build runs in Actions, then deploys with your `CLOUDFLARE_*` / `NETLIFY_*` secrets).
- **Cloudflare Pages / Netlify with `seite deploy` from your machine:** either commit the generated workflow (and its secrets) so GitHub Actions rebuilds daily, or use the host's free **build hook**: Netlify (*Site configuration → Build & deploy → Build hooks*) and Cloudflare Pages Git-connected projects (*Settings → Builds → Deploy hooks*) give you a URL that starts a build when called. Call it daily from any free scheduler, for example a scheduled GitHub Actions workflow:

  ```yaml
  on:
    schedule:
      - cron: '17 5 * * *'
  jobs:
    rebuild:
      runs-on: ubuntu-latest
      steps:
        - run: curl -fsS -X POST "${{ secrets.BUILD_HOOK_URL }}"
  ```

  A build hook needs the host to build the site itself (Netlify reads the generated `netlify.toml`; Cloudflare Pages needs a Git-connected project with `seite build` as its build command).

Pick a cron hour after midnight in your `[site] timezone`, so posts dated today are published that morning.

#### Set every protected project to Fail closed

Every request to a password-enabled Pages project runs the Worker. On the Workers Free plan that uses up a [daily request allowance](https://developers.cloudflare.com/pages/functions/pricing/), and each Pages project has a **Fail open / Fail closed** setting that decides what happens once the allowance is exhausted. With **Fail open**, Cloudflare [serves static assets without running the Worker](https://developers.cloudflare.com/pages/functions/routing/). The protected pages, `.md` alternates, and `/private-assets/` files are static assets, so anyone could load them without a password until the allowance resets. Cloudflare's API defaults new projects to fail open.

seite sets production and preview to **Fail closed** on every protected project (the main project when a main-site collection is private, and each private subdomain's project):

- `seite access set-password` sets it after uploading the secrets, and exits with an error if it can't.
- `seite deploy --setup` sets it after creating the projects, and exits with an error if it can't, because the GitHub Actions workflow it writes uploads with `wrangler` directly and skips seite's checks.
- `seite deploy` runs a **Cloudflare fail closed** pre-flight check. It reads the setting through the Cloudflare API and offers to fix it. If the setting still fails open or can't be verified, the deploy stops unless you confirm interactively or pass `--skip-checks`. `--yes` applies the fix, but it never skips this check.

The API calls use `CLOUDFLARE_API_TOKEN` (or your `wrangler login` session). Set `CLOUDFLARE_ACCOUNT_ID` if your login has access to more than one account, because seite won't guess which account holds the project. If you deploy another way, such as your own `wrangler pages deploy` workflow, check the setting once in the dashboard: **Workers & Pages → your project → Settings → Runtime → Fail open / closed → Fail closed**. Once the allowance runs out, a fail-closed project shows visitors a Cloudflare error page instead of your site. The Workers Paid plan removes the daily limit.

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
├── sitemap.md           # Every page's markdown URL, for agents
├── robots.txt           # Robots + AI crawler directives, Content Signals
├── llms.txt             # LLM summary
├── llms-full.txt        # Full LLM content
├── atom.xml             # Atom feed
├── search-index.json    # Basic search data (titles, descriptions, tags)
├── search/              # Full-text search index ([build] search = "full")
├── og/                  # Generated social cards ([og])
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

- [Configuration](/docs/configuration), all deploy-related settings in `seite.toml`
- [CLI Reference](/docs/cli-reference): complete list of `seite deploy` flags and options
- [Collections](/docs/collections): configure subdomain deploys and collection routing
- [Getting Started](/docs/getting-started): set up a new seite site from scratch
