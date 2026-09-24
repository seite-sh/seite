# seite — Static Site Generator with LLM Integration

## What This Is

`seite` is a Rust CLI static site generator designed to be AI-native. Content and templates are structured for LLM generation and consumption. Sites ship with `llms.txt`, `llms-full.txt`, and markdown versions of every page alongside the HTML.

The `seite agent` command spawns Claude Code as a subprocess with full site context — no API keys needed, uses the user's Claude Code subscription directly.

## Quick Commands

```bash
cargo build          # Build the binary
cargo test           # Run all tests
cargo fmt --all      # Format — CI enforces this
cargo clippy         # Lint — must be zero warnings
cargo run -- init mysite --title "My Site" --collections posts,docs,pages
cargo run -- build   # Build site from seite.toml
cargo run -- serve   # Dev server with REPL (live reload)
cargo run -- serve --open  # Dev server + open browser
cargo run -- serve --host 0.0.0.0  # Bind to all interfaces
cargo run -- new post "My Post" --tags rust,web
cargo run -- agent "create a blog post about Rust"
cargo run -- theme create "coral brutalist with lime accents"
cargo run -- deploy  # Commit, push, build, and deploy
cargo run -- skill install seomachine
cargo run -- upgrade # Upgrade project config to current binary
cargo run -- self-update
cargo run -- completions bash  # Generate shell completions
```

## Module Map

```
src/
  main.rs              CLI entrypoint (clap dispatch)
  lib.rs, error.rs     Module declarations, PageError enum (thiserror)
  themes.rs            10 bundled themes + src/themes/*.tera
  shortcodes/          ShortcodeRegistry, parser, builtins (youtube, vimeo, gist, callout, figure, contact_form)
  build/               build pipeline (mod.rs), analytics, base_path, code_copy, links, markdown, feed, sitemap, discovery, images, math, mermaid
  docs.rs              Embedded docs (15 pages from seite-sh/content/docs/)
  i18n.rs              Language-map resolution + `i18n`/`localize` Tera filter (per-language data values)
  meta.rs              Project metadata (.seite/config.json)
  mcp/                 MCP server (JSON-RPC over stdio): mod.rs, resources.rs, tools.rs
  cli/                 subcommands: init, new, build, serve, deploy, agent, theme, mcp, workspace, upgrade, contact, collection, access, skill, self_update, completions, perf, telemetry
  update_check.rs      Background update check (24h cache)
  scaffold/            Static markdown for generated AGENTS.md + .claude/rules/ (include_str!)
  config/              SiteConfig, CollectionConfig, defaults
  data/                Data file loading (YAML/JSON/TOML)
  content/             Frontmatter parsing, ContentItem, slug generation
  deploy/              GitHub Pages + Cloudflare + Netlify
  workspace/           Multi-site workspaces (config, build, server, deploy)
  output/              CommandOutput trait, human (colored), json
  server/              tiny_http dev server, file watcher, live reload
  templates/           Tera template loading with embedded defaults
tests/integration.rs   Integration tests (assert_cmd + tempfile)
build.rs               Generates releases.md from changelog at compile time
```

## Config (seite.toml)

```toml
[site]
title = "My Site"
description = ""
base_url = "http://localhost:3000"
language = "en"

[[collections]]
name = "posts"
# subdomain = "blog"     # optional: deploy separately
# private = true         # optional: exclude from discovery
# access_group = "members" # optional: password group when [access] is enabled

[access]                 # optional; Cloudflare Pages only
mode = "password"
session_hours = 168

[build]
output_dir = "dist"
minify = true
fingerprint = true
math = true              # KaTeX
mermaid = true           # Mermaid diagrams (```mermaid fences, client-side)

[deploy]
target = "github-pages"  # or "cloudflare" or "netlify"
auto_commit = true

[languages.es]
title = "Mi Sitio"

[images]
widths = [480, 800, 1200]
quality = 80
webp = true

[trust]
company = "Acme Corp"
frameworks = ["soc2", "iso27001"]

[analytics]
provider = "google"
id = "G-XXXXXXXXXX"

[contact]
provider = "formspree"
endpoint = "xpznqkdl"
```

## Patterns and Conventions

### Error Handling
- Library code: `crate::error::Result<T>` (PageError + thiserror). Never `unwrap()`.
- CLI commands: `anyhow::Result<()>`

### Output
- Human-readable output goes through `output::human::{success,info,warning,error,header}` — never raw `println!`.
- `--json` mode: set the command's payload with `output::json::set_data()`; the top-level `{"ok","command","data","warnings"}` / `{"ok":false,"error":{...}}` document is assembled and printed by `main.rs`. `CommandOutput` trait covers older per-command JSON.
- Interactive prompts always go through `src/cli/prompt.rs`, never `dialoguer` directly — it degrades safely under `--yes`/`SEITE_YES=1` or a non-TTY (default if there is one, else an error naming the missing flag).

### Versioning
- Source of truth: `Cargo.toml` (semver). **Every code change must bump version.**
- PATCH: bug fixes, refactors. MINOR: new features/commands/config. MAJOR: breaking changes.
- **Always add a changelog entry** in `seite-sh/content/changelog/YYYY-MM-DD-vX-Y-Z.md` when bumping the version. Missing entries crash the deploy (`build.rs` embeds them into the binary).

### Testing
- `cargo fmt --all && cargo clippy && cargo test` before every commit
- **Check code coverage before committing**: `cargo tarpaulin --out Html` (or `cargo llvm-cov`). Coverage must not regress.
- Integration tests: `assert_cmd` + `tempfile`. Naming: `test_{command}_{behavior}`

### Adding a User-Facing Feature
1. Config model → `src/config/mod.rs`
2. Build pipeline → `src/build/mod.rs`
3. Init scaffolding → `src/cli/init.rs`
4. Docs → `seite-sh/content/docs/*.md` (compiled into binary)
5. MCP compliance → verify visibility via `seite://config` and `seite_lookup_docs`
6. AGENTS.md → update this file
7. Tests → unit + integration
8. Deploy fixtures → update `src/deploy/mod.rs` if SiteConfig changed
9. i18n → `{{ t.key }}` for UI text, `{{ lang_prefix }}` for links, `{{ value | i18n(lang=lang) }}` for per-language data prose (language maps; see `src/i18n.rs`)

### Generated Site Structure
- `seite init` creates lean AGENTS.md, a CLAUDE.md import shim, and `.claude/rules/*.md` (path-scoped context)
- `seite upgrade` adds rules files for existing sites (non-destructive)
- Rules files use YAML frontmatter with `paths:` for automatic loading
- Skills in `.claude/skills/` (theme-builder, brand-identity, landing-page)
- `rules_file()` helper in `src/cli/init.rs` wraps scaffold content with frontmatter

## Context Rules

Detailed reference guides are in `.claude/rules/` and load automatically when working with matching files:
- `build-pipeline.md` — build pipeline steps, content model, data files, shortcodes
- `seo-guardrails.md` — SEO/GEO meta tags, JSON-LD, robots.txt, i18n in themes
- `templates-themes.md` — template variables, blocks, 10 bundled themes, gallery
- `collections-content.md` — 6 presets, i18n, trust center, contact forms, changelog/roadmap
- `cli-commands.md` — subcommands, agent, dev server, workspace, deploy, skills
- `mcp-server.md` — resources, tools, architecture
- `release-deploy.md` — version flow, CI, installers, self-update, upgrade system
- `testing.md` — test patterns, CI gates
- `documentation.md` — docs site, scaffold files, what to update
