---
name: seite-templates
version: 0.21.0
description: Available themes and templates in seite.
---

# Seite Themes

Seite ships with 11 bundled themes: 4 starters and 7 gallery examples. Each is a self-contained
`base.html` template compiled into the binary — no downloads needed. Preview them all at
https://seite.sh/docs/theme-gallery.

## Apply a Theme

```bash
seite theme list            # starters, gallery, installed themes
seite theme apply <name>    # apply a bundled or installed theme
seite init mysite --theme blank   # pick a starter at init: default, docs, landing, blank
```

This overwrites `templates/base.html` (a customized one is backed up). Collection templates
(`post.html`, `doc.html`, `page.html`) are not affected.

## Design Your Own Theme

```bash
seite theme new             # scaffold the blank starter: templates/base.html + static/css/site.css
```

Then ask your coding agent: `/seite theme "coral brutalist with lime accents"` (Claude Code, Codex
CLI, OpenCode, Cursor, Gemini CLI or GitHub Copilot; `$seite theme …` in Codex). With the optional
impeccable skill installed (`seite skill install impeccable`), it uses `/impeccable` for the design.
From a terminal outside an agent session, `seite theme create "<description>"` starts your coding
agent on the same task (with impeccable installed, it prints the `/impeccable` steps instead).

---

## Starters

### `default`
Clean, readable theme with system fonts. Good starting point for any site.

### `docs`
Documentation-focused theme with sidebar navigation. Best for doc-heavy sites.

### `landing`
Marketing and landing page theme with hero sections and CTAs. Best for product pages.

### `blank`
Bare semantic layout with its CSS in `static/css/site.css`, made for designing your own theme.

## Gallery

### `minimal`
Ultra-minimal, typography-first theme. Serif body text, minimal UI chrome.

### `dark`
Dark mode theme, easy on the eyes. True black background with violet accent.

### `brutalist`
Neo-brutalist theme with thick borders and hard shadows. Yellow accent, no border-radius.

### `bento`
Card grid layout inspired by bento box design. Rounded corners, soft shadows, mixed card sizes.

### `terminal`
Monospace hacker theme with green-on-black terminal aesthetic.

### `magazine`
Multi-column editorial layout with featured articles. Best for content-heavy blogs.

### `academic`
Scholarly serif theme for research and long-form writing. Clean, citation-friendly.

---

## All Themes Include

Every bundled theme includes seite's `seite/…` head and script partials, so it ships with:
- SEO meta tags (canonical, Open Graph, Twitter Card, JSON-LD)
- RSS autodiscovery
- Markdown alternate links (for LLM consumption)
- `llms.txt` discovery link
- Responsive design
- Client-side search (full-text with `[build] search = "full"`)
- Syntax highlighting for code blocks
- i18n support (language switcher, hreflang)
- Cookie consent banner (when analytics configured)
