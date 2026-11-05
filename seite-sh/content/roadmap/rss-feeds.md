---
title: RSS Feeds
description: RSS 2.0 feeds per collection and per language with autodiscovery
tags:
- done
weight: 25
---

RSS 2.0 feed generation for collections with `has_rss: true`. Per-language feeds at `/{lang}/feed.xml`. Each item includes title, link, description, pubDate, and guid. RSS autodiscovery `<link>` tags in all theme `<head>` blocks. Atom 1.0 feeds (`atom.xml`) are generated alongside RSS (see [Atom Feed Format](/roadmap/atom-feeds)).
