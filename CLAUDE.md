# page — Static Site Generator with LLM Integration

## What This Is

`page` is a Rust CLI static site generator designed to be AI-native. Content and templates are structured for LLM generation and consumption. Sites ship with `llms.txt`, `llms-full.txt`, and markdown versions of every page alongside the HTML.

The `page agent` command spawns Claude Code as a subprocess with full site context — no API keys needed, uses the user's Claude Code subscription directly.

## Quick Commands

```bash
cargo build          # Build the binary
cargo test           # Run all tests (36 unit + 96 integration)
cargo clippy         # Lint — must be zero warnings before committing
cargo run -- init mysite --title "My Site" --description "" --deploy-target github-pages --collections posts,docs,pages
cargo run -- build   # Build site from page.toml in current dir
cargo run -- serve   # Dev server with REPL (live reload, port auto-increment)
cargo run -- new post "My Post" --tags rust,web
cargo run -- new post "Mi Post" --lang es   # Create Spanish translation
cargo run -- new doc "Getting Started"
cargo run -- agent "create a blog post about Rust error handling"
cargo run -- agent   # Interactive Claude Code session with site context
cargo run -- theme list
cargo run -- theme apply dark
cargo run -- theme create "coral brutalist with lime accents"   # AI-generated custom theme
cargo run -- theme install https://example.com/theme.tera       # Install from URL
cargo run -- theme export my-theme --description "My theme"     # Export current theme
cargo run -- deploy
cargo run -- deploy --dry-run                       # Preview what deploy would do
cargo run -- deploy --target netlify                 # Deploy to Netlify
cargo run -- deploy --target cloudflare --dry-run    # Cloudflare dry run

# Install (end users)
curl -fsSL https://raw.githubusercontent.com/sanchezomar/page/main/install.sh | sh
```

## Architecture

### Module Map

```
src/
  main.rs              CLI entrypoint (clap dispatch)
  lib.rs               Module declarations
  error.rs             PageError enum (thiserror)
  themes.rs            6 bundled themes (default, minimal, dark, docs, brutalist, bento)
  themes/
    default.tera       Default theme template
    minimal.tera       Minimal theme template
    dark.tera          Dark mode theme template (true black + violet)
    docs.tera          Documentation sidebar theme template
    brutalist.tera     Neo-brutalist theme template
    bento.tera         Card grid bento theme template
  shortcodes/
    mod.rs             ShortcodeRegistry, expand(), value types
    parser.rs          Character-level scanner, code block skip, arg parsing
    builtins.rs        BuiltinShortcode struct, all() with include_str!
    builtins/
      youtube.html     Responsive YouTube embed
      vimeo.html       Responsive Vimeo embed
      gist.html        GitHub Gist embed
      callout.html     Admonition/callout body shortcode
      figure.html      Semantic figure with caption
  build/
    mod.rs             12-step build pipeline
    links.rs           Post-build internal link validation
    markdown.rs        pulldown-cmark wrapper
    feed.rs            RSS generation
    sitemap.rs         XML sitemap generation
    discovery.rs       robots.txt, llms.txt, llms-full.txt
    images.rs          Image processing (resize, WebP, srcset)
  cli/
    mod.rs             Cli struct + Command enum (7 subcommands)
    init.rs            Interactive project scaffolding
    new.rs             Create content files
    build.rs           Build command
    serve.rs           Dev server + interactive REPL
    deploy.rs          Deploy command
    agent.rs           AI agent (spawns Claude Code with site context)
    theme.rs           Theme management
  config/
    mod.rs             SiteConfig, CollectionConfig, ResolvedPaths
    defaults.rs        Default values
  data/mod.rs          Data file loading (YAML/JSON/TOML from data/ dir)
  content/mod.rs       Frontmatter parsing, ContentItem, slug generation
  deploy/mod.rs        GitHub Pages (git push) + Cloudflare (wrangler)
  output/
    mod.rs             CommandOutput trait
    human.rs           Colored terminal output
    json.rs            JSON output mode
  server/mod.rs        tiny_http dev server, file watcher, live reload
  templates/mod.rs     Tera template loading with embedded defaults
tests/
  integration.rs       88+ integration tests using assert_cmd + tempfile
```

### Build Pipeline (12 steps)

1. Clean output directory (`dist/`)
2. Load Tera templates (user-provided + embedded defaults)
2b. Load shortcode registry (built-in + user-defined from `templates/shortcodes/`)
2.5. Load data files (YAML/JSON/TOML from `data/` directory → `{{ data.filename }}` in templates)
3. Process each collection: walk content dir, parse frontmatter, **expand shortcodes**, render markdown to HTML, detect language from filename, resolve slugs/URLs, compute word count/reading time/excerpt/ToC, build translation map, sort
4. Render index page(s) — per-language if multilingual, with optional homepage content from `content/pages/index.md`. Also renders: paginated collection indexes, 404 page, tag index + per-tag archive pages
5. Generate RSS feed(s) — default language at `/feed.xml`, per-language at `/{lang}/feed.xml`
6. Generate sitemap — all items, with `xhtml:link` alternates for translations
7. Generate discovery files — per-language `llms.txt` and `llms-full.txt`
8. Output raw markdown alongside HTML (`slug.md` next to `slug.html`)
9. Generate search index — `search-index.json` (default lang), `/{lang}/search-index.json` (per-language)
10. Copy static files
11. Process images (resize to configured widths, generate WebP variants)
12. Post-process HTML (rewrite `<img>` tags with srcset, `<picture>` for WebP, `loading="lazy"`)

### Collections System

Three presets defined in `CollectionConfig::from_preset()`:

| Preset | has_date | has_rss | listed | nested | url_prefix | template |
|--------|----------|---------|--------|--------|------------|----------|
| posts  | true     | true    | true   | false  | /posts     | post.html |
| docs   | false    | false   | true   | true   | /docs      | doc.html |
| pages  | false    | false   | false  | false  | (empty)    | page.html |

### Output Pattern

URLs are clean (no extension): `/posts/hello-world`
Files on disk use flat pattern: `dist/posts/hello-world.html` + `dist/posts/hello-world.md`
The dev server resolves `/posts/hello-world` to `posts/hello-world.html`.

### Content Model

```rust
// Frontmatter fields (YAML between --- delimiters)
struct Frontmatter {
    title: String,
    date: Option<NaiveDate>,       // required for posts, auto-parsed from filename
    updated: Option<NaiveDate>,    // last-modified date → JSON-LD dateModified, sitemap lastmod
    description: Option<String>,
    image: Option<String>,         // absolute URL or path → og:image / twitter:image
    slug: Option<String>,          // override auto-generated slug
    tags: Vec<String>,
    draft: bool,                   // excluded from build unless --drafts
    template: Option<String>,      // override collection default template
    robots: Option<String>,        // per-page <meta name="robots">, e.g. "noindex"
    weight: Option<i32>,           // ordering for non-date collections (lower first, unweighted sort last alphabetically)
    extra: HashMap<String, Value>, // arbitrary key-value data → {{ page.extra.field }}
}

// Resolved during build
struct ContentItem {
    frontmatter: Frontmatter,
    raw_body: String,              // original markdown
    html_body: String,             // rendered HTML
    source_path: PathBuf,
    slug: String,                  // e.g., "hello-world" or "guides/setup"
    collection: String,            // e.g., "posts"
    url: String,                   // e.g., "/posts/hello-world" or "/es/posts/hello-world"
    lang: String,                  // e.g., "en", "es" — detected from filename
}
```

### Config (page.toml)

```toml
[site]
title = "My Site"
description = ""
base_url = "http://localhost:3000"
language = "en"
author = ""

[[collections]]
name = "posts"
# ... all CollectionConfig fields

[build]
output_dir = "dist"
data_dir = "data"    # optional: directory for data files (YAML/JSON/TOML)
minify = true        # optional: strip CSS/JS comments + collapse whitespace
fingerprint = true   # optional: write name.<hash8>.ext + dist/asset-manifest.json

[deploy]
target = "github-pages"  # or "cloudflare" or "netlify"
# project = "my-site"    # Cloudflare/Netlify project name
# domain = "example.com" # Custom domain (auto-attached via API)

# Optional: multi-language support (omit for single-language sites)
[languages.es]
title = "Mi Sitio"              # optional language-specific overrides
description = "Un sitio estático"

[languages.fr]
title = "Mon Site"

# Optional: image processing (omit for no processing)
[images]
widths = [480, 800, 1200]  # generate resized copies at these pixel widths
quality = 80               # JPEG/WebP quality (1-100)
lazy_loading = true        # add loading="lazy" to <img> tags
webp = true                # generate WebP variants alongside originals
```

### Data Files

The `data/` directory holds structured data files (YAML, JSON, TOML) that are loaded at build time and injected into all template contexts as `{{ data.filename }}`.

**How it works:**
- `data/nav.yaml` → `{{ data.nav }}` (array or object)
- `data/authors.json` → `{{ data.authors }}`
- `data/settings.toml` → `{{ data.settings }}`
- `data/menus/main.yaml` → `{{ data.menus.main }}` (nested directories create nested keys)

**Conflict detection:**
- Two files with the same stem (`authors.yaml` + `authors.json`) → build error
- A file and a directory with the same name (`nav.yaml` + `nav/main.yaml`) → build error
- Unknown file extensions → skipped with warning

**Theme integration:**
All 6 bundled themes conditionally render `data.nav` (navigation links) and `data.footer` (footer links + copyright). Example `data/nav.yaml`:
```yaml
- title: Blog
  url: /posts
- title: About
  url: /about
```
Example `data/footer.yaml`:
```yaml
links:
  - title: GitHub
    url: https://github.com/user/repo
copyright: "2026 My Company"
```

**Configuration:** The `data_dir` field in `[build]` defaults to `"data"`. Change it via `data_dir = "my_data"` in `page.toml`.

### Agent System

`page agent` spawns Claude Code (`claude` CLI) as a subprocess with a rich system prompt containing:
- Site config (title, description, base_url, collections)
- Content inventory (titles, dates, tags of existing content per collection)
- Template list
- Frontmatter format with examples
- File naming conventions
- Available `page` CLI commands

Two modes:
- `page agent "prompt"` — non-interactive, runs `claude -p` and exits
- `page agent` — interactive Claude Code session with full site context

The agent has access to `Read`, `Write`, `Edit`, `Glob`, `Grep`, and `Bash` tools.
Requires Claude Code CLI: `npm install -g @anthropic-ai/claude-code`

### Dev Server

- `page serve` starts HTTP server + file watcher in background threads
- Returns `ServerHandle` (stop with `Drop` or `.stop()`)
- Interactive REPL with commands: new, agent, theme, build, status, stop
- Live reload via `/__livereload` polling endpoint + injected `<script>`
- Auto-increments port if default (3000) is taken

### Release & Distribution

- **Version source of truth**: `Cargo.toml` `version` field
- **Auto-tag workflow** (`.github/workflows/release-tag.yml`): detects version changes on `main`, auto-creates `v{version}` git tag
- **Release workflow** (`.github/workflows/release.yml`): triggers on `v*` tag push, runs 4 jobs:
  1. `build` — matrix builds for macOS x86_64, macOS aarch64, Linux x86_64, Linux aarch64
  2. `release` — creates GitHub Release with `page-{target}.tar.gz` archives + `checksums-sha256.txt`
  3. `provenance` — SLSA Level 3 attestations via `slsa-framework/slsa-github-generator`
  4. `deploy-site` — builds and deploys `site/` to Cloudflare Pages (pagecli.dev)
- **Shell installer** (`install.sh`): `curl -fsSL .../install.sh | sh` — detects platform, downloads binary, verifies checksum
- **Release flow**: bump version in `Cargo.toml` + update `site/content/docs/releases.md` → push to `main` → auto-tag → auto-release → auto-deploy docs
- **Required GitHub secrets**: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`

### Themes

6 bundled themes compiled into the binary (no downloads). Each theme is a Tera template file in `src/themes/` embedded via `include_str!` — edit the `.tera` files directly, Cargo auto-recompiles. The `.tera` extension keeps editors from running HTML validators over the Jinja2 syntax.

`page theme create "<description>"` generates a custom theme by spawning Claude with a rich prompt including all template variable docs, Tera block requirements, and the search/pagination patterns. Claude writes `templates/base.html` directly. Requires Claude Code.

- `default` — 720px centered column, system-ui font, blue links (`#0057b7`). Sensible baseline.
- `minimal` — 600px column, Georgia serif, bottom-border-only search input. Literary/essay feel.
- `dark` — True black (`#0a0a0a`), violet accent (`#8b5cf6`), styled focus rings.
- `docs` — Fixed 260px sidebar with auto-scrolling nav, GitHub-style colors, table and code support.
- `brutalist` — Cream (`#fffef0`), 3px black borders, hard 6px/0 shadows, yellow (`#ffe600`) accent. No border-radius.
- `bento` — 1000px column, CSS grid sections, article cards (border-radius 20px, soft shadow), nth-child dark/indigo accent cards.

Template files: `src/themes/{default,minimal,dark,docs,brutalist,bento}.tera`
Each registers as `base.html` when applied; user `templates/base.html` overrides any bundled theme.

#### Theme Gallery & Sharing

Themes can be installed from URLs and exported for sharing:

- `page theme install <url>` — downloads a `.tera` file and saves to `templates/themes/<name>.tera`
- `page theme install <url> --name <name>` — install with a custom name
- `page theme export <name>` — packages `templates/base.html` as `templates/themes/<name>.tera` with metadata
- `page theme export <name> --description "..."` — include a description in the exported theme

Installed themes are stored in `templates/themes/` and discovered at runtime. `page theme list` shows both bundled and installed themes. `page theme apply` checks bundled first, then installed.

Theme metadata format: `{#- theme-description: Description here -#}` as a Tera comment in the first 10 lines of the file. The REPL in `page serve` also supports installed themes via the `theme` command.

## Patterns and Conventions

### Error Handling
- Library code returns `crate::error::Result<T>` (uses `PageError` with thiserror)
- CLI commands return `anyhow::Result<()>` for convenience
- Never `unwrap()` in library code; `unwrap()` only acceptable in tests and CLI entry points

### Output
- Use `output::human::success()`, `info()`, `error()` for terminal output
- Implement `CommandOutput` trait for structured output (supports `--json` flag)

### Testing & Linting
- Integration tests use `assert_cmd::Command` + `tempfile::TempDir`
- Helper: `init_site(tmp, name, title, collections)` scaffolds a site in a temp dir
- Test naming: `test_{command}_{behavior}` (e.g., `test_build_excludes_drafts_by_default`)
- **Before committing, always run both:** `cargo test` and `cargo clippy`
- All tests must pass and clippy must produce zero warnings before any commit
- Never `unwrap()` in library code — handle errors properly or use `unwrap_or_else`/`unwrap_or_default` with explicit fallbacks

### Documentation
- The documentation site lives in `site/` and is built with `page` itself
- Docs are in `site/content/docs/` — one markdown file per topic
- **When changing user-facing features (CLI flags, commands, config options, deploy behavior, build steps), update the corresponding docs:**
  - `site/content/docs/cli-reference.md` — all CLI commands and flags
  - `site/content/docs/deployment.md` — deploy targets, pre-flight checks, setup
  - `site/content/docs/configuration.md` — `page.toml` options
  - `site/content/docs/collections.md` — collection presets and config
  - `site/content/docs/templates.md` — template variables and blocks
  - `site/content/docs/i18n.md` — multi-language features
- Also update `CLAUDE.md` itself when adding new patterns, conventions, or architecture

### CLI
- clap 4.5 with derive macros
- Each subcommand has its own file in `src/cli/` with `{Command}Args` struct + `pub fn run(args) -> anyhow::Result<()>`
- Interactive prompts use `dialoguer` (only when CLI args are not provided)

### Templates
- Tera (Jinja2-compatible) templates
- All templates extend `base.html`
- Template variables: `{{ site.title }}`, `{{ page.title }}`, `{{ page.content | safe }}`, `{{ collections }}`, `{{ lang }}`, `{{ translations }}`, `{{ nav }}`, `{{ data }}`
- Additional page variables: `{{ page.description }}`, `{{ page.date }}`, `{{ page.updated }}`, `{{ page.image }}`, `{{ page.slug }}`, `{{ page.tags }}`, `{{ page.url }}`, `{{ page.collection }}`, `{{ page.robots }}`, `{{ page.word_count }}`, `{{ page.reading_time }}`, `{{ page.excerpt }}`, `{{ page.toc }}`, `{{ page.extra }}`
- Embedded defaults in `src/templates/mod.rs`; user templates in `templates/` override them
- All bundled themes include hreflang tags and language switcher UI when `translations` is non-empty
- All bundled themes emit canonical URL, Open Graph, Twitter Card, JSON-LD structured data, `<meta name="robots">` (when set), markdown alternate link, and llms.txt link in `<head>`
- All bundled themes provide overridable blocks: `{% block title %}`, `{% block content %}`, `{% block head %}`, `{% block extra_css %}`, `{% block extra_js %}`, `{% block header %}`, `{% block footer %}`
- All bundled themes include accessibility features: skip-to-main link, `role="search"`, `aria-label`, `aria-live="polite"` on search results, `prefers-reduced-motion: reduce`

### SEO and GEO (Generative Engine Optimization) Guardrails

Every bundled theme `<head>` emits a full SEO+GEO-optimized block. When creating or modifying theme templates, ensure all of the following are present:

**Required meta tags (all pages):**
- `<link rel="canonical">` — always `{{ site.base_url }}{{ page.url | default(value='/') }}`
- `<meta name="description">` — use `{{ page.description | default(value=site.description) }}` (per-page first, site fallback)
- `og:type` — `"article"` when `page.collection` is set, `"website"` for index/homepage
- `og:url`, `og:title`, `og:description`, `og:site_name`, `og:locale`
- `og:image` — conditional on `page.image` (set `image:` in frontmatter)
- `twitter:card` — `"summary_large_image"` when `page.image` exists, `"summary"` otherwise
- `twitter:title`, `twitter:description`

**Structured data (JSON-LD):**
- Posts (`page.collection == 'posts'`): `BlogPosting` with `headline`, `description`, `datePublished`, `dateModified` (from `page.updated`), `author`, `publisher`, `url`
- Docs/pages (`page.collection` set but not posts): `Article` with same fields minus dates
- Index/homepage: `WebSite` with `name`, `description`, `url`

**Discovery links:**
- `<link rel="alternate" type="application/rss+xml">` — RSS feed
- `<link rel="alternate" type="text/plain" title="LLM Summary" href="/llms.txt">` — LLM discovery
- `<link rel="alternate" type="text/markdown">` — markdown version (when `page.url` is set)

**Per-page robots:**
- `<meta name="robots" content="{{ page.robots }}">` — only emitted when `robots:` is set in frontmatter
- Use `robots: "noindex"` in frontmatter for pages that should not be indexed

**Frontmatter fields for SEO:**
- `description:` — page-specific description for meta/OG/Twitter/JSON-LD
- `image:` — absolute URL or `/static/…` path to social preview image
- `updated:` — last-modified date (YYYY-MM-DD) for JSON-LD `dateModified`
- `robots:` — per-page robots directive (e.g., `"noindex"`, `"noindex, nofollow"`)

### Shortcodes
- Two syntax forms: inline `{{< name(args) >}}` (raw HTML) and body `{{% name(args) %}} markdown {{% end %}}`
- Named args only: `key="string"`, `key=42`, `key=3.14`, `key=true`
- Shortcodes expanded **before** `markdown_to_html()` — output goes through the markdown pipeline
- `raw_body` on `ContentItem` stays unexpanded (for `.md` output and `llms-full.txt`)
- Built-in shortcodes: `youtube`, `vimeo`, `gist`, `callout` (body), `figure`
- User-defined shortcodes: Tera templates in `templates/shortcodes/*.html`
- User shortcodes override built-ins with the same name
- Shortcodes inside fenced code blocks and inline code spans are NOT expanded
- `ShortcodeRegistry` uses a separate Tera instance (not the page template Tera)
- All 6 bundled themes include CSS for `.video-embed`, `.callout-*`, `figure`/`figcaption`
- To add a built-in shortcode: create template in `src/shortcodes/builtins/`, add entry in `builtins.rs`

### Frontmatter Serialization
- `serde_yaml_ng` for YAML parsing
- `skip_serializing_if` on all optional fields — only emit what's set
- Draft field only serialized when `true`

### Adding a New Collection Preset
1. Add variant to `CollectionConfig::from_preset()` in `src/config/mod.rs`
2. Add default template in `src/templates/mod.rs`
3. Update `get_default_template()` match
4. Update `init.rs` template writing match
5. Add integration tests

### Singular→Plural Normalization
`find_collection()` in `src/config/mod.rs` normalizes "post" → "posts", "doc" → "docs", "page" → "pages" so users can type either form.

### Multi-language (i18n) Support

Filename-based translation system. Fully backward compatible — single-language sites work identically.

**How it works:**
- Default language content: `about.md` → `/about`
- Translation files: `about.es.md` → `/es/about`
- Language suffix must match a configured language in `[languages.*]` — random `.xx` suffixes are ignored
- Non-default languages get `/{lang}/` URL prefix
- Items with the same slug across languages are linked as translations

**Files involved:**
- `src/config/mod.rs` — `LanguageConfig` struct, `languages` field, helper methods (`is_multilingual()`, `all_languages()`, `title_for_lang()`, etc.)
- `src/content/mod.rs` — `extract_lang_from_filename()`, `strip_lang_suffix()`, `lang` field on `ContentItem`
- `src/build/mod.rs` — `TranslationLink` struct, `resolve_slug_i18n()`, translation map, per-language rendering
- `src/build/sitemap.rs` — `xhtml:link` alternates, per-language index URLs
- `src/themes.rs` — hreflang `<link>` tags in `<head>`, language switcher nav

**Per-language outputs:**
- `dist/index.html` (default lang), `dist/{lang}/index.html` (other langs)
- `dist/feed.xml` (default), `dist/{lang}/feed.xml` (per-lang RSS)
- `dist/llms.txt`, `dist/{lang}/llms.txt` (per-lang discovery)
- `dist/sitemap.xml` — single file with `xhtml:link` alternates for all translations

### Homepage as Special Page

If `content/pages/index.md` exists, its rendered content is injected into the index template context as `{{ page.content }}`. This allows custom hero/landing content on the homepage while still listing collections below it. The homepage page is extracted from the pages collection before rendering, so it doesn't collide with `dist/index.html`. Translations of the homepage (`index.es.md`) work as expected.

## Design Trends & Theme Direction (2026)

Context for deciding which themes to ship and what design prompts to include in the agent scaffold CLAUDE.md generated by `page init`.

### Trends worth building themes around

**Bento Grid** — Modular card layout with rounded corners (16–32px radius), mixed card sizes, subtle shadows. Each card carries one message. Gap: 12–20px. Colors: warm neutrals (`#F5F0EB`, `#E8E4DF`) mixed with dark cells (`#1C1C1E`). Good candidate for a new `bento` theme.

**Neo-Brutalism** — Thick black borders (2–4px solid), hard non-blurred drop shadows (`box-shadow: 4px 4px 0 #000`), saturated fills. Palettes: yellow `#FFE600` + black, lime `#AAFF00` + black, coral `#FF4D00` + black on white/cream. Heavy grotesque type (Inter Black 900) at large sizes. Buttons "press in" on hover. Good candidate for a `brutalist` theme.

**Glassmorphism** — `backdrop-filter: blur(12–20px)` on panels, `rgba(255,255,255,0.08–0.15)` fill, `1px solid rgba(255,255,255,0.2)` border. Used over gradient mesh backgrounds (violet→teal, indigo→electric blue). Keep it selective (nav, cards, modals) — not full-page.

**Dark Mode as Default** — True black (`#000000`, `#0A0A0A`) surfaces, neon accents (green `#00FF87`, blue `#0066FF`, pink `#FF0080`), off-white text (`#E8E8E8`). Subtle grain texture overlaid. Our current `dark` theme uses an arbitrary navy-purple — consider revising toward true black or offering both.

**Bold/Expressive Color** — Moving away from dusty sage/clay toward high-saturation palettes. Key combos: electric lime `#B9FF66` + deep forest `#191A23`; electric blue `#4361EE` + vibrant orange `#FF6B35`; near-black `#0D0D0D` + acid yellow `#EEFF00`.

**Card Play / Rounded Corners** — Standard grid of cards with generously rounded corners replacing sharp edges, subtle animated transitions between states. Overlaps with Bento Grid but simpler — standard responsive columns.

**Archival / Editorial** — Moodboard-meets-magazine. Grid structure, strong typographic hierarchy, pinned navigation blocks, no decorative clutter. Closest to our `minimal` theme but with more layout structure.

**Kinetic / Motion Narrative** — Large animated typography, scroll-driven reveals, endless-loop motion. Not practical for a static SSG theme without JS, but worth noting for agent-generated page templates.

**Glassmorphic Dark** — Combination of dark-mode surfaces with glassmorphism floating panels. Violet-to-indigo mesh background + blurred white-tinted panels.

### Current theme gaps

- No high-contrast/accessible-first theme
- No card-grid/bento layout theme
- No bold-color personality theme (neo-brutalist or expressive)
- `dark` uses arbitrary navy-purple — not aligned with any deliberate 2026 palette
- `default` has no distinguishing personality — could be the "canonical clean" baseline while new themes take stronger positions

### Agent scaffold design prompts

When `page init` generates `.claude/CLAUDE.md` for a new site, include these prompts to guide the AI agent when asked to redesign or create themes:

```
## Design Prompts for Theme Work

When redesigning or creating a theme, consider these directions:

### Minimal / Editorial
Single column, max 620px, Georgia or similar serif for body, geometric sans for UI.
Generous whitespace, no decorative elements. Bottom-border-only inputs. Typography carries
all personality. Colors: white/off-white (#FAF9F6) background, near-black (#1A1A1A) text,
one muted accent for links.

### Bold / Neo-Brutalist
Thick black borders (3px solid #000000), hard non-blurred box shadows (6px 6px 0 #000).
No border-radius. Saturated primary fill: yellow (#FFE600), lime (#AAFF00), or coral (#FF4D00).
White or cream (#FFFEF0) page background. Font: heaviest available weight (font-weight: 900),
large display sizes (headline: 4rem+). Buttons shift their shadow on hover to simulate pressing.

### Bento / Card Grid
Responsive CSS grid, gap 16px, all cards border-radius 20px. Mixed card sizes (1-col, 2-col,
3-col spans). Cards have independent background colors. Soft floating shadow:
box-shadow: 0 4px 24px rgba(0,0,0,0.08). No borders between cards — shadow creates separation.
Warm neutral palette with one dark-card accent per row.

### Dark / Expressive
True black (#000000 or #0A0A0A) surface. One neon accent (green #00FF87, blue #0066FF,
or violet #8B5CF6). Off-white text (#E8E8E8). Subtle noise grain texture via SVG data URI.
Navigation: translucent bar with backdrop-filter: blur(12px). High-contrast focus rings.

### Glass / Aurora
Gradient mesh background (violet #7B2FBE bleeding into teal #00C9A7 or indigo #1A1040
into electric blue #4361EE). Floating panels with backdrop-filter: blur(16px),
rgba(255,255,255,0.10) fill, 1px rgba(255,255,255,0.2) border. Use only for cards/modals,
not full layout. Dark text on light panels, light text on dark panels.

### Accessible / High-Contrast
WCAG AAA ratios throughout. Minimum 16px body text. 3px colored focus rings on all
interactive elements (don't hide them — make them a visual feature). Large click targets
(min 44px). One semantic accent color only. No color-only information encoding.
prefers-reduced-motion: reduce fully implemented.
```

### Theme file locations

Edit `src/themes/{name}.tera` to modify bundled themes. The `.tera` extension prevents
editor HTML validators from flagging Tera/Jinja2 syntax. Files are embedded at compile time
via `include_str!` — no runtime file loading. Add new themes in `src/themes.rs`.

## Roadmap

Tasks are ordered by priority. Mark each `[x]` when complete.

### Done

- [x] Collections system (posts, docs, pages with presets)
- [x] Build pipeline with markdown output alongside HTML
- [x] AI agent via Claude Code (`page agent` spawns `claude` subprocess with site context)
- [x] Discovery files (robots.txt, llms.txt, llms-full.txt)
- [x] Bundled themes (default, minimal, dark, docs, brutalist, bento)
- [x] Interactive REPL in serve mode
- [x] Live reload dev server with port auto-increment
- [x] Clean URL output pattern (slug.html / slug.md)
- [x] RSS feed (posts only) + XML sitemap (all collections)
- [x] Nested docs support (docs/guides/setup.md → /docs/guides/setup)
- [x] Draft exclusion with --drafts flag
- [x] Deploy to GitHub Pages + Cloudflare Pages + Netlify
- [x] Syntax highlighting (syntect, inline styles, base16-ocean.dark theme)
- [x] Docs sidebar navigation (auto-generated from collection items, grouped by directory)
- [x] Claude Code scaffolding (`page init` creates `.claude/settings.json` + `CLAUDE.md`)
- [x] Homepage as special page (`content/pages/index.md` → custom homepage content)
- [x] Multi-language (i18n) support — filename-based translations, per-language URLs, hreflang tags, language switcher, per-language RSS/sitemap/discovery files
- [x] Search — `search-index.json` generated per language, inline client-side JS in all 6 themes, lazy-loaded, filters by title/description/tags
- [x] Deploy improvements — GitHub Actions workflow, `--dry-run`, Netlify support, better Cloudflare errors + auto-detect project name
- [x] Image handling — auto-resize, WebP conversion, srcset/`<picture>` elements, `loading="lazy"`, configurable widths/quality
- [x] Pagination — `paginate = N` on collections, generates `/posts/`, `/posts/page/2/`, etc. with full pagination context
- [x] Asset pipeline — `build.minify = true` strips CSS/JS comments, `build.fingerprint = true` writes `name.<hash8>.ext` + `asset-manifest.json`
- [x] Reading time + word count — `{{ page.reading_time }}` and `{{ page.word_count }}` in all templates, 238 WPM average
- [x] Excerpts — auto-extracted from `<!-- more -->` marker or first paragraph, available as `{{ page.excerpt }}` and `{{ item.excerpt }}`
- [x] 404 page — generates `dist/404.html` using `404.html` template, dev server serves it on 404
- [x] Table of contents — auto-generated `{{ page.toc }}` from heading hierarchy, headings get `id` anchors
- [x] Tag pages — `/tags/` index and `/tags/{tag}/` archive pages, i18n-aware, included in sitemap
- [x] Custom template blocks — `{% block head %}`, `{% block extra_css %}`, `{% block extra_js %}`, `{% block header %}`, `{% block footer %}` in all 6 themes
- [x] Extra frontmatter — `extra:` field in frontmatter passes arbitrary data to templates as `{{ page.extra.field }}`
- [x] URL collision detection — errors on duplicate URLs, warns on missing content directories
- [x] Accessibility — skip-to-main link, `role="search"`, `aria-label`, `aria-live="polite"`, `prefers-reduced-motion: reduce` in all 6 themes
- [x] REPL theme-apply rebuild — applying a theme in the REPL automatically rebuilds the site
- [x] Build timing — per-step timing in build stats output (12 instrumented steps)
- [x] Deploy pre-flight checks — validates output dir, base_url (warns on localhost), CLI tools, git repo/remote, project config before deploying
- [x] GitHub Pages deploy hardening — auto-generates `.nojekyll`, `CNAME` for custom domains, sets git user identity, timestamped commit messages
- [x] base_url lifecycle management — `--base-url` flag overrides base_url at deploy time without modifying page.toml; pre-flight warns on localhost URLs
- [x] Preview/staging deploys — `--preview` flag creates non-production deploys on Cloudflare (branch deploy) and Netlify (draft deploy)
- [x] Deploy guided setup — `--setup` flag runs interactive setup: creates repos/projects, configures auth, generates CI workflows, writes config to page.toml
- [x] CI workflows for all targets — `page init` now generates GitHub Actions workflow for all three targets (not just GitHub Pages); Netlify also gets `netlify.toml`
- [x] Custom domain management — `--domain` flag shows DNS records, updates base_url + `deploy.domain` in page.toml, attaches domain to Cloudflare Pages via API, runs `netlify domains:add` for Netlify, auto-generates CNAME for GitHub Pages. Preflight checks verify domain is attached.
- [x] Post-deploy verification — auto-verifies homepage returns 200, checks robots.txt/sitemap.xml/llms.txt reachability after production deploys
- [x] Interactive deploy recovery — failed pre-flight checks prompt to auto-fix (install CLIs, init git, create projects, login, fix base_url), with manual instructions as fallback. Cloudflare verifies project exists remotely; Netlify checks site is linked.
- [x] Shell installer + release CI — `curl | sh` installer, GitHub Actions release workflow (4 platform binaries), SLSA Level 3 provenance, auto-tag from Cargo.toml version, auto-deploy docs site on release
- [x] Data files — `data/` directory with YAML/JSON/TOML files injected into template context as `{{ data.filename }}`. All 6 bundled themes conditionally render `data.nav` and `data.footer`. Nested directories create nested keys. Conflict detection for duplicate stems and path collisions.

### Up Next

#### Competitive gaps (from 2026 SSG competitive analysis vs Hugo, Astro, Zola, Eleventy, Next.js)

**Priority 1 — Close critical content authoring gaps (these block adoption):**

- [x] Shortcodes — reusable content components in markdown. Hugo-style dual syntax: `{{< name(args) >}}` for inline HTML, `{{% name(args) %}} body {{% end %}}` for markdown-processed bodies. 5 built-in shortcodes (youtube, vimeo, gist, callout, figure) + user-defined from `templates/shortcodes/`. Character-level parser with code block protection. All 6 themes include shortcode CSS.
- [x] Internal link checking — validate all internal links at build time; broken links become build errors. Zola has this built-in. After rendering all content, scan HTML for `<a href="/...">` and verify each target URL exists in the output set. Warn on broken links, error with `--strict` flag
- [x] Data files — support a `data/` directory with YAML/JSON/TOML files injected into template context as `{{ data.filename }}`. Enables navigation menus, author profiles, site-wide config without frontmatter. Hugo and Eleventy both have this. Load at build time alongside templates

**Priority 2 — Improve build pipeline and content model:**

- [ ] Incremental builds — only rebuild changed pages in dev mode. Hugo does partial rebuilds; Astro uses Vite HMR. Track content file mtimes, template dependencies, and config changes to determine minimum rebuild set. Critical for sites with 100+ pages where full rebuilds slow down the dev loop
- [ ] Content from external sources — fetch JSON/YAML from URLs at build time and inject into template context. Astro's Content Layer API can pull from any CMS/API/database. Start simple: `[data_sources]` section in page.toml with `name = "posts"`, `url = "https://api.example.com/posts"`, `format = "json"`. Fetch at build step 3, merge with filesystem content
- [ ] Math/LaTeX rendering — server-side KaTeX or MathJax rendering in markdown. Hugo added this in 2024. Render `$inline$` and `$$display$$` math blocks to HTML during markdown processing. Use `katex` crate or shell out to katex CLI. Important for technical/academic sites

**Priority 3 — Polish and ecosystem:**

- [ ] AVIF image format — generate AVIF variants alongside WebP in the image pipeline. Eleventy Image v6.0 supports AVIF. AVIF is smaller than WebP at comparable quality. Add to `<picture>` element sources with proper type attribute
- [x] Theme gallery/sharing — documentation page showcasing all bundled themes with HTML preview cards, `page theme install <url>` to download community themes, `page theme export <name>` to share custom/AI-generated themes, installed themes stored in `templates/themes/` and managed alongside bundled themes
- [ ] Related posts — auto-suggest related content based on shared tags/keywords, available as `{{ page.related }}`. Use tag overlap + TF-IDF on titles/descriptions to rank similarity. Show top 3-5 related items per page
- [ ] Theme community ecosystem — build discoverability and a contributor on-ramp for community themes:
  - [ ] Curated theme registry — `themes.json` in the repo (served on pagecli.dev) listing community themes with name, description, author, install URL, preview URL, and tags. Seed with the 6 bundled themes
  - [ ] `page theme browse` command — fetches the registry and displays available community themes with descriptions; `page theme browse --tags dark` to filter; install directly from browse results
  - [ ] GitHub discovery conventions — `page-theme` GitHub topic, `page-theme-template` template repo with correct structure/metadata/required blocks, naming convention `page-theme-{name}` with `theme.tera` at root
  - [ ] `page theme validate` command — checks a `.tera` file for all required pieces (HTML structure, SEO meta tags, template blocks, search JS, pagination, accessibility features) before publishing; outputs pass/fail with specific missing items
  - [ ] Community showcase on pagecli.dev — extend the gallery docs page to include community themes with live preview links and one-click install commands
  - [ ] Theme contributor guide — docs page explaining how to create, test, validate, and submit a theme to the registry

**Priority 4 — Deploy improvements (existing roadmap items):**

- [ ] Deploy history + diff — write `.deploy-log.json` with timestamp, target, commit hash, build duration, content hash; `--dry-run` shows what changed since last deploy
- [ ] Rollback — `page deploy rollback` restores previous deploy; keep last N commits on GitHub Pages instead of orphan force-push; use Netlify/Cloudflare rollback APIs
- [ ] Deploy diff — `page deploy --dry-run` shows new/modified/deleted files compared to last deploy via content hash comparison
- [ ] Environment-aware builds — detect CI environment variables (`GITHUB_ACTIONS`, `NETLIFY`, `CF_PAGES`) and auto-configure behavior (skip prompts, use env secrets for base_url)
- [ ] Multi-environment config — support `[deploy.production]` and `[deploy.staging]` sections with different base_url, targets, and settings
- [ ] Atomic deploys with content hashing — skip deploy if content hash unchanged since last deploy; useful in CI to avoid empty deploys
- [ ] S3/generic hosting target — AWS S3 + CloudFront support via `aws s3 sync` wrapper
- [ ] Webhook/notification support — post-deploy webhook (Slack, Discord, email) for team workflows
- [ ] Subdomain deploys — per-collection subdomain support (`subdomain = "docs"` on a collection → `docs.example.com`). Three phases: (1) per-collection `output_dir` override to build collections into separate directories, (2) per-collection `base_url` to make URL resolution, sitemap, RSS, and discovery files subdomain-aware, (3) multi-deploy orchestration so `page deploy` loops over subdomain configs and deploys each one. Config: `subdomain` field on `CollectionConfig`, per-collection domain in `DeploySection`. Affects every layer: config model, build pipeline, URL resolution, sitemap/RSS/discovery generation, and deploy orchestration

#### What NOT to build (deliberate non-goals based on competitive analysis)

- **JS framework support / component islands** — Astro's territory. Stay opinionated as content-first Tera-based SSG. Adding React/Vue would dilute the single-binary advantage
- **Server-side rendering / ISR** — Not our market. Stay purely static
- **GraphQL data layer** — This killed Gatsby. Don't repeat it
- **Plugin system** — Premature. Focus on making the core excellent with built-in features. A plugin API can come later when there's community demand
