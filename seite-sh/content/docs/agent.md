---
title: "AI Agent for Your Static Site"
description: "Let Claude write content, generate themes, debug builds, and manage your static site generator project, all with full project context. No API keys required."
weight: 9
---

Most static site generators treat AI as an afterthought: a plugin, a separate API call, a bolted-on chatbot. seite takes a different approach. The `seite agent` command gives Claude Code full context about your site's configuration, content, templates, and conventions. It reads your project, understands the structure, and writes files that actually fit. No other static site generator ships with this kind of native AI integration.

## Overview

`seite` integrates directly with Claude Code. The `seite agent` command spawns a Claude Code session pre-loaded with your site's live context: config, collections, content inventory, and templates. Your project's `AGENTS.md` (created by `seite init`) already covers conventions like content format and file naming, so Claude Code loads that on its own — the agent only adds what changes at runtime. This context is also exposed to other AI tools through the [MCP server](/docs/mcp-server), so your site stays AI-accessible beyond just the agent.

No API keys needed. It uses your Claude Code subscription directly. You can also point `seite agent` at Codex, opencode, or Cursor with `--with` — see [Other Agent Harnesses](#other-agent-harnesses).

## Setup

Install Claude Code (see the [getting started guide](/docs/getting-started) if you haven't set up your project yet):

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

Claude receives a system prompt with the live, per-project context it can't get any other way:
- Site config (title, base URL, language)
- Collections table (directory, URL prefix, dated, nested)
- Content inventory (titles, dates, tags of every existing page)
- Available templates
- A pointer to `AGENTS.md` for everything else (content format, file naming, available commands, shortcodes)

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
The agent already knows your site structure, collections, existing content, and frontmatter format. You don't need to explain where files go or what fields to include, just describe what you want.
{{% end %}}

## Theme Generation

Generate custom themes with AI:

```bash
seite theme create "dark mode with neon green accents and brutalist layout"
```

Claude receives detailed instructions about required template blocks, available variables, SEO requirements, search patterns, and accessibility features. It writes `templates/base.html` directly. For more on themes, see the [theme gallery](/docs/theme-gallery).

## Other Agent Harnesses

`seite agent` defaults to Claude Code, but you can drive other coding agents with the same live site context via `--with`:

```bash
seite agent --with codex "write a post about our latest release"
seite agent --with opencode "reorganize the docs into guides/ and reference/"
seite agent --with cursor "add a testimonials section to the homepage"
```

Set `SEITE_AGENT` to change the default without passing `--with` every time:

```bash
export SEITE_AGENT=codex
seite agent "fix the broken links in my content"
```

Each harness needs its own CLI installed and on `PATH`:

| Harness | Binary | Install |
|---|---|---|
| `claude` (default) | `claude` | `npm install -g @anthropic-ai/claude-code` |
| `codex` | `codex` | `npm install -g @openai/codex` |
| `opencode` | `opencode` | `curl -fsSL https://opencode.ai/install \| bash` |
| `cursor` | `cursor-agent` | `curl https://cursor.com/install -fsS \| bash` |

If the binary isn't found, `seite agent` fails immediately with the install command instead of hanging or falling back silently.

Codex has no system-prompt flag, so for `codex` and `opencode`/`cursor` the site context is prepended to the prompt itself, clearly delimited from your instructions. Auto-approving a harness's own side effects (writes, MCP servers) stays opt-in: pass the global `-y`/`--yes` flag to also pass `opencode run --auto` or `cursor-agent --force --approve-mcps`.

## Stop Hooks

`seite init` installs a turn-end hook for each selected coding agent. When the agent finishes a turn, the hook runs `seite check --hook <agent>`. If the site has **errors**, the agent gets the compiler-style diagnostics back (at most 40 lines) and keeps working until they're fixed; warnings never block, and a clean site produces no output at all.

| Agent | Hook lives in | Notes |
|---|---|---|
| Claude Code | `.claude/settings.json` → `hooks.Stop` | Shown to Claude as "Stop hook feedback" |
| Codex CLI | `.codex/hooks.json` → `hooks.Stop` | Codex runs project hooks only after you review and trust them once: open `/hooks` in Codex (they're skipped until then) |
| Cursor | `.cursor/hooks.json` → `hooks.stop` | Needs a trusted workspace. Fires in the editor and interactive `cursor-agent`; `cursor-agent -p` doesn't run `stop` hooks |
| OpenCode | `.opencode/plugins/seite-check.js` | A small plugin that checks on `session.idle` and sends the errors back as a prompt. Works in the TUI and `opencode serve`; one-shot `opencode run` exits before plugins finish |

Loops can't run away: each hook hands the errors back at most once per turn (Claude's and Codex's `stop_hook_active`, Cursor's `loop_count`, the plugin's own flag), then lets the agent stop. The hook exits 0 on every path, stays silent outside a seite site, and never fails the session if seite itself breaks. The hook runs the `seite` found on the agent's `PATH` (often your login shell's), so keep that binary current.

**Opting out:** delete the hook entry (or the plugin file). `seite upgrade` records which hooks it has installed in `.seite/config.json` (`hooks_installed`) and adds a hook only to agents that never had one, so a hook you removed stays removed. Existing projects get the hooks on their next `seite upgrade`, merged into existing hook configs without touching your other hooks.

## REPL Integration

The dev server REPL also supports the agent:

```
seite> agent "write a post about our latest release"
```

This is useful for quick content creation while previewing your site.

## What the Agent Can Do

With Claude Code, the agent has access to these tools, scoped to content and theme work:
- **Read**: read any file in your project
- **Write**: create new content files
- **Edit**: modify existing files
- **Glob**: find files by pattern
- **Grep**: search file contents
- **`mcp__seite`**: every tool of the [seite MCP server](/docs/mcp-server)
- **Bash**, limited to `seite <anything>`, `git status`, `git diff`, `git log`, and `ls` — no general shell access

This keeps the agent able to write content, run builds, and inspect git state without handing it an unrestricted shell.

### Concrete example: writing a blog post

Without the agent, creating a well-structured post means remembering the frontmatter format, putting the file in the right directory with the right filename pattern, picking appropriate tags from your existing taxonomy, and making sure the description is under 160 characters for SEO.

With the agent, you say:

```bash
seite agent "write a post about our new pricing model, mention the 3 tiers, \
  keep the tone conversational, and tag it appropriately"
```

The agent checks your existing posts for tone and tag conventions, creates `content/posts/2026-03-13-new-pricing-model.md` with correct frontmatter (title, date, description, tags), writes the body matching your site's voice, and runs `seite build` to verify everything compiles cleanly. One command, no manual scaffolding.

## How It Compares

Other static site generators require you to set up AI workflows yourself: connect to an LLM API, write prompts that describe your project structure, and hope the output matches your conventions. If your site uses Hugo, you need to explain Hugo's content model. If you use Jekyll, you explain Jekyll's. Every time.

seite's agent understands your project natively. It knows the [configuration](/docs/configuration) format, the collection types, your existing content, and the template system. The system prompt is generated from your actual `seite.toml` and content directory, not a generic "you are a helpful assistant" preamble. This means the agent produces files that build on the first try, use your existing tag taxonomy, and follow your site's established patterns.

## Agent instruction scaffolding

When you run `seite init`, it creates an `AGENTS.md` with site-specific instructions — the one canonical instruction file that Claude Code (through a one-line `CLAUDE.md` import), Codex, OpenCode, and Cursor all read. It then sets the site up for each coding agent you pick with `--agents` (default: all four), generating every agent's files from the same bundled content:

- **Claude Code** — `.mcp.json` (declares the seite MCP server), `.claude/settings.json` (permissions plus `enabledMcpjsonServers`), path-scoped guides in `.claude/rules/`, and skills in `.claude/skills/`
- **Cursor** — `.cursor/mcp.json` and the same guides as `.cursor/rules/*.mdc` (auto-attached by `globs`)
- **Codex CLI** — `.codex/config.toml` with `[mcp_servers.seite]`
- **OpenCode** — `opencode.json` with the MCP server and permission defaults (allow reads, site edits, and `seite build/new/serve/theme`; ask for everything else; deny `.env` reads)

Codex, Cursor, and OpenCode read the bundled skills (`/theme-builder`, `/brand-identity`, `/landing-page`) from `.agents/skills/`. Agents without path-scoped rules (Codex, OpenCode) find the guides through the "Context Rules" index in `AGENTS.md`, and the "MCP Server" section lists each agent's one-time approval step.

`seite agent` itself drives Claude Code; the other agents work in the project directly.

Existing sites can migrate without losing their custom instructions:

```bash
seite upgrade
```

The upgrade moves existing project guidance from `CLAUDE.md` to `AGENTS.md`, then replaces `CLAUDE.md` with the `@AGENTS.md` compatibility import. If both files already exist, Seite preserves the Claude-specific content and adds the import. It also adds any missing files for the selected agents (older sites are treated as `all`), merging into existing configs rather than replacing them. Change the selection with `seite upgrade --agents claude,cursor`; files of deselected agents are left in place but no longer maintained.

## Next Steps

- [Theme Gallery](/docs/theme-gallery): browse bundled themes and generate custom ones with AI
- [CLI Reference](/docs/cli-reference), all `seite agent` flags and options
- [Getting Started](/docs/getting-started): initial site setup if you haven't started yet
- [AI Static Site Generator: What It Means and Why It Matters](/blog/ai-static-site-generator): how AI agent integration fits into the bigger picture of AI-native site generation
