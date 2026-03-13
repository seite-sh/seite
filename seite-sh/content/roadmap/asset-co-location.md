---
title: Asset Co-location
description: Place images and files alongside content markdown in the same directory
tags:
- planned
weight: 7
---

Allow `content/posts/my-post/index.md` to reference `./hero.jpg` from the same directory instead of requiring all images in `static/`. Modern SSGs (Astro, Hugo page bundles) support this natively. Reduces friction: authors keep content and its assets together.

### Implementation

- Detect `index.md` inside a subdirectory as a page bundle
- Resolve relative image paths against the content file's directory
- Copy co-located assets to the output directory alongside the rendered HTML
- Feed co-located images into the existing responsive image pipeline (srcset, WebP, AVIF)
- Preserve backward compatibility with `static/` references
