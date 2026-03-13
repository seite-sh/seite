---
title: Scheduled Publishing
description: "Filter content with future publish_date from production builds, enabling schedule-ahead workflows"
tags:
- planned
weight: 2
---

Add `publish_date` frontmatter field to complement existing `draft` support. Content with a `publish_date` in the future is excluded from production builds but visible in dev mode. Enables write-ahead workflows where authors schedule posts days or weeks in advance, and CI rebuilds surface them on the right date. Small extension of the existing draft filtering logic in the build pipeline.
