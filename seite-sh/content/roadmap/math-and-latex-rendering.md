---
title: Math and LaTeX Rendering
description: Server-side KaTeX rendering for inline and display math blocks in markdown
tags:
- done
weight: 4
---

Server-side KaTeX rendering is now built in. Enable with `math = true` in `[build]` config. Supports `$inline$` and `$$display$$` math blocks, rendered to HTML during the markdown processing step. No client-side JavaScript required — all rendering happens at build time.
