---
title: AVIF Image Format
description: Generate AVIF variants alongside WebP in the image pipeline
tags:
- done
weight: 5
---

AVIF image generation is now supported. Enable with `avif = true` in `[images]` config. AVIF variants are generated alongside WebP at all configured widths, with configurable quality via `avif_quality` (default 70). The post-processing step adds AVIF sources to `<picture>` elements with the correct `type="image/avif"` attribute, ordered before WebP for optimal browser selection.
