---
title: Multi-LLM Agent Integration
description: "Support Claude Code, Codex, OpenCode, Cursor, Gemini CLI, and GitHub Copilot as coding agents"
tags:
- done
weight: 1
---

Shipped in v0.20.0 (Codex, OpenCode, Cursor) and v0.21.0 (Gemini CLI, GitHub Copilot). Six coding agents work with a seite site: Claude Code, Codex, OpenCode, Cursor, Gemini CLI and GitHub Copilot.

## What shipped

- **`AGENTS.md`**: a compact, portable project context file every agent reads; Claude Code and Gemini CLI import it through a one-line `CLAUDE.md` / `GEMINI.md`
- **Per-agent setup**: `seite init --agents claude,codex,opencode,cursor,gemini,copilot` writes each agent's rules, skill and hook files; interactive init preselects the agents it finds installed, and `seite upgrade --agents …` adds more later
- **One routed `/seite` skill** (`$seite` in Codex) for checking, writing, theming, auditing and deploying
- **Check hooks**: after each edit and at the end of each turn, `seite check` hands errors back to the agent
- **Optional MCP config** for every agent with `--mcp`
- **`seite agent --with <agent>`** (or `SEITE_AGENT`): a thin launcher for any of the six

The provider abstraction with its own `[agent]` config section, `seite agent switch` and `seite agent doctor` were dropped in favor of each agent's own CLI and settings.
