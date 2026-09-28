---
title: "Password Protect Cloudflare Pages Without Writing a Worker"
date: 2026-09-28
description: "Password protect Cloudflare Pages paths or whole subdomains from seite.toml. Separate password groups, signed sessions and no Worker code or Zero Trust setup."
tags:
  - cloudflare
  - deployment
  - security
  - static-site-generator
extra:
  primary_keyword: "password protect cloudflare pages"
---

Cloudflare Pages has no password toggle. Search for how to password protect Cloudflare Pages and you get two answers: write your own middleware in Pages Functions, or set up Cloudflare Access in the Zero Trust dashboard. Both work. Neither is "add a password to `/investors` and move on."

[seite v0.18](/changelog/v0-18-0) made it a config change. Mark a collection `private = true`, add an `[access]` section to `seite.toml`, run `seite access set-password`, deploy. seite generates the Cloudflare Worker, keeps the gated pages out of your sitemap and llms.txt, and stages the password as a Cloudflare secret so it never touches your repo.

This post covers the three ways to gate a Pages site and when each one fits. It explains the difference between *private* and *password-protected* (people mix them up constantly) and walks through the full setup: one path, several passwords, then a gated trust center on its own subdomain. It ends with the limits, including the cases where Cloudflare Access is the better tool.

## Can You Password Protect Cloudflare Pages? Three Options

Not out of the box. Cloudflare Pages has no standard shared-password setting, and the top-ranking guides say so up front. Here's what people use instead.

**1. Pages Functions middleware.** The most-linked answer is [Charca's cloudflare-pages-auth](https://github.com/Charca/cloudflare-pages-auth): copy a `functions/` directory into your project, set a `CFP_PASSWORD` variable in the dashboard, get a login form and a cookie. It's a good template.

It also gives you one password for the whole project, and you now own that auth code. Other guides use HTTP Basic Auth with the credentials hardcoded in the Worker source, which means the password lives in git.

**2. Cloudflare Access.** Zero Trust puts an identity check in front of your hostname: one-time PIN by email, Google, GitHub, Okta. It's the right tool for real user identity, and the free plan has a [50-user limit](https://www.cloudflare.com/en-au/products/cloudflare-access).

It isn't a shared password, though. Every visitor needs an allowed email or identity, and you configure it per hostname in a separate dashboard.

**3. seite's built-in password access.** Config in `seite.toml`, a generated Worker, one password per *group*. Different paths or subdomains can use different passwords.

| | Functions middleware | Cloudflare Access | seite `[access]` |
|---|---|---|---|
| Setup | Copy code, set env var | Zero Trust dashboard policies | `seite.toml` + one command |
| Credential | One shared password | Per-user identity (email, SSO) | Shared password per group |
| Multiple passwords | Write it yourself | N/A (policies per user/group) | Built in (`access_group`) |
| Protect a subdomain | Separate copy per project | Yes, per hostname | Yes, `private` + `subdomain` |
| Keeps pages out of sitemap/feeds | No | No | Yes |
| Audit who visited | No | Yes | No |

That second-to-last row matters more than it looks. Middleware blocks the page, but your static site generator still lists it in `sitemap.xml`, RSS and llms.txt. The title and URL of your "Q3 investor update" end up in public files.

## Private vs Password-Protected: Two Different Switches

seite has two settings that sound alike and do different jobs.

**`private = true` hides a [collection](/docs/collections).** Every page still builds, including the collection's hub page. What changes is discovery: the pages are excluded from the homepage listing, `sitemap.xml`, `llms.txt`, `llms-full.txt`, `search-index.json`, RSS/Atom feeds and tag pages, and every page gets `<meta name="robots" content="noindex, nofollow">`. The build log tells you how many pages it held back:

```
ℹ 1 private pages excluded from discovery
```

**`[access]` locks it.** Add an `[access]` section and every `private = true` collection sits behind a password on Cloudflare Pages.

| Setting | Hidden from sitemap, llms.txt, search, feeds | `noindex, nofollow` | Requires a password |
|---|---|---|---|
| `private = true` alone | Yes | Yes | **No** |
| `private = true` + `[access]` | Yes | Yes | Yes (Cloudflare Pages) |

{{% callout(type="warning") %}}
`private = true` without `[access]` is not access control. The HTML is still publicly fetchable by anyone who has the URL. It's the right setting when something else does the gating (Cloudflare Access, HTTP auth at another layer), and the wrong one if you think you've locked the door.
{{% end %}}

The split is deliberate. v0.15 shipped `private` as discovery-only for sites that already sit behind Cloudflare Access, and v0.18 added `[access]` on top without changing what existing `private` configs do. The full field reference is in the [private collections configuration](/docs/configuration#private-collections).

## How to Password Protect Cloudflare Pages with seite

This assumes your site already deploys to Cloudflare Pages with `seite deploy`. If it doesn't, start with [deploying a static site to Cloudflare Pages](/blog/deploy-static-site-cloudflare-pages) and come back. You'll need Wrangler installed and logged in, since seite uses it to upload secrets.

The example: a startup site with a public blog and an `/investors` section for board updates.

### 1. Mark the Collection Private

```toml
[deploy]
target = "cloudflare"
project = "acme-site"

[[collections]]
name = "investors"
label = "Investors"
directory = "investors"
url_prefix = "/investors"
default_template = "page.html"
private = true
```

Markdown goes in `content/investors/` like any other collection.

### 2. Enable Password Access

```toml
[access]
mode = "password"
session_hours = 168
```

`mode = "password"` is currently the only mode. `session_hours` controls how long a browser stays logged in: the default is 168 (one week), and any value from 1 to 8760 is accepted.

Each private collection gets a password group. It defaults to the collection's `name`, so this one is `investors`.

### 3. Check What's Protected

```bash
seite access groups
```

```
GROUP                SCOPES                         CLOUDFLARE PROJECTS
----------------------------------------------------------------------------
investors            path /investors                acme-site
```

This is the map the Worker will enforce. `/investors`, everything under it and the `.md` alternates of those pages all require the `investors` password. If a group shows `(project not configured)`, set `deploy.project` (or `deploy_project` for a subdomain) first.

### 4. Set the Password

```bash
seite access set-password investors
```

```
Password for access group 'investors': 
Confirm password: 
✓ Stored password for 'investors' on Cloudflare Pages project 'acme-site' (production and preview)
✓ Cloudflare Pages project 'acme-site' fails closed (protected files stay behind the password when the Workers request limit is reached)
```

The password is prompted twice and piped to `wrangler pages secret put` over stdin. It is never read from a flag, written to `seite.toml` or printed.

seite also generates a random session-signing secret per group. Both are staged for **production and preview**, so staging deploys are gated too. If only one group exists, you can drop the group name.

### 5. Deploy

```bash
seite deploy
```

Staged secrets apply to the *next* deployment, so the password isn't live until you deploy. Deploy from the Pages project's production branch (`main` for projects seite created) and run `seite deploy --preview` to activate it on preview. Visitors to `/investors` now get a login form. The rest of the site stays public.

`set-password` also switches the Pages project to **Fail closed**, and `seite deploy` checks that setting before it uploads (since v0.20.1). Every request on a password-enabled project runs the Worker. In *Fail open* mode, which Cloudflare's API uses by default, Cloudflare serves static files without the Worker once your Workers request limit is reached. More on that in the limits below.

The [password-protecting paths and subdomains](/docs/deployment#password-protecting-paths-and-subdomains) section of the deployment guide has the same flow in reference form.

## One Site, Several Passwords

Most DIY guides stop at one password. Real sites need more. Investors shouldn't share a password with customers, and a partner portal shouldn't unlock your internal docs.

`access_group` handles that. Collections with the same group share a password and a login session. Collections with different groups get independent ones:

```toml
[[collections]]
name = "investors"
label = "Investors"
directory = "investors"
url_prefix = "/investors"
default_template = "page.html"
private = true
access_group = "board"

[[collections]]
name = "handbook"
label = "Handbook"
directory = "handbook"
url_prefix = "/handbook"
default_template = "doc.html"
private = true
access_group = "staff"

[[collections]]
name = "runbooks"
label = "Runbooks"
directory = "runbooks"
url_prefix = "/runbooks"
default_template = "doc.html"
private = true
access_group = "staff"
```

Board members get one password. Staff get another that opens both `/handbook` and `/runbooks`. Run `seite access set-password` once per group.

A few rules keep this predictable:

- **An empty `url_prefix` protects the whole domain.** Use it for a fully private site.
- **When scopes overlap, the most-specific path wins.** A private site at `/` with a separately gated `/board` uses the `board` password for `/board`.
- **Translations are covered.** If you have `[languages.es]`, `/es/investors` is protected with the same group.
- **Conflicts fail the build.** Two collections that assign the same path to different groups are a config error, not a silent override.
- **Group names** can use ASCII letters, numbers, `_` and `-`.

Files need gating too. Anything under `static/` is public. With `[access]` enabled, put protected downloads (the pitch deck PDF, say) in `static/private/<group>/`. They're served from `/private-assets/<group>/` behind that group's password.

## Password-Protect a Subdomain: A Gated Trust Center

Some content belongs on its own hostname. The common case is a security trust center at `trust.example.com` that customers under NDA can read but the public can't.

seite's trust collection preset already deploys to a subdomain. Add `private` and a group:

```toml
[[collections]]
name = "trust"
label = "Trust Center"
directory = "trust"
url_prefix = "/trust"
nested = true
default_template = "trust-item.html"
private = true
access_group = "customers"
subdomain = "trust"
subdomain_base_url = "https://trust.example.com"
deploy_project = "acme-trust"
```

A private collection with `subdomain` protects **the entire subdomain output**, not a path inside it. `seite access groups` shows it that way:

```
GROUP                SCOPES                         CLOUDFLARE PROJECTS
----------------------------------------------------------------------------
customers            subdomain trust (entire site)  acme-trust
investors            path /investors                acme-site
```

The order matters: `seite deploy --setup` creates the `acme-trust` Pages project, then `seite access set-password customers` uploads its secrets, then `seite deploy` activates it. Attaching `trust.example.com` to that project is a one-time step in the Cloudflare dashboard (Custom domains), because seite only auto-attaches the main site's domain. The [trust center documentation](/docs/trust-center) covers the certifications, subprocessors and FAQ data that fill the hub.

The same pattern works for `docs.example.com` as internal documentation, or a `partners.` subdomain.

## What Happens at the Edge

With `[access]` on, `seite build` writes two files into the output:

- **`_worker.js`**, an advanced-mode Pages Worker with your route table baked in.
- **`_routes.json`**, which sends every request (`/*`) through that Worker.

On a protected path without a valid session, the Worker serves a small login form (not a Basic Auth browser prompt). A correct password sets a `__Host-` session cookie marked `Secure`, `HttpOnly` and `SameSite=Lax`, signed with HMAC-SHA256 using the group's secret and valid for `session_hours`. Password checks use a constant-time comparison, and the login page is served with `Cache-Control: no-store` and `X-Robots-Tag: noindex, nofollow`.

It fails closed. If a group's secrets are missing (say you deployed before running `set-password`), protected paths return a 503 "Access unavailable" page instead of the content. The Worker decodes each path before matching it, so encoded separators can't slip past a protected route, and it rejects `..` segments outright.

seite owns both generated files. If your `public/` directory already has a custom `_worker.js` or `_routes.json`, the build fails rather than guess which one should win.

## Limits and When Cloudflare Access Is the Better Fit

This is a shared-password gate for a static site. Know where it stops.

- **Cloudflare Pages only.** Deploy pre-flight rejects password-enabled sites targeting GitHub Pages or Netlify, and `set-password` refuses to run for them.
- **No per-user identity.** Everyone in a group types the same password. There's no record of *who* visited and no way to revoke one person without changing the password for all. If you need SSO, per-user access or an audit trail, use Cloudflare Access. In that setup, keep `private = true` without `[access]` so the content stays out of discovery files, and let Access do the gating.
- **Rotation logs everyone out.** Running `set-password` again creates a new signing secret. The next deploy invalidates every existing session in that environment.
- **Nothing is live until you deploy.** Staged secrets apply to the next deployment only.
- **Set it from a terminal.** `set-password` needs an interactive terminal and refuses to read passwords from flags, so it can't run unattended in CI.
- **No login rate limiting.** The Worker doesn't throttle attempts. Use a long, random password.
- **Every request runs the Worker.** Because `_routes.json` includes `/*`, all requests on that project invoke the Worker, and Workers Free allows [100,000 requests per day](https://developers.cloudflare.com/workers/platform/limits/). [Fail open](https://developers.cloudflare.com/pages/functions/routing/) would serve static assets without running the Worker once the limit is hit, so seite sets each protected project to **Fail closed**, and `seite deploy`'s pre-flight stops if it can't confirm that. After the limit, visitors get a Cloudflare error page until it resets. If you deploy with your own `wrangler` workflow instead of `seite deploy`, check **Settings > Runtime > Fail open / closed** once yourself.

For investor pages, internal docs, client previews and a gated trust center, a shared password per group is usually the right amount of security. For anything regulated, pair seite's discovery controls with Access.

## FAQ

### Does Cloudflare Pages Have Built-In Password Protection?

No. Cloudflare Pages has no shared-password setting. You can [restrict preview deployments with Cloudflare Access](https://developers.cloudflare.com/pages/configuration/preview-deployments/), write Pages Functions middleware or use a tool that generates the Worker for you. seite does the last one from `seite.toml` with `[access]` and `private = true`.

### Is Cloudflare Access Free?

Cloudflare's Zero Trust free plan includes Access with a 50-user limit. It authenticates people by email one-time PIN or an identity provider rather than a shared password. That's better for audit and per-user revocation, and more setup than a single password for a handful of investors.

### Can I Password Protect Only One Page or Folder?

Yes. Password protection in seite applies per collection, so put the gated pages in their own collection with its own `url_prefix` (for example `/investors`) and mark it `private = true`. Everything outside that prefix stays public. Protected downloads go in `static/private/<group>/`.

### Does `private = true` Password Protect My Pages?

Not on its own. `private = true` removes pages from the sitemap, llms.txt, search index, feeds and homepage and adds `noindex, nofollow`, but anyone with the URL can still load them. Add an `[access]` section to require a password on Cloudflare Pages.

### How Do I Change the Password?

Run `seite access set-password <group>` again, then deploy. The new password takes effect with that deployment, and sessions signed with the old secret stop working, so everyone in that group logs in again. Other groups are unaffected.

### What About StatiCrypt or Client-Side Encryption?

StatiCrypt-style tools encrypt the HTML and decrypt it in the browser with JavaScript. They work on any host, including GitHub Pages. seite checks the password at Cloudflare's edge before serving anything, so protected files never reach an unauthenticated browser (seite keeps the Pages project set to fail closed), and there's no client-side decryption step.

## Gate It, Then Get Back to Shipping

Password protecting Cloudflare Pages shouldn't mean owning auth code. With seite it's five steps:

1. Mark the collection `private = true`.
2. Add `[access]` with `mode = "password"`.
3. Check the scopes with `seite access groups`.
4. Upload each group's password with `seite access set-password`.
5. Deploy.

Use `access_group` when different audiences need different passwords, and `subdomain` when a whole hostname should be gated. If you're building the rest of a company site around it, the guide to a [static site generator for startups](/blog/static-site-generator-for-startups) covers the landing page, docs and changelog that sit next to your investor pages. Every `seite access` flag is in the [CLI reference](/docs/cli-reference).
