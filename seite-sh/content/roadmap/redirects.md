---
title: Redirects via Frontmatter
description: "Redirect old URLs to new ones via aliases in frontmatter, generating HTML meta-refresh files"
tags:
- planned
weight: 3
---

Support `aliases: ["/old-path", "/another-old-path"]` in frontmatter. During build, generate lightweight HTML files at each alias path containing a `<meta http-equiv="refresh">` redirect to the canonical URL. Essential for site migrations from WordPress or other SSGs without breaking inbound links and SEO equity. Also generate a `_redirects` file for Netlify/Cloudflare compatibility.
