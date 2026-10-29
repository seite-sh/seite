<p align="center">
  <strong>seite</strong>
</p>

<p align="center">
  The static site generator your coding agent can run end to end, for free.
</p>

<p align="center">
  <a href="https://github.com/seite-sh/seite/actions/workflows/rust.yml"><img src="https://github.com/seite-sh/seite/actions/workflows/rust.yml/badge.svg" alt="CI"></a>
  <a href="https://codecov.io/gh/seite-sh/seite"><img src="https://codecov.io/gh/seite-sh/seite/branch/main/graph/badge.svg" alt="Coverage"></a>
  <a href="https://crates.io/crates/seite"><img src="https://img.shields.io/crates/v/seite.svg" alt="Crates.io"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
</p>

---

seite is a single-binary static site generator written in Rust. A new site comes set up for your coding agent (Claude Code, Codex, OpenCode, Cursor, Antigravity CLI, Gemini CLI, or GitHub Copilot): a compact `AGENTS.md`, one `/seite` skill, and check hooks that hand errors back to the agent. The agent writes content, designs the theme, checks its own work, previews, and deploys. seite itself needs no API keys and no paid services (your deploy host, and any form, comment or newsletter provider, has its own account).

## Why switch to seite

1. **Your coding agent runs the site, whichever one you use.** Typed frontmatter schemas, `seite check`, the check hooks, and `seite context` let an agent find and fix its own mistakes. Works with all six agents above, including OpenCode, which can run free and local models.
2. **Zero subscriptions.** Full-text search, social cards, comments (giscus or utterances), newsletter and contact forms (providers with free tiers), a Git-based CMS (Pages CMS or Decap), scheduled publishing (a daily cron in the generated GitHub Actions workflow for GitHub Pages and Cloudflare), an SEO audit, and deploys to GitHub Pages, Cloudflare Pages, or Netlify.
3. **One binary, everything included.** Image resizing and WebP, social cards, full-text search, RSS and Atom feeds, sitemaps, SEO and GEO metadata, i18n, and optional Tailwind CSS through its standalone CLI. No Node.js.
4. **Visible to AI by default.** `llms.txt`, `llms-full.txt`, a `.md` copy of every page, `sitemap.md`, JSON-LD, Content Signals in `robots.txt`, and `Accept: text/markdown` negotiation (dev server, and Cloudflare Pages with `markdown_negotiation = true` or `[access]`).
5. **A free SEO and GEO audit.** `seite audit` reports missing or weak descriptions, thin and orphan pages, duplicate titles, heading structure, missing alt text, AI visibility, and page weight, each with the file to edit. `--json` output is made for agents.
6. **Switch in one command.** `seite import wordpress|ghost|jekyll|hugo` moves posts, pages, tags, images, and old URLs (as redirects) into a seite site, then your coding agent finishes with `/seite check` and `seite audit`.

## Quickstart

```bash
curl -fsSL https://seite.sh/install.sh | sh
seite init mysite --title "My Site" --collections posts,docs,pages --deploy-target github-pages
cd mysite
```

Open your coding agent in `mysite/` and ask it for something:

```text
/seite new post "My first post"
/seite theme "warm editorial serif, one coral accent"
/seite check
/seite deploy
```

(`$seite …` in Codex.) To see the site yourself:

```bash
seite serve --open
```

The dev server (`http://127.0.0.1:3000`) reloads on every change, and build errors show up in the page as an overlay.

### The `/seite` skill for an existing site

`seite init` is not required to give an agent the skill. Install it for any agent with the [skills CLI](https://skills.sh), or install the skill plus the check hooks as a Claude Code plugin:

```bash
npx skills add seite-sh/seite
```

```text
/plugin marketplace add seite-sh/seite
/plugin install seite@seite
```

See [Coding Agents](https://seite.sh/docs/agent#using-seite-with-an-existing-site).

## Working with coding agents

`seite init --agents claude,codex,opencode,cursor,gemini,copilot,antigravity` (all seven by default) writes:

- `AGENTS.md`, imported by one-line `CLAUDE.md` and `GEMINI.md` files (Copilot and Antigravity CLI read `AGENTS.md` directly)
- the `/seite` skill: `check`, `audit`, `new`, `preview`, `build`, `deploy`, `theme`, `brand`, `landing`, `collection`, `upgrade`, `hooks`
- path-scoped context rules, project permissions, and `.seite/docs/` (the docs for your seite version)
- two check hooks per agent: after each edit (checks that file) and at the end of a turn (checks the site); `seite hooks on|off|status` manages them

Agents use the CLI. Commands built for them, all with `--json`:

```bash
seite context        # once per session: site, collections, theme, dev server, directives
seite check          # config, frontmatter, templates, links; never writes dist/
seite audit          # SEO, GEO, accessibility, content, performance findings and a score
seite preview        # running dev server: URL, last build, browser errors
seite docs <query>   # also: page, search, stats, templates, frontmatter, explain
```

`seite agent` starts an agent in the site and passes your prompt through unchanged:

```bash
seite agent "write a post about our launch"
seite agent --with codex --once "fix every seite check error"
```

The MCP server (`seite mcp`) is for chat apps without a shell, such as Claude Desktop. Coding agents get it only with `seite init --mcp` or `seite upgrade --mcp`. See [MCP Server](https://seite.sh/docs/mcp-server).

## Features

- **Themes**: four starters (`default`, `docs`, `landing`, `blank`) and a gallery of seven (`minimal`, `dark`, `brutalist`, `bento`, `terminal`, `magazine`, `academic`). SEO, search, and i18n come from seite-managed partials, so a redesign keeps them. `seite theme new` starts your own design.
- **Collections**: six presets (posts, docs, pages, changelog, roadmap, trust center), typed frontmatter schemas, taxonomies, series, related pages, and scheduled publishing.
- **Search**: `[build] search = "full"` writes a sharded full-text index that works on any static host.
- **Social cards**: `[og]` draws a 1200x630 PNG per page, in your brand colors.
- **Multi-language**: filename-based translations with per-language URLs, feeds, sitemaps, search, and hreflang.
- **Images**: resizing, WebP, `srcset`/`<picture>`, lazy loading.
- **Deploy**: GitHub Pages, Cloudflare Pages, Netlify, with guided setup, pre-flight checks, `--dry-run`, and optional password-protected collections on Cloudflare Pages.
- **Also**: shortcodes, data files, analytics with an optional consent banner, Tailwind CSS v4 without Node, multi-site workspaces, `seite self-update` with SHA-256 verification.

## Install

**macOS / Linux:**

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://seite.sh/install.ps1 | iex
```

**From source (Rust 1.88 or newer):**

```bash
cargo install seite
```

Both installers put `seite` in `~/.local/bin` (override with `INSTALL_DIR`) and check the archive's SHA-256 against the release's checksum file. To pin a version:

```bash
curl -fsSL https://seite.sh/install.sh | VERSION=v0.21.0 sh
```

```powershell
$env:VERSION = "v0.21.0"; irm https://seite.sh/install.ps1 | iex
```

## Build output

```
dist/
├── index.html
├── posts/
│   ├── hello-world.html       # HTML for browsers
│   └── hello-world.md         # Markdown for LLMs
├── og/                        # social cards, with [og]
├── search/                    # full-text index, with search = "full"
├── feed.xml, atom.xml
├── sitemap.xml, sitemap.md
├── search-index.json
├── robots.txt
├── llms.txt, llms-full.txt
├── 404.html
└── static/
```

Every HTML page has a canonical URL, Open Graph and Twitter tags, JSON-LD, and a link to its markdown copy when one is written. Builds are byte-identical run to run.

## Telemetry

seite collects **anonymous, opt-out** usage telemetry to see which commands are used and whether they succeed. Each command sends its name, the seite version, OS and architecture, whether it succeeded, and a coarse duration bucket (`<1s`, `1-5s`, `5-30s`, `30s+`). The shell and PowerShell installers send one install event (version, OS, architecture, installer).

**Never collected:** file paths, site domains or URLs, page content, command arguments or flag values, environment variables, usernames, or any stable identifier. Data goes to the `cli.seite.sh` partition, separate from your site's analytics. Details: **[seite.sh/telemetry](https://seite.sh/telemetry)**

```bash
seite telemetry off        # persisted opt-out
seite telemetry status
export SEITE_TELEMETRY=0   # or DO_NOT_TRACK=1 (also honored by the installers)
```

Telemetry is off when the `CI` environment variable is set.

## Documentation

Full docs at **[seite.sh/docs](https://seite.sh/docs/getting-started)**:

- [Getting Started](https://seite.sh/docs/getting-started)
- [Coding Agents](https://seite.sh/docs/agent)
- [Audit](https://seite.sh/docs/audit)
- [Import](https://seite.sh/docs/import)
- [Configuration](https://seite.sh/docs/configuration)
- [Collections](https://seite.sh/docs/collections)
- [Templates](https://seite.sh/docs/templates) and [Theme Gallery](https://seite.sh/docs/theme-gallery)
- [Deployment](https://seite.sh/docs/deployment)
- [CMS](https://seite.sh/docs/cms)
- [MCP Server](https://seite.sh/docs/mcp-server)
- [CLI Reference](https://seite.sh/docs/cli-reference)
- [Error Reference](https://seite.sh/docs/errors)

## Contributing

Issues, PRs, and feedback are welcome. Especially useful:

- **Bug reports** with a repro case (even a minimal `seite.toml` and one content file)
- **Reports from running seite with your agent**: what it got wrong, and which agent and model
- **Docs improvements**: if something confused you, it will confuse others

```bash
git clone https://github.com/seite-sh/seite
cargo build
cargo test
cargo clippy      # must be zero warnings
cargo fmt --all
```

`./local-install.sh` builds a release binary and installs it to `~/.local/bin`. Open an issue before starting a large PR.

## License

[MIT](LICENSE)
