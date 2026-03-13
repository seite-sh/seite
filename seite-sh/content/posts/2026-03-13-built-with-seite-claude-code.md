---
title: "We Built Our Website with Our Own Tool (and Claude Code)"
date: 2026-03-13
description: "A behind-the-scenes account of building seite.sh with seite and Claude Code: what the agent got right, what took longer, and what we'd do differently."
tags:
 - ai
 - static-site-generator
 - claude-code
 - behind-the-scenes
extra:
 primary_keyword: "build website with Claude Code"
 meta_title: "We Built Our Website with Our Own Tool and Claude Code | seite"
 meta_description: "Building seite.sh with seite and Claude Code: what the agent built, what took longer, and what we'd do differently. An honest behind-the-scenes account."
 url_slug: "built-with-seite-claude-code"
---

The first version of seite.sh was built with [Next.js](https://nextjs.org/). That lasted two weeks.

Not because Next.js is bad. It's excellent. But we were building a static site generator, and maintaining a Next.js marketing site while building a competing tool felt like exactly the kind of cognitive dissonance that kills projects. So we switched to [Hugo](https://gohugo.io/) for a few weeks. Better. Still not right.

The real problem was credibility. If we're asking developers to build websites with Claude Code using seite, we need to use the same workflow ourselves. Every version of seite.sh that wasn't built with seite was, at some level, an admission that we didn't trust our own tool enough to stake something on it.

So we cut over to seite before seite was finished. This article is the diary of that process: what we used Claude Code for, what seite handled automatically, where we hit real friction and what we'd do differently if we started over. It is not a success story with no rough edges. It's a build log.

## Why Dogfooding Was Not Optional

There's a phrase we kept coming back to during the build: "the bug pressure of using your own tool is the best QA you can run." It's true in a way that no test suite fully captures. When you're using your own CLI every day to manage a real site, you find the rough edges that tests miss. You discover that `seite new post` generates the right file but opens it in the wrong directory. You notice that the dev server's live reload fires twice on certain file saves. You care about these things because they're your own pain.

We also needed to verify something fundamental: that seite, as an [AI-native static site generator](/blog/ai-static-site-generator), could actually be used by a Claude Code agent without constant hand-holding. The only way to test that is to run the agent against a real project with real stakes.

The cutover happened on a Tuesday morning. We had a partially built seite binary, an empty `seite.toml` and two days of focused time. Eleven hours of actual work later, seite.sh was live.

## What Claude Code Actually Built

The honest breakdown: roughly 65% of the total word count on the site was agent-assisted. Every first draft came from [Claude Code](https://docs.anthropic.com/en/docs/claude-code/overview). Every page you read on seite.sh started as agent output that we then edited.

Here are the actual commands we ran during the build:

```bash
seite agent "write a getting started doc for seite, covering init, build, serve and deploy"
seite agent "create changelog entries for v0.1.0 through v0.4.0 from the git log"
seite agent "write the homepage hero copy, three variations, brand voice is pragmatic and builder-focused"
seite theme create "minimal developer tool aesthetic, dark mode toggle, monospace accents"
```

The agent handled first drafts of all 15 documentation pages. It read the project context from the auto-generated `CLAUDE.md` and the MCP server's `seite://config` resource, then produced structured docs with the right frontmatter, correct internal link targets and accurate command syntax. Not perfect, but 80% of the way there on the first pass.

The changelog entries were genuinely impressive. We ran `seite agent "create changelog entries for v0.1.0 through v0.4.0 from the git log"`, and the agent read the commit history, grouped changes by type, wrote human-readable summaries and formatted them correctly for the changelog collection. That task would have taken two hours manually. It took 12 minutes.

Homepage copy was harder. We ran three full iterations before landing on the current version. The agent's first draft was competent but generic. The second was better but overexplained. The third was close. We edited the third. If you're curious about the workflow behind that process, the [seite agent docs](/docs/agent) go into more detail on how to structure those sessions.

**What the agent got right without prompting:** Internal link structure (it knew the site map through the MCP server), frontmatter field names (it read existing files), and build command syntax (it read `CLAUDE.md`). The context infrastructure worked the way it was supposed to.

**What the agent got wrong:** It kept reaching for "simply" and "just" in explanations. We caught this on the second doc page and added a forbidden words list to `context/style-guide.md`. It also defaulted to three heading levels when two were enough, and it occasionally over-explained CLI flags to readers who clearly already know what a flag is.

## What seite Handled Without Being Asked

This section is shorter than the previous one, and that's the point.

We did not write a single `<meta>` tag. We did not configure a sitemap. We did not set up RSS. We did not create a `robots.txt` or an `llms.txt`. All of that was generated automatically on the first `seite build`.

For a site whose whole pitch is "SEO and AI discoverability, built in," the moment we realized we had not touched any of that infrastructure felt significant. The canonical URL, Open Graph tags, JSON-LD structured data, `llms.txt`, `llms-full.txt`, markdown output alongside every HTML page: all of it was just there. We checked the build output because we didn't trust it, and it was correct.

Deploy was equally hands-off. `seite deploy` commits the build output, pushes to the configured remote and triggers a [Cloudflare Pages](https://developers.cloudflare.com/pages/) build. End-to-end, from `seite build` to the live site updating, takes about 45 seconds. We ran it dozens of times during the build. It never failed.

The search index was auto-generated every build. The asset fingerprinting was configured with two lines in `seite.toml`. We did not think about any of this during the content push.

If you want to understand what the build pipeline actually does, the [getting started guide](/docs/getting-started) walks through the full flow step by step.

## Honest Friction Points

Four things were harder than we expected. All of them are worth knowing before you start.

**The bootstrapping problem.** We were writing documentation for features that were still being built. The agent would occasionally try to document a command that didn't fully exist yet, or reference a config key that had been renamed. We kept a running `DRAFTS.md` file that tracked what was real versus aspirational. This was manual overhead that added maybe 90 minutes to the total. It's unavoidable when you're building and documenting in parallel, but worth accounting for.

**Theme iteration took longer than expected.** `seite theme create` is useful, but "useful" and "done" are different. Getting from "a minimal dark developer tool aesthetic" to the actual seite.sh design required six Claude Code sessions, not the three we budgeted. Each session produces a new `base.html` that's 70% right and 30% in need of hand-editing. The hand-editing is not difficult, but it's not instant either. Budget more time than you think for this step.

**MCP context lag.** The MCP server's `seite://content` resource reflects the last build, not the current file system state. When we added new content without running `seite build` first, the agent would reference pages that didn't exist yet in the index, or miss pages that did exist but hadn't been indexed. The fix is obvious in retrospect: always run `seite build` before starting an agent session. We now do this automatically. But we wasted time on two occasions before the pattern was clear. The [MCP server docs](/docs/mcp-server) explain the resource model in detail, and this timing relationship is documented there.

**Over-reliance creep.** At one point mid-build, we had the agent write three blog posts back-to-back without reading them carefully. The structure was correct. The frontmatter was right. The voice was slightly off, more explanatory than direct, more hedged than the brand calls for. We caught it in edit, but the lesson was: agent output needs a human read before it goes anywhere near a commit. We built a two-step habit: agent writes, we read and edit, then we commit. Not complicated, but it requires discipline when you're moving fast.

One more friction point that's worth naming: we started filling in `context/brand-voice.md` and `context/style-guide.md` on week two. We should have done it on day one. The difference in agent output quality before and after those files existed was noticeable. The agent's first drafts before we had brand context were generic but competent. After we gave it explicit voice and style rules, the drafts were much closer to publishable on the first pass.

## What We'd Do Differently

Four specific changes, in order of impact:

**Fill in the context files before touching the agent.** `context/brand-voice.md`, `context/style-guide.md`, `context/target-keywords.md`: write these first, even if they're rough. The agent uses them to calibrate every output. An hour spent on context files saves three hours of editing.

**Write one complete piece manually before handing off to the agent.** Build one blog post or doc page end-to-end yourself. It establishes a reference point for structure, voice and depth. Then give that file to the agent as an example when you prompt it. The output quality goes up meaningfully.

**Use `seite serve` during every writing session.** We spent too much of the first day running `seite build` and inspecting the `dist/` directory directly. The dev server with live reload is much better for iterating on copy. You see the rendered output immediately, which catches formatting issues and template problems before they compound.

**Commit agent output immediately.** We lost two Claude Code sessions to terminal crashes with unsaved files. The fix is trivial: when the agent finishes a piece, commit it before you edit it. You get a clean diff of your edits, you keep the agent's output as a baseline, and you don't lose anything to a crash.

If you want to go deeper on how to use Claude Code for website builds, the [build website with AI coding agent guide](/blog/how-to-build-website-ai-coding-agent) covers the full workflow with more detail than we have space for here.

## The Numbers

Real numbers are more useful than vague impressions, so here's the actual timeline.

| Task | Time | Who |
|---|---|---|
| `seite init` + base config | 15 minutes | Manual |
| Theme selection + iteration | 2 hours | `seite theme create` + manual edits |
| All 15 docs pages, first draft | 3 hours | Agent |
| Docs editing and QA | 2 hours | Manual |
| Blog posts, first five | 1.5 hours | Agent |
| Blog editing | 1 hour | Manual |
| Homepage copy, three iterations | 1 hour | Agent + manual |
| Deploy configuration | 30 minutes | Manual |

**Total: approximately 11 hours across two days.**

Content volume: 15 documentation pages, 10 blog post first drafts, one homepage, one about page, one changelog. Agent-assisted words: roughly 65% of the total. Agent-assisted first drafts: 100%.

Build performance: average build time was 0.3 seconds for 40 pages. Deploy time from push to live was about 35 seconds via Cloudflare Pages.

The honest summary: two full days, one developer, one Claude Code session running alongside. Most of the time was spent editing and making decisions, not writing. The agent did the writing. The human did the thinking about whether the writing was right.

## What This Tells You

seite is good enough to run seite.sh. That's not a small claim. The site has 40+ pages, an active changelog, a full documentation section, and SEO infrastructure that we did not configure by hand. It builds in 0.3 seconds and deploys in under a minute.

The workflow is real: agent for first drafts, human for voice and judgment, seite for all the infrastructure in between. The friction points are real too, but they're solvable. Most of them come down to better upfront context setup, which costs an hour and saves many more.

If you're considering whether seite is the right tool for your project, the fastest way to answer that question is to run it. `seite init` takes 30 seconds. The [getting started guide](/docs/getting-started) will get you from blank directory to live site in an afternoon.

That's what we did. It worked.
