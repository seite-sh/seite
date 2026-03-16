---
name: seite-templates
version: 1.0.0
description: Available themes and templates in seite.
---

# Seite Themes

Seite ships with 10 bundled themes. Each is a self-contained `base.html` template compiled into
the binary — no downloads needed.

## Apply a Theme

```bash
seite theme list            # see all available themes
seite theme apply <name>    # apply a bundled theme
```

This overwrites `templates/base.html`. Collection templates (`post.html`, `doc.html`, `page.html`)
are not affected.

## Create a Custom Theme

```bash
seite theme create "coral brutalist with lime accents"
```

This uses Claude Code to generate a custom `base.html` matching your description. Requires Claude Code.

---

## Bundled Themes

### `default`
Clean, readable theme with system fonts. Good starting point for any site.

### `minimal`
Ultra-minimal, typography-first theme. Serif body text, minimal UI chrome.

### `dark`
Dark mode theme, easy on the eyes. True black background with violet accent.

### `docs`
Documentation-focused theme with sidebar navigation. Best for doc-heavy sites.

### `brutalist`
Neo-brutalist theme with thick borders and hard shadows. Yellow accent, no border-radius.

### `bento`
Card grid layout inspired by bento box design. Rounded corners, soft shadows, mixed card sizes.

### `landing`
Marketing and landing page theme with hero sections and CTAs. Best for product pages.

### `terminal`
Monospace hacker theme with green-on-black terminal aesthetic.

### `magazine`
Multi-column editorial layout with featured articles. Best for content-heavy blogs.

### `academic`
Scholarly serif theme for research and long-form writing. Clean, citation-friendly.

---

## All Themes Include

Every bundled theme ships with:
- SEO meta tags (canonical, Open Graph, Twitter Card, JSON-LD)
- RSS autodiscovery
- Markdown alternate links (for LLM consumption)
- `llms.txt` discovery link
- Responsive design
- Client-side search
- Syntax highlighting for code blocks
- i18n support (language switcher, hreflang)
- Cookie consent banner (when analytics configured)
