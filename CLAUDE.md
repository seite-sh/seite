# seite — Static Site Generator with LLM Integration

## What This Is

`seite` is a Rust CLI static site generator designed to be AI-native. Content and templates are structured for LLM generation and consumption. Sites ship with `llms.txt`, `llms-full.txt`, and markdown versions of every page alongside the HTML.

The `seite agent` command spawns Claude Code as a subprocess with full site context — no API keys needed, uses the user's Claude Code subscription directly.

## Quick Commands

```bash
cargo build          # Build the binary
cargo test           # Run all tests (135 unit + 192 integration)
cargo fmt --all      # Format — CI enforces `cargo fmt --all -- --check`
cargo clippy         # Lint — must be zero warnings before committing
cargo run -- init mysite --title "My Site" --description "" --deploy-target github-pages --collections posts,docs,pages
cargo run -- init trustsite --title "Acme" --collections posts,pages,trust --trust-company "Acme Corp" --trust-frameworks soc2,iso27001
cargo run -- build   # Build site from seite.toml in current dir
cargo run -- serve   # Dev server with REPL (live reload, port auto-increment)
cargo run -- new post "My Post" --tags rust,web
cargo run -- new post "Mi Post" --lang es   # Create Spanish translation
cargo run -- new doc "Getting Started"
cargo run -- new changelog "v1.0.0" --tags new,improvement
cargo run -- new roadmap "Dark Mode" --tags planned
cargo run -- agent "create a blog post about Rust error handling"
cargo run -- agent   # Interactive Claude Code session with site context
cargo run -- theme list
cargo run -- theme apply dark
cargo run -- theme create "coral brutalist with lime accents"   # AI-generated custom theme
cargo run -- theme install https://example.com/theme.tera       # Install from URL
cargo run -- theme export my-theme --description "My theme"     # Export current theme
cargo run -- deploy                                 # Commit, push, build, and deploy
cargo run -- deploy --no-commit                     # Deploy without auto-commit/push
cargo run -- deploy --dry-run                       # Preview what deploy would do
cargo run -- deploy --target netlify                 # Deploy to Netlify
cargo run -- deploy --target cloudflare --dry-run    # Cloudflare dry run

# Collection management
cargo run -- collection list                         # List site collections
cargo run -- collection add changelog                # Add a preset collection

# Workspace commands
cargo run -- workspace init my-workspace           # Initialize workspace
cargo run -- workspace add blog --collections posts,pages  # Add a site
cargo run -- workspace list                         # List sites
cargo run -- workspace status                       # Detailed status
cargo run -- build --site blog                      # Build one site in workspace
cargo run -- serve --site docs                      # Serve one site in workspace

# Project upgrade & self-update
cargo run -- upgrade                               # Upgrade project config to current binary version
cargo run -- upgrade --check                       # Check if upgrade needed (exit 1 = outdated, for CI)
cargo run -- upgrade --force                       # Upgrade without confirmation
cargo run -- self-update                           # Update seite binary to latest release
cargo run -- self-update --check                   # Check for new version without installing
cargo run -- self-update --target-version 0.2.0    # Pin a specific version

# Install (end users)
curl -fsSL https://seite.sh/install.sh | sh       # macOS/Linux
irm https://seite.sh/install.ps1 | iex            # Windows
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
    mod.rs             13-step build pipeline
    analytics.rs       Analytics injection + cookie consent banner
    links.rs           Post-build internal link validation
    markdown.rs        pulldown-cmark wrapper (CommonMark + GFM: tables, strikethrough, footnotes, task lists) + syntax highlighting (syntect)
    feed.rs            RSS generation
    sitemap.rs         XML sitemap generation
    discovery.rs       robots.txt, llms.txt, llms-full.txt
    images.rs          Image processing (resize, WebP, srcset)
  docs.rs              Embedded documentation pages (14 docs, include_str! from seite-sh/content/docs/)
  meta.rs              Project metadata (.seite/config.json) — version tracking, upgrade detection
  mcp/
    mod.rs             MCP server core (JSON-RPC over stdio, method dispatch)
    resources.rs       MCP resource providers (docs, config, content, themes, mcp-config)
    tools.rs           MCP tool implementations (build, create_content, search, apply_theme, lookup_docs)
  cli/
    mod.rs             Cli struct + Command enum (12 subcommands)
    init.rs            Interactive project scaffolding (creates .seite/config.json + MCP config)
    new.rs             Create content files
    build.rs           Build command (workspace-aware, nudges on outdated project)
    serve.rs           Dev server + interactive REPL (workspace-aware)
    deploy.rs          Deploy command (workspace-aware)
    agent.rs           AI agent (spawns Claude Code with site context)
    theme.rs           Theme management
    mcp.rs             MCP server entry point (launches stdio JSON-RPC server)
    workspace.rs       Workspace CLI (init, list, add, status)
    upgrade.rs         Upgrade project config to current binary (version-gated steps)
    collection.rs      Collection management (add, list)
    self_update.rs     Self-update binary from GitHub Releases
  scaffold/            Static markdown sections for generated CLAUDE.md (include_str! at compile time)
    seo-requirements.md  SEO/GEO requirements section
    repl.md            Dev server REPL commands
    i18n.md            Multi-language support guide
    data-files.md      Data files system guide
    templates.md       Templates, themes, variables, blocks, SEO guardrails
    features.md        Feature list
    config-reference.md  Optional config sections (images, analytics)
    mcp.md             MCP server + state-awareness guidance
    shortcodes.md      Shortcode syntax and reference
    design-prompts.md  Theme design directions
  config/
    mod.rs             SiteConfig, CollectionConfig, ResolvedPaths
    defaults.rs        Default values
  data/mod.rs          Data file loading (YAML/JSON/TOML from data/ dir)
  content/mod.rs       Frontmatter parsing, ContentItem, slug generation
  deploy/mod.rs        GitHub Pages (git push) + Cloudflare (wrangler)
  workspace/
    mod.rs             WorkspaceConfig, ExecutionContext, resolve_context()
    build.rs           Multi-site build orchestration
    server.rs          Unified dev server with per-site routing + file watching
    deploy.rs          Multi-site deploy orchestration
  output/
    mod.rs             CommandOutput trait
    human.rs           Colored terminal output
    json.rs            JSON output mode
  server/mod.rs        tiny_http dev server, file watcher, live reload
  templates/mod.rs     Tera template loading with embedded defaults
tests/
  integration.rs       192 integration tests using assert_cmd + tempfile
```

### Build Pipeline (13 steps)

1. Clean output directory (`dist/`)
2. Load Tera templates (user-provided + embedded defaults)
2b. Load shortcode registry (built-in + user-defined from `templates/shortcodes/`)
2.5. Load data files (YAML/JSON/TOML from `data/` directory → `{{ data.filename }}` in templates)
3. Process each collection: walk content dir, parse frontmatter, **expand shortcodes**, render markdown to HTML, detect language from filename, resolve slugs/URLs, compute word count/reading time/excerpt/ToC, build translation map, sort
3b. Inject i18n context — compute `lang_prefix` (empty for default language, `"/{lang}"` for others), `default_language`, and `t` (UI strings merged from defaults + `data/i18n/{lang}.yaml`) into every template context
4. Render index page(s) — per-language if multilingual, with optional homepage content from `content/pages/index.md`. Also renders: paginated collection indexes, 404 page, tag index + per-tag archive pages
5. Generate RSS feed(s) — default language at `/feed.xml`, per-language at `/{lang}/feed.xml`
6. Generate sitemap — all items, with `xhtml:link` alternates for translations
7. Generate discovery files — per-language `llms.txt` and `llms-full.txt`
8. Output raw markdown alongside HTML (`slug.md` next to `slug.html`)
9. Generate search index — `search-index.json` (default lang), `/{lang}/search-index.json` (per-language)
10. Copy static files
11. Process images (resize to configured widths, generate WebP variants)
12. Post-process HTML (rewrite `<img>` tags with srcset, `<picture>` for WebP, `loading="lazy"`)
13. Inject analytics scripts (and optional cookie consent banner) into all HTML files

### Collections System

Six presets defined in `CollectionConfig::from_preset()`:

| Preset | has_date | has_rss | listed | nested | url_prefix | template |
|--------|----------|---------|--------|--------|------------|----------|
| posts  | true     | true    | true   | false  | /posts     | post.html |
| docs   | false    | false   | true   | true   | /docs      | doc.html |
| pages  | false    | false   | false  | false  | (empty)    | page.html |
| changelog | true  | true    | true   | false  | /changelog | changelog-entry.html |
| roadmap | false   | false   | true   | false  | /roadmap   | roadmap-item.html |
| trust  | false    | false   | true   | true   | /trust     | trust-item.html |

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

### Config (seite.toml)

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
auto_commit = true        # auto-commit + push before deploy; non-main branches auto-use preview

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

# Optional: trust center (omit if not using compliance hub)
[trust]
company = "Acme Corp"                  # company name for trust center
frameworks = ["soc2", "iso27001"]      # active compliance frameworks

# Optional: analytics (omit for no analytics)
[analytics]
provider = "google"        # "google", "gtm", "plausible", "fathom", "umami"
id = "G-XXXXXXXXXX"        # measurement/tracking ID
cookie_consent = true      # show consent banner and gate analytics on acceptance
# script_url = "..."       # custom script URL (required for self-hosted Umami)
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
All 6 bundled themes conditionally render `data.nav` (navigation links) and `data.footer` (footer links + copyright). Internal links are auto-prefixed with `{{ lang_prefix }}` for i18n; external links (marked with `external: true`) get `target="_blank"`. Example `data/nav.yaml`:
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

**UI string translations:**
`data/i18n/{lang}.yaml` files override UI strings for each language. The build pipeline merges them on top of English defaults and injects the result as `{{ t }}` in templates. Example: `data/i18n/es.yaml` with keys like `search_placeholder`, `skip_to_content`, `no_results`, `newer`, `older`, etc.

**Configuration:** The `data_dir` field in `[build]` defaults to `"data"`. Change it via `data_dir = "my_data"` in `seite.toml`.

### Agent System

`seite agent` spawns Claude Code (`claude` CLI) as a subprocess with a rich system prompt containing:
- Site config (title, description, base_url, collections)
- Content inventory (titles, dates, tags of existing content per collection)
- Template list
- Frontmatter format with examples
- File naming conventions
- Available `seite` CLI commands

Two modes:
- `seite agent "prompt"` — non-interactive, runs `claude -p` and exits
- `seite agent` — interactive Claude Code session with full site context

The agent has access to `Read`, `Write`, `Edit`, `Glob`, `Grep`, and `Bash` tools.
Requires Claude Code CLI: `npm install -g @anthropic-ai/claude-code`

### MCP Server

`seite mcp` runs a JSON-RPC 2.0 server over stdio for AI tool integration (Model Context Protocol). It's spawned automatically by Claude Code via the `mcpServers.seite` entry in `.claude/settings.json`.

**Architecture:** Synchronous read loop on stdin, dispatches JSON-RPC methods, writes responses to stdout. All logging to stderr (never stdout — it corrupts the protocol). No async runtime needed.

**Resources** (read-only data):
- `seite://docs` / `seite://docs/{slug}` — 14 embedded documentation pages (compiled into binary via `include_str!`)
- `seite://config` — `seite.toml` serialized as JSON
- `seite://content` / `seite://content/{collection}` — content inventory with metadata
- `seite://themes` — bundled + installed themes
- `seite://trust` — trust center state (certifications, subprocessors, FAQs, content) — only when trust collection is configured
- `seite://mcp-config` — `.claude/settings.json`

**Tools** (executable actions):
- `seite_build` — runs build pipeline, returns stats
- `seite_create_content` — creates content files with frontmatter
- `seite_search` — searches content by title/description/tags
- `seite_apply_theme` — applies bundled or installed theme
- `seite_lookup_docs` — searches embedded docs by topic or keyword

**Files:** `src/mcp/mod.rs` (protocol), `src/mcp/resources.rs`, `src/mcp/tools.rs`, `src/docs.rs` + `seite-sh/content/docs/` (embedded docs, single source of truth)

### Dev Server

- `seite serve` starts HTTP server + file watcher in background threads
- Returns `ServerHandle` (stop with `Drop` or `.stop()`)
- Interactive REPL with commands: new, agent, theme, build, status, stop
- Live reload via `/__livereload` polling endpoint + injected `<script>`
- Auto-increments port if default (3000) is taken

### Workspace System

Multi-site workspaces let you manage multiple `seite` sites from a single directory with a `seite-workspace.toml` config.

- **Detection**: `workspace::find_workspace_root()` walks up from cwd looking for `seite-workspace.toml`
- **Execution context**: `workspace::resolve_context()` returns either `Standalone` or `Workspace` — all commands check this
- **Global `--site` flag**: filters operations to a single site within the workspace
- **Workspace build** (`workspace::build`): iterates sites, builds each with its own config/paths, per-site link validation
- **Workspace serve** (`workspace::server`): unified HTTP server routing `/<site-name>/...` to each site's output dir, per-site file watching with selective rebuilds, auto-generated workspace index at `/`
- **Workspace deploy** (`workspace::deploy`): iterates sites, runs pre-flight checks + build + deploy per-site, each site can use a different deploy target
- **Config overrides**: `seite-workspace.toml` can override `base_url` and `output_dir` per-site without touching each site's `seite.toml`

### Project Metadata & Upgrades

`.seite/config.json` stores tooling metadata — which version of `seite` last scaffolded or upgraded the project. This is fully owned by the tool; users never edit it.

```json
{
  "version": "0.1.0",
  "initialized_at": "2026-02-19T14:30:00+00:00"
}
```

**Key module:** `src/meta.rs` — `PageMeta` struct, `load()`, `write()`, `needs_upgrade()`, `project_version()`, `binary_version()`.

**Upgrade system** (`src/cli/upgrade.rs`):
- Version-gated steps: each `UpgradeStep` has an `introduced_in` version and a `check` function that returns `UpgradeAction`s
- Three action types: `Create` (new file), `MergeJson` (additive merge into JSON), `Append` (append section to text file)
- Non-destructive: never removes user content, only adds missing entries
- `--check` mode exits with code 1 if outdated (for CI)
- Adding a new upgrade step: add an `UpgradeStep` to `upgrade_steps()` with the version that introduces it

**Self-update** (`src/cli/self_update.rs`):
- Downloads binary from GitHub Releases using `ureq` (same dep as Cloudflare API)
- Detects platform via compile-time `cfg!(target_os)` / `cfg!(target_arch)`
- Verifies SHA256 checksum using system `sha256sum` / `shasum` (same approach as `install.sh`)
- Atomic binary replacement: rename current → backup, copy new → target, restore on failure

**Build nudge** (`src/cli/build.rs`): at the start of `run()`, checks `meta::needs_upgrade()` and prints a one-liner if outdated.

**MCP server config**: `seite init` writes `.claude/settings.json` with a `mcpServers.seite` block. `seite upgrade` merges this into existing settings without overwriting user entries.

### Release & Distribution

- **Version source of truth**: `Cargo.toml` `version` field
- **Auto-tag workflow** (`.github/workflows/release-tag.yml`): detects version changes on `main`, auto-creates `v{version}` git tag
- **Release workflow** (`.github/workflows/release.yml`): triggers on `v*` tag push, runs 4 jobs:
  1. `build` — matrix builds for macOS x86_64, macOS aarch64, Linux x86_64, Linux aarch64, Windows x86_64
  2. `release` — creates GitHub Release with `seite-{target}.tar.gz` archives + `checksums-sha256.txt`
  3. `provenance` — SLSA Level 3 attestations via `slsa-framework/slsa-github-generator`
  4. `publish-crate` — publishes to crates.io
  5. `deploy-site` — builds and deploys `seite-sh/` to Cloudflare Pages (seite.sh)
- **Shell installer** (`install.sh`): `curl -fsSL .../install.sh | sh` — detects platform, downloads binary, verifies checksum
- **PowerShell installer** (`install.ps1`): `irm .../install.ps1 | iex` — Windows installer
- **Release flow**: bump version in `Cargo.toml` + update `seite-sh/content/docs/releases.md` → push to `main` → auto-tag → auto-release → auto-publish crate → auto-deploy docs
- **Required GitHub secrets**: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`, `CARGO_REGISTRY_TOKEN`

### Themes

6 bundled themes compiled into the binary (no downloads). Each theme is a Tera template file in `src/themes/` embedded via `include_str!` — edit the `.tera` files directly, Cargo auto-recompiles. The `.tera` extension keeps editors from running HTML validators over the Jinja2 syntax.

`seite theme create "<description>"` generates a custom theme by spawning Claude with a rich prompt including all template variable docs, Tera block requirements, and the search/pagination patterns. Claude writes `templates/base.html` directly. Requires Claude Code.

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

- `seite theme install <url>` — downloads a `.tera` file and saves to `templates/themes/<name>.tera`
- `seite theme install <url> --name <name>` — install with a custom name
- `seite theme export <name>` — packages `templates/base.html` as `templates/themes/<name>.tera` with metadata
- `seite theme export <name> --description "..."` — include a description in the exported theme

Installed themes are stored in `templates/themes/` and discovered at runtime. `seite theme list` shows both bundled and installed themes. `seite theme apply` checks bundled first, then installed.

Theme metadata format: `{#- theme-description: Description here -#}` as a Tera comment in the first 10 lines of the file. The REPL in `seite serve` also supports installed themes via the `theme` command.

## Patterns and Conventions

### Error Handling
- Library code returns `crate::error::Result<T>` (uses `PageError` with thiserror)
- CLI commands return `anyhow::Result<()>` for convenience
- Never `unwrap()` in library code; `unwrap()` only acceptable in tests and CLI entry points

### Output
- Use `output::human::success()`, `info()`, `error()` for terminal output
- Implement `CommandOutput` trait for structured output (supports `--json` flag)

### Versioning
- **Version source of truth**: `Cargo.toml` `version` field (semver: `MAJOR.MINOR.PATCH`)
- **Every code change must bump the version** before committing — this is required for the release pipeline to deploy properly
- **PATCH** (0.1.1 → 0.1.2): bug fixes, small improvements, new build steps, internal refactors
- **MINOR** (0.1.x → 0.2.0): new user-facing features, new CLI commands, new config options, new collection presets
- **MAJOR** (0.x → 1.0): breaking changes to config format, CLI interface, or template variables
- When in doubt, bump PATCH

### Testing & Linting
- Integration tests use `assert_cmd::Command` + `tempfile::TempDir`
- Helper: `init_site(tmp, name, title, collections)` scaffolds a site in a temp dir
- Test naming: `test_{command}_{behavior}` (e.g., `test_build_excludes_drafts_by_default`)
- **Before committing, always run:** `cargo fmt --all`, `cargo clippy`, and `cargo test`
- All tests must pass, clippy must produce zero warnings, and code must be formatted before any commit
- CI also runs: `cargo-deny` (license/vulnerability audit), `cargo doc` (no warnings), MSRV check (1.88), `cargo-semver-checks` (on PRs), ShellCheck (shell scripts)
- Never `unwrap()` in library code — handle errors properly or use `unwrap_or_else`/`unwrap_or_default` with explicit fallbacks

### Documentation
- The documentation site lives in `seite-sh/` and is built with `seite` itself
- Docs are in `seite-sh/content/docs/` — one markdown file per topic
- **When changing user-facing features (CLI flags, commands, config options, deploy behavior, build steps), update the corresponding docs:**
  - `seite-sh/content/docs/cli-reference.md` — all CLI commands and flags
  - `seite-sh/content/docs/deployment.md` — deploy targets, pre-flight checks, setup
  - `seite-sh/content/docs/configuration.md` — `seite.toml` options
  - `seite-sh/content/docs/collections.md` — collection presets and config
  - `seite-sh/content/docs/templates.md` — template variables and blocks
  - `seite-sh/content/docs/i18n.md` — multi-language features
- Also update `CLAUDE.md` itself when adding new patterns, conventions, or architecture

### CLI
- clap 4.5 with derive macros
- Each subcommand has its own file in `src/cli/` with `{Command}Args` struct + `pub fn run(args) -> anyhow::Result<()>`
- Interactive prompts use `dialoguer` (only when CLI args are not provided)

### Templates
- Tera (Jinja2-compatible) templates
- All templates extend `base.html`
- Template variables: `{{ site.title }}`, `{{ page.title }}`, `{{ page.content | safe }}`, `{{ collections }}`, `{{ lang }}`, `{{ default_language }}`, `{{ lang_prefix }}`, `{{ t }}`, `{{ translations }}`, `{{ nav }}`, `{{ data }}`
- Additional page variables: `{{ page.description }}`, `{{ page.date }}`, `{{ page.updated }}`, `{{ page.image }}`, `{{ page.slug }}`, `{{ page.tags }}`, `{{ page.url }}`, `{{ page.collection }}`, `{{ page.robots }}`, `{{ page.word_count }}`, `{{ page.reading_time }}`, `{{ page.excerpt }}`, `{{ page.toc }}`, `{{ page.extra }}`
- Embedded defaults in `src/templates/mod.rs`; user templates in `templates/` override them
- All bundled themes include hreflang tags and language switcher UI when `translations` is non-empty
- All bundled themes emit canonical URL, Open Graph, Twitter Card, JSON-LD structured data, `<meta name="robots">` (when set), markdown alternate link, and llms.txt link in `<head>`
- All bundled themes provide overridable blocks: `{% block title %}`, `{% block content %}`, `{% block head %}`, `{% block extra_css %}`, `{% block extra_js %}`, `{% block header %}`, `{% block footer %}`
- All bundled themes include accessibility features: skip-to-main link, `role="search"`, `aria-label`, `aria-live="polite"` on search results, `prefers-reduced-motion: reduce`
- **i18n conventions for themes:**
  - Use `{{ lang }}` (not `{{ site.language }}`) for the current page language — `site.language` is always the *configured default* language
  - Use `{{ lang_prefix }}` to prefix internal links (empty for default language, `"/es"` for Spanish, etc.)
  - Use `{{ t.key }}` for all UI strings — never hardcode English text in themes
  - Data file links: `{{ lang_prefix }}{{ item.url }}` for internal, plain `{{ item.url }}` for `item.external`
  - `<html lang="{{ lang }}">` and `<meta property="og:locale" content="{{ lang }}">` — use `lang`, not `site.language`

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

### Adding a User-Facing Feature (Checklist)

When adding a new config section, CLI command, or build behavior, ensure all of these are updated:

1. **Config model** — add struct + field to `SiteConfig` in `src/config/mod.rs`
2. **Build pipeline** — integrate the feature in `src/build/mod.rs` (add step to `build_site()`)
3. **Init scaffolding** — set the default in the `SiteConfig` literal in `src/cli/init.rs`
4. **Docs** — update `seite-sh/content/docs/configuration.md` (or the relevant `seite-sh/content/docs/*.md` page). These are compiled into the binary via `include_str!` and also deploy to seite.sh — single source of truth, no mirroring needed
5. **MCP compliance** — verify the feature is visible through the MCP server:
   - `seite://config` auto-exposes any new `SiteConfig` fields (no code needed if serde works)
   - `seite_build` auto-includes new build steps (no code needed if added to `build_site()`)
   - `seite_lookup_docs` returns the updated embedded docs (no code needed if `seite-sh/content/docs/` is updated)
   - Add an MCP integration test confirming the new config is visible via `seite://config`
6. **CLAUDE.md** — update the module map, build pipeline step list, config example, and any relevant convention sections
7. **Tests** — unit tests in the feature module, integration tests in `tests/integration.rs`
8. **Deploy test structs** — if `SiteConfig` changed, update any test fixtures in `src/deploy/mod.rs`
9. **i18n compliance** — if adding UI-visible text to themes or templates, use `{{ t.key }}` (never hardcode English); if adding internal links in themes, use `{{ lang_prefix }}{{ url }}`; add new `t` keys to `ui_strings_for_lang()` in `src/build/mod.rs`

### Changelog Collection
- Date-based entries with RSS feed. Tags render as colored badges in all 6 themes
- Tag conventions: `new` (green), `fix` (blue), `breaking` (red), `improvement` (purple), `deprecated` (gray)
- Collection-specific index template (`changelog-index.html`) shows reverse-chronological feed
- Create entries: `seite new changelog "v1.0.0" --tags new,improvement`

### Roadmap Collection
- Weight-ordered items grouped by status tags. No dates, no RSS
- Status tags: `planned`, `in-progress`, `done`, `cancelled`
- Three index layouts: grouped list (default `roadmap-index.html`), kanban (`roadmap-kanban.html`), timeline (`roadmap-timeline.html`)
- Users switch layouts by creating `templates/roadmap-index.html` extending the desired variant
- All 6 themes include CSS for all 3 layouts (grouped, kanban, timeline) and status badges
- Create items: `seite new roadmap "Feature Name" --tags planned`

### Collection-Specific Index Templates
The build pipeline checks for `{collection.name}-index.html` before falling back to `index.html`. This applies to both paginated and non-paginated collections. Changelog and roadmap ship dedicated index templates; any collection can override its index this way.

### Trust Center

The trust center is a collection preset (`trust`) that scaffolds a compliance hub with:
- **Data files** (`data/trust/`) — `certifications.yaml`, `subprocessors.yaml`, `faq.yaml` driving the templates
- **Content pages** (`content/trust/`) — markdown prose for security overview, vulnerability disclosure, per-framework details
- **Templates** — `trust-index.html` (hub at `/trust/`) and `trust-item.html` (individual pages)
- **Config** — `[trust]` section in `seite.toml` with `company` and `frameworks` fields

**Init flow:** When `trust` is included in `--collections`, interactive prompts ask for company name, frameworks (SOC 2, ISO 27001, GDPR, HIPAA, PCI DSS, CCPA, SOC 3), sections, and per-framework status. CLI flags `--trust-company`, `--trust-frameworks`, `--trust-sections` support non-interactive mode.

**Build pipeline:** Step 4b2 renders non-paginated collection indexes — the trust collection gets `/trust/index.html` using `trust-index.html` template with `data.trust.*` context.

**MCP:** `seite://trust` resource returns trust center state (certifications, subprocessors, FAQs, content items).

**Scaffolded CLAUDE.md:** When trust is present, the generated CLAUDE.md includes a comprehensive trust center section with data file formats, management workflows, and MCP integration docs.

**Files:** `src/config/mod.rs` (TrustSection, preset_trust), `src/cli/init.rs` (scaffolding), `src/templates/mod.rs` (DEFAULT_TRUST_INDEX, DEFAULT_TRUST_ITEM), `src/build/mod.rs` (step 4b2), `src/mcp/resources.rs` (seite://trust), `seite-sh/content/docs/trust-center.md`, `src/themes/*.tera` (CSS)

### Singular→Plural Normalization
`find_collection()` in `src/config/mod.rs` normalizes "post" → "posts", "doc" → "docs", "seite" → "pages" so users can type either form.

### Multi-language (i18n) Support

Filename-based translation system. Fully backward compatible — single-language sites work identically.

**How it works:**
- Default language content: `about.md` → `/about`
- Translation files: `about.es.md` → `/es/about`
- Language suffix must match a configured language in `[languages.*]` — random `.xx` suffixes are ignored
- Non-default languages get `/{lang}/` URL prefix
- Items with the same slug across languages are linked as translations

**Template context variables for i18n:**
- `{{ lang }}` — current page language code (e.g. `"es"`)
- `{{ site.language }}` — *configured default* language from `seite.toml` (always the same, does not change per render)
- `{{ default_language }}` — same as `site.language` (explicit alias for clarity)
- `{{ lang_prefix }}` — URL prefix: empty string for default language, `"/es"` for Spanish, etc.
- `{{ t }}` — UI translation strings object with English defaults, overridable via `data/i18n/{lang}.yaml`
- `{{ translations }}` — array of `{lang, url}` links for all language variants of the current page

**Files involved:**
- `src/config/mod.rs` — `LanguageConfig` struct, `languages` field, helper methods (`is_multilingual()`, `all_languages()`, `title_for_lang()`, etc.)
- `src/content/mod.rs` — `extract_lang_from_filename()`, `strip_lang_suffix()`, `lang` field on `ContentItem`
- `src/build/mod.rs` — `TranslationLink` struct, `resolve_slug_i18n()`, translation map, per-language rendering, `ui_strings_for_lang()`, `lang_prefix_for()`, `insert_i18n_context()`
- `src/build/sitemap.rs` — `xhtml:link` alternates, per-language index URLs
- `src/themes/*.tera` — hreflang `<link>` tags in `<head>`, language switcher nav, `{{ t.xxx }}` UI strings, `{{ lang_prefix }}` on nav/footer links

**Per-language outputs:**
- `dist/index.html` (default lang), `dist/{lang}/index.html` (other langs)
- `dist/feed.xml` (default), `dist/{lang}/feed.xml` (per-lang RSS)
- `dist/llms.txt`, `dist/{lang}/llms.txt` (per-lang discovery)
- `dist/sitemap.xml` — single file with `xhtml:link` alternates for all translations

### Homepage as Special Page

If `content/pages/index.md` exists, its rendered content is injected into the index template context as `{{ page.content }}`. This allows custom hero/landing content on the homepage while still listing collections below it. The homepage page is extracted from the pages collection before rendering, so it doesn't collide with `dist/index.html`. Translations of the homepage (`index.es.md`) work as expected.

## Design Trends & Theme Direction (2026)

Context for deciding which themes to ship and what design prompts to include in the agent scaffold CLAUDE.md generated by `seite init`.

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

When `seite init` generates `.claude/CLAUDE.md` for a new site, include these prompts to guide the AI agent when asked to redesign or create themes:

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
- [x] AI agent via Claude Code (`seite agent` spawns `claude` subprocess with site context)
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
- [x] Claude Code scaffolding (`seite init` creates `.claude/settings.json` + `CLAUDE.md`)
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
- [x] base_url lifecycle management — `--base-url` flag overrides base_url at deploy time without modifying seite.toml; pre-flight warns on localhost URLs
- [x] Preview/staging deploys — `--preview` flag creates non-production deploys on Cloudflare (branch deploy) and Netlify (draft deploy)
- [x] Deploy guided setup — `--setup` flag runs interactive setup: creates repos/projects, configures auth, generates CI workflows, writes config to seite.toml
- [x] CI workflows for all targets — `seite init` now generates GitHub Actions workflow for all three targets (not just GitHub Pages); Netlify also gets `netlify.toml`
- [x] Custom domain management — `--domain` flag shows DNS records, updates base_url + `deploy.domain` in seite.toml, attaches domain to Cloudflare Pages via API, runs `netlify domains:add` for Netlify, auto-generates CNAME for GitHub Pages. Preflight checks verify domain is attached.
- [x] Post-deploy verification — auto-verifies homepage returns 200, checks robots.txt/sitemap.xml/llms.txt reachability after production deploys
- [x] Interactive deploy recovery — failed pre-flight checks prompt to auto-fix (install CLIs, init git, create projects, login, fix base_url), with manual instructions as fallback. Cloudflare verifies project exists remotely; Netlify checks site is linked.
- [x] Shell installer + release CI — `curl | sh` installer, GitHub Actions release workflow (4 platform binaries), SLSA Level 3 provenance, auto-tag from Cargo.toml version, auto-deploy docs site on release
- [x] Data files — `data/` directory with YAML/JSON/TOML files injected into template context as `{{ data.filename }}`. All 6 bundled themes conditionally render `data.nav` and `data.footer`. Nested directories create nested keys. Conflict detection for duplicate stems and path collisions.
- [x] Windows support — PowerShell installer (`install.ps1`), Windows x86_64 release binaries, platform helpers for `.cmd` shims and backslash path normalization
- [x] Multi-site workspaces — `seite workspace init/list/add/status` commands, `seite-workspace.toml` config, global `--site` flag, workspace-aware build/serve/deploy, unified dev server with per-site routing and selective file watching, per-site deploy orchestration with independent targets
- [x] Project metadata & upgrades — `.seite/config.json` tracks binary version that last scaffolded the project. `seite upgrade` applies version-gated, additive config upgrades (MCP server, CLAUDE.md sections). `seite build` nudges when outdated. `--check` mode for CI (exit 1 = outdated). Non-destructive merge into `.claude/settings.json` and append-only for CLAUDE.md.
- [x] Self-update — `seite self-update` downloads latest binary from GitHub Releases, verifies SHA256 checksum, atomic binary replacement with backup/restore. `--check` for CI, `--target-version` to pin. Uses same release infrastructure as `install.sh`.
- [x] MCP server scaffolding — `seite init` creates `.claude/settings.json` with `mcpServers.seite` block. `seite upgrade` merges MCP config into existing projects.
- [x] MCP server — `seite mcp` runs a JSON-RPC server over stdio. Resources: `seite://docs/*` (14 embedded doc pages), `seite://config`, `seite://content/*`, `seite://themes`, `seite://mcp-config`. Tools: `seite_build`, `seite_create_content`, `seite_search`, `seite_apply_theme`, `seite_lookup_docs`. Docs embedded via `include_str!` from `seite-sh/content/docs/`. Claude Code auto-starts the server via `.claude/settings.json`.
- [x] Changelog collection — `changelog` preset with dated entries, RSS feed, and colored tag badges (new/fix/breaking/improvement/deprecated). Dedicated `changelog-entry.html` and `changelog-index.html` templates with CSS in all 6 themes. Collection-specific index template resolution in build pipeline.
- [x] Roadmap collection — `roadmap` preset with weight-ordered items and status tags (planned/in-progress/done/cancelled). Three index layouts: grouped list (default), kanban (CSS grid 3-column), and timeline (vertical milestones). Dedicated templates and CSS in all 6 themes.
- [x] Trust Center collection — `trust` preset with data-driven compliance hub scaffolding. Certifications, subprocessors, and FAQ data files. Content pages for security overview, vulnerability disclosure, per-framework details. Interactive init flow with framework selection. Dedicated `trust-index.html` and `trust-item.html` templates with CSS in all 6 themes. `seite://trust` MCP resource. 17 i18n UI string keys.
- [x] Analytics & cookie consent — `[analytics]` config section with 5 providers (Google Analytics, GTM, Plausible, Fathom, Umami). Optional cookie consent banner with localStorage persistence. Injected into all HTML files at build step 13.
- [x] Collection management — `seite collection add <preset>` and `seite collection list` commands for adding collections to existing sites.

### Up Next

See the public roadmap at https://seite.sh/roadmap for detailed planned features, and the changelog at https://seite.sh/changelog for release history.

**Priority areas:**
- Multi-LLM agent support — Claude Code, OpenCode, Codex CLI, Gemini CLI as interchangeable backends
- Incremental builds — only rebuild changed pages in dev mode
- External data sources — fetch JSON/YAML from URLs at build time
- Math/LaTeX rendering — server-side KaTeX for technical/academic sites
- Theme community ecosystem — registry, browse, validate, contributor guide
- Deploy improvements — history/rollback, environment-aware builds, S3 target, subdomain deploys

#### What NOT to build (deliberate non-goals based on competitive analysis)

- **JS framework support / component islands** — Astro's territory. Stay opinionated as content-first Tera-based SSG. Adding React/Vue would dilute the single-binary advantage
- **Server-side rendering / ISR** — Not our market. Stay purely static
- **GraphQL data layer** — This killed Gatsby. Don't repeat it
- **Plugin system** — Premature. Focus on making the core excellent with built-in features. A plugin API can come later when there's community demand
