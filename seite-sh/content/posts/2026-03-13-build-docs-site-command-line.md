---
title: "Build a Docs Site from the Command Line in 5 Minutes"
date: 2026-03-13
description: "Build a docs site from the command line without installing Node.js or Python. Get search, sidebar nav, and AI-readable markdown in under 5 minutes. Start now."
tags:
  - documentation
  - static-site-generator
  - cli
  - docs-as-code
draft: false
extra:
  primary_keyword: "build docs site from command line"
---

The MkDocs getting started guide has 6 steps before you see a rendered page. Docusaurus has 5, plus a 200MB `node_modules` directory. Both require installing a programming language runtime before you write a single word of documentation.

That's backwards. You want to build a docs site from the command line. You shouldn't need Python, Node.js, or a package manager to get one.

Every documentation site generator tutorial follows the same pattern: install a runtime, install a package manager, install the tool, scaffold a project, configure it, build it, then figure out deployment on your own. The deploy step is always left as an exercise for the reader. That's a polite way of saying "we don't have a good answer."

This guide shows you how to build a docs site from the command line in three commands with zero dependencies: install, scaffold, deploy. You'll get sidebar navigation, full-text search, syntax highlighting, and something no other docs tutorial mentions: output that AI models can actually read. The whole process takes about five minutes.

## What You're Building

Before you type a command, here's what the finished docs site includes:

- **Sidebar navigation** with section grouping and active-page highlighting
- **Full-text search** (client-side, no external service)
- **Syntax highlighting** for code blocks in any language
- **Responsive layout** that works on desktop and mobile
- **Clean URLs** (`/docs/getting-started`, not `/docs/getting-started.html`)
- **SEO fundamentals** built in: canonical URLs, Open Graph, JSON-LD structured data, sitemap

And something you won't find in MkDocs or Docusaurus tutorials:

- **Markdown output** alongside HTML for every page (`/docs/setup.md` next to `/docs/setup.html`)
- **[llms.txt](/blog/what-is-llms-txt) and llms-full.txt** that tell AI models what your docs contain
- **AI crawler management** via robots.txt that allows AI search bots while blocking AI training crawlers

This is the [triple output pattern](/blog/ai-static-site-generator): HTML for browsers, markdown for AI models, and discovery files for search engines. Your docs are readable by every audience from the first build.

**Want the full feature breakdown?** See the [getting started guide](/docs/getting-started) for everything seite includes out of the box.

## Build a Docs Site from the Command Line: The 3-Command Setup

### Step 1: Install (10 Seconds)

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

That downloads a single 15MB binary to `~/.local/bin`. No runtime. No package manager. No `pip install`, no `npm install`, no virtual environment activation. The binary runs on macOS, Linux, and Windows.

On Windows:

```powershell
irm https://seite.sh/install.ps1 | iex
```

### Step 2: Scaffold (30 Seconds)

```bash
seite init my-docs --title "My Project Docs" --collections docs,pages
```

This creates a project with a docs [collection](/docs/collections) and a pages collection. Here's the generated structure:

```
my-docs/
  seite.toml              # Site configuration
  content/
    docs/
      getting-started.md  # Your first doc
    pages/
      index.md            # Homepage
  templates/              # Tera (Jinja2-compatible) templates
  static/                 # CSS, images, assets
  .claude/                # AI agent context (auto-generated)
```

The `seite.toml` [configuration](/docs/configuration) is minimal:

```toml
[site]
title = "My Project Docs"
base_url = "http://localhost:3000"

[[collections]]
name = "docs"

[[collections]]
name = "pages"
```

You can start writing docs immediately. Open `content/docs/getting-started.md` and write markdown.

### Step 3: Deploy (60 Seconds)

Set your deploy target and base URL in `seite.toml`:

```toml
[site]
base_url = "https://my-docs.pages.dev"

[deploy]
target = "cloudflare"
project = "my-docs"
```

Then deploy:

```bash
seite deploy
```

One command. It builds the site, pushes to your remote, and deploys to [Cloudflare Pages](/blog/deploy-static-site-cloudflare-pages) (or [GitHub Pages or Netlify](/docs/deployment), your choice). The output includes HTML, markdown, sitemap, RSS, search index, llms.txt, and robots.txt. Three commands total, from nothing to a live URL.

When Priya joined a startup as the first backend engineer, the team's documentation lived in three places: a Notion doc with API specs, a Google Doc with onboarding steps, and a Slack thread that nobody could find. She needed a docs site before the next hire started Monday.

She didn't want to install Python or Node for a documentation tool. She wanted to write markdown and get a URL. Three commands later, she had a deployed docs site with sidebar nav and search. The new hire's first day started with a URL, not a scavenger hunt.

## Adding Your First Doc

Create a markdown file in `content/docs/` with YAML frontmatter:

```markdown
---
title: "API Reference"
description: "Complete REST API documentation for My Project"
weight: 2
---

## Authentication

All API requests require a Bearer token in the Authorization header.

```bash
curl -H "Authorization: Bearer YOUR_TOKEN" https://api.example.com/v1/users
`` `

## Endpoints

### GET /users

Returns a paginated list of users.
```

The `weight` field controls ordering in the sidebar. Lower numbers appear first. Without weights, docs sort alphabetically.

### Nested Docs

Create subdirectories for sections. The directory name becomes a section label in the sidebar:

```
content/docs/
  getting-started.md          → /docs/getting-started
  configuration.md            → /docs/configuration
  guides/
    authentication.md         → /docs/guides/authentication
    deployment.md             → /docs/guides/deployment
  api/
    endpoints.md              → /docs/api/endpoints
    webhooks.md               → /docs/api/webhooks
```

No configuration needed for nested docs. The directory structure maps directly to URLs and sidebar sections.

### The Shortcut

Skip the file creation and frontmatter manually:

```bash
seite new doc "API Reference"
# Creates content/docs/api-reference.md with frontmatter
```

See the [CLI reference](/docs/cli-reference) for all content creation commands.

## The Docs Theme

Apply the built-in docs theme with one command:

```bash
seite theme apply docs
```

This gives you a fixed 260px sidebar with auto-scrolling navigation, section grouping by directory, GitHub-style colors, and responsive collapse on mobile. Six bundled [themes](/docs/templates) are compiled into the binary; no npm install, no downloads ever. See [6 Themes, 0 Downloads](/blog/compiled-themes-binary) for the engineering rationale.

### Search Works Automatically

Every `seite build` generates a `search-index.json` file. The docs theme includes a search input wired to it. No configuration, no external search service, no API key. Type a query, get instant results from your docs.

### Syntax Highlighting Works Automatically

Fenced code blocks with language annotations get highlighted at build time via syntect. No plugin to install. No JavaScript highlighter to load. The highlighting happens during the build, so the rendered HTML includes styled code blocks with zero client-side cost.

**Want to customize the theme?** Edit `templates/base.html` directly or run `seite theme create "your description"` to [generate a custom theme with AI](/docs/custom-themes). Every theme preserves the SEO and accessibility fundamentals.

## Why You Can Build a Docs Site from the Command Line Without Node.js

If you're looking for the best documentation site generator, the first question to ask is: what do I have to install before I write a word? Here's a comparison that no documentation site generator CLI tutorial will show you:

| | MkDocs | Docusaurus | Hugo | seite |
|---|---|---|---|---|
| **Prerequisites** | Python 3.8+, pip | Node.js 18+, npm | Go (or binary) | None |
| **Install command** | `pip install mkdocs` | `npx create-docusaurus` | `brew install hugo` | `curl ...` one-liner |
| **Steps to first build** | 6 | 5 | 4 | 3 |
| **Dependency size** | ~50MB (pip packages) | ~200MB (node_modules) | ~15MB (binary) | ~15MB (binary) |
| **Deploy built-in** | No | No | No | Yes |
| **Markdown output** | No | No | No | Yes |
| **llms.txt** | No | No | No | Yes |
| **Search built-in** | Plugin required | Yes | Theme-dependent | Yes |

[MkDocs](https://www.mkdocs.org/getting-started/) is a solid tool. So is [Docusaurus](https://docusaurus.io/docs/installation). But they carry the weight of their ecosystems.

MkDocs requires Python, pip, and ideally a virtual environment to avoid polluting your system packages. If you're evaluating it as an MkDocs alternative, the question isn't whether MkDocs works well (it does), but whether you need Python on your machine to render markdown to HTML.

Docusaurus requires Node.js, npm, and scaffolds a React project with hundreds of transitive dependencies. As a Docusaurus alternative, seite trades React customization for zero-dependency simplicity. For a static documentation site that renders markdown to HTML, 200MB of `node_modules` is a lot of machinery.

The docs-as-code approach says your documentation should live in version control, written in markdown, built by a CLI tool, and deployed automatically. All four tools in the table above follow this pattern. The difference is how much baggage they bring to the workflow.

David picked Docusaurus for his team's API docs because it was the most popular option. The initial setup went fine. Six months later, Dependabot had opened 47 pull requests for security vulnerabilities in transitive dependencies. His docs site, which displayed static markdown pages, had a dependency tree deeper than his actual application.

When he audited `node_modules`, he found 1,200+ packages powering what was essentially a markdown-to-HTML converter. He switched to a single binary tool and closed all 47 PRs the same afternoon.

The [Stack Overflow 2025 Developer Survey](https://survey.stackoverflow.co/2025/) found that 22% of developers use static site generators for documentation. [Markdown is the most admired collaboration tool](https://survey.stackoverflow.co/2025/) for three years running. The tooling should match the simplicity of the format. If you want to create a documentation website from markdown, your docs site generator should be as simple as the content format itself.

Hugo deserves a mention here. It's also a single binary, fast builds, no runtime dependencies. It's a strong [documentation site generator](/blog/hugo-alternative). Where it falls short for docs specifically: no built-in deploy command, no AI-readable output, and configuring a docs sidebar requires more template work than most developers expect. If you're coming from WordPress or another CMS, the [WordPress alternative for developers](/blog/wordpress-alternative-for-developers) guide covers why static sites are the better path for docs and blogs alike.

## Docs That AI Can Read

Here's a question no other docs site tutorial asks: can ChatGPT read your documentation?

When a developer asks an AI coding assistant about your tool, the AI fetches your docs page and tries to extract useful content from HTML. It has to parse navigation menus, cookie banners, footer links, and sidebar chrome to find the actual documentation. Often, it gets confused.

seite solves this at the build level. Every docs page ships as both HTML and markdown:

```
dist/docs/
  getting-started.html    # For browsers
  getting-started.md      # For AI models
  api-reference.html
  api-reference.md
```

The [llms.txt](/blog/what-is-llms-txt) file lists every docs page with a link to its markdown version. An AI model can read `llms.txt`, find the relevant page, and fetch clean markdown without parsing HTML. This is [generative engine optimization](/blog/generative-engine-optimization) for your documentation; making your content citable by AI search engines and readable by AI coding assistants.

You can also use an AI coding agent to generate and maintain your documentation from the command line — see [how to build a website with an AI coding agent](/blog/how-to-build-website-ai-coding-agent) for the full `seite agent` workflow, or [how we built seite.sh with Claude Code](/blog/built-with-seite-claude-code) for an honest behind-the-scenes account.

68% of developers rely on technical documentation as their primary learning resource. Increasingly, they access that documentation through AI intermediaries. If your docs are locked inside HTML that AI models struggle to parse, you're invisible to a growing percentage of your users.

A markdown documentation site that outputs both formats gives you the best of both worlds. Browsers get styled, navigable HTML. AI gets clean, parseable markdown. Neither audience compromises for the other.

## Beyond Docs: The Full Site

Documentation rarely exists alone. Most projects need docs, a blog, a changelog, and a landing page. With separate tools, that means MkDocs for docs, Ghost for the blog, GitHub releases for the changelog, and Webflow for the landing page. Four tools, four deploys, four billing accounts.

seite's [collection system](/docs/collections) handles all of these from one project:

```bash
seite init my-project --title "Acme" --collections docs,posts,changelog,pages
```

That gives you:

| Collection | Directory | URL Pattern | Features |
|---|---|---|---|
| docs | `content/docs/` | `/docs/slug` | Nested sections, sidebar nav |
| posts | `content/posts/` | `/posts/slug` | Date-ordered, RSS feed |
| changelog | `content/changelog/` | `/changelog/slug` | Version tags, RSS feed |
| pages | `content/pages/` | `/slug` | Landing, about, contact |

One repo. One build command. One deploy. Every collection gets HTML, markdown, and search indexing automatically.

Lena runs a developer tools startup. When she launched, she used MkDocs for docs, Ghost for the blog, GitHub releases for version notes, and Carrd for the landing page. Four tools meant four places to update, four deploy workflows, and four sets of credentials to manage.

When her docs referenced a blog post, she linked to a completely different domain. When her blog mentioned a feature, she linked to a different site with a different design. She consolidated everything into one seite project. Same content, one domain, one `seite deploy`. Her docs link to her blog, her blog links to her changelog, and the whole site shares one design system. For the complete startup site workflow — landing page, docs, blog, changelog, and contact form from one init command — see [static site generator for startups](/blog/static-site-generator-for-startups).

## Frequently Asked Questions

### What's the fastest way to create a documentation site?

Use a static documentation site generator that requires no runtime dependencies. Build a documentation site with static output by scaffolding a project and deploying in one step. With seite: `curl install`, `seite init`, `seite deploy`. Three commands, about five minutes, zero prerequisites. The result includes sidebar navigation, full-text search, syntax highlighting, and AI-readable markdown output.

### Do I need Node.js for a docs site?

No. Tools like Docusaurus and VitePress require Node.js, but binary-based generators like seite and Hugo need nothing installed beyond the binary itself. seite installs via a single curl command and has no runtime dependencies.

### Can I add a blog alongside my docs?

Yes. Include both `docs` and `posts` in your collections when initializing: `seite init mysite --collections docs,posts`. Each collection has its own directory, URL prefix, template, and optional RSS feed. The docs collection uses a sidebar layout; the posts collection uses a date-ordered blog layout. Both share the same base theme.

### How do I deploy a docs site to GitHub Pages or Cloudflare?

Set the `[deploy]` section in `seite.toml` with your target (`github-pages`, `cloudflare`, or `netlify`) and run `seite deploy`. One command handles the build, commit, push, and upload. See the [deployment guide](/docs/deployment) for target-specific setup.

### Can AI assistants read my docs site?

Only if your docs are output as markdown alongside HTML. Most documentation site generators produce HTML only, which forces AI models to parse navigation chrome and page structure. seite outputs both formats plus llms.txt discovery files, so AI coding assistants and AI search engines can read your documentation directly.

## Your Docs Should Take 5 Minutes, Not 5 Hours

Three things to remember:

1. **Count the steps.** If your documentation site generator requires installing a language runtime before you write a word, you're solving the wrong problem first. A docs site should go from zero to deployed in three commands.
2. **Docs are part of a site, not a standalone project.** Your documentation lives alongside your blog, changelog, and landing page. Use a tool that handles all of them from one repo with one deploy.
3. **AI is reading your docs.** If your build only outputs HTML, AI models have to parse navigation menus and page chrome to find the content. Output markdown alongside HTML and include llms.txt so AI can read your docs cleanly.

Build a docs site from the command line right now:

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

Write your docs. Deploy them. [Ship it in five minutes.](/docs/getting-started)
