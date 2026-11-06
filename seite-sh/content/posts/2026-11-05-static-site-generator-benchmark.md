---
title: "Static Site Generator Benchmark: Closing a 2.2x Gap"
date: 2026-11-05
description: "Our static site generator benchmark found Zola 2.2x faster at 10,000 pages. A profile pointed at file I/O, and seite 0.21.2 now builds them in 712 ms."
tags:
  - performance
  - rust
  - static-site-generator
extra:
  primary_keyword: "static site generator benchmark"
---

We re-ran our static site generator benchmark this week, on seite 0.21.1 against Hugo, Zola, Eleventy, Jekyll and Astro. The result wasn't the one we wanted. At 10,000 pages, Zola built the site in 833 ms, while seite took 1.84 s, 2.2x as long.

Both are single Rust binaries. Both parse markdown and render through a template engine. A gap that size meant seite was doing something Zola wasn't, so we profiled it. About two thirds of the CPU time went to file-system calls, not to markdown or templates.

This post covers what the profile showed, the three changes in [seite 0.21.2](/changelog/v0-21-2) that came out of it, the new numbers for all six generators and the caveats you should apply before quoting any of them.

## Static Site Generator Benchmark Setup

Every generator built the same generated content: posts with YAML frontmatter (TOML for Zola), one to three tags out of 20, three to five Lorem Ipsum paragraphs and one fenced Rust code block. Each one used a minimal template: an `<h1>` plus the rendered content for a post, and an `<h1>` plus a list of links for the index.

| | |
|:--|:--|
| Machine | Apple M4 Pro (14 cores), 48 GB RAM, macOS 27.2 |
| Timing | [hyperfine](https://github.com/sharkdp/hyperfine) 2.0.0, one warmup, five runs, medians |
| Cold builds | output deleted in hyperfine's `--prepare` step, which isn't timed |
| [Hugo](https://gohugo.io/) | 0.169.0+extended |
| [Zola](https://www.getzola.org/) | 0.23.6 |
| Eleventy | 3.1.8 (Node.js 20.18.2) |
| Jekyll | 4.4.1 (Ruby 3.4.1) |
| Astro | 5.18.2 (Node.js 20.18.2) |

One fix to our own harness came first. The previous version of the script deleted the output directory inside the timed command. At 10,000 pages that deletion alone took 1.0 s for seite and 1.5 s for Zola, about as long as the builds themselves. The numbers in this post time the build only.

The minimal templates equalize the markup, not the work: seite still syntax-highlights code blocks, [checks every internal link](/blog/validate-static-site-before-deploy) and writes a `.md` copy of every page, plus [`llms.txt`](/blog/what-is-llms-txt), `llms-full.txt`, a search index, RSS and Atom feeds, a sitemap and tag pages. The Zola setup writes a sitemap, `robots.txt` and `404.html`, with no taxonomies and highlighting off. That extra work costs seite time, so the comparison doesn't favor it.

## Where seite 0.21.1 Spent Its Time

We profiled the 10,000-page build with [samply](https://github.com/mstange/samply) on a release build with symbols. Markdown, Tera rendering and related-post scoring were small by comparison, and syntax highlighting took 7.5%. File-system calls dominated, with memory allocation next:

| Call | Share of CPU time |
|:--|--:|
| `open` | 39% |
| `read` | 10% |
| `close` | 10% |
| `write` | 8% |
| `malloc` (allocation, not file I/O) | 9% |

Counting the operations explained it: seite 0.21.1 opened about 50,000 files for 10,000 pages, around two and a half times Zola's file operations. For every page it read the source, wrote the HTML, read the HTML back, rewrote it and wrote the `.md` copy. Zola reads each source once and writes each page once.

The read-back existed for a reason. After rendering, seite post-processes every page: it adds code-copy buttons, analytics and image `srcset` attributes, and it checks the page's links. That step worked on files on disk, so each page went to disk, came back and went out again.

On top of that, the writes were serial. Rendering ran in parallel across all cores, but the pages and their `.md` copies were written one at a time on one thread.

### File Creation on macOS Is Kernel-Bound

The obvious fix for serial writes is more threads. We measured that first. On this Mac, creating a file costs about 60 µs. Creating 20,000 files took 1.2 s on one thread and 0.8 s on 14 threads, whether the files shared one directory or each had its own.

Fourteen times the threads bought a third off the time. File creation goes through the kernel, and with many threads they spend their time contending there. In seite's own builds, 8 to 14 writer threads were slower than four.

## What Changed in seite 0.21.2

Three changes, all in the build pipeline. The generated site is byte-for-byte the same as 0.21.1's, and upgrading needs only `seite self-update`.

1. **Pages are written once.** Rendered pages now stay in memory until post-processing, which adds the code-copy buttons, analytics and `srcset` attributes, checks links and writes each page to disk a single time. No read-back, no rewrite.
2. **Generated files are written in parallel.** The `.md` copies are written in the background while the homepage, listings, feeds and sitemap render. Tag and taxonomy pages now render in parallel too.
3. **Four writer threads on macOS.** Bulk writes go through a dedicated pool of four threads on macOS, where four threads beat 8 to 14 in our builds. Other platforms keep using the main thread pool, as before; we haven't measured them.

Here's how the build stages moved at 10,000 pages, from `seite build -v` (mean of three runs):

| Stage | 0.21.1 | 0.21.2 |
|:--|--:|--:|
| Render pages (now render only, no writes) | 489 ms | 29 ms |
| Output `.md` copies (now in the background; this is the remaining wait) | 485 ms | 89 ms |
| Post-process HTML (now also the only page write) | 507 ms | 239 ms |
| Process collections (read, parse, markdown, highlighting) | 158 ms | 161 ms |
| Render indexes and tags (tags now in parallel) | 97 ms | 70 ms |

Reading and parsing content didn't change, at about 160 ms. Everything that wrote files did.

## Static Site Generator Benchmark Results

Median SSG build time for a cold build at four site sizes, minimal templates, five runs per tier:

| Pages | seite 0.21.2 | Zola | Hugo | seite 0.21.1 | Jekyll | Eleventy | Astro |
|--:|--:|--:|--:|--:|--:|--:|--:|
| 100 | 23 ms | **22 ms** | 42 ms | 34 ms | 219 ms | 198 ms | 833 ms |
| 1,000 | **83 ms** | 94 ms | 148 ms | 178 ms | 484 ms | 462 ms | 1.29 s |
| 5,000 | **344 ms** | 412 ms | 640 ms | 846 ms | 1.56 s | 1.77 s | 3.10 s |
| 10,000 | **712 ms** | 833 ms | 1.49 s | 1.84 s | 2.93 s | 4.21 s | 5.32 s |

What the table says, and what it doesn't:

- **At 5,000 and 10,000 pages, seite 0.21.2 was the fastest.** Zola took 1.20x and 1.17x seite's median time. The runs didn't overlap: at 10,000 pages seite's slowest run (719 ms) beat Zola's fastest (828 ms).
- **At 1,000 pages, seite led by 14%** (83 ms against 94 ms), but that lead varies between runs: in an earlier quiet run the same evening it was 4%, inside the noise.
- **At 100 pages, it's even**: Zola 22 ms, seite 23 ms. Fixed startup costs dominate a site that small.
- **Against its previous release, 0.21.2 is 1.5x to 2.6x faster**: 34 ms down to 23 ms at 100 pages, and 1.84 s down to 712 ms at 10,000.
- **Hugo took 1.8x to 2.1x seite 0.21.2's time** across the four sizes.
- **Zola vs Hugo:** Zola was faster than Hugo at every size, 833 ms against 1.49 s at 10,000 pages.
- **Jekyll, Eleventy and Astro trail at every size.** Astro's fixed startup of about 0.8 s dominates small sites.

Every generator built every tier. The Zola segfault at around 8,000 pages that we hit with Zola 0.20.0 in our earlier run didn't happen with 0.23.6.

### With the Default Theme

Minimal templates flatter every generator. With seite's bundled default theme instead (33 KB post pages with CSS, SEO head, JSON-LD and the search script; 425 MB of output at 10,000 pages), the 10,000-page build went from 2.33 s on 0.21.1 to 963 ms on 0.21.2. That measurement used 10 runs instead of five, on the same machine right after the main run.

### The Cost: Memory

Keeping pages in memory until they're written isn't free. Peak memory rose by up to 14% on the minimal templates (1,000 pages: 51 MiB to 58 MiB; about the same at 10,000 pages, 303 MiB and 301 MiB) and by 13% with the default theme (592 MiB to 667 MiB).

In context: seite's peak memory is still less than half of Hugo's at every size, and up to a quarter above Zola's. At 10,000 pages that's 301 MiB for seite, 248 MiB for Zola and 692 MiB for Hugo.

## Caveats Before You Quote These Numbers

A benchmark from the people who make one of the tools deserves suspicion. Here's what to keep in mind:

- **One machine.** Everything ran on one Apple M4 Pro desktop, not an isolated runner. CPU frequency wasn't pinned and other apps were open.
- **Noise.** We waited for the machine to go idle and ran everything back to back in one 15-minute window, but treat any difference under about 10% as noise.
- **macOS only.** The writer-thread change applies to macOS. On Linux and Windows the write path uses the main pool and we haven't benchmarked it, so the gap to Zola there may be larger or smaller.
- **Not equal work.** In these runs seite writes a `.md` copy of every page, highlights code and checks every internal link; the Zola setup doesn't. That cuts against seite, but it also means the tools aren't doing the same job.
- **Synthetic content.** Generated Lorem Ipsum posts with one code block each aren't your site. Shortcodes, images and heavy templates change the picture.
- **Old numbers don't compare.** Our earlier results for seite 0.3.2 came from a different machine, different versions and timed the output deletion. Nothing in this post can be compared with them.

The method, scripts and full results tables are in the [benchmark pull request on GitHub](https://github.com/seite-sh/seite/pull/35). To rerun it on your own machine:

```bash
cargo build --release
brew install hyperfine hugo zola        # plus Node.js and Ruby + Bundler
./bench/run-comparison.sh --tiers "100 1000 5000 10000" --runs 5 --warmup 1
```

Set `SEITE_BIN` to benchmark a different seite binary, `SEITE_BASELINE_BIN` to add a second one to the same run and `--skip <ssg>` to drop a generator. If you run your own static site generator benchmark, treat anything under 10% as noise.

## Is seite the Fastest Static Site Generator?

In this static site generator benchmark, on this machine, at 5,000 pages and up: yes, Zola took 17% to 20% longer, clearly above the noise. At 1,000 pages seite led too, but by a margin that moved between runs, and at 100 pages the two are even. For most blogs and docs sites, a thousand pages or fewer, every Rust and Go generator here finishes in about 150 ms or less, and the difference won't change your workflow.

The more useful lesson is where the time was. The profile pointed at work around the build, not the build itself: writing a file, reading it back and writing it again. If your own build is slow, profile it before you optimize the parser.

To get the faster builds, run `seite self-update`, or start a new site with the [getting started guide](/docs/getting-started). If you're comparing generators for reasons beyond speed, the [Hugo alternative](/blog/hugo-alternative) and [Astro alternative](/blog/astro-alternative-for-static-sites) posts cover the rest of the tradeoffs.

## FAQ

### How much faster is seite 0.21.2 than 0.21.1?

Between 1.5x and 2.6x with minimal templates, depending on site size. A 10,000-page build went from 1.84 s to 712 ms, and a 1,000-page build from 178 ms to 83 ms. With the default theme, the 10,000-page build went from 2.33 s to 963 ms.

### Do I need to change my site to get the speedup?

No. Run `seite self-update`. There's no `seite upgrade` step, and the generated output is byte-for-byte the same as 0.21.1's. See the [v0.21.2 changelog](/changelog/v0-21-2).

### Why does seite use four writer threads on macOS?

Creating a file on macOS is bound by the kernel. More threads don't help much: 20,000 file creates took 1.2 s on one thread and 0.8 s on 14. In our builds, 8 to 14 writer threads were slower than four, because they contend in the kernel.

### Is Zola slower than seite?

Not across the board. On this benchmark the two were even at 100 pages, Zola took 14% longer at 1,000 (a margin that moved between runs) and 17% to 20% longer at 5,000 and 10,000, while doing less work per page than seite. Both are fast enough that build time is rarely the deciding factor.

### Does the speedup apply on Linux?

The two main changes, writing each page once and writing generated files in parallel, are platform-independent. The four-thread writer pool is macOS-only. We haven't benchmarked Linux or Windows for this release, so we don't have numbers for them.
