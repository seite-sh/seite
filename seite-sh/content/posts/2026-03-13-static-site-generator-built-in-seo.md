---
title: "Static Site Generator with Built-In SEO: 2026 Checklist"
date: 2026-03-13
description: "Not all SSGs include canonical URLs, JSON-LD, and llms.txt out of the box. Here's the 12-point checklist to evaluate any static site generator for SEO."
tags:
  - seo
  - static-site-generator
  - structured-data
  - geo
extra:
  primary_keyword: "static site generator with built-in SEO"
  meta_title: "Static Site Generator with Built-In SEO for 2026 | seite"
  meta_description: "Not all SSGs earn the label. This 12-point checklist tests any static site generator with built-in SEO for canonical URLs, JSON-LD, and GEO features."
  url_slug: "static-site-generator-built-in-seo"
  secondary_keywords:
    - static site SEO
    - best static site generator for SEO
    - static site generator canonical URL
    - static site generator json-ld
    - SEO static site generator
    - generative engine optimization
---

# Static Site Generator with Built-In SEO: 2026 Checklist

Every static site generator claims built-in SEO. Most of them ship you an empty `<meta name="description">` tag and call it done. Getting static site SEO right means more than a tag that exists: it means a tag that works correctly for every page type, generated from your config, without theme maintenance.

"Built-in" turns out to be a marketing word, not a technical guarantee. It can mean anything from "the theme has a placeholder for your meta title" to "canonical URLs, Open Graph tags, JSON-LD structured data, and AI-discoverable output are generated automatically on every build." The gap between those two interpretations is the gap between ranking and not ranking.

This is a 12-point checklist for evaluating any static site generator with built-in SEO, before you build 200 pages on it and discover what's missing. We'll also cover the three emerging GEO (generative engine optimization) features that separate truly modern SSGs from tools that treat 2024 as the finish line. For context on the broader AI-native SSG landscape, see the [AI static site generator](/blog/ai-static-site-generator) deep dive.

## What "Built-In SEO" Actually Means

Mei had done her homework. She chose a static site generator that advertised "SEO-ready themes" and "built-in meta tags." She spent two weeks building her SaaS marketing site, launched, and submitted to Google Search Console. Three weeks later, Search Console flagged a critical issue: every page on her site was claiming `https://localhost:3000` as its canonical URL.

The SSG had a canonical URL tag. It was hardcoded to use a theme-specific variable. She had set `base_url` correctly in her config but the theme author had wired it to a different key. Built-in SEO, in this case, meant the tag existed. Whether it worked was a separate question.

"Built-in" has two versions:

**Version 1: Structural**: The theme includes the HTML tag. Someone has to wire it up correctly, maintain it when templates change, and verify it on every page type.

**Version 2: Architectural**: The build pipeline generates the tag from your config and frontmatter automatically, for every page type, and you can verify it with a single build and View Source.

The 12-point checklist below tests for architectural SEO. Not whether the tag exists somewhere in a theme, but whether it works correctly by default, for every page, without custom template work. If you want to see architectural SEO in practice, [the getting started guide](/docs/getting-started) walks through a full `seite build` output in five minutes.

## The 12-Point Checklist for Any Static Site Generator with Built-In SEO

**Tier 1: Non-Negotiables**

1. Canonical URL generated from `base_url` config (not hardcoded, not localhost)
2. Meta description from frontmatter `description:` field with site fallback
3. Sitemap.xml auto-generated at every build
4. robots.txt generated with configurable rules

**Tier 2: Social and Structured Data**

5. Open Graph tags: `og:title`, `og:description`, `og:type`, `og:url`, `og:site_name`
6. Twitter/X Card tags: `twitter:card`, `twitter:title`, `twitter:description`
7. JSON-LD structured data: `BlogPosting` for posts, `Article` for docs, `WebSite` for index

**Tier 3: Advanced SEO**

8. Per-page robots control via frontmatter (`robots: noindex`)
9. RSS autodiscovery in `<head>` via `<link rel="alternate">`
10. hreflang alternates for multilingual sites
11. Asset fingerprinting for cache-busting (performance = SEO)
12. Markdown alternate link for AI-readable content (`<link rel="alternate" type="text/markdown">`)

## Tier 1: The Non-Negotiables

These four features should be in every static site generator claiming SEO support. If any are missing, the tool is not SEO-ready out of the box.

### 1. Canonical URLs

The static site generator canonical URL tag (`<link rel="canonical">`) tells search engines which version of a page is authoritative. Without it, duplicate content such as trailing slashes, `www` vs. non-`www` and HTTP vs. HTTPS splits your ranking signals across multiple URLs.

The thing to verify: does the canonical tag use your configured `base_url`, or does it use a hardcoded or theme-specific variable? Build the site with a production domain in config, open any generated HTML file, and search for `canonical`. The `href` attribute should match your production domain exactly.

### 2. Meta Description from Frontmatter

A meta description that reads as empty or falls back to `site.description` for every page is not built-in SEO. It's a default. Each page should pull from the `description:` value in its frontmatter, with the site description as a fallback when none is set.

The template expression to look for: `{{ page.description | default(value=site.description) }}`, not `{{ site.description }}` alone.

### 3. Sitemap.xml

Auto-generated sitemaps are table stakes. What varies is quality. A basic sitemap lists every page URL. A good sitemap includes `<lastmod>` dates from your frontmatter's `updated:` field, per-language variants with `xhtml:link` alternates, and proper exclusion of draft pages. Verify that the sitemap regenerates on every build and that draft content is excluded from the default build.

### 4. robots.txt

A static robots.txt file with no configuration is adequate for simple sites. But when you need to control which crawlers access which paths, especially for AI search, you need a robots.txt that reflects your site's configuration. More on AI crawler configuration in the GEO section below.

**See all four working in one build.** Run `seite build` and open `dist/robots.txt`, `dist/sitemap.xml`, and View Source on any page. Every feature is wired to your `seite.toml` config — canonical URL uses your `base_url`, meta description uses the page frontmatter, sitemap excludes drafts, robots.txt separates AI search from AI training crawlers. [Get started in five minutes](/docs/getting-started).

## Tier 2: Social and Structured Data

Most static site generators have partial Tier 1 support. Tier 2 is where the gaps appear, and where the consequences for missing features are most visible.

### 5. Open Graph Tags

Open Graph tags in a static site generator control how your pages look when shared on LinkedIn, Slack, iMessage, and social platforms. The minimum set: `og:title`, `og:description`, `og:type`, `og:url` and `og:site_name`. Posts need `og:type = article`. The homepage needs `og:type = website`. When a post has an `image:` frontmatter field, `og:image` should appear automatically.

Test this by pasting your URL into the [Meta Tags debugger](https://metatags.io). If you see the correct title, description and image, the Open Graph implementation is working. If you see only your site title and nothing else, you have a theme gap.

### 6. Twitter/X Card Tags

Twitter/X Cards use a parallel tag system. `twitter:card = summary_large_image` when the page has an image. `twitter:card = summary` when it does not. This should switch automatically based on whether `image:` is set in frontmatter, not require a separate template condition you maintain manually.

### 7. JSON-LD Structured Data

Static site generator JSON-LD support is the most commonly broken Tier 2 feature, and the hardest to detect without a deliberate audit. This is where Rafael discovered the problem.

Rafael built a developer tools blog on a popular static site generator with a highly-rated SEO theme. He checked Google Search Console's Rich Results Test six months after launch and found zero structured data detected. The theme had the right slots in the HTML head, but the JSON-LD block was an empty comment placeholder. The original theme author had removed it in a minor update and nobody caught it.

Six months of content. Zero structured data in search results.

JSON-LD tells search engines and AI engines what your content *is*, not just what it says. `BlogPosting` includes the author, publish date and modified date. `Article` covers docs and evergreen content. `WebSite` covers the homepage and enables sitelinks search in Google results. This should be generated from your content model automatically, not depend on theme maintenance or a plugin someone else has to update. Verify with [Google's Rich Results Test](https://search.google.com/test/rich-results) after your first build.

## Tier 3: Advanced SEO That Separates Good from Great

If a static site generator clears Tiers 1 and 2, it has solid conventional SEO coverage. Tier 3 is where modern development requirements come in.

### 8. Per-Page Robots Control

Some pages should not appear in search results: tag archives, internal preview pages, draft content published under a flag. Per-page robots control via frontmatter (`robots: noindex`) lets you exclude individual pages without editing your `robots.txt` or adding conditions to your templates. Set it in the file's frontmatter, and the build handles the rest.

### 9. RSS Autodiscovery in `<head>`

RSS autodiscovery via `<link rel="alternate" type="application/rss+xml">` in the HTML `<head>` lets feed readers and aggregators find your feed without being told where to look. Browsers also use this to surface the RSS icon in the address bar. It's a small tag, but it connects your content to a distribution channel that many developer tools blogs depend on.

### 10. hreflang for Multilingual Sites

If you publish in multiple languages, `hreflang` alternates in your `<head>` and sitemap tell search engines which version to serve for which locale. Missing hreflang means Google may serve the English version to Spanish-speaking users or treat translated pages as duplicate content. This should be generated from [your language configuration](/docs/i18n) automatically, not requiring per-template edits for each locale.

### 11. Asset Fingerprinting

Asset fingerprinting adds content-hash suffixes to your CSS and JS files (`main.a1b2c3d4.css`). This enables infinite browser caching: the filename changes only when the content changes, which improves Largest Contentful Paint and Cumulative Layout Shift scores. [Google's Core Web Vitals](https://web.dev/vitals/) are a confirmed ranking signal. Fingerprinting is enabled by setting `fingerprint = true` under `[build]` in `seite.toml` — see [the configuration reference](/docs/configuration) — and it is a build pipeline feature with a direct line to search performance.

### 12. Markdown Alternate Link

The `<link rel="alternate" type="text/markdown">` tag in your HTML `<head>` tells AI crawlers that a clean markdown version of this page exists at the corresponding `.md` URL. AI models process markdown more efficiently than HTML: no tags to strip, no navigation elements to filter, no JavaScript to ignore. Every page that ships a `.md` file alongside its `.html` file and includes this alternate link is more accessible to LLMs than a page that ships HTML only.

No other mainstream static site generator generates this automatically.

## The GEO Layer: SEO for AI Search Engines

The 12 features above cover traditional SEO: Google, Bing, browser behavior, social sharing. But a static site generator with built-in SEO in 2026 also needs to answer to AI search engines: ChatGPT, Perplexity, Claude and Google AI Overviews.

[Generative engine optimization](/blog/generative-engine-optimization) (GEO) is the practice of making your content citable in AI-generated answers. Three features matter for this layer.

**llms.txt and llms-full.txt**: Plain text files at your site root that give AI models a structured summary of your content. Think of llms.txt as `robots.txt` for discoverability: it tells AI systems what your site covers and which pages to prioritize. [What llms.txt is and how to implement it](/blog/what-is-llms-txt) covers the format and best practices. The short version: most SSGs don't generate it. Those that do typically require a plugin.

**AI-aware robots.txt**: The old advice was "don't block search engines." The precise version is: allow AI *search* crawlers (`ChatGPT-User`, `PerplexityBot`, `OAI-SearchBot`) for citation visibility, block AI *training* crawlers (`GPTBot`, `CCBot`, `Google-Extended`) to protect your content from training use. These are different user agents with different purposes, and your robots.txt should distinguish between them. Blanket `User-agent: * / Allow: /` rules make no distinction at all.

**Markdown output per page**: Every page generates a `.md` file alongside the `.html` file. AI crawlers that request the markdown version get clean, token-efficient content without parsing HTML. This is GEO infrastructure that traditional static site generators do not produce.

## How Major SSGs Compare on Built-In SEO

Here is how the major static site generators score across all 15 features (12 SEO plus 3 GEO):

| Feature | Hugo | Gatsby | Eleventy | Astro | seite |
|---------|------|--------|----------|-------|-------|
| Canonical URL | Theme | Plugin | Theme | Plugin | Built-in |
| Meta description from frontmatter | Theme | Plugin | Theme | Plugin | Built-in |
| Sitemap.xml | Built-in | Plugin | Plugin | Plugin | Built-in |
| robots.txt | Manual | Plugin | Manual | Manual | Built-in |
| Open Graph tags | Theme | Plugin | Theme | Plugin | Built-in |
| Twitter/X Card tags | Theme | Plugin | Theme | Plugin | Built-in |
| JSON-LD structured data | Theme | Plugin | Theme | Plugin | Built-in |
| Per-page robots (frontmatter) | No | Plugin | No | No | Built-in |
| RSS autodiscovery in `<head>` | Theme | Plugin | No | No | Built-in |
| hreflang for multilingual | Manual | Plugin | Manual | Manual | Built-in |
| Asset fingerprinting | No | Built-in | Plugin | Built-in | Built-in |
| Markdown alternate link | No | No | No | No | Built-in |
| llms.txt | No | No | No | No | Built-in |
| AI-aware robots.txt | Manual | Plugin | Manual | Manual | Built-in |
| Markdown output per page | No | No | No | No | Built-in |

"Theme" means the feature exists if your chosen theme implements it correctly. "Plugin" means you install and configure a separate package. "Built-in" means the build pipeline generates it from your config and frontmatter, for every page, on every build.

Hugo is fast. Gatsby has an ecosystem. Eleventy is flexible. Astro is excellent for content-heavy sites. But in all four cases, reaching full SEO coverage requires theme selection, plugin installation or manual template work. When you're evaluating a static site generator with built-in SEO, this table is what that actually looks like across tools. A seite site produces all 15 on the first `seite build`, with no plugin list and no [theme research](/docs/theme-gallery) required. For a direct feature comparison between Astro and seite including build speed, Node.js dependency, and AI-native output, see the [Astro alternative for static sites](/blog/astro-alternative-for-static-sites) breakdown.

**Want to run the comparison yourself?** Build a seite site and check the [configuration reference](/docs/configuration) to see which `seite.toml` options map to which SEO outputs.

## How to Verify Before You Commit

Don't trust marketing pages. Verify with the build output.

Run these five checks on any static site generator you're evaluating:

**1. Canonical check**: Build with a production domain in config. Open any generated HTML. Search for `canonical`. Does `href` match your domain?

**2. OG check**: Create a post with `description:` and `image:` in frontmatter. Build. Are `og:description` and `og:image` in the output with the correct values?

**3. JSON-LD check**: Open any post's HTML. Search for `application/ld+json`. Is the block present and correctly typed as `BlogPosting`?

**4. robots.txt check**: Open `dist/robots.txt`. Does it address AI search crawlers and AI training crawlers separately, or is it a single catch-all rule?

**5. Sitemap check**: Open `dist/sitemap.xml`. Is every page listed? Are draft pages excluded? For multilingual sites, do `xhtml:link` alternates appear?

If any check fails, that's configuration debt you carry for the life of your site. Factor it into your evaluation.

## Frequently Asked Questions

### What does "built-in SEO" mean for a static site generator?

Built-in SEO means the build pipeline generates canonical URLs, meta descriptions, Open Graph tags, Twitter Cards, JSON-LD structured data, sitemap.xml and robots.txt automatically from your config and frontmatter — without theme customization or plugins. The test: do all features work correctly on the first build, or do they require a compatible theme that someone maintains?

### What is the best static site generator for SEO?

seite generates all 12 standard SEO features plus three GEO features (llms.txt, AI-aware robots.txt, markdown output per page) automatically on every build. Hugo and Astro cover some features natively but rely on themes or plugins for structured data, hreflang and per-page robots control. Eleventy provides a minimal core where SEO is almost entirely theme-dependent.

### Do I need a plugin for JSON-LD structured data in Hugo?

Hugo does not generate JSON-LD structured data natively. You need a theme that includes the JSON-LD template block, or you add it to your layout files manually. Hugo's Internal Templates include basic Open Graph support but not JSON-LD. If structured data matters to you, verify your chosen theme includes it and test with Google's Rich Results Test.

### What is GEO and why does it matter for static sites?

Generative engine optimization is the practice of making your content citable in AI-generated search results from ChatGPT, Perplexity and Google AI Overviews. Static sites have a structural advantage for GEO: pre-rendered HTML, fast response times and no JavaScript dependency for content display. But most SSGs don't generate the files AI engines look for: llms.txt, markdown copies and AI-aware robots.txt. See the [full GEO implementation guide](/blog/generative-engine-optimization) for details.

### What is the markdown alternate link and why does it matter?

The `<link rel="alternate" type="text/markdown">` tag in a page's HTML `<head>` points AI crawlers to a clean markdown version of that page. AI models process markdown more efficiently than HTML. When your site ships `.md` files alongside `.html` files and includes this alternate link, AI crawlers can access your content without HTML parsing. seite generates both the `.md` files and the alternate link automatically. No other major SSG does this by default.

## SEO as a Build Output, Not a Configuration Project

The 12-point checklist above is a diagnostic, not a wish list. Every item should work out of the box. If the tool you're evaluating requires theme research, plugin installation or manual template editing to reach feature 7 or 12, that's configuration debt you carry for the life of your site.

SEO should be a build output. Not a plugin. Not a theme feature. Not something you verify by reading a theme's GitHub README. An SEO static site generator treats every feature in this checklist as a first-class build step, not a theme option or a plugin someone else maintains. A `seite build` produces canonical URLs, Open Graph tags, JSON-LD, sitemap, robots.txt, RSS autodiscovery, hreflang alternates, asset fingerprinting, markdown alternate links, llms.txt and markdown copies for every page — from a single config file.

Three things to remember:

1. **"Built-in" is not binary.** Check whether each feature is generated architecturally (from config and frontmatter) or structurally (exists in a theme someone else maintains).
2. **Verify with View Source, not the marketing page.** The build output tells the truth.
3. **GEO is now part of SEO.** llms.txt, AI-aware robots.txt and markdown output are not future features. AI search already sends measurable traffic, and sites that skip GEO are leaving it on the table.

See how seite handles all 15 in one build:

```bash
curl -fsSL https://seite.sh/install.sh | sh
seite init mysite --title "My Site" --collections posts,docs
cd mysite
seite build
# Open dist/index.html — all 15 features are there.
```

See the [templates and themes guide](/docs/templates) for customizing your design without touching the SEO layer.
