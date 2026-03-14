---
title: Incremental Builds
description: Only rebuild changed pages in dev mode for faster iteration on large sites
tags:
- done
weight: 2
---

Only rebuild changed pages in dev mode. Track content file mtimes, template dependencies, and config changes to determine the minimum rebuild set. Critical for sites with 100+ pages where full rebuilds slow down the dev loop.

Shipped in v0.10.0. The dev server uses a build cache (`.seite/build-cache.json`) that tracks file mtimes and FNV-1a content hashes. Content-only changes skip re-rendering unchanged pages. Template, data, or config changes trigger full rebuilds for correctness.
