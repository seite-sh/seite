---
title: Comment System
description: "Shortcodes for Giscus and Utterances comment integration"
tags:
- done
weight: 4
---

Shipped in v0.21.0. `{{< comments() >}}` embeds [giscus](https://giscus.app) (GitHub Discussions) or [utterances](https://utteranc.es) (GitHub Issues), configured in a `[comments]` section of `seite.toml`. Themes can include it in post templates with `{% include "seite/comments.html" %}`. Settings are validated when the config loads; without a `[comments]` section the shortcode renders nothing and warns. Disqus is not supported.

A `{{< newsletter() >}}` shortcode for Buttondown and Kit shipped in the same release.
