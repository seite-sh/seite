---
title: Related Posts
description: "Auto-suggest related content based on shared tags and taxonomy terms"
tags:
- done
weight: 6
---

Shipped in v0.21.0. `page.related` lists up to 3 pages from the same collection and language that share tags or [taxonomy](/docs/collections) terms; a shared category counts twice a shared tag, and ties go to the closest date. `[build] related = N` sets the size, and `0` (site-wide or per collection) turns it off. The default `post.html` shows a "Related" list, styled by every bundled theme.
