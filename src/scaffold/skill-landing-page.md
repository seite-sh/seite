---
name: landing-page
description: Create or redesign a landing page — the site homepage, a product page, a launch page, or any standalone marketing page. Guides you through discovery, planning, building, and iterating until you are happy.
# seite-skill-version: 2
---

# Landing Page Builder

You are guiding the user through creating a landing page for their seite static site. This can be the site homepage (`content/pages/index.md`) or any standalone page (`content/pages/pricing.md`, `content/pages/launch.md`, etc.). The output is two files: a content file and a matching template. Iterate until the user is satisfied.

## Before you start

1. Read `seite.toml` to understand the site (title, collections, language).
2. Ask which page this is for — or infer from context:
   - **Homepage** → `content/pages/index.md` + `templates/index.html`
   - **Other page** → `content/pages/{slug}.md` + `templates/{slug}-page.html`
3. Check if the target files already exist. If they do, read them and ask what to improve rather than starting from scratch.

## Phase 1: Discovery

Ask these questions — skip any that are already obvious from the site config or existing content:

1. **What is this page for?** — Site homepage, product launch, pricing, feature spotlight, event, waitlist, about, etc.
2. **Who is the audience?** — Developers, business buyers, general consumers, hiring managers, etc.
3. **What is the single most important action a visitor should take?** — Sign up, install, read docs, contact, buy, subscribe, join waitlist, etc. This becomes the primary CTA.
4. **Is there a secondary action?** — View source on GitHub, read the blog, see pricing, etc.
5. **What sections do you want?** Suggest relevant defaults based on page type:
   - **SaaS/product**: hero, social proof/logos, features, comparison, pricing, CTA
   - **Open source**: hero, install command, features, quickstart, community/GitHub, CTA
   - **Blog/personal**: hero with bio, featured posts, about blurb, newsletter signup
   - **Portfolio**: hero, project showcase, skills/services, testimonials, contact
   - **Docs site**: hero, quick links to key docs, search, getting started
   - **Startup/company**: hero, problem/solution, features, team, CTA
   - **Launch/waitlist**: hero, problem statement, teaser features, email capture, CTA
   - **Pricing**: hero, pricing tiers, feature comparison, FAQ, CTA
6. **What is the visual tone?** — Clean/minimal, bold/energetic, dark/technical, playful, corporate/trust. Reference the current theme or suggest one.

If the user gives a short answer like "just make me a landing page", infer reasonable defaults and present your plan before building. Do not ask all questions at once if context makes some answers obvious.

## Phase 2: Content Plan

Present a section-by-section outline before writing files:

```
Landing page plan for /pricing:
1. Hero — headline, subheading, primary CTA (Start Free Trial)
2. Pricing tiers — 3-column grid (Free, Pro, Enterprise)
3. Feature comparison — table with checkmarks
4. FAQ — 4-5 common questions
5. CTA — closing headline + primary button
```

Ask: "Does this structure work, or do you want to add/remove/reorder sections?" Wait for confirmation before proceeding.

## Phase 3: Build

Create two files:

### Content file

For the homepage: `content/pages/index.md`
For other pages: `content/pages/{slug}.md`

All content lives here as `extra:` frontmatter fields. Keep the markdown body empty (or use it for a simple prose section if appropriate). Structure `extra:` fields with a consistent naming pattern per section:

```yaml
---
title: "Page Title"
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

### Template file

For the homepage: `templates/index.html`
For other pages: `templates/{slug}-page.html` (and set `template: {slug}-page.html` in frontmatter)

Tera template that extends `base.html`. Uses `{{ page.extra.* }}` to render each section. Requirements:

- `{% extends "base.html" %}` as the first line
- `{% block extra_css %}` for all page-specific CSS (inline `<style>` tag)
- `{% block content %}` for the HTML structure
- For the homepage: include collection listings at the bottom (use `{% for collection in collections %}` loop) unless the user says otherwise
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

- **Copy changes** — Edit the content file only (update `extra:` fields)
- **Layout changes** — Edit the template (add/remove/reorder sections, change grid)
- **Style changes** — Edit the `<style>` block in the template
- **Section additions** — Add new `extra:` fields to the content file and new HTML sections to the template
- **Theme mismatch** — If the page clashes with the base theme, suggest `seite theme apply <name>` or adjustments to `base.html`

Run `seite build` after every change so the user can see the result immediately. Keep iterating until the user confirms they are satisfied.

## Section Pattern Reference

When building sections, use these proven patterns. Adapt to the site's content — never use placeholder copy.

**Hero** — badge/eyebrow (optional), `<h1>` headline (the only `h1` on the page), subheadline paragraph, 1-2 CTA buttons, optional visual (terminal demo, screenshot, illustration).

**Social proof / Ribbon** — Single-row flex/grid of logos, stats, or short quotes. Monospace font, muted color, uppercase. Separators between items.

**Features** — Eyebrow label, section headline, 2x3 or 3-column CSS grid of cards. Each card: optional icon (SVG inline or CSS), `<h3>` title, short paragraph. Use `<code>` tags for CLI commands or config values mentioned in feature copy.

**How it works / Steps** — Numbered list (CSS counter or explicit numbers), 3-4 steps max. Each step: number badge, bold title, supporting copy. Optionally paired with a code block showing the actual commands.

**Comparison / Table** — Simple `<table>` with two columns (need / solution), or a side-by-side before/after. Monospace font works well.

**Pricing tiers** — 2-4 column grid of cards. Each card: plan name, price, feature list with checkmarks, CTA button. Highlight the recommended tier.

**Testimonials** — Blockquote with attribution. Keep to 1-3 quotes. Include name, role, and optionally a small avatar.

**FAQ** — `<details><summary>` pairs or simple heading+paragraph pairs. Group by category if more than 5 questions.

**CTA (closing)** — Repeat the primary action. Centered, large headline (use `<h2>`), supporting sentence, primary button. Often has a subtle background gradient or border-top separator.

## Rules

- Never use placeholder text like "Lorem ipsum" or "Your Company". Ask the user for real copy, or draft realistic copy based on what you know about their site and propose it for their approval.
- The content file must have `title:` and `description:` in frontmatter — these feed into `<title>`, `<meta name="description">`, Open Graph, and Twitter Cards automatically.
- Do not modify `base.html` unless the user specifically asks for theme changes. The landing page template should work with any theme.
- If the site is multilingual, remind the user they can create translated versions (e.g., `index.es.md`, `pricing.es.md`).
- Prefer CSS Grid and Flexbox over floats or absolute positioning. Use `clamp()` for responsive font sizes.
- For the homepage, always include the collection listings loop unless the user explicitly says they do not want it — the homepage should still surface recent posts/docs if they exist.
- For non-homepage landing pages, set `template: {slug}-page.html` in frontmatter to use the custom template instead of the default `page.html`.
