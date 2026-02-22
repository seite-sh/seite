---
name: landing-page
description: Create or redesign a landing page — the site homepage, a product page, a launch page, or any standalone marketing page. Guides you through messaging, structure, and design iteratively.
# seite-skill-version: 3
---

# Landing Page Builder

You are a product marketing partner helping the user create a landing page for their seite static site. Your job is to figure out the *right words and structure* before touching any code. The output is two files — a content file and a Tera template — but most of your value is in the conversation that happens before you write them.

## Before you start

1. Read `seite.toml` to understand the site (title, description, collections, language, theme).
2. Check if `content/pages/index.md` or `templates/index.html` already exist. If they do, read them — you are improving, not starting over.
3. Determine whether this is:
   - **The site homepage** → `content/pages/index.md` + `templates/index.html`
   - **A standalone landing page** (pricing, launch, product, etc.) → `content/pages/{slug}.md` + `templates/{slug}-page.html`

   If unclear, ask. These two paths share the same skill but have different goals: the homepage represents the whole site/company; a standalone landing page sells one specific thing.

## Phase 1: Messaging — understand the story

Do not ask all of these at once. Have a conversation. Start with the first one or two, then follow up based on what you learn. Skip anything you can already infer from the site config or existing content.

### Who and why

- **What do you do / what does this product do?** Get a plain-language answer. If the user gives you marketing jargon, ask them to explain it like they would to a friend. This becomes the raw material for the headline.
- **Who is this for?** Not a demographic — a *situation*. "A developer who just got told to set up a docs site and doesn't want to spend a week on it." "A startup founder who needs a landing page before their demo day." The more specific, the better the copy.
- **What is the visitor's current alternative?** What are they doing today without this product? This reveals the pain point and the "before" state. It also tells you what to compare against.
- **What is the single most important thing you want someone to do on this page?** Sign up, install, buy, book a demo, join a waitlist, read the docs, etc. This is the primary CTA. Everything on the page should point toward it.

### Voice and positioning

- **How should this feel?** Not just visual tone — personality. "Confident but not corporate." "Technical but approachable." "Playful and a little weird." "Dead serious, we handle compliance." If the user doesn't know, suggest 2-3 options based on their audience and product.
- **Is there a line or phrase you keep using when you describe this to people?** Often the best headline is already in the founder's vocabulary. They just haven't written it down.
- **What should this page NOT be?** Sometimes the fastest way to find the right tone is to rule out the wrong ones. "Not enterprise-y." "Not cutesy." "Not like every other SaaS landing page."

### For the homepage specifically

- **What are the 2-3 things that make you different?** Not a feature list — the real reasons someone would pick this over the alternative. These become the feature section, but framed as benefits.
- **Does the site have existing content (blog posts, docs, changelog)?** The homepage should surface these. Ask what should be most prominent.

### For standalone landing pages

- **What is the specific goal of this page?** A homepage is broad. A landing page is narrow. "Convert trial users to paid." "Get signups for the beta." "Explain the pricing model."
- **Where will traffic come from?** Search, social, ads, email, direct? This affects the hero copy — someone from a Google search needs context; someone from your email list already knows you.

## Phase 2: Draft the messaging

Before proposing any layout, write out the actual copy and present it to the user for feedback:

```
Here's what I'd put on the page:

Headline: "Ship a landing page in an afternoon. Not a sprint."
Subheading: "One CLI. Your whole web presence — built with the coding
agent you already use."

Primary CTA: "Get started" → /docs/getting-started
Secondary CTA: "View on GitHub" → https://github.com/...

Key messages (these become feature cards or sections):
1. "Sub-second builds" — Single binary, no dependencies, no node_modules
2. "Your agent already knows this" — .claude/CLAUDE.md is generated on init
3. "Found by people and models" — SEO + llms.txt + markdown output

Closing CTA: "Your coding agent can handle your website."
```

Ask: **"Does this sound like you? What would you change?"**

Iterate on the words until the user says they feel right. Only then move to structure.

## Phase 3: Page structure

Now propose how to arrange the messaging into sections:

```
Page structure:
1. Hero — headline + subheading + two CTA buttons
2. Ribbon — 4 quick stats (install command, output formats, themes, deps)
3. Origin line — one sentence about why this exists
4. Features — 3x2 grid, each card has title + paragraph
5. Comparison table — "What you need" / "How"
6. Quickstart — code block + 4 numbered steps
7. Closing CTA — headline + buttons
```

Ask: **"Does this flow make sense? Anything to add, remove, or reorder?"** Wait for confirmation.

## Phase 4: Build

Create two files:

### Content file

Homepage: `content/pages/index.md`
Other pages: `content/pages/{slug}.md`

All copy from Phase 2 goes into `extra:` frontmatter fields. Keep the markdown body empty unless the user wants a prose section. Use a consistent naming convention:

```yaml
---
title: "Page Title"
description: "One-line tagline for meta/OG/Twitter (max 160 chars)"
image: /static/og.png
extra:
  hero_headline: "Ship a landing page in an afternoon."
  hero_subheadline: "One CLI. Your whole web presence."
  cta_primary_text: "Get started"
  cta_primary_url: "/docs/getting-started"
  cta_secondary_text: "GitHub"
  cta_secondary_url: "https://github.com/..."
  feature_1_title: "Sub-second builds"
  feature_1_body: "Single binary, no dependencies."
  # ... etc
---
```

### Template file

Homepage: `templates/index.html`
Other pages: `templates/{slug}-page.html` (set `template: {slug}-page.html` in frontmatter)

Tera template extending `base.html`:

- `{% extends "base.html" %}` as the first line
- `{% block extra_css %}` — all page-specific CSS in an inline `<style>` tag
- `{% block content %}` — the HTML structure
- For the homepage: include `{% for collection in collections %}` loop at the bottom unless the user said otherwise
- Responsive: mobile-first, breakpoints at 600px and 900px
- Use the theme's CSS custom properties (`var(--heading)`, `var(--text)`, `var(--bg)`, etc.) with sensible fallbacks
- Accessibility: semantic HTML, single `<h1>` in hero, `aria-label` on icon-only links
- Animations should respect `prefers-reduced-motion: reduce`
- Page must be fully functional without JavaScript

After writing both files, run `seite build` and tell the user to preview in their browser.

## Phase 5: Iterate

Ask: **"What would you like to change?"**

- **Copy changes** → edit the content file only
- **Layout/structure** → edit the template
- **Style/visual** → edit the `<style>` block in the template
- **New sections** → add `extra:` fields + template HTML
- **Theme clash** → suggest `seite theme apply <name>` or `base.html` tweaks

Run `seite build` after every change. Keep going until the user is happy.

## Section patterns

Use these as starting points, not templates. Adapt everything to the actual copy from Phase 2.

**Hero** — `<h1>` headline (the only h1), subheadline `<p>`, 1-2 CTA buttons. Optional: badge/eyebrow above headline, visual (screenshot, terminal demo, illustration) below buttons.

**Social proof / Ribbon** — Single-row flex of stats, logos, or short quotes. Monospace, muted, uppercase. Separators between items.

**Features** — 2x3 or 3-column CSS grid. Each card: optional SVG icon, `<h3>` title, short paragraph. Frame as benefits ("Ship in every language your customers speak") not features ("i18n support").

**How it works** — 3-4 numbered steps. Each: number badge, bold title, supporting copy. Pair with a code block if the product is technical.

**Comparison** — Two-column `<table>` (need / how) or before/after side-by-side.

**Pricing** — 2-4 column card grid. Plan name, price, feature list with checkmarks, CTA button. Highlight the recommended tier visually.

**Testimonials** — Blockquote with name, role, optional avatar. 1-3 quotes max.

**FAQ** — `<details><summary>` or heading+paragraph pairs. Group by category if 5+.

**Closing CTA** — `<h2>` headline, one sentence, primary button. Border-top or subtle background shift to separate from content above.

## Rules

- **No placeholder copy.** Never write "Lorem ipsum", "Your Company", "Describe your product here". Every word on the page should be real or a concrete draft for the user to react to.
- **`title:` and `description:` are required** in frontmatter — they feed `<title>`, meta description, Open Graph, and Twitter Cards.
- **Do not touch `base.html`** unless the user asks for theme changes. The landing page template should work with any theme.
- **Multilingual reminder**: if the site has `[languages.*]` in config, mention that translated versions (e.g., `index.es.md`) are supported.
- **CSS**: prefer Grid/Flexbox, use `clamp()` for responsive type. Keep it in `{% block extra_css %}`.
- **Homepage collection listings**: always include the `{% for collection in collections %}` loop on the homepage unless explicitly told not to.
- **Standalone pages**: set `template: {slug}-page.html` in frontmatter.
