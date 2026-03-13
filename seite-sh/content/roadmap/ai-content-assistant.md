---
title: AI Content Assistant
description: "seite agent with full site context, skill packs, streaming output, and REPL integration"
tags:
- done
weight: 19
---

`seite agent` spawns Claude Code as a subprocess with full site context — config, content inventory, templates, collections, shortcodes, and installed skill packs injected into the system prompt. Supports one-shot and interactive modes, streaming JSON output with real-time thinking/tool/text display, and session resume via `--resume`. Integrated into the dev server REPL. Skill packs (`seite skill install`) extend the agent with domain-specific commands and knowledge. Built-in skills include theme-builder, brand-identity, and landing-page.
