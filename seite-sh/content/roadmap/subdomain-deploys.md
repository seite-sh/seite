---
title: Subdomain Deploys
description: Per-collection subdomain support for deploying collections to separate domains
tags:
- done
weight: 12
---

Per-collection subdomain support is now available. Set `subdomain = "docs"` on a collection to deploy it to `docs.example.com` with its own sitemap, RSS, robots.txt, and search index. Each subdomain collection gets a separate output directory (`dist-subdomains/{name}/`) and is deployed independently. Supports Cloudflare Pages and Netlify with per-collection `deploy_project` overrides. The dev server previews subdomain content at `/{name}-preview/`.
