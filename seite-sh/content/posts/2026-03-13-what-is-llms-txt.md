---
title: "What Is llms.txt? A Developer's Guide to AI Discoverability"
date: 2026-03-13
description: "What is llms.txt? A markdown file that tells AI what your site is about. Learn the format, what the data says, and how to generate it automatically on every build."
tags:
 - llms-txt
 - ai
 - seo
 - generative-engine-optimization
draft: false
extra:
 primary_keyword: "what is llms.txt"
---

Google's John Mueller compared llms.txt to the keywords meta tag, essentially calling it useless. Three months later, Google quietly added an llms.txt file to their own developer documentation. Then removed it.

That contradiction tells you more about the state of AI discoverability than any research study. The people building the biggest AI search products aren't sure what to do with llms.txt either. But they're paying attention.

So what is llms.txt, and should you care? If you're a developer trying to figure out whether it matters, you've probably read two kinds of articles: breathless hype ("you need this now or AI will ignore you!") and dismissive takes ("Google says it doesn't work"). Neither is useful. The honest answer is somewhere in between, and it depends on what kind of site you're building and what you expect from AI search.

This guide explains the llms.txt format, what the data actually says about its effectiveness, and how to implement it without thinking about it again. No hype. No hand-waving. Just what a developer needs to know.

## What Is llms.txt?

llms.txt is a markdown file placed at your website's root (e.g., `example.com/llms.txt`) that tells large language models what your site is about and which pages matter most. It provides a clean, structured summary that AI systems can read without parsing HTML, JavaScript, or navigation elements.

[Jeremy Howard](https://www.answer.ai/posts/2024-09-03-llmstxt.html) proposed the standard in September 2024 through Answer.AI. The core problem it solves: LLMs have limited context windows and can't efficiently crawl an entire website the way Google's spider does. When ChatGPT or Claude needs information from your site, it fetches a page, tries to extract the useful content from a soup of HTML tags, sidebars, and scripts, and often gets confused.

llms.txt cuts through that noise. Instead of forcing an AI to parse your homepage HTML and guess what matters, you hand it a clean markdown file that says: here's who we are, here's what we do, and here are the pages worth reading.

Think of it as a table of contents for AI. Not access control (that's `robots.txt`). Not a sitemap (that's `sitemap.xml`). A reading guide.

The format uses markdown rather than XML because the primary consumers are language models, and language models read markdown natively. It's one of the few web standards designed for AI readers first and human readers second.

## llms.txt vs robots.txt: Different Jobs

These two files sit next to each other in your site root, but they solve completely different problems.

| | robots.txt | llms.txt |
|---|---|---|
| **Purpose** | Access control: where bots can and can't go | Reading guide: what AI should pay attention to |
| **Format** | Plain text with directives | Markdown with headings and links |
| **Audience** | Web crawlers (Googlebot, Bingbot) | Language models (ChatGPT, Claude, Perplexity) |
| **Action** | Blocks or allows crawling | Highlights and summarizes content |
| **Analogy** | "Keep Out" sign on specific rooms | Table of contents for the whole building |
| **History** | Established standard since 1994 | Proposed September 2024 |

robots.txt tells crawlers where they *can't* go. llms.txt tells AI what it *should* read. They complement each other. You still need robots.txt to manage crawl behavior. llms.txt adds a layer specifically for AI comprehension.

For [AI-native static site generators](/blog/ai-static-site-generator), both files are build outputs. seite generates `robots.txt` with AI crawler management (allowing AI search bots like ChatGPT-User and PerplexityBot while blocking AI training crawlers like GPTBot) alongside `llms.txt` on every build. The [deployment pipeline](/docs/deployment) publishes both files automatically. They work together as part of a coherent AI discoverability strategy.

## The llms.txt Format

The [official specification](https://llmstxt.org/) is simple. An llms.txt file contains:

1. **An H1 heading** (required): your project or site name
2. **A blockquote** (recommended): a one-paragraph summary of your site
3. **Sections with links** (optional): grouped lists of important pages with brief descriptions

Here's a practical llms.txt example:

```markdown
# My Developer Tool

> My Developer Tool is an open-source CLI for building and deploying
> static websites. It supports markdown content, custom themes, and
> automated deployment to major hosting platforms.

## Documentation

- [Getting Started](https://example.com/docs/getting-started.md): Install and create your first site
- [Configuration](https://example.com/docs/configuration.md): Full config file reference
- [Deployment](https://example.com/docs/deployment.md): Deploy to GitHub Pages, Cloudflare, or Netlify
- [Templates](https://example.com/docs/templates.md): Tera template syntax and variables

## Blog

- [Why We Built This](https://example.com/posts/why-we-built-this.md): The problem we're solving
- [v2.0 Release](https://example.com/posts/v2-release.md): What's new in the latest version
```

Notice the links point to `.md` files, not `.html`. This is intentional. When an AI model follows a link from llms.txt, it should land on clean markdown, not HTML with CSS and JavaScript. This is why llms.txt works best when your site also outputs raw markdown for every page.

### llms.txt vs llms-full.txt

The specification defines two files:

**llms.txt** is the summary. It lists your most important pages with one-line descriptions and links. It's small enough to fit in any LLM's context window. Think of it as the table of contents.

**llms-full.txt** is the complete content. Every page on your site, concatenated into a single markdown file. It's large, potentially too large for a single context window, but it gives AI systems access to your entire site in one request. Think of it as the full book.

You want both. llms.txt for quick orientation. llms-full.txt for deep reading. seite generates both on every build, no configuration needed.

## Does llms.txt Actually Work?

This is the question everyone asks, and the honest answer is complicated.

### The Skeptic's Case

The data isn't encouraging right now.

[SE Ranking analyzed 300,000 domains](https://seranking.com/blog/llms-txt/) and found no correlation between having an llms.txt file and being cited by AI systems. Zero measurable impact. The adoption rate across those domains was 10.13%, and the sites with llms.txt weren't cited more often than those without.

Google's John Mueller [compared llms.txt to the keywords meta tag](https://www.searchenginejournal.com/google-says-llms-txt-comparable-to-keywords-meta-tag/544804/), implying it's a well-intentioned idea that search engines will likely ignore. Google has said explicitly that [AI Overviews rely on traditional SEO signals](https://searchengineland.com/google-says-normal-seo-works-for-ranking-in-ai-overviews-and-llms-txt-wont-be-used-459422), not llms.txt.

No major AI platform (ChatGPT, Perplexity, Google AI Overviews) has confirmed using llms.txt as a retrieval or ranking input.

These are facts. If you're looking for proof that llms.txt will improve your search rankings today, it doesn't exist.

### The Builder's Case

But the skeptic's case has gaps.

First, the contradiction. Google dismissed llms.txt, then [quietly added it to their own developer docs](https://www.omnius.so/industry-updates/google-adds-llms-txt-to-docs-after-publicly-dismissing-it). If it were truly useless, why implement it? The most likely explanation: Google doesn't want to create an SEO gold rush around a new file format, but their engineering teams see the value for AI readability.

Second, the study measures the wrong thing. The 300k-domain study measured whether llms.txt correlates with AI citations *today*. That's like measuring whether HTTPS correlated with rankings in 2013, two years before Google made it a ranking signal. The absence of a current effect doesn't predict the future direction.

Third, the cost is near zero. Adding an llms.txt file to your site takes minutes. If your build tool generates it automatically, it takes zero additional effort. The downside of having it is nothing. The downside of not having it, if AI systems start using it, is being invisible to a growing channel.

When Liam, an API documentation maintainer, noticed that ChatGPT was giving wrong answers about his company's SDK, he investigated. The AI was parsing HTML pages with navigation sidebars, footer links, and cookie banners mixed into the content. The LLM couldn't distinguish documentation from chrome. After adding llms.txt with links to clean markdown versions of each docs page, the AI answers improved noticeably. Not because llms.txt was a ranking signal, but because the AI had a clean reading path to accurate content. See [how to build a docs site](/blog/build-docs-site-command-line) that outputs both formats automatically.

### The Honest Take

llms.txt probably doesn't help your search rankings today. It probably does help AI systems read your site more accurately. Those are two different things.

If you care about AI systems understanding your content correctly (not just finding it, but getting it right), llms.txt is useful now. If you care about future-proofing for a world where AI search grows from 10% to 50% of discovery, it's a low-cost bet.

And if your build tool generates it automatically, the debate is irrelevant. You get it for free.

**Want to see what automatic llms.txt generation looks like?** [Get started with seite](/docs/getting-started) and run `seite build`. The output includes llms.txt, llms-full.txt, and per-page markdown without any configuration.

## The Bigger Picture: Triple Output

llms.txt doesn't exist in isolation. It's one piece of a larger architecture for making websites readable by AI. For a full comparison of what different static site generators include out of the box — covering canonical URLs, Open Graph tags, JSON-LD and GEO features — see [which static site generators have built-in SEO](/blog/static-site-generator-built-in-seo).

The complete pattern has three layers:

1. **HTML** for browsers and traditional search engines (Google, Bing)
2. **Markdown per page** for AI models that want to read individual pages cleanly
3. **llms.txt / llms-full.txt** as the discovery layer that tells AI which pages exist and what they contain

Most articles about llms.txt treat it as a standalone file you bolt onto an existing site. That works, but it misses the point. llms.txt is most powerful when it's part of a build pipeline that outputs all three formats from a single source.

Here's why: llms.txt links to pages. If those links point to HTML, the AI still has to parse HTML. If those links point to `.md` files that your build pipeline generates alongside every HTML page, the AI gets clean markdown from end to end. The discovery file and the content files speak the same language.

This is what [Generative Engine Optimization](/blog/generative-engine-optimization) (GEO) looks like in practice. Traditional SEO optimizes for Google's index. GEO optimizes for AI-generated answers in ChatGPT, Perplexity, Claude, and Google's AI Overviews. The triple output pattern covers both. llms.txt is one piece of a broader GEO strategy that includes markdown copies, schema markup, and AI crawler management.

Traditional static site generators like [Hugo](/blog/hugo-alternative) produce HTML only. You'd need to build the markdown output and llms.txt generation yourself, page by page. seite produces all three formats on every `seite build`:

```
dist/
 posts/
 my-post.html # HTML for browsers
 my-post.md # Markdown for AI
 index.html
 sitemap.xml # Traditional search
 feed.xml # RSS
 llms.txt # AI discovery (summary)
 llms-full.txt # AI discovery (full content)
 search-index.json # Client-side search
 robots.txt # Crawler management
```

One build command. Every format every audience needs.

## How to Implement llms.txt

llms.txt implementation ranges from manual to fully automated. Here are your three options.

### Option 1: Write It by Hand

Create a file called `llms.txt` at your site's root directory. Write it in markdown following the [specification](https://llmstxt.org/). Upload it alongside your site.

This works for small sites. The problem: it gets out of sync. Every time you add, rename, or remove a page, you need to remember to update llms.txt. For a five-page site, that's manageable. For a blog with 50 posts, you'll forget.

### Option 2: Use a Generator or Plugin

Several tools can generate llms.txt for you:

- **WordPress:** AIOSEO and other SEO plugins now include llms.txt generators
- **Documentation platforms:** [Mintlify](https://www.mintlify.com/blog/how-to-generate-llmstxt-file-automatically), [GitBook](https://www.gitbook.com/blog/what-is-llms-txt), and Fern have built-in support
- **Standalone tools:** [llmstxtgenerator.org](https://llmstxtgenerator.org/) generates files from any URL

These solve the generation problem but add a dependency. You need the plugin to keep working, and you need to remember to regenerate when content changes. If you're a developer considering [moving away from WordPress](/blog/wordpress-alternative-for-developers), a build-pipeline approach eliminates the plugin dependency entirely.

### Option 3: Build Pipeline Integration

The best approach: make llms.txt a build step that runs automatically every time you build your site.

With seite, this is the default. Every `seite build` generates `llms.txt` and `llms-full.txt` from your [content inventory](/docs/collections). No plugin to install. No [configuration](/docs/configuration) to set. No manual maintenance. When you add a new blog post or documentation page, the next build updates llms.txt automatically.

```bash
seite build
# Output includes:
# dist/llms.txt (summary with titles and links)
# dist/llms-full.txt (complete content of every page)
# dist/posts/my-post.md (per-page markdown)
```

The llms.txt file lists every page with its title and a link to the markdown version. llms-full.txt concatenates all content into a single file. Both are regenerated on every build, so they're never stale.

This is the approach that scales. You don't think about llms.txt. Your build tool handles it. See the [CLI reference](/docs/cli-reference) for the full list of build outputs.

## What Goes in Your llms.txt

Not everything on your site belongs in llms.txt. Here's what to include and what to leave out.

**Include:**
- Your site name and a clear one-paragraph summary
- Documentation pages (especially getting started guides and API references)
- Key blog posts that define your product or perspective
- Changelog or release notes (helps AI give accurate version information)
- Any page that answers a question people commonly ask AI about your product

**Exclude:**
- Admin pages, login pages, or utility pages
- Tag archive pages or pagination pages
- Duplicate content or thin pages
- Pages with `robots: noindex` in their metadata

**Writing tips:**
- Keep descriptions to one sentence per link
- Use plain language. No marketing copy. AI doesn't respond to superlatives.
- Link to markdown versions of pages (`.md`) when available, not HTML
- Update the summary whenever your product direction changes

## Frequently Asked Questions

### Who created llms.txt?

[Jeremy Howard](https://www.answer.ai/posts/2024-09-03-llmstxt.html), co-founder of fast.ai and Answer.AI, proposed the llms.txt standard in September 2024. The specification is maintained at [llmstxt.org](https://llmstxt.org/).

### Does llms.txt help SEO?

No. Google has stated that AI Overviews use traditional SEO signals, not llms.txt. John Mueller compared it to the keywords meta tag. However, llms.txt may help with AI search engines like ChatGPT, Perplexity, and Claude, which operate differently from Google.

### What is the difference between llms.txt and llms-full.txt?

llms.txt is a summary: your site name, a description, and links to important pages. llms-full.txt contains the complete content of every page in a single markdown file. Together, they give AI systems both a quick overview and deep access to your full content.

### Do ChatGPT and Perplexity use llms.txt?

Neither has confirmed using llms.txt as a ranking or retrieval signal. However, AI coding tools and documentation assistants are increasingly aware of the format, and early adopters like Cloudflare and Mintlify have implemented it for their developer docs.

### Is llms.txt a standard or a proposal?

It's a proposal that has gained significant community adoption. There's no W3C or IETF standardization process behind it. Adoption is voluntary, similar to how `robots.txt` started as an informal convention before becoming an internet standard decades later.

### How do I create an llms.txt file?

The simplest approach: create a markdown file called `llms.txt` in your site root with an H1, a blockquote summary, and links to your key pages. For a more scalable solution, use a build tool that generates it automatically. seite creates both `llms.txt` and `llms-full.txt` on every `seite build` without any configuration.

### How often should I update llms.txt?

Every time your site content changes meaningfully. If you add a new documentation page or publish an important blog post, llms.txt should reflect it. The easiest approach is to generate it automatically as part of your build process, so it's always current.

## The Standard Is Young. The Direction Is Clear.

llms.txt was proposed 18 months ago. robots.txt was proposed in 1994 and didn't become an [official internet standard until 2022](https://searchengineland.com/llms-txt-proposed-standard-453676), 28 years later. Standards take time.

The data today says llms.txt doesn't affect AI citations. The trajectory says AI search is growing, AI systems are getting better at using structured content, and the websites that provide clean, AI-readable formats will have an advantage when that shift reaches critical mass.

The cost of implementing llms.txt is near zero. The cost of not having it, if the standard gains traction, is rebuilding your content pipeline later.

Three things to remember:

1. **llms.txt is a reading guide for AI, not a ranking signal.** It helps AI systems understand your site accurately. That's valuable even if it doesn't move your search position today.
2. **The triple output pattern matters more than any single file.** HTML + markdown + llms.txt together make your site readable by every audience. llms.txt alone is a half-measure. For the full argument on why your site now needs to reach three distinct audiences, see [The Third Audience: Why Your Website Needs to Speak AI](/blog/website-ai-discoverability).
3. **The best implementation is the one you don't think about.** If your build tool generates llms.txt automatically, the debate about whether it "works" becomes irrelevant. You get it for free.

If you want a site that outputs HTML, markdown, and llms.txt from a single build command:

```bash
curl -fsSL https://seite.sh/install.sh | sh
```

Build your site. Let AI read it. [Ship it in an afternoon.](/docs/getting-started)
