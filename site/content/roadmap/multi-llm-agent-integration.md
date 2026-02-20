---
title: Multi-LLM Agent Integration
description: Support Claude Code, OpenCode, Codex CLI, and Gemini CLI as interchangeable agent backends
tags:
- planned
weight: 1
---

Support Claude Code, OpenCode, Codex CLI, and Gemini CLI as interchangeable agent backends. Currently `page agent` only works with Claude Code.

## Key Components

- **Provider abstraction** — `AgentProvider` trait with per-provider implementations for command construction, tool name mapping, streaming JSON parsing, and MCP config generation
- **`[agent]` config section** — provider selection and optional model override in `page.toml`
- **`AGENTS.md` migration** — migrate from `.claude/CLAUDE.md` to the cross-tool `AGENTS.md` standard
- **MCP config generation** — write provider-specific MCP config for all detected tools
- **CLI detection** — auto-detect installed agent tools during `page init`
- **`page agent switch`** — change default provider
- **`page agent doctor`** — health check for agent configuration
- **`page agent --provider`** — one-off provider override
- **Stream event parsers** — normalize provider-specific JSONL events to a common format
