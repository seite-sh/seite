---
title: Shortcodes and Template Customization
description: Built-in and user-defined shortcodes with inline and body syntax for extending content without ejecting themes
tags:
- done
weight: 15
---

6 built-in shortcodes (youtube, vimeo, gist, callout, figure, contact_form) with two syntax forms: inline `{{</* name(args) */>}}` and body `{{% name(args) %}} content {{% end %}}`. Users can add custom shortcodes by placing `.html` files in `templates/shortcodes/` — built-ins can be overridden by name. Named arguments support string, integer, float, and boolean types. Page and site context are available inside shortcode templates.
