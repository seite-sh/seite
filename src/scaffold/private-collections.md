## Private (gated) collections & access control

A collection can set `private = true` in `seite.toml` to keep its content out of every public
discovery surface while still building its hub and pages:

```toml
[[collections]]
name = "trust"
private = true
url_prefix = "/trust"
default_template = "trust-item.html"
```

When `private = true`, the collection's pages (hub + items, all languages) are excluded from the
homepage listing, `sitemap.xml`, `llms.txt`, `llms-full.txt`, `search-index.json`, RSS/Atom feeds,
and tag pages, and every page is stamped `<meta name="robots" content="noindex, nofollow">`
(unless the page sets its own `robots`). It is independent of `listed`, and the build logs how many
pages were excluded. Absent or `false` is the normal, fully-discoverable behavior.

### `private` does NOT lock access

**Important: `private` only hides content from discovery — it does not authenticate or restrict
who can load the pages.** The HTML is still publicly fetchable by anyone who has the URL. seite has
no Cloudflare Access / auth integration; it does not create any access policy for you. To actually
gate the content you must put an access layer in front of the path or subdomain yourself:

- **Cloudflare Access (Zero Trust)** — the usual choice. In the Cloudflare dashboard: Zero Trust →
  Access → Applications → add a **self-hosted** application covering the gated host or path (e.g.
  `trust.example.com` or `example.com/trust*`), then attach an access policy (email domain, Google/
  Okta SSO, one-time PIN, etc.).
- Or HTTP basic auth / a Worker in front, depending on the host.

So the gated pattern is: **`private = true` (no discovery + `noindex`) + an access layer you
configure on the host/path.**

### Serving gated content on a subdomain

For a clean boundary, deploy the collection to its own subdomain and lock the whole subdomain:

```toml
[[collections]]
name = "trust"
private = true
subdomain = "trust"
subdomain_base_url = "https://trust.example.com"
default_template = "trust-item.html"
```

- `seite deploy --setup` auto-creates the Cloudflare Pages **project** (`{site}-trust`).
- `seite deploy` pushes `dist-subdomains/trust/` to it; the subdomain **root** renders the
  collection's own index template (e.g. `trust-index.html`), not the generic site index.
- **Manual one-time step:** attach the custom domain (`trust.example.com`) to that Pages project
  (Cloudflare dashboard → the Pages project → Custom domains). seite does **not** auto-attach a
  subdomain's custom domain — it only attaches the *main* site's domain. Once attached, Cloudflare
  creates the DNS record automatically if the zone is on the same Cloudflare account.
- Then add a Cloudflare Access application over `trust.example.com` to require auth.

Note: `domain = "..."` on a `[[collections]]` block is **not** a recognized field and is ignored —
a subdomain's URL comes from `subdomain_base_url` (or auto-derives to `{subdomain}.{base_domain}`).
