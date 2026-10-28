---
title: Link Checking
description: "Post-build internal link validation with --strict mode for CI"
tags:
- done
weight: 23
---

`check_internal_links()` validates all internal `href` links against generated files after every build. Handles clean URLs, directory indices, fragments, query strings, and protocol-relative URLs. Results displayed with broken link counts and source file locations. `seite build --strict` treats broken links as build errors, failing CI pipelines. Subdomain link rewriting automatically adjusts cross-subdomain links to absolute URLs.

v0.21.0 added `seite check --external`, which checks external links in the rendered site (cached for 7 days, `[check] external_skip` for hosts to leave alone) and reports broken ones at the source line.
