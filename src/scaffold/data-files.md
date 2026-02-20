## Data Files

The `data/` directory holds structured data files (YAML, JSON, TOML) that are loaded at build time and injected into all template contexts as `{{ data.filename }}`.

### How it works

- `data/nav.yaml` → `{{ data.nav }}`
- `data/authors.json` → `{{ data.authors }}`
- `data/settings.toml` → `{{ data.settings }}`
- Nested directories create nested keys: `data/menus/main.yaml` → `{{ data.menus.main }}`

Supported file types: `.yaml` / `.yml`, `.json`, `.toml`.

### Theme integration

All 6 bundled themes conditionally render `data.nav` (navigation links) and `data.footer` (footer links + copyright). Example `data/nav.yaml`:

```yaml
- title: Blog
  url: /posts
- title: About
  url: /about
- title: GitHub
  url: https://github.com/user/repo
  external: true
```

Example `data/footer.yaml`:

```yaml
links:
  - title: GitHub
    url: https://github.com/user/repo
    external: true
copyright: "2026 My Company"
```

Internal links are auto-prefixed with `{{ lang_prefix }}` for i18n; external links (marked with `external: true`) get `target="_blank"`.

### Configuration

The data directory defaults to `data/`. Change it with `data_dir = "my_data"` under `[build]` in `page.toml`.

### Conflict detection

- Two files with the same stem (`authors.yaml` + `authors.json`) → build error
- A file and a directory with the same name → build error

