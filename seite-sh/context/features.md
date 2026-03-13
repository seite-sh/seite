# seite — Features & Benefits

## Core Value Propositions

### 1. **Single Rust Binary**
- **Feature**: One binary, zero runtime dependencies. Install via curl, cargo, or download.
- **Benefit**: No Node.js, no version managers, no dependency hell. Works identically everywhere.
- **Conversion Angle**: "Install in 10 seconds. Build your site in under 1 second."

### 2. **AI-Native Architecture**
- **Feature**: Every project auto-generates `.claude/CLAUDE.md` and MCP server config. `seite agent` spawns Claude Code with full site context.
- **Benefit**: Your AI coding agent understands your site schema, content, and commands from minute one.
- **Conversion Angle**: "Your coding agent can handle your website. Give it the right structure and it will."

### 3. **Triple Output (HTML + Markdown + LLM files)**
- **Feature**: Builds HTML pages, raw markdown copies, llms.txt, and llms-full.txt for every page.
- **Benefit**: Content is readable by browsers, search engines, and AI models — maximizing discoverability.
- **Conversion Angle**: "Ship for people, search engines, and AI models — all from one build."

### 4. **6 Bundled Themes**
- **Feature**: default, minimal, dark, docs, brutalist, bento — compiled into the binary. Plus AI-generated custom themes via `seite theme create`.
- **Benefit**: Instant professional design with no downloads or CDN. Or describe one and let AI build it.
- **Conversion Angle**: "Pick a theme or describe one. Ship in minutes, not days."

### 5. **Collection System with Presets**
- **Feature**: Posts (blog with RSS), docs (nested sidebar nav), pages, changelog, roadmap, trust center — each with templates, URL patterns, and feeds.
- **Benefit**: Common site patterns work out of the box. No config ceremony for standard use cases.
- **Conversion Angle**: "Blog, docs, changelog, roadmap — all built-in. One command to add each."

### 6. **SEO + GEO Optimized by Default**
- **Feature**: Canonical URLs, Open Graph, Twitter Cards, JSON-LD structured data, hreflang, robots.txt with AI crawler controls, sitemaps, llms.txt.
- **Benefit**: Every page is search-engine and AI-engine optimized without manual configuration.
- **Conversion Angle**: "SEO best practices aren't optional features — they're the default."

### 7. **One-Command Deploy**
- **Feature**: GitHub Pages, Cloudflare Pages, Netlify — built-in with pre-flight checks, auto-commit, preview deploys on non-main branches.
- **Benefit**: One command to go from local to live. No CI/CD config needed.
- **Conversion Angle**: "`seite deploy` — that's it."

### 8. **Multi-language Support**
- **Feature**: Filename-based translations (`about.es.md`), per-language URLs/RSS/sitemaps/search, hreflang tags, UI string system.
- **Benefit**: Fully internationalized sites with zero overhead for single-language sites.
- **Conversion Angle**: "Add `.es.md` to any file. seite handles the rest."

## Technical Features

### Build & Performance
- **Sub-second builds**: Rust-powered, typically < 0.5s for 50+ pages
- **Image processing**: Resize, WebP, AVIF, srcset — automatic
- **Math rendering**: KaTeX for inline ($) and display ($$) math
- **Syntax highlighting**: syntect with theme support
- **Minification**: CSS/JS comment stripping + whitespace collapse
- **Asset fingerprinting**: Content-hash filenames for cache busting

### Content Management
- **Shortcodes**: YouTube, Vimeo, Gist, callouts, figures, contact forms + custom user-defined
- **Data files**: YAML/JSON/TOML from `data/` directory, available in all templates
- **Frontmatter**: YAML with drafts, weights, custom templates, arbitrary extras
- **Client-side search**: JSON search index generated at build time

### Developer Experience
- **Dev server + REPL**: Live reload, interactive commands (new, agent, theme, build, status)
- **Workspaces**: Multi-site management from a single directory
- **Self-update**: `seite self-update` pulls latest from GitHub Releases
- **Project upgrades**: `seite upgrade` adds new features non-destructively

### AI & Integrations
- **MCP server**: JSON-RPC over stdio, auto-configured for Claude Code
- **Skill packs**: Optional Claude Code agent bundles (e.g., SEOMachine for SEO)
- **Contact forms**: Formspree, Web3Forms, Netlify Forms, HubSpot, Typeform
- **Analytics**: Google Analytics, GTM, Plausible, Fathom, Umami

## Competitive Differentiators

### vs. Hugo / Eleventy / Jekyll
- **AI-native**: Only SSG with built-in agent context, MCP server, and skill packs
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
- Need a website fast, already use Claude Code
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
