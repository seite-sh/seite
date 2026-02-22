---
name: homepage
description: Create or redesign the site's homepage/landing page. Guides you through discovery, planning, building, and iterating until you are happy.
# seite-skill-version: 1
---

# Homepage Builder

You are guiding the user through creating a homepage for their seite static site. The output is two files: `content/pages/index.md` (content data) and `templates/index.html` (layout). Iterate until the user is satisfied.

Read `seite.toml` and check if `content/pages/index.md` or `templates/index.html` already exist before starting. If they exist, read them and ask what to improve rather than starting from scratch.

## Phase 1: Discovery

Ask these questions — skip any that are already obvious from the site config or existing content:

1. **What is this site for?** — Product/SaaS, personal blog, portfolio, documentation, startup, open source project, agency, nonprofit, etc.
2. **Who is the audience?** — Developers, business buyers, general consumers, hiring managers, etc.
3. **What is the single most important action a visitor should take?** — Sign up, install, read docs, contact, buy, subscribe, etc. This becomes the primary CTA.
4. **Is there a secondary action?** — View source on GitHub, read the blog, see pricing, etc.
5. **What sections do you want?** Suggest relevant defaults based on site type:
   - **SaaS/product**: hero, social proof/logos, features, comparison, pricing, CTA
   - **Open source**: hero, install command, features, quickstart, community/GitHub, CTA
   - **Blog/personal**: hero with bio, featured posts, about blurb, newsletter signup
   - **Portfolio**: hero, project showcase, skills/services, testimonials, contact
   - **Docs site**: hero, quick links to key docs, search, getting started
   - **Startup/company**: hero, problem/solution, features, team, CTA
6. **What is the visual tone?** — Clean/minimal, bold/energetic, dark/technical, playful, corporate/trust. Reference the current theme or suggest one.

If the user gives a short answer like "just make me a landing page", infer reasonable defaults and present your plan before building. Do not ask all questions at once if context makes some answers obvious.

## Phase 2: Content Plan

Present a section-by-section outline before writing files:

```
Homepage plan:
1. Hero — headline, subheading, primary CTA (Get Started), secondary CTA (GitHub)
2. Social proof ribbon — 4 key stats or partner logos
3. Features — 3x2 grid with icon, title, one-liner per feature
4. How it works — 3-step numbered walkthrough
5. CTA — closing headline + primary button
```

Ask: "Does this structure work, or do you want to add/remove/reorder sections?" Wait for confirmation before proceeding.

## Phase 3: Build

Create two files:

### `content/pages/index.md`

All content lives here as `extra:` frontmatter fields. Keep the markdown body empty (or use it for a simple prose section if appropriate). Structure `extra:` fields with a consistent naming pattern per section:

```yaml
---
title: "Site Name"
description: "One-line tagline for meta/OG/Twitter"
image: /static/og.png
extra:
  # Hero
  hero_badge: "Now in beta"
  hero_headline: "Your main headline"
  hero_subheadline: "Supporting copy — one or two sentences"
  cta_primary_text: "Get Started"
  cta_primary_url: "/docs/getting-started"
  cta_secondary_text: "GitHub"
  cta_secondary_url: "https://github.com/..."

  # Features
  features_eyebrow: "Features"
  features_headline: "Everything you need"
  feature_1_title: "Fast"
  feature_1_body: "Sub-second builds"
  feature_2_title: "Simple"
  feature_2_body: "Zero dependencies"
  # ... etc
---
```

### `templates/index.html`

Tera template that extends `base.html`. Uses `{{ page.extra.* }}` to render each section. Requirements:

- `{% extends "base.html" %}` as the first line
- `{% block extra_css %}` for all homepage-specific CSS (inline `<style>` tag)
- `{% block content %}` for the HTML structure
- Collection listings at the bottom if the site has posts/docs (use `{% for collection in collections %}` loop)
- Responsive design (mobile-first, with breakpoints at 600px and 900px)
- Respect the theme's CSS custom properties (e.g., `var(--heading)`, `var(--text)`, `var(--bg)`, `var(--green)`, `var(--border)`) when available
- Accessibility: semantic HTML, proper heading hierarchy (`<h1>` only in hero), sufficient color contrast, `aria-label` on icon-only links

CSS guidelines:
- All styles go inside `{% block extra_css %}<style>...</style>{% endblock %}`
- Use the theme's CSS custom properties where they exist — fall back to sensible defaults
- Keep animations subtle and respect `prefers-reduced-motion: reduce`
- Ensure the page is fully functional without JavaScript

After writing both files, run `seite build` and tell the user to check the result in their browser (`seite serve` if not already running).

## Phase 4: Iterate

After the user sees the result, ask: "What would you like to change?" Common follow-ups:

- **Copy changes** — Edit `content/pages/index.md` only (update `extra:` fields)
- **Layout changes** — Edit `templates/index.html` (add/remove/reorder sections, change grid)
- **Style changes** — Edit the `<style>` block in `templates/index.html`
- **Section additions** — Add new `extra:` fields to `index.md` and new HTML sections to the template
- **Theme mismatch** — If the homepage clashes with the base theme, suggest `seite theme apply <name>` or adjustments to `base.html`

Run `seite build` after every change so the user can see the result immediately. Keep iterating until the user confirms they are satisfied.

## Section Pattern Reference

When building sections, use these proven patterns. Adapt to the site's content — never use placeholder copy.

**Hero** — badge/eyebrow (optional), `<h1>` headline (the only `h1` on the page), subheadline paragraph, 1-2 CTA buttons, optional visual (terminal demo, screenshot, illustration).

**Social proof / Ribbon** — Single-row flex/grid of logos, stats, or short quotes. Monospace font, muted color, uppercase. Separators between items.

**Features** — Eyebrow label, section headline, 2x3 or 3-column CSS grid of cards. Each card: optional icon (SVG inline or CSS), `<h3>` title, short paragraph. Use `<code>` tags for CLI commands or config values mentioned in feature copy.

**How it works / Steps** — Numbered list (CSS counter or explicit numbers), 3-4 steps max. Each step: number badge, bold title, supporting copy. Optionally paired with a code block showing the actual commands.

**Comparison / Table** — Simple `<table>` with two columns (need / solution), or a side-by-side before/after. Monospace font works well.

**Testimonials** — Blockquote with attribution. Keep to 1-3 quotes. Include name, role, and optionally a small avatar.

**CTA (closing)** — Repeat the primary action. Centered, large headline (use `<h2>`), supporting sentence, primary button. Often has a subtle background gradient or border-top separator.

## Rules

- Never use placeholder text like "Lorem ipsum" or "Your Company". Ask the user for real copy, or draft realistic copy based on what you know about their site and propose it for their approval.
- The `content/pages/index.md` file must have `title:` and `description:` in frontmatter — these feed into `<title>`, `<meta name="description">`, Open Graph, and Twitter Cards automatically.
- Do not modify `base.html` unless the user specifically asks for theme changes. The homepage template should work with any theme.
- If the site is multilingual, remind the user they can create `content/pages/index.es.md` (etc.) for translated homepages.
- Prefer CSS Grid and Flexbox over floats or absolute positioning. Use `clamp()` for responsive font sizes.
- Always include the collection listings loop unless the user explicitly says they do not want it. The homepage should still surface recent posts/docs if they exist.
