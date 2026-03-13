---
title: Draft Support
description: "Draft content filtered from production builds, visible with --drafts flag"
tags:
- done
weight: 26
---

Content with `draft: true` in frontmatter is excluded from production builds. The `--drafts` flag on `seite build` and `seite serve` includes draft content for local preview. Scheduled publishing (future-date filtering via `publish_date`) tracked separately as a planned enhancement.
