---
name: seite
version: 0.21.0
description: The static site generator your coding agent can run end to end, for free. Install, scaffold, check, and deploy sites from the command line.
homepage: https://seite.sh
metadata:
  seite:
    emoji: "🏗️"
    category: web
    requires:
      bins: [curl]
      anyBins: [curl, wget]
---

# Seite

Seite is a Rust-powered static site generator built for coding agents (Claude Code, Codex CLI, OpenCode,
Cursor, Gemini CLI, GitHub Copilot). It's a local CLI tool — install
the binary, scaffold a project, create content, and deploy. No accounts, no API keys, no SaaS.
Everything runs on your machine.

## Companion Files

| File | URL |
|------|-----|
| **skill.md** (this file) | `https://seite.sh/skill.md` |
| **heartbeat.md** | `https://seite.sh/heartbeat.md` |
| **deploy.md** | `https://seite.sh/deploy.md` |
| **templates.md** | `https://seite.sh/templates.md` |

---

## Install

### macOS / Linux

```bash
curl -fsSL https://seite.sh/install.sh | sh
export PATH="$HOME/.local/bin:$PATH"
```

### Windows (PowerShell)

```powershell
irm https://seite.sh/install.ps1 | iex
[Environment]::SetEnvironmentVariable('Path', "$HOME\.local\bin;$env:Path", 'User')
```

### Cargo (any platform)

```bash
cargo install seite
```

Verify: `seite --version`

---

## Create Your First Site

```bash
seite init mysite \
  --title "My Site" \
  --description "A site built by my agent" \
  --collections posts,docs,pages \
  --deploy-target cloudflare
cd mysite
```

Without a terminal, `--deploy-target` is required; the other flags have defaults (title from the
directory name, collections `posts,pages`, all six agents for `--agents`; add `--mcp` to wire in the
MCP server). Available collections: `posts`, `docs`, `pages`,
`changelog`, `roadmap`, `trust`. Deploy targets: `github-pages`, `cloudflare`, `netlify`.

---

## What Gets Scaffolded

After `seite init`, your project includes:

- `seite.toml` — site configuration (title, collections, deploy target)
- `content/` — markdown content directories per collection
- `templates/` — Tera (Jinja2-compatible) HTML templates
- `static/` — static assets (copied as-is to output)
- `AGENTS.md` — compact project guide every agent reads (about 4 KB); `CLAUDE.md` and `GEMINI.md`
  import it, Codex, OpenCode, Cursor and GitHub Copilot read it directly
- The `/seite` skill (`$seite` in Codex), version 7: `.claude/skills/seite/` for Claude Code,
  `.agents/skills/seite/` for the others. Commands: `check`, `audit`, `new`, `preview`, `build`,
  `deploy`, `theme`, `brand`, `landing`, `collection`, `upgrade`, `hooks`
- Path-scoped context rules (`.claude/rules/`, `.cursor/rules/`, `.github/instructions/`)
- Permissions (`.claude/settings.json`, `.cursor/cli.json`, `opencode.json`) for the site's
  read-only and editing `seite` commands; `seite deploy`, `seite agent` and `seite theme create` still ask
- Check hooks per agent: `seite check` on each edited file and on the whole site at the end of a turn
- `.seite/docs/` — the docs for the installed seite version (gitignored)

The agent starts each session with `seite context` and works through `/seite <command>`. No MCP
server is configured unless you pass `--mcp`.

---

## Core Commands

```bash
# Content
seite new post "My Post Title" --tags rust,web     # Create a blog post
seite new doc "Setup Guide"                         # Create a doc page
seite new page "About"                              # Create a standalone page
seite new post "Draft Post" --draft                 # Create as draft

# Build & Preview
seite build                    # Build site to dist/
seite build --drafts           # Include draft content
seite serve                    # Dev server with live reload + REPL
seite serve --open             # Dev server + open browser
seite preview                  # Running dev server: URL, last build, browser errors

# Check
seite check                    # Config, frontmatter, templates, links (never writes dist/)
seite audit                    # SEO, GEO, accessibility, content, performance findings + score
seite context                  # Agent orientation: site summary and directives

# Deploy
seite deploy                   # Build, commit, push, deploy
seite deploy --dry-run         # Preview without deploying

# Themes
seite theme list               # List all available themes
seite theme apply dark         # Apply a bundled theme
seite theme new                # Blank starter for your own design
seite theme create "coral brutalist with lime accents"  # Design one with your coding agent

# Coding agent
seite agent                    # Open Claude Code in the site (--with codex|opencode|cursor|gemini|copilot)
seite agent "write a blog post about Rust"  # Pass a prompt through unchanged

# Maintenance
seite self-update              # Update to latest version
seite upgrade                  # Upgrade project config to current binary
seite completions bash         # Generate shell completions (bash/zsh/fish)
```

---

## MCP Integration

Seite includes a local MCP server that runs over stdio (`seite mcp`). Coding agents use the CLI by
default; the MCP server is opt-in:

```bash
seite init mysite --mcp       # new site
seite upgrade --mcp           # existing site
```

This writes `.mcp.json` (Claude Code and GitHub Copilot), `.cursor/mcp.json`, `.codex/config.toml`,
the `mcp` key of `opencode.json`, and `mcpServers` in `.gemini/settings.json`.

### Available MCP Tools (16)

| Tool | Description |
|------|-------------|
| `seite_build` | Run the build pipeline, returns build stats |
| `seite_check` | Validate config, content, templates and links without writing the output |
| `seite_audit` | SEO, GEO, accessibility, content and performance findings with a score |
| `seite_create_content` | Scaffold new content with proper frontmatter |
| `seite_get_page` | One page as the build sees it: resolved frontmatter, URL, body, rendered HTML |
| `seite_update_frontmatter` | Edit a page's frontmatter without touching its body |
| `seite_search` | Search content by title, description, or tags |
| `seite_content_stats` | Drafts, missing descriptions/tags/translations, future-dated items |
| `seite_list_templates` | Template overrides, base blocks, context variables, shortcodes, data keys |
| `seite_apply_theme` | Apply a bundled or installed theme |
| `seite_create_collection` | Add a preset collection (posts, docs, pages, changelog, roadmap, trust) |
| `seite_lookup_docs` | Search seite's embedded documentation |
| `seite_get_errors` | Running dev server: last build and browser errors |
| `seite_get_preview_url` | Running dev server: preview URL for a content file or path |
| `seite_list_routes` | Every built HTML page as a URL |
| `seite_reload` | Rebuild through the running dev server and reload open pages |

### Available MCP Resources

| Resource | Description |
|----------|-------------|
| `seite://docs` | Embedded documentation pages |
| `seite://config` | Current site configuration |
| `seite://content` | Content inventory by collection |
| `seite://themes` | Available themes |
| `seite://trust` | Trust center data (when the site has one) |

No authentication. No remote server. Runs locally as a subprocess.

---

## Content Format

Content files are markdown with YAML frontmatter:

```yaml
---
title: "Post Title"
date: 2026-03-16           # required for posts
description: "Page description for SEO"
tags:
  - rust
  - web
draft: true                 # excluded from builds unless --drafts
image: /static/og.png       # social preview image
---

Your markdown content here.
```

---

## Skill Packs

Extend seite with skill packs that add agents, commands, and skills to your project:

```bash
seite skill install seomachine    # SEO research, writing, and optimization
seite skill list                  # List installed packs
seite skill update                # Update installed packs
seite skill remove seomachine     # Remove a pack
```

SEOMachine adds 11 agents, 24 commands (like `/research`, `/write`, `/optimize`, plus seite's `/publish-draft`), and 25 skills
for content marketing workflows.

---

## Quick Reference

| Task | Command |
|------|---------|
| Install seite | `curl -fsSL https://seite.sh/install.sh \| sh` |
| Create site | `seite init mysite --title "..." --description "..." --collections posts,pages --deploy-target cloudflare` |
| New blog post | `seite new post "Title" --tags tag1,tag2` |
| Build | `seite build` |
| Preview | `seite serve --open` |
| Deploy | `seite deploy` |
| Check | `seite check` |
| Audit | `seite audit` |
| Coding agent | `seite agent` |
| Apply theme | `seite theme apply dark` |
| Update seite | `seite self-update` |

---

*Seite is a local CLI tool. No accounts, no API keys, no SaaS. Your machine, your sites.*
