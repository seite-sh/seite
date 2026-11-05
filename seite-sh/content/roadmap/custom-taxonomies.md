---
title: Custom Taxonomies
description: "User-defined taxonomies beyond tags, such as categories, plus series, with auto-generated listing pages and feeds"
tags:
- done
weight: 8
---

Shipped in v0.21.0. `[[taxonomies]]` in `seite.toml` adds groupings beyond the built-in `tags`, such as `categories`. Pages assign terms with a top-level frontmatter key (`categories: [Rust, Web]`). Each taxonomy gets a term index (`/categories/`), a page per term (optionally paginated), per-term RSS and Atom feeds, per-language versions and sitemap entries, rendered with the `taxonomy.html` and `term.html` templates. Tags keep working out of the box; custom taxonomies are opt-in.

Series shipped alongside: `series: "Rust from scratch"` with `series_order` fills `page.series` (parts, previous, next) and adds `/series/<slug>/`. Author archive pages are tracked separately under [multi-author support](/roadmap/multi-author).
