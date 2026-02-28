# seite — Static Site Generator with LLM Integration

## What This Is

`seite` is a Rust CLI static site generator designed to be AI-native. Content and templates are structured for LLM generation and consumption. Sites ship with `llms.txt`, `llms-full.txt`, and markdown versions of every page alongside the HTML.

The `seite agent` command spawns Claude Code as a subprocess with full site context — no API keys needed, uses the user's Claude Code subscription directly.

## Quick Commands

```bash
cargo build          # Build the binary
cargo test           # Run all tests (1001 unit + 309 integration)
cargo fmt --all      # Format — CI enforces `cargo fmt --all -- --check`
cargo clippy         # Lint — must be zero warnings before committing
cargo run -- init mysite --title "My Site" --description "" --deploy-target github-pages --collections posts,docs,pages
cargo run -- init trustsite --title "Acme" --collections posts,pages,trust --trust-company "Acme Corp" --trust-frameworks soc2,iso27001
cargo run -- init contactsite --title "My Site" --collections posts,pages --contact-provider formspree --contact-endpoint xpznqkdl
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

# Contact form management
cargo run -- contact setup                                           # Interactive setup
cargo run -- contact setup --provider formspree --endpoint xpznqkdl  # Non-interactive
cargo run -- contact status                                          # Show current config
cargo run -- contact remove                                          # Remove config

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
      contact_form.html  Contact form (5 providers: Formspree, Web3Forms, Netlify, HubSpot, Typeform)
  build/
    mod.rs             13-step build pipeline
    analytics.rs       Analytics injection + cookie consent banner
    base_path.rs       Base path rewriting for subdirectory deploys
    code_copy.rs       Copy-to-clipboard button injection for code blocks
    links.rs           Post-build internal link validation
    markdown.rs        pulldown-cmark wrapper (CommonMark + GFM: tables, strikethrough, footnotes, task lists) + syntax highlighting (syntect)
    feed.rs            RSS generation
    sitemap.rs         XML sitemap generation
    discovery.rs       robots.txt, llms.txt, llms-full.txt
    images.rs          Image processing (resize, WebP, AVIF, srcset)
    math.rs            Math/LaTeX pre-processing (KaTeX rendering of $inline$ and $$display$$ blocks)
  docs.rs              Embedded documentation pages (15 docs, include_str! from seite-sh/content/docs/)
  meta.rs              Project metadata (.seite/config.json) — version tracking, upgrade detection
  mcp/
    mod.rs             MCP server core (JSON-RPC over stdio, method dispatch)
    resources.rs       MCP resource providers (docs, config, content, themes, mcp-config)
    tools.rs           MCP tool implementations (build, create_content, search, apply_theme, lookup_docs)
  cli/
    mod.rs             Cli struct + Command enum (13 subcommands)
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
    contact.rs         Contact form management (setup, remove, status)
    collection.rs      Collection management (add, list)
    self_update.rs     Self-update binary from GitHub Releases
  update_check.rs        Background update check with 24h cache (~/.seite/update-cache.json)
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
    contact-form.md    Contact form provider setup and shortcode reference
    design-prompts.md  Theme design directions
    theme-builder.md   Brief CLAUDE.md mention of /theme-builder skill
    skill-theme-builder.md  Full SKILL.md for /theme-builder Claude Code skill
    landing-page-builder.md  Brief CLAUDE.md mention of /landing-page skill
    skill-landing-page.md  Full SKILL.md for /landing-page Claude Code skill
    brand-identity.md  Brief CLAUDE.md mention of /brand-identity skill
    skill-brand-identity.md  Full SKILL.md for /brand-identity Claude Code skill
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
build.rs               Cargo build script — generates releases.md from changelog at compile time
scripts/
  generate-release-docs.sh  Consolidate changelog entries → releases.md (for docs site deploy)
  prepare-release.sh        Scaffold new changelog entry for current version
```

### Build Pipeline (15 steps)

1. Clean output directory (`dist/`)
1b. Copy public files (`public/` → `dist/` root, no prefix, no minification)
2. Load Tera templates (user-provided + embedded defaults)
2b. Load shortcode registry (built-in + user-defined from `templates/shortcodes/`)
2.5. Load data files (YAML/JSON/TOML from `data/` directory → `{{ data.filename }}` in templates)
3. Process each collection: walk content dir, parse frontmatter, **expand shortcodes**, render markdown to HTML, detect language from filename, resolve slugs/URLs, compute word count/reading time/excerpt/ToC, build translation map, sort
3b. Inject i18n context — compute `lang_prefix` (empty for default language, `"/{lang}"` for others), `default_language`, and `t` (UI strings merged from defaults + `data/i18n/{lang}.yaml`) into every template context
4. Render index page(s) — per-language if multilingual, with optional homepage content from `content/pages/index.md` or collection `index.md` for subdomain roots (with `redirect_to` support). Also renders: paginated collection indexes (with optional collection `index.md` content on page 1 + cached nav), non-paginated collection indexes (with optional collection `index.md` content + cached nav + `redirect_to`), 404 page, tag index + per-tag archive pages. Docs collections use `docs-index.html` template with sidebar nav and auto-generated section overview.
5. Generate RSS feed(s) — default language at `/feed.xml`, per-language at `/{lang}/feed.xml`
6. Generate sitemap — all items, with `xhtml:link` alternates for translations
7. Generate discovery files — per-language `llms.txt` and `llms-full.txt`
8. Output raw markdown alongside HTML (`slug.md` next to `slug.html`)
9. Generate search index — `search-index.json` (default lang), `/{lang}/search-index.json` (per-language)
10. Copy static files
11. Process images (resize to configured widths, generate WebP and AVIF variants)
12. Post-process HTML (rewrite `<img>` tags with srcset, `<picture>` for WebP, `loading="lazy"` — first image per page skipped for LCP)
13. Inject analytics scripts (and optional cookie consent banner) into all HTML files
14. Build subdomain sites — for each collection with `subdomain` set, create a synthetic config and run the full pipeline into `dist-subdomains/{name}/`

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

Any collection can optionally set `subdomain` to deploy to `{subdomain}.{base_domain}`. When set, the collection gets its own output dir (`dist-subdomains/{name}/`), own sitemap/RSS/robots.txt, and is excluded from main site output. Optional `subdomain_base_url` overrides the auto-derived URL (useful when `base_url` contains `www`). Optional `deploy_project` sets the Cloudflare/Netlify project for that subdomain.

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
# subdomain = "blog"          # optional: deploy to blog.{base_domain}
# subdomain_base_url = "https://blog.example.com"  # optional: explicit URL override
# deploy_project = "my-blog"  # optional: Cloudflare/Netlify project for subdomain

[build]
output_dir = "dist"
data_dir = "data"    # optional: directory for data files (YAML/JSON/TOML)
public_dir = "public" # optional: root-level files copied to dist/ without prefix
minify = true        # optional: strip CSS/JS comments + collapse whitespace
fingerprint = true   # optional: write name.<hash8>.ext + dist/asset-manifest.json
math = true          # optional: enable $inline$ and $$display$$ math rendering via KaTeX

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
avif = false               # generate AVIF variants (better compression than WebP)
avif_quality = 70          # AVIF quality (1-100, lower is fine — AVIF compresses better)

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
# extensions = ["tagged-events", "outbound-links"]  # Plausible script extensions

# Optional: contact form (omit for no contact form)
[contact]
provider = "formspree"     # "formspree", "web3forms", "netlify", "hubspot", "typeform"
endpoint = "xpznqkdl"     # provider-specific form ID / access key
# redirect = "/thank-you" # custom redirect after submission
# subject = "New inquiry"  # email subject prefix
# region = "na1"          # HubSpot only (na1 or eu1)
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
- `seite://docs` / `seite://docs/{slug}` — 15 embedded documentation pages (compiled into binary via `include_str!`)
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

**Background update check** (`src/update_check.rs`):
- Runs after every CLI command (except `self-update` and `mcp`)
- Checks `https://seite.sh/version.txt` at most once every 24 hours
- Caches result in `~/.seite/update-cache.json` (global, not per-project)
- Uses a 3-second HTTP timeout; silently swallows all errors
- Prints a one-liner via `human::info()` when a newer version is available

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
- **Release documentation**:
  - `build.rs` — generates `releases.md` from changelog entries at compile time (embedded into binary via `include_str!` in `src/docs.rs`). No committed generated file — `seite-sh/content/docs/releases.md` is gitignored
  - `scripts/prepare-release.sh` — scaffolds a changelog entry for the current Cargo.toml version with git log for reference
  - `scripts/generate-release-docs.sh` — generates `seite-sh/content/docs/releases.md` from changelog entries (used by CI `deploy-site` job to build the docs website)
- **Release flow**: run `scripts/prepare-release.sh` → fill in changelog entry → bump version in `Cargo.toml` → commit → push to `main` → CI validates changelog exists → auto-tag → auto-release → auto-publish crate → auto-deploy docs (CI generates releases.md before site build)
- **CI gates**: `release-tag.yml` blocks tag creation if changelog entry is missing for the version being tagged
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
- **MANDATORY before every commit:** run `cargo fmt --all && cargo clippy && cargo test` and verify all three pass. Never skip this step — CI will reject unformatted code. Run formatting *last* (after all edits are done) so no subsequent file writes undo it.
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
- `<link rel="icon" href="/favicon.ico">` — favicon (user places `favicon.ico` in `public/`)
- `<link rel="canonical">` — always `{{ site.base_url }}{{ page.url | default(value='/') }}`
- `<meta name="description">` — use `{{ page.description | default(value=site.description) }}` (per-page first, site fallback)
- `og:type` — `"article"` when `page.collection` is set, `"website"` for index/homepage
- `og:url`, `og:title`, `og:description`, `og:site_name`, `og:locale`
- `og:image` — conditional on `page.image`, must be absolute URL. Use `{% set _abs_image = page.image %}{% if not page.image is starting_with("http") %}{% set _abs_image = site.base_url ~ page.image %}{% endif %}` to handle both absolute URLs and `/static/…` paths
- `og:image:width` (`1200`) and `og:image:height` (`630`) — standard social preview dimensions, emitted alongside `og:image`
- `article:published_time` — emitted when `page.collection` and `page.date` are set
- `article:modified_time` — emitted when `page.collection` and `page.updated` are set
- `twitter:card` — `"summary_large_image"` when `page.image` exists, `"summary"` otherwise
- `twitter:title`, `twitter:description`, `twitter:image` (same absolutization as `og:image`, using `_abs_image`)

**Structured data (JSON-LD):**
- Posts (`page.collection == 'posts'`): `BlogPosting` with `headline`, `description`, `datePublished`, `dateModified` (from `page.updated`), `author`, `publisher`, `url`
- Docs/pages (`page.collection` set but not posts): `Article` with same fields minus dates
- Index/homepage: `WebSite` with `name`, `description`, `url`
- `BreadcrumbList` — emitted on all collection pages (Home → Collection → Page). Uses `{% set _bc_col_url = site.base_url ~ lang_prefix ~ "/" ~ page.collection %}` for the collection URL

**Discovery links:**
- `<link rel="alternate" type="application/rss+xml">` — RSS feed
- `<link rel="alternate" type="text/plain" title="LLM Summary" href="/llms.txt">` — LLM discovery
- `<link rel="alternate" type="text/markdown" title="Markdown">` — markdown version (when `page.url` is set, must include `title` attribute)

**AI crawler management (robots.txt):**
- AI search crawlers (ChatGPT-User, OAI-SearchBot, PerplexityBot) — `Allow: /` so content appears in AI-generated answers
- AI training crawlers (GPTBot, Google-Extended, CCBot, Bytespider) — `Disallow: /` to prevent training use without blocking search visibility

**Per-page robots:**
- `<meta name="robots" content="{{ page.robots }}">` — only emitted when `robots:` is set in frontmatter
- Use `robots: "noindex"` in frontmatter for pages that should not be indexed

**Frontmatter fields for SEO:**
- `description:` — page-specific description for meta/OG/Twitter/JSON-LD
- `image:` — absolute URL or `/static/…` path to social preview image (automatically absolutized in themes)
- `updated:` — last-modified date (YYYY-MM-DD) for JSON-LD `dateModified`, `article:modified_time` OG tag, and sitemap `lastmod` (takes priority over `date`)
- `robots:` — per-page robots directive (e.g., `"noindex"`, `"noindex, nofollow"`)

### Shortcodes
- Two syntax forms: inline `{{< name(args) >}}` (raw HTML) and body `{{% name(args) %}} markdown {{% end %}}`
- Named args only: `key="string"`, `key=42`, `key=3.14`, `key=true`
- Shortcodes expanded **before** `markdown_to_html()` — output goes through the markdown pipeline
- `raw_body` on `ContentItem` stays unexpanded (for `.md` output and `llms-full.txt`)
- Built-in shortcodes: `youtube`, `vimeo`, `gist`, `callout` (body), `figure`, `contact_form`
- User-defined shortcodes: Tera templates in `templates/shortcodes/*.html`
- User shortcodes override built-ins with the same name
- Shortcodes inside fenced code blocks and inline code spans are NOT expanded
- `ShortcodeRegistry` uses a separate Tera instance (not the page template Tera)
- All 6 bundled themes include CSS for `.video-embed`, `.callout-*`, `figure`/`figcaption`, `.contact-form`
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
The build pipeline checks for `{collection.name}-index.html` before falling back to `index.html`. This applies to both paginated and non-paginated collections. Changelog, roadmap, and docs ship dedicated index templates; any collection can override its index this way. The docs collection uses `docs-index.html` which includes sidebar navigation and auto-generated section overview.

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

### Contact Forms

Built-in contact form support via the `{{< contact_form() >}}` shortcode and `[contact]` config section.

**Two integration patterns:**
- **HTML POST** (pure HTML, no JS): Formspree, Web3Forms, Netlify Forms — renders a `<form>` with provider-specific action URL, hidden fields, and honeypot spam protection
- **JS embed** (loads external script): HubSpot, Typeform — renders a container `<div>` with provider SDK script

**Config:** `[contact]` section in `seite.toml` with `provider`, `endpoint`, optional `redirect`, `subject`, `region` (HubSpot only).

**Shortcode args:** `name_label`, `email_label`, `message_label`, `submit_label` (label overrides), `subject`, `redirect` (per-instance overrides), `height` (Typeform only).

**CLI:** `seite contact setup` (interactive/non-interactive), `seite contact status`, `seite contact remove`. Deploy-aware: auto-suggests Netlify Forms when deploy target is Netlify.

**Init flow:** `seite init --contact-provider formspree --contact-endpoint xpznqkdl` or interactive prompt. Creates `content/pages/contact.md` with shortcode when pages collection exists.

**Deploy integration:** `seite deploy --setup` offers contact form configuration after deploy setup.

**Upgrade:** `seite upgrade` appends Contact Forms documentation to CLAUDE.md for existing sites.

**i18n:** Labels use `{{ t.contact_name }}`, `{{ t.contact_email }}`, `{{ t.contact_message }}`, `{{ t.contact_submit }}`. Override per language in `data/i18n/{lang}.yaml`.

**Files:** `src/config/mod.rs` (ContactProvider, ContactSection), `src/shortcodes/builtins/contact_form.html`, `src/cli/contact.rs`, `src/build/mod.rs` (shortcode context), `src/scaffold/contact-form.md`, `seite-sh/content/docs/contact-forms.md`, `src/themes/*.tera` (CSS)

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

### Collection Index Pages

Any collection can have a custom index page via `content/{collection}/index.md`. This works identically to the homepage pattern: the `index.md` content is extracted from the collection (so it doesn't appear as a regular item) and injected into the collection's index template context as `{{ page.content }}`.

This powers:
- **Collection landing pages**: `content/docs/index.md` → custom content at `/docs/`
- **Subdomain root pages**: when a collection has `subdomain` set, its `index.md` becomes the subdomain root content (e.g., `docs.example.com/`)
- **Paginated collections**: `index.md` content appears only on page 1
- **Redirect to a specific page**: set `extra.redirect_to` in the index.md frontmatter to generate an instant redirect (e.g., `/docs/` → `/docs/getting-started`)

All frontmatter fields from the `index.md` are available in the template context (`page.title`, `page.description`, `page.extra`, etc.).

**Docs index template**: The docs collection ships a dedicated `docs-index.html` template that renders sidebar navigation alongside the index content. With `index.md`, it shows custom content with sidebar; without it, it auto-generates a section overview grouped by subdirectory.

**Sidebar nav on collection indexes**: The `{{ nav }}` variable (pre-built for nested collections like docs) is passed to all collection index templates — both paginated and non-paginated. This is cached via `collection_nav_cache` during individual page rendering and reused in index rendering steps.

**Redirect support**: Collection index pages support `extra.redirect_to` in frontmatter. When set, the build pipeline writes an instant HTML redirect (meta refresh + JavaScript) instead of rendering the template. Works for both regular collection indexes and subdomain roots.

### Theme Builder Skill (Claude Code)

The `/theme-builder` skill (`.claude/skills/theme-builder/SKILL.md`) guides Claude Code through a structured 4-phase workflow when users ask to create or customize themes:

1. **Understand the Vision** — Ask about site type, audience, mood, references/screenshots, colors, typography, layout, must-haves
2. **Generate the Theme** — Write `templates/base.html` with all SEO/GEO, search, pagination, i18n, accessibility, and collection-specific CSS requirements
3. **Preview and Iterate** — Build, preview, and refine based on user feedback
4. **Save and Export** — Offer to export the theme via `seite theme export`

The skill is scaffolded unconditionally by `seite init` (every site needs themes) and upgraded via `seite upgrade` with version tracking (`# seite-skill-version: N` in frontmatter).

Design prompts for theme directions are in `src/scaffold/design-prompts.md`.

### Brand Identity Builder Skill (Claude Code)

The `/brand-identity` skill (`.claude/skills/brand-identity/SKILL.md`) guides Claude Code through a 5-phase workflow when users want to create a visual identity:

1. **Understand the Brand** — Conversational dialogue about identity, personality, and visual preferences
2. **Design the Color Palette** — 6-8 colors with clear roles, saved to `data/brand.yaml`
3. **Design the Logo** — SVG logo (lettermark, geometric, wordmark, or combination)
4. **Generate the Favicon** — Simplified SVG optimized for small sizes
5. **Apply to Theme (Optional)** — Integrate colors into `templates/base.html` or suggest `/theme-builder`

The skill is scaffolded unconditionally by `seite init` and upgraded via `seite upgrade` with version tracking (`# seite-skill-version: N` in frontmatter).

Output files: `static/logo.svg`, `public/favicon.svg`, `public/favicon.ico` (optional), `data/brand.yaml`.
