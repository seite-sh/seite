---
title: "Getting Started with seite"
description: "Install the AI-native static site generator in 10 seconds, create your first site, and deploy, all from the command line. No runtime dependencies."
weight: 1
---

seite is an AI-native static site generator built in Rust. It ships as a single binary with no runtime dependencies, no Node.js, no Go, no Ruby. Install it, scaffold a site, and deploy in minutes. Every build produces HTML, raw markdown, and [LLM discovery files](/docs/mcp-server) automatically, so your content is readable by browsers, search engines, and AI models from day one.

By the end of this guide you'll have a working site with a blog post, a docs page, and a theme: built and running locally. The whole process takes about five minutes.

## Prerequisites

No runtime dependencies are required. seite is a single binary that runs on macOS, Linux, and Windows. If you want to install from source rather than using the prebuilt binary, you'll need the [Rust toolchain](https://rustup.rs/).

To let a coding agent run the site, install any of [Claude Code](https://docs.anthropic.com/en/docs/claude-code), Codex CLI, OpenCode, Cursor, Gemini CLI, or GitHub Copilot CLI. seite sets the site up for all of them and needs no API keys of its own; see [Coding Agents](/docs/agent).

## Installation

### Quick install (macOS and Linux)

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

This downloads a prebuilt binary for your platform, checks its SHA-256 against the release's checksum file, and installs it to `~/.local/bin` (set `INSTALL_DIR` to change that).

To install a specific version:

```bash
curl -fsSL https://seite.sh/install.sh | VERSION=v0.21.0 sh
```

### Quick install (Windows)

```powershell
irm https://seite.sh/install.ps1 | iex
```

It installs to `~/.local/bin` as well; set `$env:VERSION = "v0.21.0"` first to pin a version.

### Install from source

If you have the Rust toolchain installed (1.88 or newer, all platforms):

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
├── AGENTS.md      # Instructions for every coding agent (+ CLAUDE.md, GEMINI.md imports)
├── .claude/       # Claude Code rules, /seite skill, permissions, hooks, launch.json
├── .cursor/       # Cursor rules, permissions, hooks
├── .codex/        # Codex CLI hooks
├── .gemini/       # Gemini CLI hooks and /seite command
├── .opencode/     # OpenCode /seite command and hook plugin (+ opencode.json)
├── .github/       # GitHub Copilot hooks and instructions
├── .agents/       # The /seite skill for Codex, Cursor, OpenCode, Gemini CLI, and Copilot
└── .seite/        # Project metadata and the docs for this seite version (.seite/docs/)
```

Without `--agents`, `seite init` sets up all six agents (the interactive picker preselects the ones it finds on your machine). Pass `--agents claude,cursor` (any subset of `claude,codex,opencode,cursor,gemini,copilot`) to generate only the ones you use, and `--mcp` to also wire in the MCP server; see the [CLI reference](/docs/cli-reference#seite-init).

The [`seite.toml`](/docs/configuration) file controls your site's title, description, base URL, and collections. The defaults work out of the box. You can tune settings later as needed.

## Let Your Coding Agent Run the Site

Open your coding agent in `mysite/`. It finds everything it needs in the project:

- **`AGENTS.md`**: what the site is, its layout and collections, the content format, and the hard rules. Claude Code and Gemini CLI read it through `CLAUDE.md` / `GEMINI.md`.
- **The `/seite` skill** (`$seite` in Codex): `check`, `audit`, `new`, `preview`, `build`, `deploy`, `theme`, `brand`, `landing`, `collection`, `upgrade`, `hooks`.
- **Check hooks**: after each edit, seite checks the edited file; at the end of a turn, it checks the whole site and hands any errors back so the agent fixes them before it stops.

Each agent has a one-time trust step before it uses the project config (for example, Claude Code's workspace trust prompt); `AGENTS.md` lists them under "Agent setup".

The agent starts a session with `seite context`, which prints the config, collections, theme, running dev server, and pending upgrades. Then ask for what you want:

```text
/seite new post "My first post"
/seite theme "minimal serif with warm colors"
/seite check
/seite preview
```

`/seite check` runs `seite check`, fixes each error at its file and line, and checks once more. `/seite preview` opens the running dev server (in the built-in browser of Claude's desktop app and Cursor) or starts one. While `seite serve` runs, `seite preview` reports its URL, the last build's errors, and errors from the browser console, so the agent sees what you see.

You can also start an agent from the terminal: `seite agent "write a post about our launch"` opens Claude Code (or `--with codex|opencode|cursor|gemini|copilot`) with that prompt. See [Coding Agents](/docs/agent) for everything the agent setup includes.

The rest of this guide shows the same steps as commands you run yourself.

## Create Content

Add a blog post:

```bash
seite new post "My first post" --tags intro,welcome
```

This creates `content/posts/2026-02-18-my-first-post.md` (dated today) with frontmatter:

```yaml
---
title: "My first post"
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
- **Search index**: with `[build] search = "full"` (written by `seite init`), a full-text index under `/search/` that the bundled themes' search box queries; `/search-index.json` (titles, descriptions, tags) is written too.
- **Social cards** under `/og/`: a 1200x630 image per page for link previews, with `[og] generate = true` (also written by `seite init`).
- **LLM discovery files** at `/llms.txt` and `/llms-full.txt`, plus `/sitemap.md` (every page's markdown URL), making your site discoverable by AI search engines and coding agents. Each page's `.md` copy points to them.

This triple output (HTML + Markdown + LLM files) is what makes seite an AI-native static site generator. Your content is readable by browsers, search engines, and AI models from a single build command.

## Checking Your Site

Before (or instead of) a full build, `seite check` validates your config, frontmatter, shortcodes, data files, templates, and internal links and assets:

```bash
seite check
```

It renders the site into a temporary directory to catch real render errors and broken links, then throws that directory away — `dist/` is never created or touched, so it's safe to run at any time, including in an editor or CI as a fast "does this still work" gate. Every problem is reported at once, compiler-style:

```
content/posts/hello-world.md:8:1: warning[broken-link]: link to `/posts/goodbye-world` does not match any page
  hint: did you mean `/posts/hello-world`?
```

Use `--strict` to also fail on warnings (like unknown `seite.toml` keys or broken links), and `--drafts` to include draft content:

```bash
seite check --strict
```

`seite build` runs the same link and asset validation as part of a normal build (reported as warnings, or errors with `seite build --strict`), so `seite check` is most useful when you want to validate content without producing output, or want the fastest possible feedback loop while editing.

For SEO, AI visibility, accessibility, and page weight, run the audit. It prints findings with the file to edit and a score:

```bash
seite audit
```

See [Audit](/docs/audit).

## Development Server

Start a dev server with live reload:

```bash
seite serve --open
```

The `--open` flag launches the site in your default browser automatically. The server starts at `http://localhost:3000` (auto-increments if the port is taken), displays both local and network URLs, and watches for file changes. An interactive REPL lets you run commands without restarting:

```
seite> new post "Another Post"
seite> theme dark
seite> build
seite> status
seite> stop
```

{{% callout(type="tip") %}}
The REPL is the fastest way to iterate. Create content, switch themes, and rebuild, all without leaving the dev server.
{{% end %}}

## Choose a Theme

seite ships 11 [bundled themes](/docs/theme-gallery) compiled into the binary, no downloads or CDN required: four starters (`default`, `docs`, `landing`, and `blank` for designing your own) and a gallery of seven example looks. `seite init` picks a starter (`--theme` chooses).

List available themes:

```bash
seite theme list
```

Apply a bundled theme:

```bash
seite theme apply dark
```

The bundled themes are examples and starting points. Browse them in the [Theme Gallery](/docs/theme-gallery).

For your own design, start from the blank starter and design it with your coding agent (see [Design your own theme](/docs/custom-themes)):

```bash
seite theme new
```

Or generate a custom theme in one step:

```bash
seite theme create "minimal serif with warm colors"
```

This runs your coding agent with `/seite theme <description>`, which builds `templates/base.html` with the SEO, accessibility, and i18n contract in place. With the impeccable design skill installed (`seite skill install impeccable`), it points you to `/impeccable` instead. See [Building Custom Themes](/docs/custom-themes) for details.

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

This runs pre-flight checks, builds the site, and pushes it live. On Cloudflare Pages and Netlify, non-main branches automatically get preview deploys (GitHub Pages has none, so `seite deploy` refuses there). See the [Deployment docs](/docs/deployment) for full setup instructions for each target.

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

This adds any new configuration that shipped with the new version (e.g. new permission entries and hooks), refreshes the `/seite` skill and `.seite/docs/`, and adds any missing files for your coding agents. It merges into your existing settings rather than replacing them.

## Next Steps

Now that your static site generator is up and running, explore these guides to get the most out of seite:

- [Collections](/docs/collections): understand how posts, docs, pages, changelog, and roadmap collections work and how to add custom ones
- [Configuration](/docs/configuration): the full `seite.toml` reference for images, analytics, contact forms, and more
- [Templates & Themes](/docs/templates): customize the look, override template blocks, and browse the bundled starter and gallery themes
- [Building Custom Themes](/docs/custom-themes): start from the `blank` starter and design a theme with your coding agent
- [Deployment](/docs/deployment): detailed setup for GitHub Pages, Cloudflare Pages, and Netlify including custom domains
- [Multi-Language](/docs/i18n): add translations with filename-based i18n and per-language RSS, sitemap, and search
- [Shortcodes](/docs/shortcodes): embed YouTube, Vimeo, callouts, figures, contact forms, and custom components
- [Contact Forms](/docs/contact-forms): add a contact form with Formspree, Web3Forms, Netlify Forms, HubSpot, or Typeform
- [Workspaces](/docs/workspace): manage multiple sites in a single repository
- [Coding Agents](/docs/agent): run the site with Claude Code, Codex, OpenCode, Cursor, Gemini CLI, or GitHub Copilot
- [Audit](/docs/audit): SEO, GEO, accessibility, and performance findings for the built site
- [Import](/docs/import): move a WordPress, Ghost, Jekyll, or Hugo site into seite
- [MCP Server](/docs/mcp-server): structured access for chat apps such as Claude Desktop
- [CLI Reference](/docs/cli-reference): every command and flag in one place
