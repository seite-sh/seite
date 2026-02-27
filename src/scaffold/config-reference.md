## Optional Configuration

These sections in `seite.toml` enable additional features. Omit them entirely to disable.

### Build Options

```toml
[build]
math = true  # enable $inline$ and $$display$$ math rendering via KaTeX
```

When `math = true`, the build pipeline renders LaTeX math expressions to HTML using server-side KaTeX. KaTeX CSS is automatically loaded from CDN. Code blocks and inline code spans are skipped.

### Image Processing

```toml
[images]
widths = [480, 800, 1200]  # generate resized copies at these pixel widths
quality = 80               # JPEG/WebP quality (1-100)
webp = true                # generate WebP variants alongside originals
avif = true                # generate AVIF variants (better compression than WebP)
avif_quality = 70          # AVIF quality (1-100, lower is fine — AVIF compresses better)
lazy_loading = true        # add loading="lazy" to <img> tags
```

When enabled, the build pipeline auto-resizes images, generates WebP and/or AVIF variants, and rewrites `<img>` tags with `srcset` and `<picture>` elements. AVIF sources are emitted before WebP in `<picture>` elements for optimal compression. The first image on each page is excluded from `loading="lazy"` to avoid hurting Largest Contentful Paint (LCP) performance.

### Analytics

```toml
[analytics]
provider = "plausible"     # "google", "gtm", "plausible", "fathom", "umami"
id = "example.com"         # measurement/tracking ID
cookie_consent = true      # show consent banner and gate analytics on acceptance
# script_url = "..."       # custom script URL (required for self-hosted Umami)
# extensions = ["tagged-events", "outbound-links"]  # Plausible script extensions
```

Analytics scripts are injected into all HTML files at build time. When `cookie_consent = true`, a banner is shown and analytics only load after the user accepts. Consent is stored in `localStorage`.

For Plausible, `extensions` appends [script extensions](https://plausible.io/docs/script-extensions) to the filename (e.g., `script.tagged-events.outbound-links.js`). Ignored when `script_url` is set or for non-Plausible providers.

### Subdomain Deploys

```toml
[[collections]]
name = "docs"
subdomain = "docs"             # deploy to docs.{base_domain}
deploy_project = "my-site-docs" # Cloudflare/Netlify project (optional)
```

When `subdomain` is set on a collection, it gets its own output directory (`dist-subdomains/{name}/`), own sitemap, RSS, robots.txt, and search index. Internal links targeting subdomain collections are auto-rewritten to absolute URLs. The dev server previews subdomain content at `/{name}-preview/`. `deploy_project` sets the Cloudflare Pages or Netlify project name for that subdomain (auto-created by `seite deploy --setup`).

