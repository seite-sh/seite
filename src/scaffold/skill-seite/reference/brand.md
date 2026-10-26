# /seite brand

A logo, a color palette, and a favicon, saved as site files.

1. Look for existing assets first (`public/favicon*`, `static/logo*`, `data/brand.yaml`, colors in `templates/base.html`). Refine them rather than replace them, unless the user asks.
2. Ask for what you cannot infer (name, what it does, the feel), once, in one message. With impeccable installed, use `/impeccable` for the design decisions.
3. Palette: write `data/brand.yaml`:
   ```yaml
   name: "Site name"
   colors:
     primary: "#0057b7"
     primary_hover: "#003d82"
     secondary: "#f59e0b"
     background: "#ffffff"
     surface: "#f8f9fa"
     text: "#1a1a1a"
     text_muted: "#6b7280"
     border: "#e5e7eb"
   ```
   Templates read it as `{{ data.brand.colors.primary }}`, and generated social cards use `background`, `text` and `primary` unless `[og]` sets colors. Text on its background needs WCAG AA contrast (4.5:1, or 3:1 for large text).
4. Logo: `static/logo.svg`, valid SVG with `xmlns="http://www.w3.org/2000/svg"`, legible at 32px wide. Set `logo = "static/logo.svg"` under `[og]` in seite.toml to put it on social cards; check one with `seite og preview /` (it writes `.seite/cache/og-preview.png`).
5. Favicon: `public/favicon.svg` and, if you can convert it, `public/favicon.ico`. `public/` is copied to the site root, and themes link `/favicon.ico`.
6. To apply the palette, change only color values in `templates/base.html` (or read `data.brand.colors.*` there). Then verify as in [check.md](check.md).
