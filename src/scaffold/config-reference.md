## Optional Configuration

These sections in `seite.toml` enable additional features. Omit them entirely to disable.

### Image Processing

```toml
[images]
widths = [480, 800, 1200]  # generate resized copies at these pixel widths
quality = 80               # JPEG/WebP quality (1-100)
webp = true                # generate WebP variants alongside originals
lazy_loading = true        # add loading="lazy" to <img> tags
```

When enabled, the build pipeline auto-resizes images, generates WebP variants, and rewrites `<img>` tags with `srcset` and `<picture>` elements. The first image on each page is excluded from `loading="lazy"` to avoid hurting Largest Contentful Paint (LCP) performance.

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

