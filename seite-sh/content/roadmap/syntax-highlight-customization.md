---
title: Syntax Highlight Theme Customization
description: Configurable code block color themes to match site design
tags:
- planned
weight: 4
---

Make the Syntect highlighting theme configurable via `[build]` in `seite.toml` instead of hardcoded `base16-ocean.dark`. Ship a curated set of bundled themes (Dracula, Solarized, One Dark, GitHub, Nord) and allow custom `.tmTheme` files in the project. Small change — the Syntect infrastructure already exists.

```toml
[build]
syntax_theme = "dracula"  # or path to .tmTheme
```
