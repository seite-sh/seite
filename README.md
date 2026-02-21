<p align="center">
  <strong>seite</strong>
</p>

<p align="center">
  <em>The AI-native static site generator.</em>
</p>

<p align="center">
  <a href="https://github.com/seite-sh/seite/actions/workflows/rust.yml"><img src="https://github.com/seite-sh/seite/actions/workflows/rust.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/seite"><img src="https://img.shields.io/crates/v/seite.svg" alt="Crates.io"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
</p>

---

Every page ships as **HTML** for browsers, **markdown** for LLMs, and **JSON-LD** for search engines. Single binary. Zero config. Built with Rust.

```bash
curl -fsSL https://seite.sh/install.sh | sh
seite init mysite --title "My Site" --collections posts,docs,pages
cd mysite && seite serve
```

## Why seite

Most static site generators produce HTML and stop there. seite builds for **three audiences at once**:

- **Browsers** get pages with full SEO metadata, Open Graph, Twitter Cards, and JSON-LD structured data
- **LLMs** get `llms.txt`, `llms-full.txt`, and a `.md` copy of every page for direct consumption
- **AI tools** get a built-in MCP server that exposes your entire site as structured resources

All from markdown + YAML frontmatter. No JavaScript runtime. No build dependencies. One binary does everything.

## Features

- **AI agent** — `seite agent` spawns Claude Code with full site context. Create posts, apply themes, and manage content conversationally
- **MCP server** — `seite mcp` exposes docs, config, content, and themes to any AI tool via the Model Context Protocol
- **6 bundled themes** — default, minimal, dark, docs, brutalist, bento — or generate a custom one with `seite theme create "coral brutalist with lime accents"`
- **6 collection presets** — posts, docs, pages, changelog, roadmap, and trust center (compliance hub)
- **Multi-language** — filename-based i18n with per-language URLs, RSS feeds, sitemaps, search indexes, and hreflang tags
- **Image pipeline** — auto-resize, WebP conversion, srcset/`<picture>` elements, lazy loading
- **Deploy anywhere** — GitHub Pages, Cloudflare Pages, Netlify with guided setup, pre-flight checks, and `--dry-run`
- **Analytics** — Google Analytics, GTM, Plausible, Fathom, Umami with optional cookie consent banner
- **Shortcodes** — `youtube`, `vimeo`, `gist`, `callout`, `figure` built-in, plus user-defined templates
- **Multi-site workspaces** — manage multiple sites from one directory with unified dev server
- **Self-update** — `seite self-update` fetches the latest release with SHA256 checksum verification

## Install

**macOS / Linux:**

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://seite.sh/install.ps1 | iex
```

<details>
<summary><strong>Other methods</strong></summary>

**From source (requires Rust toolchain):**

```bash
cargo install seite
```

**Pin a specific version:**

```bash
VERSION=v0.1.0 curl -fsSL https://seite.sh/install.sh | sh
```

</details>

## Quickstart

```bash
# Create a new site with blog posts, documentation, and static pages
seite init mysite --title "My Site" --collections posts,docs,pages
cd mysite

# Start the dev server with live reload
seite serve
```

Open `http://localhost:3000`. Edit content in `content/`, templates in `templates/`. The dev server live-reloads on every change.

```bash
# Create content
seite new post "Hello World" --tags intro,rust
seite new doc "Getting Started"
seite new changelog "v1.0.0" --tags new,improvement

# Deploy
seite deploy                  # commit, push, build, deploy
seite deploy --dry-run        # preview what would happen
seite deploy --setup          # guided first-time setup
```

## Build output

`seite build` runs a 13-step pipeline. For a site with posts and docs, it produces:

```
dist/
├── index.html                 # Homepage
├── posts/
│   ├── hello-world.html       # HTML for browsers
│   └── hello-world.md         # Markdown for LLMs
├── docs/
│   └── getting-started.html
├── feed.xml                   # RSS feed
├── sitemap.xml                # XML sitemap with hreflang
├── search-index.json          # Client-side search index
├── robots.txt
├── llms.txt                   # LLM discovery summary
├── llms-full.txt              # Full markdown for LLM indexing
├── 404.html
└── static/
```

Every HTML page includes canonical URLs, Open Graph tags, Twitter Cards, JSON-LD structured data (BlogPosting/Article/WebSite), and links to its markdown alternate.

## AI integration

### Agent

`seite agent` spawns Claude Code as a subprocess with rich site context — config, content inventory, templates, and available commands. No API keys needed; it uses your Claude Code subscription directly.

```bash
seite agent "write a blog post about Rust error handling"
seite agent     # interactive session
```

### MCP server

`seite mcp` runs a Model Context Protocol server over stdio. Claude Code auto-starts it via `.claude/settings.json` (created by `seite init`).

**Resources** — `seite://docs`, `seite://config`, `seite://content`, `seite://themes`, `seite://trust`

**Tools** — `seite_build`, `seite_create_content`, `seite_search`, `seite_apply_theme`, `seite_lookup_docs`

### LLM discovery

Every build generates `llms.txt` (summary) and `llms-full.txt` (complete markdown) for LLM indexing, plus a `.md` copy of every page. Multilingual sites get per-language versions.

## Collections

Six built-in presets, each with dedicated templates and theme CSS:

| Preset | Dated | RSS | Nested | Use case |
|--------|:-----:|:---:|:------:|----------|
| **posts** | ✓ | ✓ | — | Blog posts, articles |
| **docs** | — | — | ✓ | Documentation with sidebar navigation |
| **pages** | — | — | — | Standalone pages (About, Contact) |
| **changelog** | ✓ | ✓ | — | Release notes with colored tag badges |
| **roadmap** | — | — | — | Public roadmap with status tracking |
| **trust** | — | — | ✓ | Compliance hub (SOC 2, ISO 27001, GDPR, ...) |

```bash
seite collection add changelog    # add to existing site
seite new changelog "v2.0" --tags new,breaking
```

## Themes

Six themes ship with the binary — no downloads, no CDNs:

| Theme | Description |
|-------|-------------|
| **default** | Clean centered column, system fonts, blue links |
| **minimal** | Georgia serif, literary feel, generous whitespace |
| **dark** | True black `#0a0a0a`, violet accents, visible focus rings |
| **docs** | Fixed sidebar with auto-scrolling nav, GitHub-style |
| **brutalist** | Cream background, thick black borders, hard shadows, yellow accents |
| **bento** | CSS grid cards, rounded corners, mixed sizes, soft shadows |

```bash
seite theme list                              # show all themes
seite theme apply dark                        # switch theme
seite theme create "neon cyberpunk on black"  # AI-generated custom theme
seite theme install https://example.com/t.tera  # install from URL
seite theme export my-theme                   # share your theme
```

All themes include: responsive design, accessibility (skip-to-main, ARIA labels, focus rings, `prefers-reduced-motion`), search, pagination, language switcher, and full SEO/structured data output.

## Multi-language

Filename-based translations. Fully backward-compatible — single-language sites work identically.

```
content/posts/
├── hello-world.md        → /posts/hello-world
├── hello-world.es.md     → /es/posts/hello-world
└── hello-world.fr.md     → /fr/posts/hello-world
```

```toml
# seite.toml
[languages.es]
title = "Mi Sitio"

[languages.fr]
title = "Mon Site"
```

Each language gets its own index, RSS feed, search index, and LLM discovery files. The sitemap includes `xhtml:link` alternates for all translations.

## Deployment

```bash
seite deploy              # commit, push, build, deploy
seite deploy --dry-run    # preview without deploying
seite deploy --setup      # interactive guided setup
seite deploy --domain example.com  # configure custom domain
```

Three targets with pre-flight checks, auto-recovery on failures, and post-deploy verification:

```toml
[deploy]
target = "cloudflare"    # or "github-pages" or "netlify"
```

`seite init` auto-generates the CI workflow for your chosen target.

## Configuration

```toml
# seite.toml — minimal config (everything else is optional)
[site]
title = "My Site"
base_url = "https://example.com"

[[collections]]
name = "posts"
```

<details>
<summary><strong>Full config reference</strong></summary>

```toml
[site]
title = "My Site"
description = "A site built with seite"
base_url = "https://example.com"
language = "en"
author = "Your Name"

[[collections]]
name = "posts"
# name = "docs" | "pages" | "changelog" | "roadmap" | "trust"
# paginate = 10          # enable pagination

[build]
output_dir = "dist"
data_dir = "data"         # YAML/JSON/TOML → {{ data.filename }} in templates
minify = true             # strip CSS/JS comments, collapse whitespace
fingerprint = true        # name.<hash8>.ext + asset-manifest.json

[deploy]
target = "github-pages"   # or "cloudflare" or "netlify"
auto_commit = true

[images]
widths = [480, 800, 1200]
quality = 80
webp = true
lazy_loading = true

[analytics]
provider = "plausible"    # or "google", "gtm", "fathom", "umami"
id = "example.com"
cookie_consent = false    # set true to show consent banner
# script_url = "..."      # custom URL for self-hosted analytics

[languages.es]
title = "Mi Sitio"

[trust]
company = "Acme Corp"
frameworks = ["soc2", "iso27001"]
```

</details>

## Data files

Drop YAML, JSON, or TOML files in `data/` and access them in any template:

```yaml
# data/nav.yaml
- title: Blog
  url: /posts
- title: Docs
  url: /docs
- title: GitHub
  url: https://github.com/user/repo
  external: true
```

```html
<!-- Available as {{ data.nav }} in all templates -->
```

All bundled themes automatically render `data.nav` (header) and `data.footer` (footer links + copyright).

## Workspaces

Manage multiple sites from a single directory:

```bash
seite workspace init my-workspace
seite workspace add blog --collections posts,pages
seite workspace add docs --collections docs
seite build --site blog    # build one site
seite serve                # unified dev server at /<site-name>/
seite deploy               # deploy all sites
```

## Documentation

Full docs at **[seite.sh](https://seite.sh/docs/getting-started)**

- [Getting Started](https://seite.sh/docs/getting-started)
- [Configuration](https://seite.sh/docs/configuration)
- [Collections](https://seite.sh/docs/collections)
- [Templates](https://seite.sh/docs/templates)
- [Shortcodes](https://seite.sh/docs/shortcodes)
- [Multi-language (i18n)](https://seite.sh/docs/i18n)
- [Deployment](https://seite.sh/docs/deployment)
- [CLI Reference](https://seite.sh/docs/cli-reference)
- [Trust Center](https://seite.sh/docs/trust-center)

## Contributing

Contributions are welcome. Please open an issue to discuss larger changes before submitting a PR.

```bash
cargo build          # build
cargo test           # 331 tests (139 unit + 192 integration)
cargo clippy         # must be zero warnings
```

## License

[MIT](LICENSE)
