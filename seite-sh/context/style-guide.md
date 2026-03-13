# seite Style Guide

Writing conventions, formatting standards, and editorial guidelines for all seite content.

---

## Grammar & Mechanics

### Capitalization

**Headlines & Subheadings**: Title Case
- Capitalize all major words (nouns, verbs, adjectives, adverbs)
- Lowercase: articles (a, an, the), coordinating conjunctions (and, but, or), prepositions under 5 letters (in, on, at, for, to)
- Example: "How to Deploy a Static Site to Cloudflare Pages"

**Product Names**:
- seite: always lowercase, even at start of sentence — write "seite builds fast" not "Seite builds fast"
- CLI commands: always in backticks — `seite build`, `seite deploy`
- File names: in backticks — `seite.toml`, `base.html`
- Collection names: lowercase — posts, docs, pages, changelog, roadmap

**Industry Terms**:
- AI: always caps
- CLI: always caps
- RSS: always caps
- SSG: always caps (Static Site Generator)
- LLM: always caps
- MCP: always caps
- llms.txt: always lowercase with extension
- markdown: lowercase
- frontmatter: one word, lowercase

### Oxford Comma

**No Oxford comma.**
- ✅ "posts, docs and pages"
- ❌ "posts, docs, and pages"

### Numbers

- Spell out: one through nine
- Numerals: 10 and above
- Always numerals: percentages (5%), file sizes (3 MB), build times (0.5s), version numbers (v0.4.0)
- Large numbers: "1 million" not "1,000,000" — except in code examples

### Punctuation

**Em Dashes**: — (em dash, no spaces)
- "seite builds your site — and only your site — in under a second."
- Not: " — " (with spaces)

**Ellipses**: Use sparingly, no spaces around: ...

**Colons**: Lowercase the word after a colon unless it's a proper noun or starts a complete independent clause.

---

## Word Choice & Usage

### Preferred Terms

**Say This** → **Not That**:
- site generator → content management system
- build → compile / transpile / generate
- deploy → publish / go live / push to production
- collection → content type / taxonomy / post type
- agent → AI assistant / copilot / AI helper
- skill pack → plugin / extension / add-on
- theme → template (theme covers templates + styles)
- frontmatter → front matter (one word here)
- sub-second → blazing fast / lightning fast
- dev server → development server (in casual contexts, dev server is fine)
- seite → the tool (never "the platform" or "the product")

### Words to Avoid

- "very", "really", "actually", "simply", "just" (cut unless load-bearing)
- "revolutionary", "game-changing", "groundbreaking" (hype language)
- "powerful", "robust", "seamless" (vague marketing filler)
- "click here" or "read more" for links (use descriptive anchors)
- "easy" or "simple" (show it, don't say it — "run one command" beats "it's easy")
- Passive constructions when active is available

### Inclusive Language

- Gender-neutral: use "they/their" not "he/she"
- "developer" or "builder" not gendered terms
- Avoid idioms that don't translate globally

---

## Formatting Standards

### Text Formatting

**Bold**: Key concepts, important commands, emphasis on a specific word
- Don't overuse — if everything is bold, nothing stands out
- Example: "Set `draft: true` to **exclude content from the default build**."

**Italics**: Sparingly — for emphasis or when introducing a new term
- Example: "This is what we mean by *AI-native*."

**Code formatting**:
- Inline: backticks for all commands, file names, config keys, URLs — `seite build`, `seite.toml`, `draft: true`
- Code blocks: always include language identifier

```bash
seite init mysite --title "My Site"
seite build
seite deploy
```

**Callout boxes**: Use the `{{% callout %}}` shortcode for tips, warnings, important notes.

### Lists

**Bulleted lists**:
- Capitalize first word
- Period if complete sentence, no period if a fragment
- Keep parallel structure (all sentences or all fragments — not mixed)

**Numbered lists**:
- Use for sequential steps only
- Same capitalization and punctuation rules as bullets

**Nested lists**: Max 2 levels deep.

### Links

**Anchor text**:
- Descriptive and keyword-relevant: "deploy to Cloudflare Pages" not "click here"
- 2-6 words typically
- Always refer to `context/internal-links-map.md` for internal link targets
- 3-5 internal links per article minimum

---

## Content Structure

### Article Introduction (150-250 words)

1. **Hook** (1-2 sentences): Bold claim, surprising fact, or sharp question
2. **Problem** (2-3 sentences): What's broken or missing for the reader?
3. **Promise** (2-3 sentences): What will they learn or be able to do?
4. **Preview** (optional): Brief outline of what's covered

Primary keyword must appear in the first 100 words.

The seite brand opens strong — no throat-clearing, no "In this article, we will..."

### Section Length

- Minimum: 150 words per section
- Maximum: 500 words (break into subsections if longer)
- Subheading every 300-400 words

### Conclusion (100-200 words)

1. Brief recap (1 short paragraph or 3-4 bullets)
2. Clear next action (what should the reader do or try?)
3. One internal link to a relevant doc or related post

---

## SEO-Specific Style

### Meta Titles

- 50-60 characters
- Format: `Primary Keyword: Supporting Benefit | seite`
- Example: `Hugo Alternative: Speed, Simplicity, and AI | seite`

### Meta Descriptions

- 150-160 characters
- Include primary keyword + a concrete benefit + implied CTA
- Example: "seite is a single-binary static site generator with built-in AI integration. Ship a blog, docs, and changelog in one afternoon."

### URL Slugs

- Primary keyword in the slug
- 3-5 words, lowercase, hyphens
- No stop words unless needed for clarity
- seite filename format for posts: `YYYY-MM-DD-slug-here.md`

---

## Dates & Time

- Date format: `March 13, 2026` (Month DD, YYYY)
- In frontmatter: `2026-03-13` (ISO 8601)
- Avoid relative dates in published content ("last year", "recently")

---

## Code Examples

Every how-to post should include at least one runnable CLI example. Prefer showing the full command with real flags over pseudocode.

**Good:**
```bash
seite new post "How to Deploy a Static Site" --tags deployment,cloudflare
seite build
seite deploy
```

**Avoid:**
```
[run the build command]
[deploy your site]
```

Always show what the reader will see or get — if a command produces output, show the output.

---

## Voice & Tone Reminders

Imagine explaining something to a developer friend over coffee. You're direct, specific and you actually shipped the thing you're talking about.

- Lead with the benefit or answer, not the setup
- Use short sentences for emphasis: "That's it."
- Show commands, not descriptions of commands
- Acknowledge tradeoffs honestly — don't oversell
- No corporate hedging ("may help", "can potentially", "in some cases")

### By Content Type

- **Blog posts**: Conversational, specific, concrete examples throughout
- **Docs**: Task-focused, minimal preamble, imperative voice ("Run `seite build`")
- **Changelog**: Past tense, specific about what changed and why ("Fixed: sidebar nav now scrolls to active item")
- **Homepage/landing**: Punchy, benefit-led, short sentences

---

## Editing Checklist

**Before publishing:**
- [ ] Primary keyword in H1, first 100 words, meta title and description
- [ ] Title Case on all headings
- [ ] No Oxford comma
- [ ] seite is lowercase throughout
- [ ] CLI commands in backticks
- [ ] 3-5 internal links with descriptive anchors
- [ ] Active voice, no filler words
- [ ] Code examples are runnable
- [ ] Strong opening — no throat-clearing
- [ ] Conclusion has a clear next action
