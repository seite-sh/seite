---
title: Multi-Author Support
description: Per-content author field with author archive pages, bios, and social links
tags:
- planned
weight: 9
---

Promote `author` from a site-level config field to a first-class per-content frontmatter field. Authors become entities with bios, avatars, and social links. Auto-generate per-author archive pages (like tag pages). Currently achievable via `extra.author` in frontmatter but without archive pages or structured data.

### Components

- `author` frontmatter field (string or object with name, bio, avatar, links)
- Author data file (`data/authors.yaml`) for centralized author profiles
- Auto-generated `/authors/{slug}/index.html` archive pages
- `Person` JSON-LD structured data per author
- Template variables: `page.author`, `site.authors`, author filtering in collections

Depends on or overlaps with the custom taxonomies feature.
