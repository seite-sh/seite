# page — Static Site Generator with LLM Integration

## What This Is

`page` is a Rust CLI static site generator designed to be AI-native. Content and templates are structured for LLM generation and consumption. Sites ship with `llms.txt`, `llms-full.txt`, and markdown versions of every page alongside the HTML.

The `page agent` command spawns Claude Code as a subprocess with full site context — no API keys needed, uses the user's Claude Code subscription directly.

## Quick Commands

```bash
cargo build          # Build the binary
cargo test           # Run all tests (13 unit + 26 integration)
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
cargo run -- deploy
```

## Architecture

### Module Map

```
src/
  main.rs              CLI entrypoint (clap dispatch)
  lib.rs               Module declarations
  error.rs             PageError enum (thiserror)
  themes.rs            4 bundled themes (default, minimal, dark, docs)
  build/
    mod.rs             9-step build pipeline
    markdown.rs        pulldown-cmark wrapper
    feed.rs            RSS generation
    sitemap.rs         XML sitemap generation
    discovery.rs       robots.txt, llms.txt, llms-full.txt
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
  content/mod.rs       Frontmatter parsing, ContentItem, slug generation
  deploy/mod.rs        GitHub Pages (git push) + Cloudflare (wrangler)
  output/
    mod.rs             CommandOutput trait
    human.rs           Colored terminal output
    json.rs            JSON output mode
  server/mod.rs        tiny_http dev server, file watcher, live reload
  templates/mod.rs     Tera template loading with embedded defaults
tests/
  integration.rs       26 integration tests using assert_cmd + tempfile
```

### Build Pipeline (9 steps)

1. Clean output directory (`dist/`)
2. Load Tera templates (user-provided + embedded defaults)
3. Process each collection: walk content dir, parse frontmatter + markdown, detect language from filename, resolve slugs/URLs, build translation map, sort
4. Render index page(s) — per-language if multilingual, with optional homepage content from `content/pages/index.md`
5. Generate RSS feed(s) — default language at `/feed.xml`, per-language at `/{lang}/feed.xml`
6. Generate sitemap — all items, with `xhtml:link` alternates for translations
7. Generate discovery files — per-language `llms.txt` and `llms-full.txt`
8. Output raw markdown alongside HTML (`slug.md` next to `slug.html`)
9. Copy static files

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
    description: Option<String>,
    slug: Option<String>,          // override auto-generated slug
    tags: Vec<String>,
    draft: bool,                   // excluded from build unless --drafts
    template: Option<String>,      // override collection default template
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

[deploy]
target = "github-pages"  # or "cloudflare"

# Optional: multi-language support (omit for single-language sites)
[languages.es]
title = "Mi Sitio"              # optional language-specific overrides
description = "Un sitio estático"

[languages.fr]
title = "Mon Site"
```

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

### Themes

4 bundled themes compiled into the binary (no downloads):
- `default` — Clean, system fonts
- `minimal` — Typography-first, serif
- `dark` — Dark mode
- `docs` — Sidebar layout for documentation

Each theme is a `base.html` Tera template with inline CSS.

## Patterns and Conventions

### Error Handling
- Library code returns `crate::error::Result<T>` (uses `PageError` with thiserror)
- CLI commands return `anyhow::Result<()>` for convenience
- Never `unwrap()` in library code; `unwrap()` only acceptable in tests and CLI entry points

### Output
- Use `output::human::success()`, `info()`, `error()` for terminal output
- Implement `CommandOutput` trait for structured output (supports `--json` flag)

### Testing
- Integration tests use `assert_cmd::Command` + `tempfile::TempDir`
- Helper: `init_site(tmp, name, title, collections)` scaffolds a site in a temp dir
- Test naming: `test_{command}_{behavior}` (e.g., `test_build_excludes_drafts_by_default`)
- All tests must pass before committing: `cargo test`

### CLI
- clap 4.5 with derive macros
- Each subcommand has its own file in `src/cli/` with `{Command}Args` struct + `pub fn run(args) -> anyhow::Result<()>`
- Interactive prompts use `dialoguer` (only when CLI args are not provided)

### Templates
- Tera (Jinja2-compatible) templates
- All templates extend `base.html`
- Template variables: `{{ site.title }}`, `{{ page.title }}`, `{{ page.content | safe }}`, `{{ collections }}`, `{{ lang }}`, `{{ translations }}`, `{{ nav }}`
- Embedded defaults in `src/templates/mod.rs`; user templates in `templates/` override them
- All bundled themes include hreflang tags and language switcher UI when `translations` is non-empty

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

## Roadmap

Tasks are ordered by priority. Mark each `[x]` when complete.

### Next Up

- [ ] **Asset pipeline** — CSS/JS minification, image optimization, cache-busting with fingerprinted filenames. Consider `lightningcss` for CSS and a simple hash-based renaming for fingerprints.

- [ ] **Deploy improvements** — Current deploy is basic:
  - GitHub Pages: Add GitHub Actions workflow generation
  - Cloudflare: Better error messages, auto-detect project name
  - Add `--dry-run` flag
  - Netlify support

- [ ] **Search** — Client-side search using a generated JSON index. Build step produces `search-index.json`, JS in templates provides search UI.

- [ ] **Image handling** — Auto-resize images, generate srcset, lazy loading attributes, WebP conversion.

- [ ] **Pagination** — For collections with many items, generate paginated index pages (`/posts/page/2`, etc.).

### Done

- [x] Collections system (posts, docs, pages with presets)
- [x] Build pipeline with markdown output alongside HTML
- [x] AI agent via Claude Code (`page agent` spawns `claude` subprocess with site context)
- [x] Discovery files (robots.txt, llms.txt, llms-full.txt)
- [x] Bundled themes (default, minimal, dark, docs)
- [x] Interactive REPL in serve mode
- [x] Live reload dev server with port auto-increment
- [x] Clean URL output pattern (slug.html / slug.md)
- [x] RSS feed (posts only) + XML sitemap (all collections)
- [x] Nested docs support (docs/guides/setup.md → /docs/guides/setup)
- [x] Draft exclusion with --drafts flag
- [x] Deploy to GitHub Pages + Cloudflare Pages
- [x] Syntax highlighting (syntect, inline styles, base16-ocean.dark theme)
- [x] Docs sidebar navigation (auto-generated from collection items, grouped by directory)
- [x] Claude Code scaffolding (`page init` creates `.claude/settings.json` + `CLAUDE.md`)
- [x] Homepage as special page (`content/pages/index.md` → custom homepage content)
- [x] Multi-language (i18n) support — filename-based translations, per-language URLs, hreflang tags, language switcher, per-language RSS/sitemap/discovery files
