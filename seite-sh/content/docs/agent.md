---
title: "AI Agent"
description: "Use seite agent to get AI assistance with content creation, site management, and theme generation."
weight: 8
---

## Overview

`seite` integrates directly with Claude Code. The `seite agent` command spawns a Claude Code session pre-loaded with your site's full context — configuration, content inventory, templates, and available commands.

No API keys needed. It uses your Claude Code subscription directly.

## Setup

Install Claude Code:

```bash
npm install -g @anthropic-ai/claude-code
```

Verify it's available:

```bash
claude --version
```

## Interactive Mode

Launch an interactive session:

```bash
seite agent
```

Claude receives a rich system prompt containing:
- Your site config (title, description, base URL, collections)
- Content inventory (titles, dates, tags of every existing page)
- Available templates
- Frontmatter format with examples
- File naming conventions
- All `seite` CLI commands

You can ask it to write blog posts, reorganize content, update templates, debug build errors, or anything else.

{{% callout(type="info") %}}
The agent can run `seite build` and `seite serve` to verify its own changes. It will catch build errors, broken links, and template issues before you even look at the output.
{{% end %}}

## One-Shot Mode

Pass a prompt directly for non-interactive use:

```bash
seite agent "create a blog post about Rust error handling best practices"
seite agent "add a docs page explaining the deployment process"
seite agent "update the homepage to include a features section"
```

Claude writes the files directly and exits.

## Example Prompts

Here are prompts that work well, grouped by use case:

### Content creation

```bash
seite agent "write a technical tutorial about async Rust, include code examples and a summary"
seite agent "create a blog post comparing static site generators, with a table of features"
seite agent "add an FAQ page with 10 common questions about our product"
```

### Site management

```bash
seite agent "add the tag 'tutorial' to all posts that contain code blocks"
seite agent "create Spanish translations for all docs pages"
seite agent "reorganize the docs into guides/ and reference/ subdirectories"
```

### Theme and design

```bash
seite agent "update the homepage to add a testimonials section with three cards"
seite agent "add a custom footer with social links and a newsletter signup"
```

### Debugging

```bash
seite agent "the build is failing on my Spanish translations, help me fix it"
seite agent "find and fix any broken internal links in my content"
seite agent "my RSS feed is missing some posts, diagnose the issue"
```

## Tips for Effective Prompts

{{% callout(type="tip") %}}
Be specific about format and tone. "Write a 1500-word technical tutorial about Rust error handling with code examples" gives better results than "write a post about Rust".
{{% end %}}

{{% callout(type="tip") %}}
The agent already knows your site structure, collections, existing content, and frontmatter format. You don't need to explain where files go or what fields to include — just describe what you want.
{{% end %}}

## Theme Generation

Generate custom themes with AI:

```bash
seite theme create "dark mode with neon green accents and brutalist layout"
```

Claude receives detailed instructions about required template blocks, available variables, SEO requirements, search patterns, and accessibility features. It writes `templates/base.html` directly.

## REPL Integration

The dev server REPL also supports the agent:

```
seite> agent "write a post about our latest release"
```

This is useful for quick content creation while previewing your site.

## What the Agent Can Do

The agent has access to these tools:
- **Read** — read any file in your project
- **Write** — create new content files
- **Edit** — modify existing files
- **Glob** — find files by pattern
- **Grep** — search file contents
- **Bash** — run CLI commands (build, serve, deploy)

## Claude Code Scaffolding

When you run `seite init`, it creates `.claude/settings.json` with pre-configured permissions and a `CLAUDE.md` with site-specific instructions for the agent. This means Claude Code immediately understands your site's structure and conventions.

## Next Steps

- [Theme Gallery](/docs/theme-gallery) — browse bundled themes and generate custom ones with AI
- [CLI Reference](/docs/cli-reference) — all `seite agent` flags and options
- [Getting Started](/docs/getting-started) — initial site setup if you haven't started yet
