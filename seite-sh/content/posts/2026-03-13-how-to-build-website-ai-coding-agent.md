---
title: "How to Build a Website with an AI Coding Agent"
date: 2026-03-13
updated: 2026-10-29
description: "Use Claude Code or another coding agent to build a complete website in under an hour. seite's AGENTS.md, /seite skill and check hooks give it site context."
tags:
  - ai
  - static-site-generator
  - claude-code
  - tutorial
extra:
  primary_keyword: "how to build a website with AI coding agent"
  meta_title: "How to Build a Website with an AI Coding Agent | seite"
  meta_description: "Use Claude Code to build a complete website in under an hour. seite gives AI coding agents full site context via AGENTS.md and a /seite skill. Tutorial."
  url_slug: "how-to-build-website-ai-coding-agent"
---

Your AI coding agent already knows how to build websites. The problem is your static site generator doesn't tell it anything.

Claude Code, Cursor, GitHub Copilot — [76% of developers use or plan to use AI tools in their development process](https://survey.stackoverflow.co/2024/ai/), per Stack Overflow's 2024 survey — these tools are genuinely capable. They write clean code, follow patterns, understand context. But drop one into a folder with a [Hugo](/blog/hugo-alternative) or Eleventy project and the first five messages are the agent asking questions: where do posts go? What's the frontmatter format? How do I run the build? What's the deploy command? Every session starts with the same orientation, and the agent retains none of it.

Learning how to build a website with an AI coding agent requires two things most tutorials miss: a site generator that gives the agent structured context, and a workflow where the agent can take meaningful action instead of answering questions. This tutorial covers both. You'll build a complete site with landing page, blog, documentation and deploy using seite and Claude Code. The whole thing takes under an hour. The agent does most of the work.

## The Problem: AI Coding Agents Don't Understand Most Sites

Tavita is a developer who uses Claude Code daily for backend work. When he tried to apply the same workflow to his marketing site, a Hugo project, the experience was completely different.

Every session started the same way. He'd open Claude Code, ask it to write a blog post, and spend the next fifteen minutes answering orientation questions. Where do post files go? `content/posts/`. What's the date format in the filename? `YYYY-MM-DD`. What frontmatter fields does the theme expect? Three fields he had to look up. What's the build command? `hugo --minify`. After that setup, the agent could help, but it didn't retain context between sessions. The next day: same questions again.

This is not a failure of the AI. It is a failure of the site generator to communicate with the agent. Hugo was built before AI coding agents existed. It has no mechanism for telling an agent what the project looks like, what commands are available, or what conventions to follow.

The solution is not a better prompt. It's a site generator built for this workflow.

## How seite Solves the Context Problem

When you run `seite init`, the tool does something most static site generators don't: it generates a root `AGENTS.md` file that describes your entire site to AI agents, plus a one-line `CLAUDE.md` compatibility import. The canonical file includes:

- What the site is and how it's laid out
- Your collections with directory paths, URL patterns and date conventions
- The content format and frontmatter conventions
- Hard rules the agent must follow
- The workflow: run `seite context` once per session, then use the `/seite` skill

The agent reads this file once and understands your project. No orientation questions. No guessing.

`seite init` also writes a routed `/seite` skill (`$seite` in Codex): `/seite new`, `/seite check`, `/seite theme`, `/seite audit`, `/seite deploy` and more, each with its own reference file. And it installs check hooks: after each edit the agent's hook runs `seite check` on that file, and at the end of a turn the full check runs, so errors go straight back to the agent before it says it's done.

If you want an [MCP server](/docs/mcp-server) as well, `seite init --mcp` registers one for each agent. It uses the [Model Context Protocol](https://modelcontextprotocol.io/) to give the agent typed access to your content, config and build pipeline. It's optional: every MCP tool has a CLI equivalent with `--json`.

[Get started with seite](/docs/getting-started) and see the difference in the first session.

## How to Build a Website with an AI Coding Agent: Step by Step

Here is the full workflow. Install, initialize, describe what you want, review, build, deploy.

### Step 1: Install seite

seite is a single Rust binary. No Node.js, no runtime dependencies, no package manager.

```bash
# macOS / Linux
curl -fsSL https://seite.sh/install.sh | sh

# Windows
irm https://seite.sh/install.ps1 | iex
```

Verify the install:

```bash
seite --version
```

### Step 2: Initialize Your Project

```bash
seite init mysite --title "My Company" --collections posts,docs,pages
cd mysite
```

This creates your `seite.toml` config, content directories, Tera templates, and critically, the root `AGENTS.md` context file plus the skill, rules, permissions and check hooks for each coding agent: Claude Code, Codex, OpenCode, Cursor, Gemini CLI and GitHub Copilot (pick with `seite init --agents`). Open the project directory in your agent and the context loads automatically.

### Step 3: Run the Agent

Open your coding agent in the project directory, for example [Claude Code](https://docs.anthropic.com/en/docs/claude-code/overview):

```bash
claude
```

Or let seite launch it with a task. The **seite agent command** passes your prompt to the agent as-is; `--with` picks another agent. See the [agent docs](/docs/agent) for all flags:

```bash
seite agent "create a landing page with hero section, feature list and pricing table"
seite agent --with codex "write three blog posts about our CLI tool's best features"
seite agent --with gemini "build a documentation section covering installation, configuration and deployment"
```

The agent knows your content model, templates and CLI commands from `AGENTS.md` and the `/seite` skill. It creates files in the correct directories with the right frontmatter, and the check hooks report any errors before it finishes.

### Step 4: Preview and Iterate

After the agent finishes, review with `seite serve`:

```bash
seite serve
```

The dev server starts at `http://localhost:3000` with live reload. Use the interactive REPL for quick changes without leaving the terminal:

```
> new post "Why We Built This"
> agent "improve the pricing section to be more conversion-focused"
> build
```

### Step 5: Deploy

When the site looks right:

```bash
seite deploy
```

One command handles commit, push, build and deploy to GitHub Pages, Cloudflare Pages or Netlify. See the [Cloudflare Pages walkthrough](/blog/deploy-static-site-cloudflare-pages) for first-time setup details.

## What an AI Coding Agent Can Do with Full Site Context

When the agent has your site structure, it stops answering questions and starts doing work.

### Create and Edit Content

The agent creates properly formatted content files with correct frontmatter, appropriate tags and content that matches the site's tone. It edits existing content just as easily.

```bash
seite agent "rewrite the about page to focus on the founding story"
seite agent "add a FAQ section to the getting-started docs page"
seite agent "create this month's content calendar: five posts on developer productivity"
```

### Build and Modify Templates

The agent edits Tera templates with awareness of all available variables, block structure and SEO requirements. It will not accidentally remove the canonical URL tag or break the JSON-LD block because it knows they exist from the AGENTS.md context.

```bash
seite agent "add a table of contents to the doc template"
seite agent "create a changelog collection with a custom template"
```

### Debug Build Errors

When a build fails, the agent has the full error output and your site structure simultaneously. It identifies template syntax errors, missing frontmatter fields or broken internal links and fixes them directly.

### Handle SEO Tasks

```bash
seite agent "audit all posts for missing description frontmatter and fill them in"
seite agent "add internal links between the docs pages that cover related topics"
```

For [generative engine optimization](/blog/generative-engine-optimization) work, the agent understands the llms.txt format seite generates and can optimize its content directly.

## How to Build a Website with an AI Coding Agent vs. Hugo or Eleventy

Jess runs a developer tools startup. She needed a website before her product launch in two weeks, landing page, documentation, blog, and wanted to use the same AI coding agent workflow she used for everything else.

Her first attempt was with a popular Eleventy starter. She spent an afternoon with Claude Code trying to understand the project structure, fighting template inheritance, getting the agent to produce content in the right format. The agent kept creating files in wrong directories and generating frontmatter that didn't match the theme's expectations. After three hours she had a half-working homepage.

She switched to seite. Ran `seite init` to create the project, then `seite agent "build my marketing site"` with a brief product description. The agent read `AGENTS.md`, understood the collections and started creating files. Two hours later she had a complete site with landing page, three docs pages and two blog posts. She ran `seite deploy`. It was live.

The difference was not the agent's capability. It was the information available to the agent. Unlike a dedicated **Claude Code website builder** tool, seite keeps you in a standard developer workflow while giving the agent full project context. For founders who need a complete startup site live in an afternoon, the [static site generator for startups](/blog/static-site-generator-for-startups) guide covers the exact workflow with realistic time estimates.

| | Without seite (Hugo / Eleventy) | With seite |
|---|---|---|
| Agent orientation per session | 10-20 minutes | 0, reads AGENTS.md once |
| Content file creation | Often wrong directory or format | Correct by default |
| Template editing | Risky without full variable context | Agent knows all template variables |
| Build verification | Manual | Hooks run `seite check` after each edit and before the agent finishes |
| Deploy | Requires manual CI/CD setup | `seite deploy` one command |
| Session continuity | Agent forgets project structure | AGENTS.md is always present |

## The MCP Server: Structured Access for Any AI Tool

The optional [MCP server](/docs/mcp-server) makes seite's content and build pipeline accessible to any MCP-compatible tool, including Claude Desktop.

When the MCP server is running (enable it with `seite init --mcp` or `seite upgrade --mcp`, and your coding agent starts it when it opens the project), AI tools can call `seite_check` to validate the whole site with file-and-line diagnostics, `seite_build` to trigger a build and get structured results, `seite_create_content` to create a properly formatted content file, `seite_search` to find existing content by title or tag, and read `seite://config` for your site configuration as typed JSON.

This means your AI assistant works with site concepts, collections, content items, themes; instead of parsing raw files. The result is fewer errors and faster iteration.

Read the [full MCP server documentation](/docs/mcp-server) for all available tools and connection details.

## What to Expect: A Realistic Session Timeline

**0-5 minutes:** Install seite, run `seite init`, open your coding agent in the project directory.

**5-35 minutes:** Give the agent your site description. Agent creates landing page content, two or three docs pages, initial blog post. For a simple content site, this is mostly done.

**35-50 minutes:** Review with `seite serve`, iterate in the same agent session. Apply a theme: `seite theme apply dark` or describe one with `/seite theme minimal sans-serif with warm gray tones`.

**50-60 minutes:** Run `seite deploy`. Site is live.

For building a docs site specifically, see [how to build a docs site from the command line](/blog/build-docs-site-command-line), the docs collection preset handles sidebar navigation, search and nested organization automatically. The AI agent workflow is the same; the collection does more heavy lifting out of the box.

## Frequently Asked Questions

### What AI coding agent works best with seite?

seite sets up six: [Claude Code](https://docs.anthropic.com/en/docs/claude-code/overview), Codex, OpenCode, Cursor, Gemini CLI and GitHub Copilot. Each gets the same `AGENTS.md` (Claude Code and Gemini CLI through a one-line import), the `/seite` skill (`$seite` in Codex) and check hooks in its own format. The [`seite agent` command](/docs/agent) starts Claude Code by default; `--with` picks another. Use the one you already work in.

### Do I need an API key to use the AI agent features?

No. seite uses the coding agent you already run, with its own sign-in or subscription, and needs no API key or paid service of its own. seite itself is free and open source.

### How is this different from AI website builders like Relume?

AI website builders generate a finished site from a prompt. You get HTML files, no content model, no CLI. seite gives the agent a system to work within: markdown files, templates, a build pipeline, a deploy command. The agent creates and edits content inside a developer workflow you own. You get git history, reproducible builds and a site you control. See the [AI static site generator](/blog/ai-static-site-generator) article for the full architectural comparison.

### Can the agent work with a site that already exists?

Yes. Open your agent in an existing seite project; `seite context` gives it the current collections and content counts, and `seite search` finds existing pages. It knows what already exists and can add to, edit or reorganize it. For migrating from another platform, `seite import wordpress|ghost|jekyll|hugo` converts the content and keeps old URLs working, and writes a report your agent can work through.

### What is the real experience like: what does the agent actually get right and wrong?

For an honest behind-the-scenes account of using seite and Claude Code to build a complete website — including what the agent produced well, what took longer than expected, and what we'd do differently — see [We Built Our Website with seite and Claude Code](/blog/built-with-seite-claude-code).

### What's in the AGENTS.md file and can I edit it?

`seite init` generates a compact root `AGENTS.md`: the workflow (run `seite context`, use the `/seite` skill, verify with `seite check`), the project layout, your collections, the content format and a few rules, plus `CLAUDE.md` and `GEMINI.md` files that import it. Commands live in the `/seite` skill rather than in `AGENTS.md`. Edit `AGENTS.md` to add project-specific context, team conventions or content guidelines; `seite upgrade` refreshes only its own marked blocks and keeps your edits. Path-scoped [rules files](/docs/agent) (`.claude/rules/` for Claude Code, `.cursor/rules/` for Cursor, `.github/instructions/` for Copilot) add context that only loads when working with matching files.

## Give Your Agent a Site It Can Work With

The missing ingredient in most guides on **how to build a website with an AI coding agent** is structured project context. The agent is not the bottleneck. The project is.

`seite init` gives the agent an AGENTS.md it can read, a `/seite` skill it can follow, hooks that check its work and a CLI it can call. The agent stops asking questions and starts building.

Three things to remember:

1. **Context is the multiplier.** Agent output quality scales directly with the quality of project context available. AGENTS.md, the `/seite` skill and the check hooks provide that automatically.
2. **`seite agent` is a shortcut, not a requirement.** Coding agents read `AGENTS.md` directly (Claude Code and Gemini CLI through a one-line import), and `/seite` works in any of the six. The structured context works regardless of how you invoke the agent.
3. **The workflow stays yours.** Git history, markdown files, CLI commands. AI works within your developer workflow, not as a replacement for it.

Build your first AI-assisted site:

```bash
curl -fsSL https://seite.sh/install.sh | sh
seite init mysite --title "My Site" --collections posts,docs,pages
cd mysite
seite agent "build my site"
seite deploy
```

Ready to **build a website with Claude Code**? See the [CLI reference](/docs/cli-reference) for all available agent commands and options.
