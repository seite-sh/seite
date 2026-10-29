# seite

The static site generator your coding agent can run end to end, for free. HTML, Markdown and llms.txt from one Rust binary.

A static site built with [seite](https://seite.sh): markdown in, HTML + markdown + `llms.txt` out.

## Workflow

1. Run `seite context` once per session: config, collections, theme, dev server, pending upgrades.
2. Work through the `seite` skill: `/seite <command>` (Codex: `$seite <command>`), e.g. `check`, `new`, `preview`, `theme`, `deploy`.
3. Reference: `.seite/docs/` (docs for this seite version, written by seite and gitignored) or `seite docs <query>`.

## This site

seite.sh is seite's own website, built by the `seite` in this repository. Two things here ship inside the binary:

- `content/docs/*.md` are embedded as seite's docs (`seite docs`, `.seite/docs/`, MCP). Keep them accurate to the code in `../src/`.
- `content/changelog/` entries become the release notes.

Templates in `templates/` are a custom design; keep the `{% include "seite/…" %}` partials.

## Layout

```
content/<collection>/  markdown + YAML frontmatter
templates/             Tera templates; base.html is the theme
data/                  YAML/JSON/TOML -> {{ data.<file> }}
static/                copied to dist/static/
public/                copied to the dist/ root (favicon.ico)
seite.toml             site config
dist/                  build output (generated)
```

## Collections

| Collection | Files | URL | Template |
|---|---|---|---|
| Documentation (`seite new doc`) | `content/docs/slug.md` | `/docs/<slug>` | `doc.html` |
| Pages (`seite new page`) | `content/pages/slug.md` | `/<slug>` | `page.html` |
| Changelog (`seite new changelog`) | `content/changelog/YYYY-MM-DD-slug.md` | `/changelog/<slug>` | `changelog-entry.html` |
| Roadmap (`seite new roadmap`) | `content/roadmap/slug.md` | `/roadmap/<slug>` | `roadmap-item.html` |
| Posts (`seite new post`) | `content/posts/YYYY-MM-DD-slug.md` | `/blog/<slug>` | `post.html` |

Changelog: one entry per release, `YYYY-MM-DD-vX-Y-Z.md`, tags `feature`, `fix`, `security`. `build.rs` compiles these entries into the seite binary (release notes, `seite upgrade --agent`), so a missing entry breaks the release.
Roadmap status tags: `planned`, `in-progress`, `done`, `cancelled`; `weight:` orders items.

## Content format

```yaml
---
title: "Title"           # required
date: 2026-01-15         # required in dated collections
description: "..."       # meta, Open Graph, JSON-LD
image: /static/og.png    # social preview
updated: 2026-02-01      # last modified
tags: [a, b]             # tag pages and feeds
draft: true              # left out of builds
weight: 1                # order in undated collections
extra: {key: value}      # -> {{ page.extra.key }}
---
```

## Rules

- Link between content files with relative `.md` paths (`[intro](../docs/intro.md)`); the build turns them into URLs.
- Never edit `dist/`.
- Templates extend `base.html`; render content with `{{ page.content | safe }}`.
- Themes: keep the `{% include "seite/…" %}` partials; seite updates them.
- Shortcodes: `{{< name(arg="v") >}}`, or `{{% name() %}}…{{% end %}}` with a body. No `{{< /name >}}`.

## Verify

Run `seite check` after changes. Fix every error at its `file:line`, run `seite check` once more, then stop and report what is left. Hooks (`.claude/settings.json`, `.codex/hooks.json`, `.opencode/plugins/seite-check.js`, `.cursor/hooks.json`, `.gemini/settings.json`) run `seite check --file` after each edit and `seite check` when a turn ends, and hand errors back to you; `seite hooks status|on|off` manages them.

## SEOMachine

<!-- seite-seomachine-start -->
### SEOMachine Integration

This project has [SEOMachine](https://github.com/TheCraigHewitt/seomachine) installed for SEO content research, writing, and optimization.

**Key commands:** `/research`, `/write`, `/article`, `/optimize`, `/analyze-existing`, `/rewrite`, `/priorities`, `/publish-draft`, `/cluster`, `/performance-review`

**Context files** in `context/` define your brand voice, target keywords, and SEO guidelines. Fill these in for best results.

**Directory layout:**
- `content/posts/` — articles (use `draft: true` for work-in-progress)
- `research/` — research briefs and analysis reports
- `output/` — agent optimization reports
- `topics/` — raw topic ideas
- `context/` — brand voice, style guide, target keywords, competitor analysis

**Analytics (optional):** Python scripts in `scripts/seo/` connect to Google Analytics, Search Console, and DataForSEO. Set up `.env` with API credentials. Run `pip install -r requirements-seo.txt` to install dependencies.

**Publishing:** Use `/publish-draft <file>` to remove draft status, build, and deploy. This replaces SEOMachine's WordPress integration with seite's native deploy.

## seite Integration Context

You are working inside a **seite** static site project (https://seite.sh). Adapt your workflow to use seite's content model and CLI:

### Content model
- Content lives in `content/{collection}/*.md` with YAML frontmatter
- Use `seite new post "Title" --tags tag1,tag2` to create a new post
- Set `draft: true` in frontmatter for work-in-progress drafts
- Run `seite build` to build the site, `seite serve` for live preview, `seite deploy` to publish

### Directory mapping (SEOMachine → seite)
| SEOMachine | seite equivalent |
|---|---|
| `drafts/` | `content/{collection}/` with `draft: true` in frontmatter |
| `published/` | `content/{collection}/` (no draft flag) |
| `research/` | `research/` (same) |
| `output/` | `output/` (same, for agent reports) |
| `topics/` | `topics/` (same, for idea capture) |

### Frontmatter format
```yaml
---
title: "SEO-Optimized Title (50-60 chars)"
date: 2026-03-12
description: "Compelling meta description with primary keyword (150-160 chars)"
tags:
  - keyword1
  - keyword2
image: /static/og-image.png
draft: false
extra:
  primary_keyword: "target keyword"
---
```

### SEO features (automatic)
seite themes automatically handle: canonical URLs, Open Graph tags, Twitter Cards, JSON-LD structured data (BlogPosting/Article/WebSite), hreflang tags (multilingual), robots.txt with AI crawler management, llms.txt + llms-full.txt for AI discovery, and raw markdown alongside HTML. Focus on content quality — technical SEO is built in.

### Publishing workflow
Instead of WordPress REST API, use:
1. Write content to `content/posts/` (or other collection)
2. Remove `draft: true` when ready to publish
3. Run `seite build` to verify
4. Run `seite deploy` to publish

### Site context
- Read `seite.toml` for site configuration (title, collections, base_url, language)
- Read `data/brand.yaml` for brand identity (if created via `/seite brand`)
- Read `AGENTS.md` for full site documentation

<!-- seite-seomachine-end -->

## Agent setup

<!-- seite:agent-setup -->
| Agent | Config | MCP config | One-time step |
|---|---|---|---|
| Claude Code | `.claude/settings.json`, `.claude/launch.json` | `.mcp.json` | Accept the workspace trust prompt once (until then Claude Code ignores `.claude/settings.json`); approve the server with `/mcp` if asked |
| Codex CLI | `.codex/hooks.json` | `.codex/config.toml` | Trust the project when Codex asks, then approve the hooks once in `/hooks`; check the server with `/mcp` |
| OpenCode | `opencode.json`, `.opencode/plugins/seite-check.js` | `opencode.json` | None |
| Cursor | `.cursor/cli.json`, `.cursor/hooks.json` | `.cursor/mcp.json` | Trust the workspace (`cursor-agent --trust` or answer the prompt); approve the server in Cursor's MCP settings or run `cursor-agent mcp enable seite` |
| Gemini CLI | `.gemini/settings.json` | `.gemini/settings.json` | Trust the folder (`gemini trust` or the prompt) and approve the project hooks when asked; check the server with `/mcp` |
<!-- /seite:agent-setup -->

## Context Rules

<!-- seite:context-rules -->
Detailed guides live in `.claude/rules/` (Cursor: same guides as `.cursor/rules/*.mdc`). Claude Code and Cursor load them automatically for matching files; other agents should read the matching guides before editing:

- `templates/**`: `seo-requirements.md`, `templates.md`, `i18n.md`, `features.md`
- `content/**`: `i18n.md`, `shortcodes.md`, `features.md`, `private-collections.md`
- `data/i18n/**`: `i18n.md`
- `data/**`: `data-files.md`
- `templates/shortcodes/**`: `shortcodes.md`
- `seite.toml`: `config-reference.md`, `private-collections.md`
<!-- /seite:context-rules -->
