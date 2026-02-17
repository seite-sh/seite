# page — Static Site Generator with LLM Integration

## What This Is

`page` is a Rust CLI static site generator designed to be AI-native. Content and templates are structured for LLM generation and consumption. Sites ship with `llms.txt`, `llms-full.txt`, and markdown versions of every page alongside the HTML.

## Quick Commands

```bash
cargo build          # Build the binary
cargo test           # Run all tests (8 unit + 16 integration)
cargo run -- init mysite --title "My Site" --description "" --deploy-target github-pages --collections posts,docs,pages
cargo run -- build   # Build site from page.toml in current dir
cargo run -- serve   # Dev server with REPL (live reload, port auto-increment)
cargo run -- new post "My Post" --tags rust,web
cargo run -- new doc "Getting Started"
cargo run -- ai "write about Rust" --type post
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
  credential.rs        File-based API key storage (~/.config/page/credentials.toml)
  themes.rs            4 bundled themes (default, minimal, dark, docs)
  ai/mod.rs            LLM client (Claude + OpenAI APIs)
  build/
    mod.rs             9-step build pipeline
    markdown.rs        pulldown-cmark wrapper
    feed.rs            RSS generation
    sitemap.rs         XML sitemap generation
    discovery.rs       robots.txt, llms.txt, llms-full.txt
  cli/
    mod.rs             Cli struct + Command enum (8 subcommands)
    init.rs            Interactive project scaffolding
    new.rs             Create content files
    build.rs           Build command
    serve.rs           Dev server + interactive REPL
    deploy.rs          Deploy command
    auth.rs            Browser-based API key login
    ai.rs              AI content/template generation
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
  integration.rs       16 integration tests using assert_cmd + tempfile
```

### Build Pipeline (9 steps)

1. Clean output directory (`dist/`)
2. Load Tera templates (user-provided + embedded defaults)
3. Process each collection: walk content dir, parse frontmatter + markdown, resolve slugs/URLs, sort
4. Render index page with listed collections
5. Generate RSS feed (from `has_rss` collections only)
6. Generate sitemap (all items)
7. Generate discovery files (robots.txt, llms.txt, llms-full.txt)
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
    url: String,                   // e.g., "/posts/hello-world"
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

[ai]
default_provider = "claude"
```

### Dev Server

- `page serve` starts HTTP server + file watcher in background threads
- Returns `ServerHandle` (stop with `Drop` or `.stop()`)
- Interactive REPL with commands: new, ai, login, theme, build, status, stop
- Live reload via `/__livereload` polling endpoint + injected `<script>`
- Auto-increments port if default (3000) is taken

### Credential Storage

File-based at `~/.config/page/credentials.toml` (or platform equivalent via `dirs` crate).
Permissions set to `0600` on unix. Stores provider → API key mappings.

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
- Template variables: `{{ site.title }}`, `{{ page.title }}`, `{{ page.content | safe }}`, `{{ collections }}`
- Embedded defaults in `src/templates/mod.rs`; user templates in `templates/` override them

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

## Roadmap

Tasks are ordered by priority. Mark each `[x]` when complete.

### Next Up

- [ ] **Syntax highlighting** — Add `syntect` for code block highlighting in markdown output. Update `build/markdown.rs` to use syntect's HTML generation for fenced code blocks with language annotations.

- [ ] **Navigation generation for docs** — Auto-generate sidebar nav from docs collection directory structure. The `docs` theme has a sidebar but no actual nav links. Should walk the docs tree and produce a nested list.

- [ ] **Asset pipeline** — CSS/JS minification, image optimization, cache-busting with fingerprinted filenames. Consider `lightningcss` for CSS and a simple hash-based renaming for fingerprints.

- [ ] **`page ai` improvements** — The AI system works but needs refinement:
  - Better prompts that include site context (existing content, collection structure)
  - Streaming output so user sees progress
  - Support for editing existing content ("rewrite this post to be more concise")
  - AI-powered content suggestions based on site structure

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
- [x] AI content + template generation (Claude + OpenAI)
- [x] Discovery files (robots.txt, llms.txt, llms-full.txt)
- [x] Bundled themes (default, minimal, dark, docs)
- [x] Interactive REPL in serve mode
- [x] Browser-based auth flow with file-based credential storage
- [x] Live reload dev server with port auto-increment
- [x] Clean URL output pattern (slug.html / slug.md)
- [x] RSS feed (posts only) + XML sitemap (all collections)
- [x] Nested docs support (docs/guides/setup.md → /docs/guides/setup)
- [x] Draft exclusion with --drafts flag
- [x] Deploy to GitHub Pages + Cloudflare Pages
