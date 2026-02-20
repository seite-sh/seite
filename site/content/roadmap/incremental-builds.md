---
title: Incremental Builds
description: Only rebuild changed pages in dev mode for faster iteration on large sites
tags:
- planned
weight: 2
---

Only rebuild changed pages in dev mode. Track content file mtimes, template dependencies, and config changes to determine the minimum rebuild set. Critical for sites with 100+ pages where full rebuilds slow down the dev loop.
