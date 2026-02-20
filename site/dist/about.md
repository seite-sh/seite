---
title: About
description: Why page exists, how it's built, and what it's designed to do.
---

## It started with frustration.

I'm the CTO of a startup. At some point I sat back and counted the tools we were paying for just to maintain a basic web presence: a CMS for the marketing site, a separate docs platform, a blog service, a changelog tool. Each one with its own login, its own pricing tier, its own paradigm. None of it connected to how we actually built the product.

Meanwhile our dev team was using Claude Code every day. Writing features, reviewing PRs, shipping fast. The website — which is a much simpler problem than the software we were building — somehow required more overhead, more context-switching, and more money than our actual development workflow.

That felt wrong. So I ran an experiment.

## The experiment

The question was simple: could I use the coding AI I was already paying for to solve the website problem properly? No new subscriptions. No new tools. Just Claude Code, pointed at a well-structured project it could actually work with.

The answer was yes — but only if the structure was right. AI agents are powerful, but without defined content types, consistent schemas, and clear conventions, the output drifts. Fast to generate, painful to maintain.

page is what came out of that experiment. It was built with Claude Code, iterated on with Claude Code, and is managed with Claude Code. This site — the docs, the changelog, the llms.txt — is produced by the same build pipeline you get when you run `page build` on your own project.

## What it's designed to do

**Replace the stack, not add to it.** Your landing page, docs, blog, changelog, and roadmap shouldn't each live in a separate tool. They're all structured content that renders as HTML. page handles all of them with collection presets — one repo, one CLI, one deploy command.

**Give your coding agent the right foundation.** Every `page init` generates a `.claude/CLAUDE.md` context file so Claude Code (and other agents) can orient themselves immediately. The agent reads your schema, your templates, and your existing content before it writes anything. The output is reviewable and maintainable because it follows your conventions, not ones it invented.

**Make every page discoverable by people and models.** Every build generates clean semantic HTML for browsers, `llms.txt` and `llms-full.txt` for language models, and structured data for search engines. Traditional SEO and GEO handled in one pipeline, automatically.

**Stay out of your way.** Single binary. No Node.js, no node_modules, no runtime dependencies. Sub-second builds. Install once, runs identically on every machine on your team.

## How it's built

page is written in Rust. The build pipeline compiles to a single static binary that ships with all six themes included — nothing to download, nothing to configure.

The stack underneath:

- **Tera** — Jinja2-compatible templates
- **pulldown-cmark** — CommonMark Markdown parsing
- **syntect** — syntax highlighting
- **image** — resizing and WebP conversion
- **Claude Code** — agent integration for `page agent` and `page theme create`

## Open source

page is MIT licensed. It started as an internal tool and is open source because the problem it solves isn't unique to our startup. If you're running multiple subscriptions to maintain a web presence that should be one repo, this was built for you.

Contributions, issues, and feedback are welcome.

[View on GitHub →](https://github.com/page-cli/page-cli)

---

We use it in production for our own marketing site and docs. The experiment is ongoing.
