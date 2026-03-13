---
title: Link Checking
description: "Post-build internal link validation with --strict mode for CI"
tags:
- done
weight: 23
---

`check_internal_links()` validates all internal `href` links against generated files after every build. Handles clean URLs, directory indices, fragments, query strings, and protocol-relative URLs. Results displayed with broken link counts and source file locations. `seite build --strict` treats broken links as build errors, failing CI pipelines. Subdomain link rewriting automatically adjusts cross-subdomain links to absolute URLs.
