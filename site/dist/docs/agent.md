---
title: AI Agent
description: Use page agent to get AI assistance with content creation, site management, and theme generation.
---

## Overview

`page` integrates directly with Claude Code. The `page agent` command spawns a Claude Code session pre-loaded with your site's full context — configuration, content inventory, templates, and available commands.

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
page agent
```

Claude receives a rich system prompt containing:
- Your site config (title, description, base URL, collections)
- Content inventory (titles, dates, tags of every existing page)
- Available templates
- Frontmatter format with examples
- File naming conventions
- All `page` CLI commands

You can ask it to write blog posts, reorganize content, update templates, debug build errors, or anything else.

## One-Shot Mode

Pass a prompt directly for non-interactive use:

```bash
page agent "create a blog post about Rust error handling best practices"
page agent "add a docs page explaining the deployment process"
page agent "update the homepage to include a features section"
```

Claude writes the files directly and exits.

## Theme Generation

Generate custom themes with AI:

```bash
page theme create "dark mode with neon green accents and brutalist layout"
```

Claude receives detailed instructions about required template blocks, available variables, SEO requirements, search patterns, and accessibility features. It writes `templates/base.html` directly.

## REPL Integration

The dev server REPL also supports the agent:

```
page> agent "write a post about our latest release"
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

When you run `page init`, it creates `.claude/settings.json` with pre-configured permissions and a `CLAUDE.md` with site-specific instructions for the agent. This means Claude Code immediately understands your site's structure and conventions.
