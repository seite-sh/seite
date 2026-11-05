# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- **Primary: technical startup founders and early engineers.** They already pay for and use a coding agent (Claude Code, Codex, OpenCode, or Cursor) and need the company's website (landing page, docs, blog, changelog) to exist and stay current without becoming anyone's job. Success for them is handing the website to the agent they already use.
- **Secondary: developers choosing a static site generator.** They compare seite with Hugo, Astro, 11ty and Jekyll on workflow, speed, and dependencies.

## Product Purpose

seite is an open-source (MIT) static site generator, shipped as a single Rust binary, designed so a coding agent can build, write, redesign, and deploy a website end to end. The marketing site, seite.sh, exists to get the primary user to install it and point their agent at it.

## Positioning

**Your agent runs the site.** Every `seite init` scaffolds the context a coding agent needs: a portable `AGENTS.md` (plus a `CLAUDE.md` import), path-scoped rules, a `/seite` workflow skill, an MCP server with 11 tools, and a stop hook that runs `seite check` so the agent fixes its own errors. This works across Claude Code, Codex, OpenCode, and Cursor, using a subscription the user already pays for (no API keys). Hugo, Astro, 11ty and Jekyll don't ship this by default.

Supporting claims (secondary to the one above):
- Every page ships as HTML for people plus Markdown, `llms.txt`/`llms-full.txt`, and structured data for search engines and AI models.
- One tool replaces several: blog, docs, changelog, roadmap, contact forms, multi-language, deploys, and password protection.

## Operating Context

- Users work in a terminal and a git repo, with an agent session open; the agent edits markdown/templates and the user reviews diffs.
- Install with `curl -fsSL seite.sh/install.sh | sh` (macOS/Linux) or `irm seite.sh/install.ps1 | iex` (Windows); `seite init`, `seite serve`, `seite agent`, `seite deploy`.
- Deploys to GitHub Pages, Cloudflare Pages, or Netlify.
- seite.sh is itself built with seite (`seite-sh/` in this repo) and deployed to Cloudflare Pages; homepage copy lives in `seite-sh/content/pages/index.md` `extra:` and layout in `seite-sh/templates/index.html`.

## Capabilities and Constraints

- Facts as of v0.20.1: 10 bundled themes plus `seite theme create "<description>"`; collection presets (posts, docs, pages, changelog, roadmap, trust center); i18n; image processing; KaTeX math; Mermaid diagrams; client-side search; RSS/Atom; sitemaps; JSON-LD; robots.txt with AI crawler controls; private collections; Cloudflare password access; subdomain deploys; multi-site workspaces.
- `context/features.md` is the maintained feature list; check it before making claims.
- Site pages must keep the SEO/GEO head block (canonical, OG, Twitter, JSON-LD, markdown alternate, llms.txt link); see `seite-sh/AGENTS.md`.
- Analytics: Plausible, with `plausible-event-name=*` classes on CTAs and copy buttons.
- The site supports light and dark themes via a user toggle.

## Brand Commitments

- **Standing preference (2026-10-22):** seite.sh follows the developer-tool category standard, executed at the craft level of bun.sh. Chosen over distinctive alternatives (galley proof, instruction manual, departure poster) after seeing sketches.

No identity assets are binding. The lowercase name, page-with-fold logo, teal/indigo colors (`seite-sh/data/brand.yaml`), and voice guide (`seite-sh/context/brand-voice.md`: pragmatic, builder-focused, honest, no hype) are existing references that a redesign may keep or replace.

## Evidence on Hand

- **Dogfooding:** seite.sh itself (docs, changelog, blog, `llms.txt`) and the maintainers' own startup site.
- **Open source:** public repo at https://github.com/seite-sh/seite (MIT); star and release counts may be shown live.
- **Benchmarks:** measured theme sizes in `seite-sh/content/posts/2026-03-13-compiled-themes-binary.md`; sub-second builds (the site's own build: 111 pages in about 0.1s).
- **Absent, do not fabricate:** named users, customer logos, testimonials, quotes, case studies, press, download counts, pricing (it's free).

## Product Principles

1. The agent is the interface. Show the agent doing the work rather than describing features.
2. Every claim is verifiable. A visitor can install the tool, read the repo, or view the page's own Markdown twin and confirm it.
3. Speak to builders. Be direct and specific; show commands and real output; no hype.
4. Founders first, developers close behind. Lead with the job of getting the website off their plate; keep the technical depth one click away.
