---
title: "How to Build a Website with an AI Coding Agent"
date: 2026-03-13
description: "Use Claude Code to build a complete website in under an hour. seite's CLAUDE.md and MCP server give AI coding agents full site context from day one."
tags:
  - ai
  - static-site-generator
  - claude-code
  - tutorial
extra:
  primary_keyword: "how to build a website with AI coding agent"
  meta_title: "How to Build a Website with an AI Coding Agent | seite"
  meta_description: "Use Claude Code to build a complete website in under an hour. seite gives AI coding agents full site context via CLAUDE.md and MCP server. Tutorial."
  url_slug: "how-to-build-website-ai-coding-agent"
---

# How to Build a Website with an AI Coding Agent

Your AI coding agent already knows how to build websites. The problem is your static site generator doesn't tell it anything.

Claude Code, Cursor, GitHub Copilot — [82% of developers now use or plan to use AI coding tools](https://survey.stackoverflow.co/2024/ai/) — these tools are genuinely capable. They write clean code, follow patterns, understand context. But drop one into a folder with a [Hugo](/blog/hugo-alternative) or Eleventy project and the first five messages are the agent asking questions: where do posts go? What's the frontmatter format? How do I run the build? What's the deploy command? Every session starts with the same orientation, and the agent retains none of it.

Learning how to build a website with an AI coding agent requires two things most tutorials miss: a site generator that gives the agent structured context, and a workflow where the agent can take meaningful action instead of answering questions. This tutorial covers both. You'll build a complete site with landing page, blog, documentation and deploy using seite and Claude Code. The whole thing takes under an hour. The agent does most of the work.

## The Problem: AI Coding Agents Don't Understand Most Sites

Tavita is a developer who uses Claude Code daily for backend work. When he tried to apply the same workflow to his marketing site, a Hugo project, the experience was completely different.

Every session started the same way. He'd open Claude Code, ask it to write a blog post, and spend the next fifteen minutes answering orientation questions. Where do post files go? `content/posts/`. What's the date format in the filename? `YYYY-MM-DD`. What frontmatter fields does the theme expect? Three fields he had to look up. What's the build command? `hugo --minify`. After that setup, the agent could help, but it didn't retain context between sessions. The next day: same questions again.

This is not a failure of the AI. It is a failure of the site generator to communicate with the agent. Hugo was built before AI coding agents existed. It has no mechanism for telling an agent what the project looks like, what commands are available, or what conventions to follow.

The solution is not a better prompt. It's a site generator built for this workflow.

## How seite Solves the Context Problem

When you run `seite init`, the tool does something no other static site generator does: it generates a `.claude/CLAUDE.md` file that describes your entire site to AI agents. This file includes:

- Your collections with directory paths, URL patterns and date conventions
- The complete frontmatter schema for each content type
- Every available CLI command with flags and examples
- Your configured templates and their available variables
- Build output structure

The agent reads this file once and understands your project. No orientation questions. No guessing.

seite also starts an [MCP server](/docs/mcp-server) automatically when Claude Code opens your project. The MCP server uses the [Model Context Protocol](https://modelcontextprotocol.io/) to give the agent real-time, typed access to your content, config and build pipeline. Instead of asking "what's the build command?", the agent calls `seite_build` and gets back structured results including page count, build time and any errors.

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

This creates your `seite.toml` config, content directories, Tera templates, and critically, the `.claude/CLAUDE.md` context file and `.claude/settings.json` MCP server config. Open the project directory in Claude Code and the context file loads automatically.

### Step 3: Run the Agent

The **seite agent command** spawns [Claude Code](https://docs.anthropic.com/en/docs/claude-code/overview) with your full site context as the system prompt. See the [agent docs](/docs/agent) for all available flags and options:

```bash
seite agent
```

Or give it a specific task directly:

```bash
seite agent "create a landing page with hero section, feature list and pricing table"
seite agent "write three blog posts about our CLI tool's best features"
seite agent "build a documentation section covering installation, configuration and deployment"
```

The agent knows your content model, templates and CLI commands. It creates files in the correct directories with the right frontmatter, then runs `seite build` to verify the output.

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

The agent edits Tera templates with awareness of all available variables, block structure and SEO requirements. It will not accidentally remove the canonical URL tag or break the JSON-LD block because it knows they exist from the CLAUDE.md context.

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

She switched to seite. Ran `seite init` to create the project, then `seite agent "build my marketing site"` with a brief product description. The agent read `CLAUDE.md`, understood the collections and started creating files. Two hours later she had a complete site with landing page, three docs pages and two blog posts. She ran `seite deploy`. It was live.

The difference was not the agent's capability. It was the information available to the agent. Unlike a dedicated **Claude Code website builder** tool, seite keeps you in a standard developer workflow while giving the agent full project context. For founders who need a complete startup site live in an afternoon, the [static site generator for startups](/blog/static-site-generator-for-startups) guide covers the exact workflow with realistic time estimates.

| | Without seite (Hugo / Eleventy) | With seite |
|---|---|---|
| Agent orientation per session | 10-20 minutes | 0, reads CLAUDE.md once |
| Content file creation | Often wrong directory or format | Correct by default |
| Template editing | Risky without full variable context | Agent knows all template variables |
| Build verification | Manual | Agent calls `seite_build` via MCP |
| Deploy | Requires manual CI/CD setup | `seite deploy` one command |
| Session continuity | Agent forgets project structure | CLAUDE.md is always present |

## The MCP Server: Structured Access for Any AI Tool

Beyond `seite agent`, the [MCP server](/docs/mcp-server) makes seite's content and build pipeline accessible to any MCP-compatible tool.

When the MCP server is running (it starts automatically when Claude Code opens your project), AI tools can call `seite_build` to trigger a build and get structured results, call `seite_create_content` to create a properly formatted content file, call `seite_search` to find existing content by title or tag, and read `seite://config` for your site configuration as typed JSON.

This means your AI assistant works with site concepts, collections, content items, themes; instead of parsing raw files. The result is fewer errors and faster iteration.

Read the [full MCP server documentation](/docs/mcp-server) for all available tools and connection details.

## What to Expect: A Realistic Session Timeline

**0-5 minutes:** Install seite, run `seite init`, open Claude Code in the project directory.

**5-35 minutes:** Run `seite agent` with your site description. Agent creates landing page content, two or three docs pages, initial blog post. For a simple content site, this is mostly done.

**35-50 minutes:** Review with `seite serve`, iterate with REPL or `seite agent` for revisions. Apply a theme: `seite theme apply dark` or describe one with `seite theme create "minimal sans-serif with warm gray tones"`.

**50-60 minutes:** Run `seite deploy`. Site is live.

For building a docs site specifically, see [how to build a docs site from the command line](/blog/build-docs-site-command-line), the docs collection preset handles sidebar navigation, search and nested organization automatically. The AI agent workflow is the same; the collection does more heavy lifting out of the box.

## Frequently Asked Questions

### What AI coding agent works best with seite?

seite is optimized for [Claude Code](https://docs.anthropic.com/en/docs/claude-code/overview). The auto-generated CLAUDE.md follows Claude Code's context file conventions, and the MCP server uses the Model Context Protocol that Claude Code supports natively. The [`seite agent` command](/docs/agent) explicitly spawns Claude Code as a subprocess with full site context injected. It also works with any editor or agent that reads project documentation files, Cursor and similar tools read `.claude/CLAUDE.md` as project context.

### Do I need an API key to use the AI agent features?

`seite agent` uses your existing Claude Code subscription. No separate API key is needed. The MCP server similarly uses Claude Code's built-in authentication. seite itself is free and open source.

### How is this different from AI website builders like Relume?

AI website builders generate a finished site from a prompt. You get HTML files, no content model, no CLI. seite gives the agent a system to work within: markdown files, templates, a build pipeline, a deploy command. The agent creates and edits content inside a developer workflow you own. You get git history, reproducible builds and a site you control. See the [AI static site generator](/blog/ai-static-site-generator) article for the full architectural comparison.

### Can the agent work with a site that already exists?

Yes. Run `seite agent` in an existing seite project and the agent reads the current content inventory via the MCP server. It knows what already exists and can add to, edit or reorganize it. For migrating content from another SSG, `seite agent "migrate my existing markdown content and preserve frontmatter fields"` is a reliable starting point.

### What is the real experience like: what does the agent actually get right and wrong?

For an honest behind-the-scenes account of using seite and Claude Code to build a complete website — including what the agent produced well, what took longer than expected, and what we'd do differently — see [We Built Our Website with Our Own Tool (and Claude Code)](/blog/built-with-seite-claude-code).

### What's in the CLAUDE.md file and can I edit it?

`seite init` generates `.claude/CLAUDE.md` with your site's structure, commands and conventions. It's a regular markdown file, edit it to add project-specific context, team conventions or content guidelines. `seite upgrade` adds new sections non-destructively when seite adds new features. You can also add [path-scoped rules files](/docs/agent) in `.claude/rules/` for context that only loads when working with specific file types.

## Give Your Agent a Site It Can Work With

The missing ingredient in most guides on **how to build a website with an AI coding agent** is structured project context. The agent is not the bottleneck. The project is.

`seite init` gives the agent a CLAUDE.md it can read, an MCP server it can query and a CLI it can call. The agent stops asking questions and starts building.

Three things to remember:

1. **Context is the multiplier.** Agent output quality scales directly with the quality of project context available. CLAUDE.md and the MCP server provide that automatically.
2. **`seite agent` is a shortcut, not a requirement.** Claude Code, Cursor or any agent tool reads `.claude/CLAUDE.md` directly. The structured context works regardless of how you invoke the agent.
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
