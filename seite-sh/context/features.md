# seite — Features & Benefits

## Core Value Propositions

### 1. **Single Rust Binary**
- **Feature**: One binary, zero runtime dependencies. Install via curl, cargo, or download.
- **Benefit**: No Node.js, no version managers, no dependency hell. Works identically everywhere.
- **Conversion Angle**: "Install in 10 seconds. Build your site in under 1 second."

### 2. **Agents Run the Site, for Free**
- **Feature**: `seite init` sets up seven coding agents (Claude Code, Codex, OpenCode, Cursor, Antigravity CLI, Gemini CLI, GitHub Copilot; pick with `--agents`): a compact `AGENTS.md` (~4 KB) with `CLAUDE.md`/`GEMINI.md` shims, path-scoped rules, one routed `/seite` skill (`$seite` in Codex: check, new, audit, preview, build, deploy, theme, brand, landing, collection, upgrade, hooks), and two-tier check hooks (`seite check --file` after each edit, full `seite check` at turn end; errors go back to the agent, warnings never block; `seite hooks on|off|status`). `seite context` orients an agent once per session. Every MCP capability has a CLI equivalent with `--json` (`seite page`, `search`, `stats`, `templates`, `frontmatter`, `docs`, `explain`, `preview`). No API keys or paid services; free agents (Gemini CLI, OpenCode with free/local models) work. Also installable without `seite init`: `npx skills add seite-sh/seite` or the Claude Code plugin. `seite agent --with <agent>` is a thin launcher.
- **Benefit**: Your coding agent builds, checks, previews and deploys the site end to end and fixes its own errors before it says "done".
- **Conversion Angle**: "The static site generator your coding agent can run end to end, for free."

### 3. **Triple Output (HTML + Markdown + LLM files)**
- **Feature**: Builds HTML pages, raw markdown copies, llms.txt, and llms-full.txt for every page.
- **Benefit**: Content is readable by browsers, search engines, and AI models — maximizing discoverability.
- **Conversion Angle**: "Ship for people, search engines, and AI models — all from one build."

### 4. **11 Bundled Themes**
- **Feature**: 4 starters (default, docs, landing, blank) and a 7-theme gallery (minimal, dark, brutalist, bento, terminal, magazine, academic), compiled into the binary. seite owns the SEO head through `{% include "seite/…" %}` partials; themes own layout and CSS. Design your own with `blank` + `seite theme new`, `/seite theme <description>` (or impeccable). Tailwind v4 without Node (`[build.tailwind]`).
- **Benefit**: Instant professional design with no downloads or CDN. Or describe one and let AI build it.
- **Conversion Angle**: "Pick a theme or describe one. Ship in minutes, not days."

### 5. **Collection System with Presets**
- **Feature**: Posts (blog with RSS), docs (nested sidebar nav), pages, changelog, roadmap, trust center — each with templates, URL patterns, and feeds.
- **Benefit**: Common site patterns work out of the box. No config ceremony for standard use cases.
- **Conversion Angle**: "Blog, docs, changelog, roadmap — all built-in. One command to add each."

### 6. **SEO + GEO Optimized by Default**
- **Feature**: Canonical URLs, Open Graph, Twitter Cards, JSON-LD (BlogPosting/Article/WebSite + BreadcrumbList), hreflang, robots.txt with AI crawler controls and Content Signals, sitemaps, llms.txt, `/sitemap.md`, generated 1200x630 social cards (`[og] generate = true`), and `seite audit`: a free, offline SEO/GEO/accessibility/content/performance audit with a score and stable codes.
- **Benefit**: Every page is search-engine and AI-engine optimized without manual configuration.
- **Conversion Angle**: "SEO best practices aren't optional features — they're the default."

### 7. **One-Command Deploy**
- **Feature**: GitHub Pages, Cloudflare Pages, Netlify — built-in with pre-flight checks, auto-commit, preview deploys on non-main branches (Cloudflare Pages and Netlify). Collections can deploy to their own subdomain (Cloudflare/Netlify). `private = true` keeps a collection out of every discovery surface (sitemap, llms.txt, search, feeds), and on Cloudflare Pages `[access]` adds built-in password protection per path or subdomain with separate password groups (`seite access set-password`); seite sets and verifies Fail closed on protected Pages projects.
- **Benefit**: One command to go from local to live. No CI/CD config needed. Gated docs, investor pages, or a trust hub without a separate auth service.
- **Conversion Angle**: "`seite deploy` — that's it."

### 8. **Multi-language Support**
- **Feature**: Filename-based translations (`about.es.md`), per-language URLs/RSS/sitemaps/search, hreflang tags, UI string system. Data file values can carry per-language variants (`{ en: ..., de: ... }`) resolved with the `i18n` filter.
- **Benefit**: Fully internationalized sites with zero overhead for single-language sites.
- **Conversion Angle**: "Add `.es.md` to any file. seite handles the rest."

## Technical Features

### Build & Performance
- **Sub-second builds**: Rust-powered, typically < 0.5s for 50+ pages
- **Image processing**: Resize, WebP, AVIF, srcset — automatic
- **Math rendering**: KaTeX for inline ($) and display ($$) math
- **Mermaid diagrams**: `mermaid = true` renders fenced ```mermaid blocks client-side, no shortcode needed
- **Syntax highlighting**: syntect with theme support
- **Minification**: CSS/JS comment stripping + whitespace collapse
- **Asset fingerprinting**: Content-hash filenames for cache busting

### Content Management
- **Shortcodes**: YouTube, Vimeo, Gist, callouts, figures, contact forms + custom user-defined
- **Data files**: YAML/JSON/TOML from `data/` directory, available in all templates
- **Frontmatter**: YAML with drafts, weights, custom templates, arbitrary extras
- **Full-text search**: `[build] search = "full"` builds a sharded full-text index that works on any static host (no service, no API key)
- **Taxonomies, series, related pages**: `[[taxonomies]]` (e.g. categories), `series:` with part navigation, `page.related`
- **Typed frontmatter schemas**: `[collections.fields.<name>]` with types, required, enums; checked by build, check and the after-edit hook
- **Scheduled publishing**: future-dated posts stay hidden until their day (`[build] schedule = true`); `expires:` takes posts down
- **Importers**: `seite import wordpress|ghost|jekyll|hugo` keeps old URLs working as `aliases:`
- **Git-based CMS**: `seite cms setup pages-cms|decap`; comments (giscus/utterances) and newsletter (Buttondown/Kit) shortcodes

### Developer Experience
- **Dev server**: Live reload (SSE), a build-error overlay in the browser, browser errors printed to the terminal; `seite preview` reports the running server's URL, last build and browser errors
- **Workspaces**: Multi-site management from a single directory
- **Self-update**: `seite self-update` pulls latest from GitHub Releases
- **Project upgrades**: `seite upgrade` adds new features non-destructively
- **Site validation**: `seite check` validates config, frontmatter, shortcodes, data files, templates, internal links and assets in one pass with compiler-style `file:line` diagnostics (`--strict`, `--json`, `--file`, `--external` for external links); `seite explain <code>` and the error reference at /docs/errors
- **Agent-friendly CLI**: real `--json` output on every command, `-y/--yes`, no-TTY-safe prompts, `seite serve --no-repl` for background dev servers
- **Performance audit**: `seite perf` runs a PageSpeed Insights audit
- **Telemetry**: anonymous, opt-out usage telemetry (commands and platform only); disable with `seite telemetry off`, `SEITE_TELEMETRY=0`, or `DO_NOT_TRACK=1`

### AI & Integrations
- **MCP server (opt-in)**: 16 tools (incl. `seite_check`, `seite_audit`, `seite_get_page`, `seite_search`, and the runtime tools `seite_get_errors`, `seite_get_preview_url`, `seite_list_routes`, `seite_reload` that talk to a running `seite serve`). Coding agents get it with `seite init --mcp` / `seite upgrade --mcp`; chat apps without a shell use `seite mcp --dir <site>` (Claude Desktop, MCPB bundle); `seite serve` also exposes MCP over HTTP at `/__seite/mcp` (local clients only)
- **Skill packs**: `seite skill install seomachine` (SEO content, pinned and checksummed) and `impeccable` (design)
- **Contact forms**: Formspree, Web3Forms, Netlify Forms, HubSpot, Typeform
- **Analytics**: Google Analytics, GTM, Plausible, Fathom, Umami

## Competitive Differentiators

### vs. Hugo / Eleventy / Jekyll
- **AI-native by default**: agent context, a `/seite` skill and check hooks for seven agents, plus triple output, in one free binary (others can add parts of this with plugins or manual config)
- **Single binary**: No Go/Node/Ruby runtime needed
- **Triple output**: HTML + Markdown + LLM discovery files from every build
- **Opinionated presets**: 6 collection types, not just "pages"

### vs. Next.js / Astro
- **No JavaScript runtime**: Pure static HTML output, no hydration
- **Sub-second builds**: No bundler, no compilation step
- **Zero config**: Works immediately after `seite init`

### vs. WordPress / Ghost / Webflow
- **Git-native**: Content in markdown, version controlled
- **No hosting lock-in**: Deploy anywhere static files are served
- **No database**: Nothing to maintain, backup, or secure

## Target Audience

### Startup Founders & Indie Hackers
- Need a website fast, already use a coding agent (Claude Code, Codex, Gemini CLI, ...)
- Want landing page + docs + blog without juggling tools
- Value speed and simplicity over deep customization

### Developers & Technical Writers
- Prefer CLI and markdown over GUIs
- Want reproducible builds and git-friendly workflows
- Need docs with nested navigation and code highlighting

### Content & SEO Teams
- Want automatic SEO best practices
- Need LLM-optimized output for AI search engines
- Value structured data and discovery files

## Pricing

seite is free and open source. MIT licensed. No paid tiers.
Install: `curl -fsSL https://seite.sh/install.sh | sh`
