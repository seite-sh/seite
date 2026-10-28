---
title: AI Content Assistant
description: "Coding agents write and maintain site content through AGENTS.md, the /seite skill, check hooks, and skill packs"
tags:
- done
weight: 19
---

First shipped as `seite agent`, which spawned Claude Code with site context injected into the system prompt. Since v0.21.0 the context lives in the project instead, so any of the six supported coding agents can use it: a compact `AGENTS.md`, the routed `/seite` skill (`/seite new`, `/seite theme`, `/seite audit`, …), check hooks that hand errors back to the agent, and `seite context` for a once-per-session orientation. `seite agent` is now a thin launcher that passes your prompt to the agent as-is (`--with` picks the agent). Skill packs (`seite skill install seomachine`, `seite skill install impeccable`) add domain-specific skills. The earlier theme-builder, brand-identity and landing-page skills are folded into `/seite theme`, `/seite brand` and `/seite landing`.
