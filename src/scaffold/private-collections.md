## Private collections and password access

A collection can set `private = true` in `seite.toml` to keep its content out of public discovery while still building its hub and pages:

```toml
[[collections]]
name = "docs"
private = true
url_prefix = "/docs"
default_template = "doc.html"
```

Its pages are excluded from the homepage listing, `sitemap.xml`, `llms.txt`, `llms-full.txt`, `search-index.json`, RSS/Atom feeds, and tag pages. Every page also gets `noindex, nofollow` unless it supplies its own `robots` frontmatter. Without `[access]`, this remains discovery-only and does not authenticate requests.

### Enable password access on Cloudflare Pages

Add the site-level access configuration:

```toml
[access]
mode = "password"
session_hours = 168
```

With `[access]` present, every `private = true` collection is password protected by the generated Cloudflare Pages Worker. A collection's `url_prefix` protects that path. An empty prefix protects the whole main domain. A private collection with `subdomain` protects its entire subdomain output.

Translated collection pages are protected under every configured language prefix. Password access owns the generated `_worker.js` and `_routes.json`; remove custom versions from `public/` before enabling it.

Each private collection has a password group. It defaults to the collection name; set `access_group` explicitly to share a password or give paths separate passwords:

```toml
[[collections]]
name = "docs"
private = true
access_group = "staff"
url_prefix = "/docs"
```

Inspect and set groups with:

```bash
seite access groups
seite access set-password staff
seite deploy
```

The password is prompted securely and staged for production and preview on the relevant Cloudflare Pages project. It is never stored in the repository or passed as a command-line argument. Deploy from the Pages project's configured production branch to activate production; projects created by seite use `main`. Run `seite deploy --preview` to activate preview. Password access currently supports Cloudflare Pages only.

### Protected assets

Ordinary files under `static/` stay public. With `[access]` enabled, put sensitive files under `static/private/<group>/`; seite emits them at `/private-assets/<group>/` behind the same password group. Image optimization is skipped for protected files so generated variants cannot become public, and the legacy `/static/private/` URL namespace is denied. Without `[access]`, those files remain under `/static/private/<group>/` and are publicly reachable.

### Private subdomains

For a private collection deployed separately, configure its project and subdomain:

```toml
[[collections]]
name = "docs"
private = true
access_group = "staff"
subdomain = "docs"
subdomain_base_url = "https://docs.example.com"
deploy_project = "my-docs"
```

`seite deploy --setup` creates the Pages project, while attaching a custom subdomain remains a one-time Cloudflare dashboard step. After the project exists, run `seite access set-password staff`, then deploy to activate it.
