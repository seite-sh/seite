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
  build/               15-step build pipeline (mod.rs), analytics, base_path, code_copy, links, markdown, feed, sitemap, discovery, images, math
  docs.rs              Embedded docs (15 pages from seite-sh/content/docs/)
  meta.rs              Project metadata (.seite/config.json)
  mcp/                 MCP server (JSON-RPC over stdio): mod.rs, resources.rs, tools.rs
  cli/                 16 subcommands: init, new, build, serve, deploy, agent, theme, mcp, workspace, upgrade, contact, collection, skill, self_update, completions, perf
  update_check.rs      Background update check (24h cache)
  scaffold/            Static markdown for generated CLAUDE.md + .claude/rules/ (include_str!)
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

[build]
output_dir = "dist"
minify = true
fingerprint = true
math = true              # KaTeX

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
- `output::human::success()`, `info()`, `error()` for terminal. `CommandOutput` trait for `--json`.

### Versioning
- Source of truth: `Cargo.toml` (semver). **Every code change must bump version.**
- PATCH: bug fixes, refactors. MINOR: new features/commands/config. MAJOR: breaking changes.
- **Always add a changelog entry** in `seite-sh/content/changelog/YYYY-MM-DD-vX-Y-Z.md` when bumping the version. Missing entries crash the deploy (`build.rs` embeds them into the binary).

### Testing
- `cargo fmt --all && cargo clippy && cargo test` before every commit
- Integration tests: `assert_cmd` + `tempfile`. Naming: `test_{command}_{behavior}`

### Adding a User-Facing Feature
1. Config model → `src/config/mod.rs`
2. Build pipeline → `src/build/mod.rs`
3. Init scaffolding → `src/cli/init.rs`
4. Docs → `seite-sh/content/docs/*.md` (compiled into binary)
5. MCP compliance → verify visibility via `seite://config` and `seite_lookup_docs`
6. CLAUDE.md → update this file
7. Tests → unit + integration
8. Deploy fixtures → update `src/deploy/mod.rs` if SiteConfig changed
9. i18n → `{{ t.key }}` for UI text, `{{ lang_prefix }}` for links

### Generated Site Structure
- `seite init` creates lean CLAUDE.md + `.claude/rules/*.md` (path-scoped context)
- `seite upgrade` adds rules files for existing sites (non-destructive)
- Rules files use YAML frontmatter with `paths:` for automatic loading
- Skills in `.claude/skills/` (theme-builder, brand-identity, landing-page)
- `rules_file()` helper in `src/cli/init.rs` wraps scaffold content with frontmatter

## Context Rules

Detailed reference guides are in `.claude/rules/` and load automatically when working with matching files:
- `build-pipeline.md` — 15-step pipeline, content model, data files, shortcodes
- `seo-guardrails.md` — SEO/GEO meta tags, JSON-LD, robots.txt, i18n in themes
- `templates-themes.md` — template variables, blocks, 6 bundled themes, gallery
- `collections-content.md` — 6 presets, i18n, trust center, contact forms, changelog/roadmap
- `cli-commands.md` — subcommands, agent, dev server, workspace, deploy, skills
- `mcp-server.md` — resources, tools, architecture
- `release-deploy.md` — version flow, CI, installers, self-update, upgrade system
- `testing.md` — test patterns, CI gates
- `documentation.md` — docs site, scaffold files, what to update
