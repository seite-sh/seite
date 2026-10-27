---
name: seite
description: The developer-tool category standard, played straight at bun.sh's craft. The product works on the page instead of being described.
colors:
  teal: "#0f766e"
  teal-dark: "#2dd4bf"
  teal-border: "rgba(15, 118, 110, 0.25)"
  teal-border-dark: "rgba(45, 212, 191, 0.28)"
  teal-wash: "rgba(15, 118, 110, 0.06)"
  teal-wash-dark: "rgba(45, 212, 191, 0.08)"
  logo-page: "#0d9488"
  logo-page-dark: "#14b8a6"
  logo-fold: "#6366f1"
  logo-fold-dark: "#818cf8"
  ink: "#0a0a0a"
  ink-dark: "#fafafa"
  on-ink: "#ffffff"
  on-ink-dark: "#0a0a0a"
  paper: "#ffffff"
  paper-dark: "#0a0a0a"
  paper-raised: "#f5f5f4"
  paper-raised-dark: "#141414"
  paper-card: "#fafafa"
  paper-card-dark: "#121212"
  rule: "#e5e5e4"
  rule-dark: "#232323"
  rule-strong: "#d4d4d2"
  rule-strong-dark: "#333333"
  heading: "#0a0a0a"
  heading-dark: "#fafafa"
  text: "#262626"
  text-dark: "#d6d6d6"
  text-muted: "#525252"
  text-muted-dark: "#a3a3a3"
  text-dim: "#6b6b6b"
  text-dim-dark: "#8a8a8a"
  warning: "#b45309"
  warning-dark: "#fbbf24"
  danger: "#b91c1c"
  danger-dark: "#f87171"
  scrim: "rgba(0, 0, 0, 0.6)"
  term-bg: "#0a0a0a"
  term-bg-dark: "#111111"
  term-bar: "#161616"
  term-bar-dark: "#191919"
  term-border: "#0a0a0a"
  term-border-dark: "#262626"
  term-rule: "#333333"
  term-text: "#ededed"
  term-dim: "#9a9a9a"
  term-ok: "#5eead4"
  term-flag: "#93c5fd"
typography:
  display:
    fontFamily: "Archivo, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "clamp(2.5rem, 4.4vw, 3.4rem)"
    fontWeight: 800
    lineHeight: 1.02
    letterSpacing: "-0.025em"
  headline:
    fontFamily: "Archivo, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "clamp(1.75rem, 2.8vw, 2.2rem)"
    fontWeight: 800
    lineHeight: 1.04
    letterSpacing: "-0.025em"
  subhead:
    fontFamily: "Archivo, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "1.35rem"
    fontWeight: 700
    letterSpacing: "-0.025em"
  numeral:
    fontFamily: "Archivo, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "2.1rem"
    fontWeight: 800
    lineHeight: 1.1
    letterSpacing: "-0.03em"
    fontFeature: "tnum"
  title:
    fontFamily: "Archivo, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "1.05rem"
    fontWeight: 700
    lineHeight: 1.3
    letterSpacing: "-0.01em"
  body-lede:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
    fontSize: "1.05rem"
    fontWeight: 400
    lineHeight: 1.6
  body:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: 1.65
  body-sm:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
    fontSize: "0.9rem"
    fontWeight: 400
    lineHeight: 1.6
  label:
    fontFamily: "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 500
    lineHeight: 1.5
  mono:
    fontFamily: "'Martian Mono', ui-monospace, 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: 1.85
    letterSpacing: "-0.01em"
rounded:
  sm: "4px"
  md: "8px"
  lg: "12px"
spacing:
  gutter: "1.5rem"
  block: "2.5rem"
  column: "3.5rem"
  section: "5.5rem"
  section-compact: "4rem"
components:
  button-ink:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.on-ink}"
    rounded: "{rounded.sm}"
    padding: "0 1.15rem"
    height: "44px"
  button-line:
    textColor: "{colors.heading}"
    rounded: "{rounded.sm}"
    padding: "0 1.15rem"
    height: "44px"
  nav-cta:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.on-ink}"
    rounded: "{rounded.sm}"
    padding: "0 0.9rem"
    height: "34px"
  segmented-tab-selected:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.on-ink}"
    typography: "{typography.label}"
    height: "34px"
  announce-bar:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.on-ink}"
    padding: "0.45rem 1rem"
  terminal:
    backgroundColor: "{colors.term-bg}"
    textColor: "{colors.term-text}"
    typography: "{typography.mono}"
    rounded: "{rounded.md}"
    padding: "1rem 1.1rem 1.2rem"
  install-command:
    backgroundColor: "{colors.term-bg}"
    textColor: "{colors.term-text}"
    typography: "{typography.mono}"
    rounded: "{rounded.sm}"
    padding: "0.3rem 0.3rem 0.3rem 0.9rem"
  command-chip:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.heading}"
    typography: "{typography.mono}"
    rounded: "{rounded.sm}"
    padding: "0.45rem 0.6rem"
  panel:
    backgroundColor: "{colors.paper-card}"
    rounded: "{rounded.lg}"
    padding: "1rem"
  code-block:
    backgroundColor: "{colors.term-bg}"
    textColor: "{colors.term-text}"
    typography: "{typography.mono}"
    rounded: "{rounded.md}"
    padding: "1.25rem 1.5rem"
  callout:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "1rem 1.25rem"
  status-chip:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.text-muted}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: "0.2rem 0.7rem"
  status-chip-done:
    backgroundColor: "{colors.teal-wash}"
    textColor: "{colors.teal}"
  status-chip-in-progress:
    textColor: "{colors.warning}"
  status-chip-breaking:
    textColor: "{colors.danger}"
  wordmark:
    textColor: "{colors.heading}"
    typography: "{typography.subhead}"
---

# Design System: seite

## Overview

**Creative North Star: "The Product, Working on the Page"**

seite.sh plays the developer-tool category standard straight, at bun.sh's level of craft. The page proves the product by running it: real commands typed into dark terminal panels, real output, a real release pulled from the changelog. Everything else stays out of the way. A white ground (near-black in dark mode), near-black ink, one teal accent, heavy Archivo headlines, system sans for reading, and Martian Mono for anything a machine would print. The same system runs on every page: homepage, docs, blog, changelog, roadmap and tags.

Density is calm and editorial rather than dashboard-tight: one centred column about 1040px wide, generous section padding, and grids of cells divided by 1px rules instead of floating cards. Depth comes from tone and line, never from shadow. The world refuses decoration (glows, gradients, mood art) and empty claims; when proof is absent, the page shows a command instead.

Tokens in the frontmatter are the light theme, which the rendered review evidence shows. A dark theme ships beside it through the user toggle and `prefers-color-scheme`; `:root` defaults to dark. Every dark counterpart is recorded in the frontmatter under the same name with a `-dark` suffix (and mirrored in the sidecar's `colorMeta`); tokens without a `-dark` entry hold one value in both themes. The prose below names both where the role changes.

**Key Characteristics:**
- Product-as-proof: terminals that type real commands carry the first viewport.
- Monochrome ink and paper, with a single teal voice.
- Archivo 700/800 for h1/h2, the wordmark, names and numbers; system sans for reading; Martian Mono for code and machine values.
- Flat: 1px rules and tonal steps, no shadows.
- Small radii (4/8/12px) and shared-border grids.

## Colors

A near-monochrome ink-and-paper palette with one teal voice, two semantic status colours, and a terminal palette that stays dark in both themes. All tokens live in `base.html` (`:root, [data-theme="dark"]` and `[data-theme="light"]`).

### Primary
- **Deep Teal** (`teal`): the single accent. Light theme `#0f766e`, dark theme bright teal `#2dd4bf`. It colours the emphasised clause of the hero headline, links, the focus ring, the `$` prompt in command chips, the active step number in the walkthrough, and the text-selection tint. **Teal Border** (`teal-border`; dark `rgba(45, 212, 191, 0.28)`) is the hover outline on the nav's outlined icon buttons and the border of info callouts. **Teal Wash** (`teal-wash`, code `--green-dim`) is the faint fill behind the "done" status chip and "new" changelog tag. Token name in code is `--green` (historical); the colour is teal.

### Secondary
Semantic status only; never decoration, never a fill.
- **Amber Warning** (`warning`): text of the "in progress" status chip and the border of warning callouts.
- **Signal Red** (`danger`): text of the "breaking" changelog tag and the border of danger callouts.

### Tertiary
Used only inside terminal panels, the install command and code blocks.
- **Terminal Black** (`term-bg`, `term-bar`, `term-border`): panel, title-bar and outline. In light the border matches the ground; dark lifts the panel and bar one step and gives it a visible border so it still separates from the page.
- **Terminal Rule** (`term-rule`): title-bar divider, window dots and the Copy button's outline and hover fill.
- **Terminal Text / Dim** (`term-text`, `term-dim`): command text and quiet output lines.
- **Terminal Teal** (`term-ok`): the prompt, success lines (`✓ ...`) and the caret.
- **Terminal Blue** (`term-flag`): flags and quoted arguments inside commands. Never used outside a terminal.

### Neutral
- **Ink** (`ink`): near-black in light, near-white `#fafafa` in dark. The fill of every primary action (hero button, nav "Get started", selected agent tab) and of the announcement bar.
- **On Ink** (`on-ink`): text on an ink fill; white in light, `#0a0a0a` in dark.
- **Paper** (`paper`): page ground; near-black in dark.
- **Scrim** (`scrim`): the dimmed backdrop behind the off-canvas docs sidebar on small screens.
- **Raised Paper** (`paper-raised`): one tonal step up; command chips, inline code, hovered and pressed walkthrough steps, hovered tabs.
- **Card Paper** (`paper-card`): the agent panel's ground, a half step above paper.
- **Rule** (`rule`): every 1px divider: section boundaries, the works-with band, cell borders in the tools and stats grids, feature rows.
- **Strong Rule** (`rule-strong`): outlines on interactive elements: secondary button, theme toggle, hamburger, selected install tab, replay button.
- **Heading** (`heading`): headlines, titles, stat numerals, command chip text.
- **Text / Muted / Dim** (`text`, `text-muted`, `text-dim`): body copy; ledes, descriptions and nav links; captions, labels, footnotes and the dim second clause of a section headline.
- **Logo Page / Logo Fold** (`logo-page`, `logo-fold`): the two fills of the document mark in the wordmark. Logo only; the fold's indigo appears nowhere else.

### Named Rules
**The One Teal Rule.** Teal is the only accent on the page. It marks emphasis and navigation (a headline clause, links, prompts, the active step, focus) and is never a fill for a large surface or a button.

**The Ink Action Rule.** Primary actions are filled with ink and set in on-ink text; secondary actions are outlined with a strong rule. The contract named teal for the primary action; the build moved it to ink, and the build is canonical.

**The Terminal Stays Dark Rule.** Terminal panels, the install command and code blocks are dark in both themes. In light mode they are the darkest thing on the page; that contrast is the point.

## Typography

**Display Font:** Archivo (with system-ui fallback), weights 700 and 800; every h1 and h2 site-wide and the wordmark
**Body Font:** system sans (system-ui, -apple-system, Segoe UI, Helvetica, Arial)
**Label/Mono Font:** Martian Mono (with ui-monospace, SF Mono, Menlo fallback), weights 400 and 500

**Character:** A heavy, tightly tracked grotesque headline over a neutral native body face, with a wide technical mono for everything typed or printed. The pairing reads like a well-made CLI's homepage: confident at the top, quiet in the paragraphs, literal in the code.

### Hierarchy
- **Display** (Archivo 800, `clamp(2.5rem, 4.4vw, 3.4rem)`, line-height 1.02, -0.025em, balanced wrap): the hero headline only.
- **Headline** (Archivo 800, `clamp(1.75rem, 2.8vw, 2.2rem)`, 1.04, -0.025em): section headlines. A second clause may follow in the same heading set in dim text.
- **Subhead** (Archivo 700, 1.35rem, -0.025em): in-content section headings (h2 inside docs, posts, changelog entries and roadmap items) and the featured post title (800). The wordmark sets this size at 800, -0.03em, line-height 1 (1rem in the footer).
- **Numeral** (Archivo 800, 2.1rem, 1.1, -0.03em, tabular figures; 1.7rem under 640px): measured figures in the stats grid.
- **Title** (Archivo 700, 1.05 to 1.1rem, -0.01em): cell and feature titles, and the names in the works-with row (1rem).
- **Body Lede** (system 400, 1.05rem hero / 1rem section, line-height 1.6, max 30 to 31rem): the sentence under a headline.
- **Body** (system 400, 16px, 1.65): default reading text.
- **Body Small** (system 400, 0.85 to 0.9rem, 1.55 to 1.65): cell descriptions, step details, feature bodies.
- **Label** (system 500 to 600, 0.75 to 0.82rem): tabs, panel captions, the works-with label, install label. Sentence case, no tracking; status chips are the exception (uppercase, 600, 0.04 to 0.06em).
- **Mono** (Martian Mono 400, 0.75rem, line-height 1.85 in terminals and 1.75 in code blocks, -0.01em): terminal output, code blocks, install command, command chips, step commands, step numbers, changelog versions and dates, tag and roadmap counts; inline code at about 0.8em.

### Named Rules
**The Machine Text Rule.** Anything a user would type or a program would print (commands, paths, flags, code, versions, dates and counts, step numbers) is set in Martian Mono. Prose never is.

**The Heavy Display Rule.** Archivo appears only at 700 and 800, only for headlines, subheads, titles, names, numerals and the wordmark. Body and UI labels stay in the system sans.

## Layout

A single centred column: homepage content is 1040px wide (`max-width: calc(1040px + 3rem)` with 1.5rem gutters). Inner pages, the nav and the footer use the base 1120px container (pre-existing; the system column is 1040px).

Sections are separated by 1px top rules and padded 5.5rem top and bottom (4rem under 640px; the hero uses 4.5rem / 3.5rem, 3rem / 2.5rem on mobile; the closing call to action 6rem / 5rem). A section opens with a two-column head: headline left, lede right, bottom-aligned, 3.5rem apart, then 2.5rem to the content.

Content patterns are two-column splits (hero 1.25fr / 0.75fr; walkthrough 0.85fr / 1.15fr; release 1fr / 1fr) and ruled grids (four tools across, a 2x2 stats grid, a two-column list of features each opened by a top rule). A full-bleed ruled band ("Works with") sits directly under the hero.

Breakpoints: at 900px the docs sidebar becomes an off-canvas drawer over the scrim; at 960px every split collapses to one column, the tools grid goes 2x2, and the walkthrough terminal moves above the steps and sticks under the nav. At 640px all grids go single column, hero buttons stretch to full width, and the nav collapses to a hamburger.

## Elevation & Depth

The system is flat. There is no `box-shadow` anywhere in the templates; dropdowns such as the docs search results are separated by a strong-rule border alone. Depth is carried by tonal steps (paper, card paper, raised paper, terminal black) and 1px rules. The only translucency is the sticky nav: a 90% paper fill with a 20px backdrop blur.

### Named Rules
**The Rule-Not-Shadow Rule.** Separate surfaces with a 1px rule or a tonal step. If an element seems to need a shadow, give it a border instead.

## Shapes

Small, consistent radii from three tokens: 4px for controls (buttons, tabs, command chips, install command, toggles, status chips, inline code), 8px for terminals, code blocks, callouts, dropdowns and walkthrough steps, 12px for the outer containers of grouped content (agent panel, tools grid, stats grid). Grouped cells share their internal 1px rules and clip to one rounded outer container instead of each cell getting its own card. Terminal title bars carry three 9px grey dots. Icons are inline SVG line icons at 14 to 18px with a 2px stroke and round caps.

## Components

### Buttons
- **Shape:** gently squared (4px radius), 44px minimum height, 0 1.15rem padding, label weight 600 at 0.95rem; an optional trailing 14px SVG arrow or leading 16px SVG icon.
- **Primary (ink):** ink fill with on-ink text. Used for the one main action per block (Read Getting Started, Read the release notes).
- **Hover / Focus:** hover drops opacity to 0.85 over 0.15s; focus shows the global 2px teal outline at 3px offset.
- **Secondary (line):** transparent with a 1px strong-rule border and heading-colour text; hover darkens the border to dim text.
- **Nav CTA:** the same ink button at 34px height and 0.85rem, pinned right in the nav; under 640px it moves into the opened menu as a full-width ink block.

### Segmented Tabs
- **Style:** a single 1px-ruled, 4px-radius strip divided into equal cells with internal rules, 34px tall, label type.
- **State:** the selected cell fills with ink and on-ink text; unselected cells are muted text and turn heading-coloured on a raised-paper hover. The install tabs are a lighter variant: borderless text tabs where the selected tab gains a strong-rule outline.

### Terminal Panel (signature)
- **Structure:** dark panel (8px radius, 1px border) with a mono title bar (three dots plus `~/mysite`) over a mono body.
- **Behaviour:** commands type out two characters every 20ms with a block caret, output lines land every 240ms, and a final prompt with a 1s step-blinking caret ends the run. A footnote row beneath states that the output is abridged and offers an outlined Replay button. Under reduced motion the whole transcript renders at once and the caret stops.
- **Colour:** prompt and success lines in terminal teal, flags and quoted strings in terminal blue, notes in terminal dim.

### Code Block
- **Style:** terminal black with a 1px terminal border, 8px radius, 1.25rem 1.5rem padding, mono 0.75rem at line-height 1.75 in terminal text. The ground is forced with `!important` over the highlighter's inline background so every code block joins the terminal palette.

### Callout
- **Style:** raised paper, 8px radius, 1rem 1.25rem padding, a 600 0.9rem title. A 1px border carries the kind: teal border (info), teal (tip), warning, danger. No thick side stripe.

### Status Chips
- **Style:** 4px radius, 0.75rem/600 uppercase with light tracking, raised paper and a 1px rule; the text carries the state: muted (planned, fix, improvement), dim (cancelled), warning (in progress), danger (breaking).
- **Done / New:** the one tinted variant: teal wash fill, teal border, teal text.

### Install Command
- **Style:** terminal-black bar (4px radius) holding the mono command and a small dark Copy button (32px, 4px radius) that reads "Copied" for two seconds after use. OS tabs sit on the label row above.

### Cards / Containers
- **Corner Style:** 12px on the outer container only.
- **Background:** paper for grids; card paper for the agent panel.
- **Shadow Strategy:** none (see Elevation & Depth).
- **Border:** 1px rule around the container and between cells.
- **Internal Padding:** about 1.25rem to 1.4rem per cell; 1rem for the agent panel.

### Command Chip
- **Style:** raised paper, 1px rule, 4px radius, mono 0.75rem in heading colour with a teal, unselectable `$ ` prompt; sits at the foot of each tool cell and scrolls horizontally rather than wrapping.

### Walkthrough Steps
- **Style:** full-width buttons on an 8px radius with a mono step number, a 0.95rem/600 title, the mono command beneath, and a description revealed only when pressed.
- **State:** hover and pressed both take raised paper; pressed adds a 1px rule and turns the step number teal.

### Navigation
- **Style:** sticky 56px bar, 1px bottom rule, translucent paper with 20px blur. Wordmark left (inline SVG document mark in logo page/fold fills plus the site title in Archivo 800 at 1.35rem; hover fades to 0.85), muted 0.875rem links, then the theme toggle and hamburger (34px squares with a strong-rule outline, 4px radius), a GitHub SVG, and the ink CTA.
- **States:** links go from muted to text colour on hover; outlined icon buttons go teal-bordered on hover.
- **Mobile:** under 640px the links drop into a full-width panel under the bar, one per ruled row, ending with the full-width ink CTA. The hamburger swaps its SVG to a close icon via `aria-expanded`.

### Announcement Bar
- **Style:** a full-width ink strip above the nav, 0.8rem on-ink text centred and truncated to one line, linking the latest changelog entry with a trailing SVG arrow; underline on hover.

## Do's and Don'ts

### Do:
- **Do** show the product working: a typed command and its real (abridged) output beats a sentence describing a feature.
- **Do** keep primary actions ink-filled at 44px with a 4px radius, and pair each with at most one outlined secondary action.
- **Do** set every command, path and printed value in Martian Mono, and prefix shell commands with a teal `$ ` that cannot be selected.
- **Do** group related cells inside one 12px-radius container with shared 1px internal rules.
- **Do** keep terminals dark in both themes and label illustrative output as abridged.
- **Do** render every motion effect static under `prefers-reduced-motion: reduce`.

### Don't:
- **Don't** add glows, gradients or mood art; the contract refuses decoration.
- **Don't** use `box-shadow` for depth; use a rule or a tonal step.
- **Don't** fill buttons or large surfaces with teal; teal marks emphasis and navigation only.
- **Don't** introduce a second accent outside the terminal; terminal blue lives only inside terminals.
- **Don't** put an uppercase tracked label above a section headline; the headline stands alone, with an optional dim second clause inside the same heading.
- **Don't** use text glyphs or icon fonts as icons; use inline SVG line icons with a 2px stroke.
