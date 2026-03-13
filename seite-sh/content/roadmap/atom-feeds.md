---
title: Atom Feed Format
description: Generate Atom 1.0 feeds alongside RSS 2.0 for broader feed reader compatibility
tags:
- planned
weight: 4
---

Add Atom 1.0 feed generation alongside existing RSS 2.0. Atom provides stricter content typing, better i18n support, and is preferred by some feed readers and aggregators. Generate `atom.xml` per collection and per language, mirroring the existing `feed.xml` structure. Add Atom autodiscovery `<link>` tags in theme `<head>` blocks.
