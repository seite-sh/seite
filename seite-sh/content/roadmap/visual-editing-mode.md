---
title: Visual Editing Mode
description: "Low-barrier content editing without terminal or YAML — exploring AI-native approaches"
tags:
- planned
weight: 5
---

The #1 pain point across all SSG communities: editing content without touching a terminal, YAML frontmatter, or git. This is the largest addressable audience gap between SSGs and WordPress/AI app builders.

**Status: partly addressed.** Since v0.21.0, `seite cms setup pages-cms` or `seite cms setup decap` writes the configuration for a Git-based CMS from your collections and frontmatter schema, so non-technical editors can change content in a browser form and your normal deploy publishes it (see [CMS](/docs/cms)).

Still open:

- **Browser-based visual editing**: a `seite edit` mode that opens a live preview and edits the page in place
- **Hybrid approach**: lightweight local web UI for frontmatter and content without a hosted CMS

The goal is to reduce activation energy for editors to near zero without building a full CMS into seite.
