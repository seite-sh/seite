# SEO & GEO Best Practices in 2026: Comprehensive Research Report

> Research compiled February 2026 for the seite static site generator project.
> Sources include Search Engine Land, Semrush, Backlinko, Ahrefs, Moz, Google Search Central, the Princeton GEO research paper (KDD 2024), and 40+ industry publications.

---

## Executive Summary

The search landscape in 2026 is defined by a **dual-engine world**: traditional search engines (Google, Bing) and AI-powered answer engines (ChatGPT, Perplexity, Google AI Overviews, Claude). Two disciplines have emerged:

- **SEO** (Search Engine Optimization) — earning position in traditional search results
- **GEO** (Generative Engine Optimization) — earning citation in AI-generated answers

These are complementary, not competing. Strong SEO builds the authority foundation that AI systems rely on (97% of AI Overview citations come from pages already in the top 20 organic results). Strong GEO ensures that authority translates into AI citations.

**Key numbers:**
- ChatGPT: 810 million daily users, 2.5 billion daily queries
- Google AI Overviews: 1.5 billion monthly users, appearing in 16–57% of searches
- AI Overviews reduce organic CTR by ~34.5%, but cited brands see 35% higher CTR
- AI-referred visitors convert at 14.2% vs. Google organic's 2.8%
- LLM traffic is projected to overtake traditional Google search by end of 2027 (Semrush)
- Backlinko reports 800% year-over-year increase in LLM referrals

---

## Part 1: SEO Best Practices 2026

### 1.1 Core Web Vitals & Technical SEO

The three Core Web Vitals remain the primary page experience metrics:

| Metric | Threshold | What It Measures |
|--------|-----------|-----------------|
| **LCP** (Largest Contentful Paint) | ≤ 2.5 seconds | Loading performance |
| **INP** (Interaction to Next Paint) | ≤ 200 milliseconds | Responsiveness to all interactions |
| **CLS** (Cumulative Layout Shift) | ≤ 0.1 | Visual stability |

**INP replaced FID in March 2024** and measures responsiveness throughout the entire page lifecycle, not just the first interaction. Sites with INP above 300ms experienced 31% traffic drops on mobile after the December 2025 core update. Cross-browser support is expanding (Firefox 144+ supports INP; Safari has begun implementation).

**New in 2026: Visual Stability Index (VSI)** — measures layout stability throughout the entire user session (not just initial load), including shifts during scrolling, interactions, and route changes. Weights shifts based on user intent and attention.

**CWV as ranking signal:** Core Web Vitals function as a tiebreaker in competitive niches. Pages at position 1 are 10% more likely to pass CWV thresholds vs. position 9. Excellent CWV won't overcome poor content, but poor CWV can prevent great content from reaching full ranking potential. Field data from real Chrome users (CrUX) is what Google uses — not lab data.

**Practical optimizations for static sites:**
- Serve images in WebP/AVIF formats
- Eliminate render-blocking resources
- Set explicit image dimensions (width/height) to prevent CLS
- Use `loading="lazy"` for below-fold images, but eager-load above-fold hero images
- Enable HTTP/2 or HTTP/3 via CDN
- Minimize CSS/JS; strip unused code
- Use `scheduler.yield()` or break long JS tasks into chunks

### 1.2 E-E-A-T (Experience, Expertise, Authoritativeness, Trustworthiness)

E-E-A-T is not a direct ranking factor but a framework that Google's algorithms are trained to detect. In 2026, following the December 2025 core update and January 2026 "Authenticity Update," E-E-A-T now applies to **all sectors** — not just YMYL (Your Money or Your Life) topics.

**The four pillars:**

1. **Experience** — the most important pillar in 2026 because it is the only thing AI cannot simulate. First-hand, real-world knowledge (testing products, visiting places, working in the field) differentiates content from AI-generated summaries.

2. **Expertise** — demonstrable knowledge through credentials, education, or a proven track record. In-depth explanations beyond the obvious, correct use of technical terms, understanding "why" not just "what."

3. **Authoritativeness** — external recognition. Other credible sources cite you, link to you, or mention you. Cannot be manufactured quickly; built over time through consistent expertise demonstration.

4. **Trustworthiness** — the foundation. Google's guidelines state that a page cannot have high E-E-A-T if it is untrustworthy, regardless of experience/expertise/authority. Trust signals: HTTPS, clear author bios, contact information, transparent sourcing, accurate claims.

**Key 2026 developments:**
- Google added a new "Authors" section to Search Central documentation (February 2026)
- Topic-level expertise is now evaluated per-topic, not per-site — niche sites can outperform large generalists
- Sites using mass-produced AI content with minimal editorial oversight saw 85–95% traffic losses in December 2025
- Experience-demonstrating content saw 23% gains after December 2025 core update

### 1.3 Content Strategy

Google's Helpful Content System now runs continuously (not discrete rollouts) and evaluates page-level (not site-level). Content that works best in 2026 is content AI cannot easily replicate:

- **Original research and proprietary data** — unique insights not available elsewhere
- **First-hand experience** — reviews, case studies, "what we tested" sections
- **Opinionated analysis** — not just facts but informed perspective
- **Answer-first formatting** — put the key answer in the first 40–60 words, then expand
- **Clear hierarchical structure** — descriptive H2/H3/H4 headings, one topic per section
- **Short paragraphs** — 2–3 sentences max; bullet points and numbered lists for scannability
- **Data and statistics** — concrete numbers with sources and dates
- **Tables for comparison data** — structured data is easier for both humans and AI to parse

**Content freshness matters more than ever.** AI systems automatically append the current year to 28.1% of sub-queries. ChatGPT prioritizes recent content — 76.4% of top-cited pages were updated in the last 30 days. Always include clear "Last updated" timestamps.

### 1.4 Structured Data & Schema.org

Structured data is now the bridge between content and AI-powered search. Pages with valid structured data are 2.3x more likely to appear in Google AI Overviews and achieve 36% advantage in AI citations. JSON-LD is Google's recommended format.

**High-value schema types in 2026:**

| Schema Type | Value | Notes |
|-------------|-------|-------|
| **Article / BlogPosting** | High | Authorship, dates, publisher info. Essential for content sites |
| **Organization** | High | Knowledge Graph entity, brand authority signal |
| **Person** | High | E-E-A-T author signals, expertise verification |
| **BreadcrumbList** | High | 40% CTR improvement documented when added; structural hierarchy signals |
| **HowTo** | Medium-High | Procedural answers, step-by-step instructions |
| **WebSite** | Medium | Site-level context, search action potential |
| **Review / AggregateRating** | Medium | Trust signals for product/service content |
| **Product** | High (e-commerce) | Comparison queries, rich results |

**Deprecated/restricted in 2026:**
- **FAQPage** — restricted to government and health websites only (since 2023)
- **Practice Problem**, **Dataset** (general search), **Sitelinks Search Box**, **SpecialAnnouncement**, **Q&A** — deprecated by Google in January 2026

**Best practices:**
- Use JSON-LD format exclusively
- Only mark up visible, on-page content (invisible markup may trigger penalties)
- Validate with Google Rich Results Test and monitor via Search Console
- Combine Article schema with Person and Organization for complete authorship chains
- Include `dateModified` for freshness signals
- BreadcrumbList schema is one of the highest-ROI implementations (nearly 40% CTR decline when removed; 7% CTR when restored)

### 1.5 Meta Tags & On-Page SEO

**Title tags:** Still the second most important on-page factor. Optimal length 50–60 characters. Place primary keyword at the start. Google rewrites over 25% of title tags.

**Meta descriptions:** Not a direct ranking factor, but critical for CTR. Stay within 150–160 characters. Google rewrites 62%+ of meta descriptions, but they still serve as input for AI extraction.

**Canonical URLs:** Essential for deduplicating content. Must be consistent with og:url, sitemap URLs, and internal links. Mixed signals cause indexing problems.

**Open Graph tags (essential):**
- `og:title`, `og:description`, `og:url` (canonical), `og:type`, `og:site_name`, `og:locale`
- `og:image` — must use absolute URLs, recommended 1200×630px
- `og:type` = `"article"` for posts, `"website"` for homepages
- For articles: `article:published_time`, `article:modified_time`

**Twitter/X Cards:**
- `twitter:card` = `summary_large_image` (wide preview) or `summary` (compact)
- Title truncated at ~70 characters, description at ~200
- Falls back to OG tags if Twitter-specific tags are missing

### 1.6 Sitemap & Indexing

**XML sitemaps:**
- Include only indexable pages (exclude noindex)
- Auto-update when pages are added or removed
- Ensure URLs match canonical URLs
- Segment by content type for large sites
- No `changefreq` or `priority` needed (modern practice: Google ignores them)

**IndexNow:** Open protocol for instant update notifications. Supported by Bing, Yandex, Seznam, Naver. Google has tested but not officially adopted.

**robots.txt:**
- Not a noindex mechanism — use `<meta name="robots">` instead
- Don't block JS/CSS/image resources needed for rendering
- **AI bot governance** is now essential (see GEO section below)

### 1.7 Link Building & Authority

Backlinks remain a top-3 ranking factor, but quality matters more than quantity:
- A link from a relevant niche site with DR 40 often outperforms a generic DR 80 link
- Topical authority clustering is the new paradigm
- Links from sites with clear E-E-A-T signals carry more weight
- Brand mentions (even without links) are becoming equally important, especially for AI-driven search
- Google's SpamBrain detects paid and manipulative links; mass outreach and PBNs are largely ineffective

### 1.8 Mobile & Page Experience

Google completed mobile-first indexing for 100% of websites by July 2024. Key requirements:
- Content parity between mobile and desktop (same content, meta tags, structured data)
- Core Web Vitals must pass on mobile
- No intrusive interstitials/popups on mobile
- Touch-friendly navigation (44px × 44px minimum targets)
- Responsive design is the recommended approach

---

## Part 2: GEO Best Practices 2026

### 2.1 What Is GEO?

Generative Engine Optimization is the practice of structuring content so AI-powered search platforms — ChatGPT, Google AI Overviews, Perplexity, Claude, Copilot — can retrieve, cite, and recommend your brand when answering user queries.

The foundational academic work is ["GEO: Generative Engine Optimization"](https://arxiv.org/abs/2311.09735) by researchers from Princeton, Georgia Tech, Allen Institute for AI, and IIT Delhi (KDD 2024). Their key finding: GEO methods can boost content visibility by up to 40% in generative engine responses.

**Core distinction:** SEO is about rankings; GEO is about citations. There is no "position #1" in ChatGPT. Instead, visibility is about **mention frequency** across many responses — a "mention rate," not a ranking.

### 2.2 The Nine GEO Optimization Methods

Ranked by effectiveness from the Princeton research paper:

| Method | Visibility Improvement | Description |
|--------|----------------------|-------------|
| **Statistics Addition** | ~30–40% | Embed concrete data, percentages, dates |
| **Cite Sources** | ~30–40% | Reference authoritative sources by name |
| **Quotation Addition** | ~28% | Include expert quotes with attribution |
| **Fluency Optimization** | Moderate | Clear, well-structured writing |
| **Unique Words** | Moderate | Distinctive, precise vocabulary |
| **Technical Terms** | Moderate | Domain-specific terminology |
| **Authoritative Tone** | Moderate | Confident, expert voice |
| **Easy-to-Understand** | Lower | Simplified language |
| **Keyword Stuffing** | Negative | Traditional SEO tactic; performs poorly for GEO |

### 2.3 Content Structure for AI Citation

LLMs don't index and rank — they synthesize. AI systems pull individual passages, not entire pages. Key structural patterns:

**Answer-first format:** Lead with a TL;DR summary (40–50 words). AI systems prioritize opening paragraphs. Data shows 44.2% of all LLM citations come from the first 30% of text.

**Self-contained sections:** Each section should address a complete concept without requiring surrounding context. AI lifts standalone sentences and sections.

**Question-answer structuring:** Content with clear Q&A pairs was 40% more likely to be cited (Princeton study).

**Tables and structured data:** Comparison tables and structured data are highly extractable. Data relationships become explicit.

**Declarative language:** Use definite statements over hedged opinions. High entity density (name specific things). Explicit dates on data ("As of 2025...").

**Markdown versions of pages:** Shipping `.md` versions alongside HTML makes content directly consumable by LLMs without HTML-to-text conversion losses. This is emerging as a best practice.

### 2.4 AI Crawlers & robots.txt

The AI crawler landscape in 2026:

| Crawler | Operator | Purpose | Recommendation |
|---------|----------|---------|---------------|
| **Googlebot** | Google | Search indexing | Always allow |
| **Bingbot** | Microsoft | Search indexing (feeds ChatGPT) | Always allow |
| **ChatGPT-User** | OpenAI | Real-time user prompt interactions | Allow (for AI search visibility) |
| **OAI-SearchBot** | OpenAI | SearchGPT/ChatGPT search indexing | Allow |
| **GPTBot** | OpenAI | Training data collection | Block (unless you want to train models) |
| **ClaudeBot** | Anthropic | Training data for Claude | Block or allow based on preference |
| **Google-Extended** | Google | AI training (Gemini) | Block (doesn't affect SEO rankings) |
| **PerplexityBot** | Perplexity | AI search indexing | Allow (for Perplexity visibility) |
| **Applebot-Extended** | Apple | Apple Intelligence training | Block unless desired |
| **CCBot** | Common Crawl | Open dataset used by many LLMs | Block unless desired |
| **Bytespider** | ByteDance | LLM training | Block |

**Recommended mixed policy:** Allow search-facing crawlers (ChatGPT-User, OAI-SearchBot, PerplexityBot) while blocking training-only crawlers (GPTBot, Google-Extended, CCBot). Blocking Google-Extended does NOT impact SEO rankings.

**Cloudflare note:** Since July 2025, Cloudflare blocks AI bots by default. Site owners must actively consent to allow AI crawler access.

### 2.5 The llms.txt Standard

Proposed by Jeremy Howard (Answer.AI) in September 2024. The specification lives at [llmstxt.org](https://llmstxt.org/).

**Format:** A Markdown file at `/llms.txt` providing a curated, LLM-friendly overview of a site's most important content.

```markdown
# Site Name

> Brief description of the site

## Section Name

- [Page Title](https://example.com/page.md): Description of what the page covers
- [Another Page](https://example.com/other.md): Another description

## Optional

- [Changelog](https://example.com/changelog.md): Version history
```

**llms-full.txt:** The companion file containing the complete text of every page concatenated into a single markdown document. llms.txt is the "table of contents"; llms-full.txt is the "encyclopedia."

**Adoption as of early 2026:**
- BuiltWith tracks 844,000+ websites with llms.txt
- SE Ranking found ~10% adoption in a 300K domain study
- Major adopters: Anthropic, Cloudflare, Stripe, Vercel, Docker, HubSpot
- Deep penetration in developer tools/docs; almost absent from mainstream web

**The controversy:**
- **No major AI provider has confirmed reading llms.txt during inference**
- Semrush server log analysis: zero visits from GPTBot, ClaudeBot, PerplexityBot to llms.txt
- SE Ranking: no measurable citation uplift from llms.txt
- 8 of 9 sites saw no measurable traffic change after implementation
- John Mueller compared it to the "keywords meta tag" (easily gameable, no verification)

**However:**
- Google included llms.txt in their Agents to Agents (A2A) protocol
- Real value exists when developers explicitly provide llms.txt to AI coding tools (Cursor, Copilot, Claude Code)
- LangChain benchmarks show "optimized llms.txt" outperforms vector search for documentation retrieval
- Low cost to implement, zero demonstrated downside

**Assessment:** llms.txt is a low-risk bet that addresses a real need. Its value today is primarily in developer documentation and explicit context loading. The next 12–24 months will determine if it becomes essential web infrastructure or fades.

### 2.6 Structured Data for GEO

JSON-LD schema markup is the bridge between content and AI systems:
- Pages with structured data are 2.3x more likely to appear in Google AI Overviews
- GPT-4 accuracy jumps from 16% to 54% when content includes structured data (Data World study)
- 82% of AI citations come from "deep" topic-specific URLs rather than homepages

Priority schema types for GEO: Article/BlogPosting, Organization, Person (E-E-A-T signals), BreadcrumbList (structural hierarchy), HowTo (procedural answers), WebSite (site-level context).

### 2.7 How Each AI Platform Sources Content

Each platform has fundamentally different sourcing strategies:

**Google AI Overviews:**
- Built on top of Google Search, integrated with Knowledge Graph
- 76.1% of cited URLs also rank in top 10 organic results (strongest correlation with traditional SEO)
- Prefers established, high-authority domains; 49.2% of cited domains are 15+ years old
- Strongest brand signal weighting

**ChatGPT / SearchGPT:**
- Uses Bing search results as retrieval layer
- Only 12% of cited URLs rank in Google's top 10 — draws from a much wider pool
- Wikipedia is the most cited source (7.8% of citations)
- Favors older domains (45.8% are 15+ years old)

**Perplexity:**
- Retrieval-first design, most transparent citation practices
- Cites ~2.8x more sources per query than ChatGPT (21+ vs ~8)
- 94% citation success rate with persistent inline citations
- Reddit (6.6%), YouTube, Wikipedia are top sources

**Critical insight:** Only 12% of URLs cited by ChatGPT, Perplexity, and Copilot rank in Google's top 10. 80% of LLM citations don't even rank in Google's top 100. Optimizing solely for Google rankings is insufficient for AI visibility.

### 2.8 MCP (Model Context Protocol) and GEO

MCP enables active, structured access to content — AI agents query your content directly through a standardized interface, rather than relying on passive web crawling.

**Adoption scale (2025):**
- SDK downloads surpassed 97 million/month by December 2025
- 10,000+ active public MCP servers
- Every major AI platform supports MCP natively (ChatGPT, Claude, Gemini, Cursor, Windsurf, VS Code)
- OpenAI adopted MCP in March 2025; Anthropic donated it to the Linux Foundation in December 2025

**WebMCP:** Websites can announce themselves to every compatible browser. The visitor's AI agent discovers your knowledge base automatically. Shipped as early preview in Chrome 146 (June 2025).

**MCP vs llms.txt:** MCP may be the more sophisticated alternative. While llms.txt is a static file, MCP provides dynamic, queryable access to structured content. They are complementary — llms.txt is passive discovery; MCP is active integration.

### 2.9 Measuring GEO Success

The primary metric has shifted from Click-Through Rate to **Citation Share** and **Mention Share**.

**Key metrics:**
- **Share of Model / AI-Generated Visibility Rate (AIGVR):** Percentage of target prompts where your brand appears across AI platforms
- **Citation Count & Attribution Rate:** Total references with source credits
- **Share of Sentiment:** Sentiment analysis of brand mentions in AI responses
- **AI Referral Traffic & Conversion:** Traffic and revenue from AI search

**The measurement challenge:** 92% of Gemini answers provide no clickable citation. 24% of ChatGPT responses omit citations. Content may influence AI responses without generating trackable traffic ("dark visibility").

**GEO tools ecosystem:** 35+ AI search monitoring tools launched in 2024–2025 (Profound, Peec, Otterly, Siftly, Bear AI, LLM Pulse, Bluefish, Conductor).

---

## Part 3: Seite Codebase Audit — Current State

### 3.1 What Seite Already Implements Well

**Comprehensive meta tag coverage** — all 6 bundled themes implement identical SEO head blocks:
- Favicon, canonical URL, meta description (with site fallback)
- Full Open Graph suite (og:type, og:url, og:title, og:description, og:site_name, og:locale, og:image)
- Full Twitter Card suite (twitter:card, twitter:title, twitter:description, twitter:image)
- Per-page robots meta tag
- RSS autodiscovery link
- hreflang links with x-default for multilingual sites
- `<html lang="{{ lang }}">` attribute

**JSON-LD structured data** — three schema types:
- `BlogPosting` for posts (with `datePublished`, `dateModified`, `author`, `publisher`, `url`)
- `Article` for other collections
- `WebSite` for homepage/index
- All values properly JSON-encoded via `json_encode()` filter

**GEO/LLM discovery files:**
- `robots.txt` with sitemap reference and llms.txt/llms-full.txt comments
- `llms.txt` following the llmstxt.org spec (H1, blockquote, H2 sections, `[title](url.md): description`)
- `llms-full.txt` with complete raw markdown for every page
- `.md` output alongside every `.html` page (unexpanded shortcodes preserved)
- Markdown alternate `<link>` tag in every page head
- llms.txt discovery `<link>` tag in every page head

**XML sitemap:**
- `<lastmod>` from `page.date`
- `xhtml:link` alternates for multilingual content
- `x-default` hreflang for index pages
- Extra URLs (tag pages, collection indexes)
- No `changefreq` or `priority` (modern best practice)

**Image optimization:**
- Responsive srcset at configurable widths
- WebP variant generation
- `<picture>` element with source fallbacks
- `loading="lazy"` attribute
- Width/height dimensions for CLS prevention

**Accessibility:**
- Skip-to-main link (visible on focus)
- Semantic landmarks (`<main>`, `<nav>` with aria-labels)
- Search with `role="search"` and `aria-live="polite"` on results
- `prefers-reduced-motion: reduce` media query
- Focus-visible indicators

**Performance:**
- CSS/JS minification
- Asset fingerprinting (optional, FNV hash)
- Code copy buttons injected post-build

**MCP server:**
- Full JSON-RPC 2.0 over stdio for AI tool integration
- Resources: docs, config, content, themes, trust, mcp-config
- Tools: build, create_content, search, apply_theme, lookup_docs

### 3.2 Gaps Against 2026 Best Practices

#### High Priority

1. **og:image not absolutized in themes** — Templates use raw `{{ page.image }}` without checking for relative paths. The CLAUDE.md scaffold documents the correct pattern (`is starting_with(pat="http")`) but themes don't implement it. Relative image URLs will fail in OG/Twitter previews.

2. **Missing BreadcrumbList schema** — One of the highest-ROI structured data implementations. Documented 40% CTR decline when removed. Currently no breadcrumb schema is emitted despite docs collection having nested navigation.

3. **No AI crawler directives in robots.txt** — robots.txt only has comments about llms.txt. No `User-agent: GPTBot`, `User-agent: ClaudeBot`, etc. directives. In 2026, the recommended practice is a mixed policy: allow search-facing crawlers, block training-only crawlers.

4. **Missing `dateModified` in sitemap** — Sitemap uses `page.date` for `<lastmod>` but doesn't check `page.updated` (which is the actual last-modified date). The `updated` frontmatter field exists and feeds JSON-LD `dateModified` but isn't used in the sitemap.

#### Medium Priority

5. **No og:image dimensions** — Missing `og:image:width`, `og:image:height`, `og:image:type`. Social platforms increasingly benefit from image metadata for preview generation.

6. **No Person schema for authors** — E-E-A-T signals are increasingly important. Currently `author` is just a string in JSON-LD (`{"@type":"Person","name":"..."}`). Could be enhanced with author page URLs, credentials, sameAs links.

7. **No BreadcrumbList schema for docs** — The docs theme has sidebar navigation with hierarchy, but no corresponding structured data.

8. **Markdown output missing frontmatter** — `.md` files contain body-only content without YAML frontmatter. LLMs would benefit from structured metadata (title, description, date, tags) in the markdown files.

9. **Lazy loading applies globally** — All images get `loading="lazy"` regardless of viewport position. Above-fold hero images should use eager loading. LCP can be negatively impacted by lazy-loading the largest contentful element.

10. **llms-full.txt lacks per-page URLs** — The full content dump doesn't include the URL for each page, making it harder for AI systems to attribute content back to its source.

#### Lower Priority

11. **No `article:published_time` / `article:modified_time` OG tags** — These Open Graph article tags provide additional date signals to social platforms.

12. **No DNS prefetch / resource hints** — No `<link rel="dns-prefetch">` or `<link rel="preconnect">` for third-party analytics domains.

13. **No explicit Content-Security-Policy guidance** — CSP headers are deployment-level but documentation could recommend them.

14. **Search index metadata could be richer** — Could include dates, reading time, tags in search JSON for better client-side filtering.

---

## Part 4: Recommendations for Seite

### Immediate Wins (High Impact, Low Effort)

1. **Fix og:image absolutization** in all 6 theme templates — use the pattern already documented in CLAUDE.md
2. **Add AI crawler directives to robots.txt** — mixed policy allowing search crawlers, commenting guidance for training crawlers
3. **Use `page.updated` (or `page.date` fallback) for sitemap `<lastmod>`** — the data already exists
4. **Add `article:published_time` and `article:modified_time` OG tags** for posts

### Medium-Term Enhancements

5. **Add BreadcrumbList JSON-LD** — especially for docs collections with nested hierarchy
6. **Add og:image dimensions** (`og:image:width`, `og:image:height`) when image processing is enabled
7. **Include frontmatter in .md output** — title, description, date, tags, url
8. **Add page URLs to llms-full.txt** entries for attribution
9. **Conditional lazy loading** — don't add `loading="lazy"` to the first image on a page (likely LCP element)
10. **Enhance Person schema** — link to author pages when author URL is available

### Forward-Looking Considerations

11. **AVIF support** — next-gen image format with better compression than WebP
12. **WebMCP exploration** — Chrome 146+ supports web-based MCP discovery
13. **Inline LLM instructions** — Vercel's `<script type="text/llms.txt">` proposal for per-page AI context
14. **IndexNow integration** — instant notification to search engines on content changes
15. **GEO-aware scaffold guidance** — update SEO requirements scaffold to include GEO-specific content structuring advice (answer-first format, self-contained sections, statistics, citations)

---

## Sources

### SEO — General & Technical
- [Core Web Vitals Optimization Guide 2026 — Sky SEO Digital](https://skyseodigital.com/core-web-vitals-optimization-complete-guide-for-2026/)
- [Core Web Vitals 2026: Technical SEO That Actually Moves the Needle — ALM Corp](https://almcorp.com/blog/core-web-vitals-2026-technical-seo-guide/)
- [How Important Are Core Web Vitals for SEO in 2026? — White Label Coders](https://whitelabelcoders.com/blog/how-important-are-core-web-vitals-for-seo-in-2026/)
- [Technical SEO Checklist for Developers 2026 — Yaam Web Solutions](https://blog.yaamwebsolutions.com/technical-seo-checklist-for-developers-2026/)
- [Understanding Core Web Vitals — Google Search Central](https://developers.google.com/search/docs/appearance/core-web-vitals)
- [Interaction to Next Paint — web.dev](https://web.dev/articles/inp)

### E-E-A-T & Content Quality
- [E-E-A-T: The Ultimate Guide to Google Rankings in 2026 — SEO-Kreativ](https://www.seo-kreativ.de/en/blog/e-e-a-t-guide-for-more-trust-and-top-rankings/)
- [E-E-A-T in 2026: Content Quality Signals That Matter — BKND Development](https://www.bknddevelopment.com/seo-insights/eeat-seo-strategy-2026-content-quality-signals/)
- [E-E-A-T as a Ranking Signal in AI-Powered Search — ClickPoint Software](https://blog.clickpointsoftware.com/google-e-e-a-t)
- [Creating Helpful, Reliable, People-First Content — Google Search Central](https://developers.google.com/search/docs/fundamentals/creating-helpful-content)

### Structured Data
- [Structured Data SEO 2026: Rich Results Guide — Digital Applied](https://www.digitalapplied.com/blog/structured-data-seo-2026-rich-results-guide)
- [Stop Using FAQ Schema: New Rules of Structured Data in 2026 — GreenSerp](https://greenserp.com/high-impact-schema-seo-guide/)
- [Structured Data: SEO and GEO Optimization for AI — Digidop](https://www.digidop.com/blog/structured-data-secret-weapon-seo)
- [Schema Markup for AI Search — WPRiders](https://wpriders.com/schema-markup-for-ai-search-types-that-get-you-cited/)

### AI Overviews & AI Search
- [Google AI Overviews Optimization: Complete Guide 2026 — Koanthic](https://koanthic.com/en/google-ai-overviews-optimization-complete-guide-2026/)
- [The Future of AI Search: What 6 SEO Leaders Predict for 2026 — Search Engine Land](https://searchengineland.com/ai-search-visibility-seo-predictions-2026-468042)
- [Google AI Mode: What SEOs Need to Know Before 2026 — SEO.com](https://www.seo.com/ai/google-ai-mode/)
- [SEO in 2025: The AI-Powered Transformation & What's Next for 2026 — Connect4 Consulting](https://connect4consulting.com/blog/seo-in-2025-the-ai-powered-transformation-whats-next-for-2026/)

### GEO — Generative Engine Optimization
- [GEO: Generative Engine Optimization — arXiv (Princeton et al.)](https://arxiv.org/abs/2311.09735)
- [Generative Engine Optimization: The 2026 Guide — LLMrefs](https://llmrefs.com/generative-engine-optimization)
- [Mastering Generative Engine Optimization in 2026 — Search Engine Land](https://searchengineland.com/mastering-generative-engine-optimization-in-2026-full-guide-469142)
- [GEO Best Practices for 2026 — Firebrand](https://www.firebrand.marketing/2025/12/geo-best-practices-2026/)
- [The Complete Guide to GEO in 2026 — EXO Rank](https://exorank.io/the-complete-guide-to-generative-engine-optimization-geo-in-2026/)
- [GEO: How to Win in AI Search — Backlinko](https://backlinko.com/generative-engine-optimization-geo)
- [10-Step GEO Framework — TryProfound](https://www.tryprofound.com/guides/generative-engine-optimization-geo-guide-2025)

### llms.txt
- [The /llms.txt File — llmstxt.org](https://llmstxt.org/)
- [What Is LLMs.txt & Should You Use It? — Semrush](https://www.semrush.com/blog/llms-txt/)
- [Should Websites Implement llms.txt in 2026? — LinkBuildingHQ](https://www.linkbuildinghq.com/blog/should-websites-implement-llms-txt-in-2026/)
- [What is llms.txt? Breaking Down the Skepticism — Mintlify](https://www.mintlify.com/blog/what-is-llms-txt)
- [Is llms.txt Dead? Current State of Adoption — llms-txt.io](https://llms-txt.io/blog/is-llms-txt-dead)
- [The Complete Guide to llms.txt — Publii](https://getpublii.com/blog/llms-txt-complete-guide.html)

### AI Crawlers & Discovery
- [Understanding AI Crawlers: Complete Guide — Qwairy](https://www.qwairy.co/blog/understanding-ai-crawlers-complete-guide)
- [Optimizing robots.txt for AI Crawlers — GenRank](https://genrank.io/blog/optimizing-your-robots-txt-for-generative-ai-crawlers/)
- [Best Practices for AI-Oriented robots.txt — Francisco Kemeny](https://medium.com/@franciscokemeny/best-practices-for-ai-oriented-robots-txt-and-llms-txt-configuration-be564ba5a6bd)

### Content Structure for AI
- [The Definitive Guide to LLM-Optimized Content — Averi AI](https://www.averi.ai/breakdowns/the-definitive-guide-to-llm-optimized-content)
- [Creating LLM-Friendly Content Formats — Wildcat Digital](https://wildcatdigital.co.uk/blog/creating-llm-friendly-content-formats/)
- [Content Freshness & AI Citations Guide — Qwairy](https://www.qwairy.co/blog/content-freshness-ai-citations-guide)
- [How to Optimize Content for AI Search — Semrush](https://www.semrush.com/blog/how-to-optimize-content-for-ai-search-engines/)

### MCP & Emerging Standards
- [A Year of MCP: From Internal Experiment to Industry Standard — Pento](https://www.pento.ai/blog/a-year-of-mcp-2025-review)
- [A Proposal for Inline LLM Instructions in HTML — Vercel](https://vercel.com/blog/a-proposal-for-inline-llm-instructions-in-html)
- [MCP's Impact on 2025 — Thoughtworks](https://www.thoughtworks.com/en-us/insights/blog/generative-ai/model-context-protocol-mcp-impact-2025)

### Measuring GEO
- [How to Track AI Citations and Measure GEO Success — Averi AI](https://www.averi.ai/how-to/how-to-track-ai-citations-and-measure-geo-success-the-2026-metrics-guide)
- [GEO Metrics That Matter — LLM Pulse](https://llmpulse.ai/blog/geo-metrics/)
- [2026 AEO/GEO Benchmarks Report — Conductor](https://www.conductor.com/academy/aeo-geo-benchmarks-report/)

### AI Platform Citation Patterns
- [AI Platform Citation Patterns — Profound](https://www.tryprofound.com/blog/ai-platform-citation-patterns)
- [ChatGPT vs Perplexity vs Google vs Bing Comparison — SE Ranking](https://seranking.com/blog/chatgpt-vs-perplexity-vs-google-vs-bing-comparison-research/)
- [100+ AI SEO Statistics for 2026 — Position Digital](https://www.position.digital/blog/ai-seo-statistics/)
