---
name: seite
version: 1.0.0
description: AI-native static site generator. Install, scaffold, and deploy sites from the command line.
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

Seite is a Rust-powered static site generator built for AI agents. It's a local CLI tool — install
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

All flags are required for non-interactive mode. Available collections: `posts`, `docs`, `pages`,
`changelog`, `roadmap`, `trust`. Deploy targets: `github-pages`, `cloudflare`, `netlify`.

---

## What Gets Scaffolded

After `seite init`, your project includes:

- `seite.toml` — site configuration (title, collections, deploy target)
- `content/` — markdown content directories per collection
- `templates/` — Tera (Jinja2-compatible) HTML templates
- `static/` — static assets (copied as-is to output)
- `CLAUDE.md` — full project documentation for AI agents
- `.claude/settings.json` — MCP server config + tool permissions (auto-loaded by Claude Code)
- `.claude/rules/` — path-scoped context files (SEO, templates, i18n, etc.)
- `.claude/skills/` — bundled skills (theme-builder, brand-identity, landing-page)

The `.claude/` directory means Claude Code is immediately productive — MCP connects automatically,
rules load contextually, and CLAUDE.md provides full project docs.

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

# Deploy
seite deploy                   # Build, commit, push, deploy
seite deploy --dry-run         # Preview without deploying

# Themes
seite theme list               # List all available themes
seite theme apply dark         # Apply a bundled theme
seite theme create "coral brutalist with lime accents"  # AI-generated theme (requires Claude Code)

# AI Agent
seite agent                    # Interactive Claude Code session with full site context
seite agent "write a blog post about Rust"  # One-shot agent prompt

# Visual Editor
seite edit                     # Browser-based content editor
seite edit --open              # Editor + open browser

# Maintenance
seite self-update              # Update to latest version
seite upgrade                  # Upgrade project config to current binary
seite completions bash         # Generate shell completions (bash/zsh/fish)
```

---

## MCP Integration

Seite includes a local MCP server that runs over stdio. It's auto-configured by `seite init` —
no setup needed. Claude Code connects to it automatically when you open the project.

The MCP server is configured in `.claude/settings.json`:

```json
{
  "mcpServers": {
    "seite": {
      "command": "seite",
      "args": ["mcp"]
    }
  }
}
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `seite_build` | Run the build pipeline, returns build stats |
| `seite_create_content` | Scaffold new content with proper frontmatter |
| `seite_search` | Search content by title, description, or tags |
| `seite_apply_theme` | Apply a bundled or installed theme |
| `seite_lookup_docs` | Search seite's embedded documentation |

### Available MCP Resources

| Resource | Description |
|----------|-------------|
| `seite://docs` | Embedded documentation pages |
| `seite://config` | Current site configuration |
| `seite://content` | Content inventory by collection |
| `seite://themes` | Available themes |

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

SEOMachine adds 11 agents, 22 commands (like `/research`, `/write`, `/optimize`), and 25 skills
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
| AI session | `seite agent` |
| Apply theme | `seite theme apply dark` |
| Update seite | `seite self-update` |

---

*Seite is a local CLI tool. No accounts, no API keys, no SaaS. Your machine, your sites.*
