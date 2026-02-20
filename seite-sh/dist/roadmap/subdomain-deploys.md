---
title: Subdomain Deploys
description: Per-collection subdomain support for deploying collections to separate domains
tags:
- planned
weight: 11
---

Per-collection subdomain support (`subdomain = "docs"` on a collection maps to `docs.example.com`). Three phases: per-collection output directory override, per-collection base_url for URL resolution, and multi-deploy orchestration looping over subdomain configs.
