---
title: Visual Editing Mode
description: "Low-barrier content editing without terminal or YAML — exploring AI-native approaches"
tags:
- planned
weight: 5
---

The #1 pain point across all SSG communities: editing content without touching a terminal, YAML frontmatter, or git. This is the largest addressable audience gap between SSGs and WordPress/AI app builders.

**Status: exploring approaches.** In the age of AI assistants, the traditional WYSIWYG editor may not be the right answer. We're evaluating several directions:

- **Deeper Claude Code integration**: using `seite agent` as the primary editing interface, where natural language replaces form fields
- **Browser-based preview with AI**: a `seite edit` mode that opens a live preview where users describe changes conversationally
- **Hybrid approach**: lightweight web UI for frontmatter and content, with AI assistance for layout and design decisions
- **MCP-powered editing**: leveraging the existing MCP server to enable editing from any AI-capable client

The goal is to find the approach that reduces activation energy to zero without building a full CMS. The AI-native angle may be seite's unique answer to this problem.
