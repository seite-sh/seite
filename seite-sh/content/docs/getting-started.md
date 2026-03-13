---
title: "Getting Started with seite"
description: "Install the AI-native static site generator in 10 seconds, create your first site, and deploy, all from the command line. No runtime dependencies."
weight: 1
---

seite is an AI-native static site generator built in Rust. It ships as a single binary with no runtime dependencies, no Node.js, no Go, no Ruby. Install it, scaffold a site, and deploy in minutes. Every build produces HTML, raw markdown, and [LLM discovery files](/docs/mcp-server) automatically, so your content is readable by browsers, search engines, and AI models from day one.

By the end of this guide you'll have a working site with a blog post, a docs page, and a theme: built and running locally. The whole process takes about five minutes.

## Prerequisites

No runtime dependencies are required. seite is a single binary that runs on macOS, Linux, and Windows. If you want to install from source rather than using the prebuilt binary, you'll need the [Rust toolchain](https://rustup.rs/).

For the optional [AI agent](/docs/agent) and AI theme generation features, install [Claude Code](https://docs.anthropic.com/en/docs/claude-code): `npm install -g @anthropic-ai/claude-code`.

## Installation

### Quick install (macOS and Linux)

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

This downloads a prebuilt binary for your platform and installs it to `~/.local/bin`.

To install a specific version:

```bash
VERSION=v0.1.0 curl -fsSL https://seite.sh/install.sh | sh
```

### Quick install (Windows)

```powershell
irm https://seite.sh/install.ps1 | iex
```

### Install from source

If you have the Rust toolchain installed (all platforms):

```bash
cargo install seite
```

### Platform support

| Platform | Architecture | Install method |
|----------|-------------|----------------|
| macOS | Apple Silicon (M1/M2/M3/M4) | Shell installer or cargo |
| macOS | Intel x86_64 | Shell installer or cargo |
| Linux | x86_64 | Shell installer or cargo |
| Linux | aarch64/arm64 | Shell installer or cargo |
| Windows | x86_64 | PowerShell installer or cargo |

### Verify

```bash
seite --version
```

{{% callout(type="tip") %}}
Run `seite --help` to see all available commands at a glance.
{{% end %}}

## Create Your First Static Site

Scaffold a new site with [posts, docs, and pages collections](/docs/collections):

```bash
seite init mysite --title "My Site" --description "A personal blog" --collections posts,docs,pages
cd mysite
```

{{% callout(type="info") %}}
All flags are optional. Run `seite init mysite` and interactive prompts will guide you through each setting. You can also add [changelog and roadmap collections](/docs/collections) later.
{{% end %}}

This creates the following structure:

```
mysite/
├── content/
│   ├── posts/     # Date-based blog posts with RSS
│   ├── docs/      # Documentation with sidebar navigation
│   └── pages/     # Standalone pages (about, contact, etc.)
├── templates/     # Tera templates (override bundled themes)
├── public/        # Root-level files (favicon.ico, .well-known/, _redirects)
├── static/        # Static assets (CSS, JS, images) → dist/static/
├── seite.toml      # Site configuration
├── .claude/       # Claude Code agent configuration (includes MCP server)
└── .seite/         # Project metadata (version tracking)
```

The [`seite.toml`](/docs/configuration) file controls your site's title, description, base URL, and collections. The defaults work out of the box. You can tune settings later as needed.

## Create Content

Add a blog post:

```bash
seite new post "Hello World" --tags intro,welcome
```

This creates `content/posts/2026-02-18-hello-world.md` with frontmatter:

```yaml
---
title: "Hello World"
date: 2026-02-18
tags:
  - intro
  - welcome
---

Your content here...
```

Add a documentation page or a standalone page:

```bash
seite new doc "Setup Guide"
seite new page "About"
```

Open the generated markdown file in your editor and write your content below the `---` frontmatter delimiter. Add a `description` field for better SEO: seite uses it for meta tags, Open Graph, and JSON-LD structured data automatically.

{{% callout(type="tip") %}}
Want AI to write your first post? Run `seite agent "write a hello world blog post about launching my site"` and Claude will create the content file for you. See the [AI Agent docs](/docs/agent) for more.
{{% end %}}

## Build Your Static Site

Build the site to the `dist/` directory:

```bash
seite build
```

Builds are fast: typically under a second, even with dozens of pages. Every build produces a complete, deployment-ready output:

- **HTML pages** with clean URLs (`/posts/hello-world`). SEO-optimized with canonical URLs, Open Graph tags, Twitter Cards, and JSON-LD structured data. No plugins or config needed.
- **Markdown copies** alongside every HTML file, so AI models and developer tools can consume your content directly.
- **RSS feed** at `/feed.xml` for subscribers.
- **XML sitemap** at `/sitemap.xml` with hreflang alternates if you add [translations](/docs/i18n).
- **Search index** at `/search-index.json`: client-side search works out of the box in all bundled themes.
- **LLM discovery files** at `/llms.txt` and `/llms-full.txt`, making your site discoverable by AI search engines and coding agents.

This triple output (HTML + Markdown + LLM files) is what makes seite an AI-native static site generator. Your content is readable by browsers, search engines, and AI models from a single build command.

## Development Server

Start a dev server with live reload:

```bash
seite serve
```

The server starts at `http://localhost:3000` (auto-increments if the port is taken) and watches for file changes. An interactive REPL lets you run commands without restarting:

```
seite> new post "Another Post"
seite> theme apply dark
seite> build
seite> status
seite> stop
```

{{% callout(type="tip") %}}
The REPL is the fastest way to iterate. Create content, switch themes, and rebuild, all without leaving the dev server.
{{% end %}}

## Choose a Theme

seite ships with [6 bundled themes](/docs/templates) compiled into the binary, no downloads or CDN required.

List available themes:

```bash
seite theme list
```

Apply a bundled theme:

```bash
seite theme apply dark
```

The bundled themes include `default`, `minimal`, `dark`, `docs` (sidebar navigation), `brutalist`, and `bento` (card grid). Browse them in the [Theme Gallery](/docs/theme-gallery).

You can also generate a custom theme with AI:

```bash
seite theme create "minimal serif with warm colors"
```

This spawns Claude Code to write a complete `templates/base.html` with all SEO, accessibility, and i18n features built in. See [Building Custom Themes](/docs/custom-themes) for details.

## Deploy Your Site

When you're ready to go live, seite deploys to GitHub Pages, Cloudflare Pages, or Netlify with a single command.

First, set your deploy target during init or in `seite.toml`:

```toml
[deploy]
target = "cloudflare"    # or "github-pages" or "netlify"
```

Then deploy:

```bash
seite deploy
```

This runs pre-flight checks, builds the site, and pushes it live. Non-main branches automatically get preview deploys. See the [Deployment docs](/docs/deployment) for full setup instructions for each target.

{{% callout(type="tip") %}}
Run `seite deploy --dry-run` first to preview what will happen without actually deploying.
{{% end %}}

## Updating

Update the binary itself:

```bash
seite self-update
```

After updating, bring your project's config files up to date:

```bash
seite upgrade
```

This adds any new configuration that shipped with the new version (e.g., MCP server settings, new permission entries). It's additive and non-destructive. Your existing settings are preserved.

## Next Steps

Now that your static site generator is up and running, explore these guides to get the most out of seite:

- [Collections](/docs/collections): understand how posts, docs, pages, changelog, and roadmap collections work and how to add custom ones
- [Configuration](/docs/configuration): the full `seite.toml` reference for images, analytics, contact forms, and more
- [Templates & Themes](/docs/templates): customize the look, override template blocks, and browse all 6 bundled themes
- [Building Custom Themes](/docs/custom-themes): create a theme from scratch or modify a bundled one with the theme builder skill
- [Deployment](/docs/deployment): detailed setup for GitHub Pages, Cloudflare Pages, and Netlify including custom domains
- [Multi-Language](/docs/i18n): add translations with filename-based i18n and per-language RSS, sitemap, and search
- [Shortcodes](/docs/shortcodes): embed YouTube, Vimeo, callouts, figures, contact forms, and custom components
- [Contact Forms](/docs/contact-forms): add a contact form with Formspree, Web3Forms, Netlify Forms, HubSpot, or Typeform
- [Workspaces](/docs/workspace): manage multiple sites in a single repository
- [AI Agent](/docs/agent): let Claude write content, debug builds, and generate themes for you
- [MCP Server](/docs/mcp-server): structured AI access to your site via the Model Context Protocol
- [CLI Reference](/docs/cli-reference): every command and flag in one place
