---
title: Plugin and Extension System
description: Build-time hooks and lifecycle events for community-built extensions
tags:
- planned
weight: 6
---

A formal plugin system that lets users extend the build pipeline without forking. Hugo's and Zola's #1 perennial community request. Users who hit a wall in a non-extensible SSG leave and don't return.

### Possible approach

- **Lifecycle hooks**: `before_build`, `after_markdown`, `after_render`, `after_build` events
- **Hook-based shortcode registration**: plugins can register custom shortcodes with processing logic beyond template rendering
- **WASM or script plugins**: execute user-defined transformations during build steps
- **Plugin manifest**: `[plugins]` section in `seite.toml` with install-from-URL support

Current extensibility points (user shortcodes in `templates/shortcodes/`, data files, skill packs) cover many use cases but don't allow build-pipeline modifications.
