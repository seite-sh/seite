## Optional Configuration

These sections in `page.toml` enable additional features. Omit them entirely to disable.

### Image Processing

```toml
[images]
widths = [480, 800, 1200]  # generate resized copies at these pixel widths
quality = 80               # JPEG/WebP quality (1-100)
webp = true                # generate WebP variants alongside originals
lazy_loading = true        # add loading="lazy" to <img> tags
```

When enabled, the build pipeline auto-resizes images, generates WebP variants, and rewrites `<img>` tags with `srcset` and `<picture>` elements.

### Analytics

```toml
[analytics]
provider = "plausible"     # "google", "gtm", "plausible", "fathom", "umami"
id = "example.com"         # measurement/tracking ID
cookie_consent = true      # show consent banner and gate analytics on acceptance
# script_url = "..."       # custom script URL (required for self-hosted Umami)
```

Analytics scripts are injected into all HTML files at build time. When `cookie_consent = true`, a banner is shown and analytics only load after the user accepts. Consent is stored in `localStorage`.

