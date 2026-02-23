---
title: Environment-Aware Builds
description: Auto-detect CI environments and support multi-environment config sections
tags:
- planned
weight: 10
---

Detect CI environment variables (`GITHUB_ACTIONS`, `NETLIFY`, `CF_PAGES`) and auto-configure behavior — skip prompts, use env secrets for base_url. Support `[deploy.production]` and `[deploy.staging]` config sections with different base_url, targets, and settings.
