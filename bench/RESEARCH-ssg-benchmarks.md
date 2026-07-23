# SSG Benchmark Research: Building a Credible, Externally-Accepted Benchmark

> Research conducted February 2026. Covers existing benchmark suites, standard
> content/image sets, measurement methodology, fairness practices, and
> recommendations for benchmarking seite against the top 5 SSGs.

---

## Table of Contents

1. [Existing SSG Benchmark Projects](#1-existing-ssg-benchmark-projects)
2. [Standard Content Sets](#2-standard-content-sets)
3. [What to Measure](#3-what-to-measure)
4. [Measurement Tools & Statistical Rigor](#4-measurement-tools--statistical-rigor)
5. [Fairness Methodology](#5-fairness-methodology)
6. [Image Processing Benchmarks](#6-image-processing-benchmarks)
7. [Top 5 SSGs to Benchmark Against](#7-top-5-ssgs-to-benchmark-against)
8. [Presenting Results Credibly](#8-presenting-results-credibly)
9. [Gaps in Existing Benchmarks](#9-gaps-in-existing-benchmarks)
10. [Concrete Recommendation for seite](#10-concrete-recommendation-for-seite)

---

## 1. Existing SSG Benchmark Projects

### Sean C. Davis / CSS-Tricks (Retired)

**Repo:** [seancdavis/ssg-build-performance-tests](https://github.com/seancdavis/ssg-build-performance-tests)
**Published:** [CSS-Tricks — Comparing Static Site Generator Build Times](https://css-tricks.com/comparing-static-site-generator-build-times/)
**Status:** No longer maintained.

The most cited SSG benchmark. Methodology:
- **SSGs tested:** Hugo, Eleventy, Gatsby, Next.js, Jekyll, Nuxt
- **Content:** Generated markdown files with randomly-generated title (frontmatter) and body (three paragraphs). No images.
- **Page counts:** Doubling tiers from 1 up to 64,000 (1, 2, 4, 8, ..., 1024, then 1K, 2K, 4K, ..., 64K)
- **Templates:** Default starter from each SSG's getting-started guide
- **Measurement:** Cold builds only. Caches cleared and markdown files regenerated for every test.
- **Presentation:** Published at ssg-build-performance-tests.netlify.app with results website

The author retired it because "it's highly unlikely that you'd choose static generation if you needed to frequently build 64,000 pages" and hybrid rendering patterns have evolved. Despite being retired, this remains the reference that people cite.

**Key finding:** Hugo was fastest regardless of size, and "it wasn't even close."

### Zach Leatherman / bench-framework-markdown

**Repo:** [zachleat/bench-framework-markdown](https://github.com/zachleat/bench-framework-markdown)
**Blog:** [Which Generator builds Markdown the fastest?](https://www.zachleat.com/web/build-benchmark/)

The most methodologically careful benchmark still referenced:
- **SSGs tested:** Astro (with MDX), Eleventy, Gatsby, Hugo, Next.js (file routing)
- **Page counts:** 250, 500, 1,000, 2,000, 4,000
- **Structure:** Per-SSG benchmark shell scripts (`bench-astro.sh`, `bench-eleventy.sh`, etc.) + shared `install.sh` + `_markdown-samples/` directory with test content
- **Measurement:** 3 runs per configuration, lowest/fastest time selected
- **Hardware:** MacBook Air M1 (2020), macOS Monterey 12.5, 16GB RAM
- **Goal:** "For each generator sample, attempted to create a reduced project with the sole use case of processing markdown files"

**Key finding:** Hugo fastest overall. Eleventy fastest JS-based. Astro on-par with Next.js at 1K pages, on-par with Gatsby at 4K.

**Credibility note:** Created by the Eleventy author. The community generally trusts it but notes the potential bias. Notably, Zola was NOT included.

### Lumeland Benchmark

**Repo:** [lumeland/benchmark](https://github.com/lumeland/benchmark)

The best-structured actively maintained benchmark:
- **SSGs tested:** Hugo, Lume (Deno), Jekyll, Eleventy
- **Page counts:** 100 (small), 1,000 (medium), 10,000 (large) — configurable via `--pages` flag
- **Content generation:** Deno script (`cli.js --generate`) creates test content
- **Iterations:** 10 runs per benchmark (configurable via `--runs`)
- **Configuration:** `config.js` for adding/removing generators

**Results (10,000 pages, 10 runs):**
| Rank | SSG | Seconds |
|------|-----|---------|
| 1 | Hugo | 7.636 |
| 2 | Jekyll | 15.874 |
| 3 | Lume | 18.208 |
| 4 | Eleventy | 23.482 |

**Key finding:** Hugo ~3x faster than nearest competitor at 10K pages.

### grego/ssg-bench

**Repo:** [grego/ssg-bench](https://github.com/grego/ssg-bench)

Small-scale but notably honest:
- **SSGs tested:** Blades, Zola, Hugo
- **Content:** Same theme (based on BOOTSTRA.386) tailored for each, "pages that have the same content (as far as possible)"
- **Page count:** Very small (~5-9 pages)
- **Measurement:** Internal timing parsed from each program's output. 100 iterations default.
- **Tools required:** ripgrep, bc

**Results (AMD Ryzen 9 5900X):**
- Blades: 0.92ms
- Zola: 19.79ms (±0.48)
- Hugo: 23.69ms (±1.21)

**Critical honesty quote:** "Benchmarking static site generators can hardly be made accurate. They come with different feature sets and scopes. Results should still be taken with a grain of salt."

### SSGBerk (Static Site Generator Benchmark)

**Org:** [github.com/ssgberk](https://github.com/ssgberk)
- Forked from TechEmpower/FrameworkBenchmarks approach
- Docker-based execution using custom containers
- Most ambitious in breadth (SSGs across Go, Python, Java, Ruby, PHP, JS)
- Lower profile / less cited

### Elmar Klausmeier: Saaze vs Hugo vs Zola

**Blog:** [Performance Comparison Saaze vs Hugo vs Zola](https://eklausmeier.goip.de/blog/2021/11-13-performance-comparison-saaze-vs-hugo-vs-zola)
- Independent blog-based comparison
- PHP (Saaze) vs Go (Hugo) vs Rust (Zola)
- Cited by the "Zola is 4x faster than Hugo" claim at [tqdev.com](https://www.tqdev.com/2023-zola-ssg-is-4x-faster-than-hugo/)

### Eleventy's Own Benchmark (Self-Comparison Only)

**Repo:** [11ty/eleventy-benchmark](https://github.com/11ty/eleventy-benchmark)
- Not a cross-SSG benchmark — used for regression detection
- 1,000 templates per format (Liquid, Nunjucks, Markdown)
- Reports median runtime over 10 runs and median time-per-template
- Output ~10KB per template

---

## 2. Standard Content Sets

**There is no universally accepted standard corpus for SSG benchmarks.** Every project generates its own. Here are the patterns:

### Page Count Tiers (Consensus Across Projects)

| Tier | Pages | Classification | Who Uses It |
|------|-------|---------------|-------------|
| Tiny | 10 | Personal blog, startup time dominates | errata-ai/static-school |
| Small | 100 | Portfolio / small blog | lumeland, static-school |
| Medium | 1,000 | Active blog / docs site | **Everyone** — the de facto standard tier |
| Large | 10,000 | Large documentation / enterprise blog | lumeland, CSS-Tricks |
| Very Large | 50,000-64,000 | Enterprise-scale stress test | CSS-Tricks |
| Extreme | 100,000+ | Hugo "million pages" territory | Hugo showcase (V&A Museum) |

The three tiers used most consistently: **100, 1,000, 10,000.**

### Content Per Page (Common Pattern)

From the CSS-Tricks benchmark (the most cited methodology):
- YAML frontmatter: `title`, `date`, `description`, `tags`
- Body: 3 paragraphs of randomly generated text (~300-500 words)
- No images (images tested separately)
- Content generated by script, regenerated fresh for each test run

### Content Generation Methods

| Project | Generator | Language |
|---------|-----------|----------|
| CSS-Tricks | Ruby script | Ruby |
| Lumeland | `cli.js --generate` | Deno/JS |
| zachleat | Shell scripts per SSG | Bash |
| ssg-bench | Manual content files (tiny scale) | Static |

### What Frontmatter Fields to Include

Minimum for credibility:
```yaml
---
title: "Generated Post Title 0042"
date: 2025-06-15
description: "A short description for the generated post."
tags: [benchmark, generated]
---
```

For a more realistic benchmark, add:
```yaml
---
title: "Understanding Async Patterns in Modern Rust"
date: 2025-06-15
updated: 2025-07-01
description: "A deep dive into async/await patterns, Pin, and executor design."
tags: [rust, async, programming]
image: /static/images/async-rust.jpg
author: "Benchmark Author"
---
```

---

## 3. What to Measure

### Commonly Measured (Every Benchmark Does This)

| Metric | Description |
|--------|-------------|
| **Cold build time** | Time to generate all output from scratch, no cache. Primary metric. |
| **Build time scaling** | How build time grows with page count (linear? superlinear?) |

### Sometimes Measured

| Metric | Description | Who Does It |
|--------|-------------|-------------|
| Memory usage | Peak RSS during build | grego/ssg-bench (partially) |
| Per-template time | Time per page rendered | Eleventy's self-benchmark |

### Rarely/Never Measured (Gaps = Opportunity)

| Metric | Description | Why It Matters |
|--------|-------------|---------------|
| **Incremental rebuild time** | Time after changing one file | Critical for DX |
| **Dev server startup** | Time from command to first served page | Developer experience |
| **Hot reload speed** | Time from file save to browser update | Developer experience |
| **Memory at scale** | Peak/resident memory at 10K+ pages | Constrains CI/deploy environments |
| **Output size** | Total bytes of generated site | Bandwidth / hosting cost |
| **Lighthouse scores** | Performance, accessibility, SEO, best practices of output HTML | Output quality |
| **Image processing time** | Time for resize + format conversion | Real-world build bottleneck |

---

## 4. Measurement Tools & Statistical Rigor

### Hyperfine (Gold Standard)

**Repo:** [sharkdp/hyperfine](https://github.com/sharkdp/hyperfine) — Rust CLI benchmarking tool.

Why it's the standard:
- Auto-calibrates shell startup overhead
- Reports mean, median, stddev, min, max
- Outlier detection via IQR method
- Warmup runs to prime filesystem/CPU caches
- Exports JSON, CSV, Markdown
- Parameterized benchmarks

**Key flags for SSG benchmarks:**

```bash
hyperfine \
  --warmup 3 \                    # 3 warmup runs (fills disk cache, CPU icache)
  --runs 30 \                     # 30 measurement runs (minimum for CLT confidence)
  --prepare 'rm -rf dist/' \      # Clean output between runs
  --export-json results.json \    # Machine-readable results
  --export-markdown results.md \  # Table for README
  'seite build' \
  'hugo' \
  'zola build' \
  'npx @11ty/eleventy' \
  'bundle exec jekyll build'
```

### How Many Iterations?

| Count | Use Case |
|-------|----------|
| 10 | Minimum (hyperfine default). Floor for any benchmark. |
| **30** | **Recommended.** Central Limit Theorem threshold for reliable CIs. |
| 50-100 | Publication-quality results with tight confidence intervals. |
| 100+ | Diminishing returns unless detecting sub-millisecond diffs. |

### Warmup Strategy

| SSG Type | Warmup Runs | Reason |
|----------|-------------|--------|
| Compiled binaries (Hugo, Zola, seite) | 2-3 | Only disk cache warming matters |
| Node.js (Eleventy, Astro) | 3-5 | V8 JIT benefits from warm-up |
| Ruby (Jekyll) | 3-5 | Ruby interpreter + gem loading |

### Cold vs Warm Cache

Run both:
- **Warm cache** (default hyperfine behavior with `--warmup`): Measures the generator itself, not the OS
- **Cold cache**: `--prepare 'sync; echo 3 | sudo tee /proc/sys/vm/drop_caches'` — measures real first-run experience

### System Controls

| Control | Why | Command |
|---------|-----|---------|
| CPU governor → "performance" | Prevent frequency scaling | `echo performance \| sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor` |
| Disable Turbo Boost | Eliminate non-deterministic frequency spikes | Intel: `echo 1 > /sys/devices/system/cpu/intel_pstate/no_turbo` |
| CPU pinning | Prevent scheduler migration | `taskset -c 2 hyperfine ...` |
| Kill background processes | Eliminate CPU/IO contention | Close browsers, IDEs, virus scanners |
| Use NVMe/SSD | HDD seeks dominate IO-heavy builds | Hardware choice |
| Disable virus scanner | Hugo docs note 400%+ slowdown from Defender | Platform-specific |

### What Statistic to Report

- **Median** (primary) — robust to outliers. A single GC pause doubles the mean but barely touches the median.
- **Mean** (secondary) — include alongside median; divergence between mean and median indicates high variance.
- **95% confidence interval** — essential for credibility.
- **Min** — some practitioners argue the minimum is the truest measure of code performance (all slower runs = system interference).

---

## 5. Fairness Methodology

This is the most debated aspect of SSG benchmarks.

### Template Strategy: Do Both

**Controlled benchmark (apples-to-apples):**
- Minimal markdown-to-HTML rendering
- Functionally equivalent templates producing the same HTML structure
- No syntax highlighting, no image processing, no SASS
- Each SSG uses its own templating language but the output HTML should be byte-similar

**Real-world benchmark (each SSG's strengths):**
- Each SSG's recommended starter/default theme
- All default features enabled
- Clearly label this as "out-of-the-box experience"

### Configuration: Default vs Optimized

| Approach | What to Do | Why |
|----------|-----------|-----|
| **Default (primary)** | Use each SSG's getting-started guide defaults | Reflects what a new user encounters |
| **Optimized (secondary)** | Enable known perf flags (e.g., Hugo `--cache`, parallel builds) | Shows peak potential |

Always document which config was used.

### Feature Parity Problem

SSGs have fundamentally different built-in features:
- Hugo: syntax highlighting, image processing, SASS — built in by default
- Zola: syntax highlighting, SASS — built in
- Eleventy: minimal by design — add plugins for everything
- Astro: JS bundling + island architecture
- Jekyll: plugins for almost everything

**Recommended approach:**
1. **Core build profile:** Markdown → HTML only. Disable syntax highlighting, image processing, SASS. This is the apples-to-apples comparison.
2. **Full build profile:** Enable all features each SSG provides by default. Acknowledge the feature asymmetry.
3. **Feature-specific profiles:** Test image processing, syntax highlighting, etc. as separate benchmarks. Only compare SSGs that actually have the feature.

**Never silently enable features on some SSGs and disable on others.**

### What Makes a Benchmark Trusted

Based on community discussions (Hacker News, Reddit r/jamstack):

**Trust signals:**
- Open-source repo with all scripts and content generators
- Honest limitations disclosure (ssg-bench's grain-of-salt caveat is exemplary)
- Multiple page-count tiers showing scaling behavior (not just one data point)
- Statistical rigor (multiple runs, warmup, outlier handling)
- Not run by a vendor of any competing SSG
- Pinned exact SSG versions
- Reproducible on reader's hardware

**Distrust signals:**
- Only showing results where the author's tool wins
- Cherry-picked page counts that favor one SSG
- Single-run measurements with no error bars
- Missing hardware/environment details
- "Benchmark" that is really marketing content
- Created by the author of one of the competing SSGs (perceived, not always actual, bias)

---

## 6. Image Processing Benchmarks

### No SSG Benchmark Currently Tests Image Processing

This is a significant gap. The grego/ssg-bench accidentally tests image galleries (Blades), but the other benchmarks avoid images entirely because not all SSGs have built-in image processing.

### Standard Image Test Sets

From the imaging/codec benchmark community:

| Dataset | Images | Resolution | License | Best For |
|---------|--------|-----------|---------|----------|
| **Kodak Lossless True Color** | 24 | 768 × 512 | Unrestricted (Eastman Kodak) | Gold standard for codec/processing benchmarks |
| CLIC Professional 2020 | 41 | ~2048 × 1365 avg | Research use | Professional photography scenarios |
| Tecnick / TESTIMAGES | 100 | Up to 1200 × 1200 | Research use | Diverse content types |
| USC-SIPI | ~100 | 512 × 512 | Academic use | Classic academic benchmarks |

**Recommendation: Use the Kodak 24-image set.**
- Universally recognized, freely usable
- 24 images is manageable
- Mix of portrait/landscape, indoor/outdoor, text/natural scenes
- Supplement with 5-10 high-resolution images (3000×2000+) to test resize pipeline

### What to Measure in Image Benchmarks

Test separately from the main markdown build:
1. Resize time (per-image and total)
2. WebP conversion time
3. AVIF conversion time
4. Full pipeline time (resize all widths + all format conversions)
5. Memory usage during image processing

Only compare SSGs that have built-in image processing (Hugo, seite, Zola partially). Make this a separate section from the core build benchmark.

---

## 7. Top 5 SSGs to Benchmark Against

Based on GitHub stars, npm downloads, community adoption, and architectural relevance to seite:

| SSG | Language | GitHub Stars | npm Downloads/week | Why Include |
|-----|----------|-------------|-------------------|-------------|
| **Hugo** | Go | ~87K | N/A (Go binary) | The speed reference. Every SSG benchmark includes Hugo. Same compiled single-binary model as seite. |
| **Zola** | Rust | ~14K | N/A (Rust binary) | Most direct competitor — Rust, Tera templates, single binary, similar feature set. Apples-to-apples. |
| **Astro** | JS/TS | ~57K | ~800K/week | Fastest-growing SSG. Represents the modern JS ecosystem mainstream. |
| **Eleventy** | JS | ~17K | ~200K/week | The pragmatic JS developer's SSG. Zero-JS output philosophy. Common benchmark baseline. |
| **Jekyll** | Ruby | ~49K | N/A (gem) | The original SSG. Powers GitHub Pages. Important as the "legacy baseline" that many migrate from. |

### Why These Five

1. **Hugo** — Obligatory. If you claim speed, you must show numbers against Hugo.
2. **Zola** — Closest architectural peer (Rust, Tera, single binary). A benchmark without Zola lacks credibility with the Rust community.
3. **Astro** — Most actively adopted new SSG (~800K npm weekly downloads, joined Cloudflare). Shows seite competing in the broader market.
4. **Eleventy** — Best of the Node.js ecosystem without React/framework overhead. The "honest comparison" choice for JS-land.
5. **Jekyll** — Still widely used (GitHub Pages). Gives a migration story data point: "switching from Jekyll to seite is Nx faster."

### Considered but Excluded

- **Next.js** (~130K stars) — Full-stack framework, not a pure SSG. Including it invites "apples to oranges" criticism.
- **Gatsby** (~56K stars) — Declining. Corporate upheaval. Less relevant in 2025-2026.
- **Hexo** (~39K stars) — Popular in Asia, niche in English-speaking world.
- **Lume** (~2K stars) — Too niche, though its benchmark repo is well-structured.
- **MkDocs** — Documentation-specific, not a general SSG.

---

## 8. Presenting Results Credibly

### Report Both Absolute and Relative

1. **Absolute wall-clock time** (median, with 95% CI) — what users actually experience
2. **Relative speedup** (e.g., "2.3× faster than Hugo") — what people remember and share
3. When aggregating across tiers, use **geometric mean** of ratios (per SPEC methodology), never arithmetic mean

### Visualization Formats

| Format | When to Use |
|--------|------------|
| **Grouped bar charts with error bars** (95% CI whiskers) | Primary comparison at each tier |
| **Line charts** (page count × build time) | Show scaling behavior |
| **Tables with exact numbers** | Alongside charts for precision |
| **Box plots / violin plots** | Show full distribution |

**Avoid:** 3D charts, truncated y-axes, showing only relative without absolute.

### Repository & Reproducibility Requirements

**Must-haves:**
- Open-source benchmark repository with all scripts, content generators, templates
- Pinned exact SSG versions (not "latest")
- Dockerfile or Nix flake for reproducible environment
- Raw data published (hyperfine JSON export)
- `README.md` with step-by-step reproduction instructions

**Should-haves:**
- CI pipeline that runs benchmarks automatically (acknowledge CI hardware is noisy)
- Hardware specification document
- Date of last run
- Comparison against previous benchmark runs (track SSG improvements over time)

### Environment Documentation Template

```
## Environment
- CPU: AMD Ryzen 9 5900X (12C/24T) @ 3.7GHz (turbo disabled)
- RAM: 32GB DDR4-3200
- Storage: Samsung 970 EVO Plus NVMe SSD
- OS: Ubuntu 22.04 LTS, kernel 5.15.x
- CPU Governor: performance (fixed frequency)
- SSG Versions:
  - seite 0.2.x
  - Hugo 0.145.0
  - Zola 0.20.0
  - Eleventy 3.1.0
  - Astro 5.x.x
  - Jekyll 4.4.x
- Hyperfine: 1.19.0
- Runs: 30 per benchmark
- Warmup: 3 runs
- Date: 2026-MM-DD
```

---

## 9. Gaps in Existing Benchmarks

No existing benchmark comprehensively covers:

1. **Incremental/watch-mode rebuild** — critical for DX, never measured cross-SSG
2. **Dev server startup time** — never measured
3. **Memory usage at scale** — only partially addressed
4. **Image processing performance** — no benchmark uses standard image sets
5. **Output quality** (Lighthouse, HTML validity) — never measured
6. **Zola + Node SSGs in one benchmark** — ssg-bench has Zola but no JS SSGs; zachleat has JS SSGs but no Zola
7. **Large-scale (50K+)** — only the retired CSS-Tricks benchmark went this high

**These gaps are seite's opportunity.** A benchmark that fills even 2-3 of these gaps would be novel and noteworthy.

---

## 10. Concrete Recommendation for seite

### Benchmark Suite Structure

```
bench/
  README.md                      # Methodology, environment, how to reproduce
  Dockerfile                     # Reproducible environment
  generate-content.sh            # Content generation script
  run-benchmarks.sh              # Main benchmark runner (wraps hyperfine)
  results/                       # Raw JSON + processed charts
  content/                       # Generated test content (gitignored, regenerated)
  templates/                     # Per-SSG minimal templates
    seite/
    hugo/
    zola/
    astro/
    eleventy/
    jekyll/
  images/                        # Kodak test images (for image benchmark)
  analysis/                      # Scripts to process results into charts
```

### Content Generation

Script that generates N markdown files:
```
---
title: "Post {i}: {randomly generated realistic title}"
date: {evenly distributed dates across 2 years}
description: "{1-2 sentence description}"
tags: [{2-3 tags from a pool of 20}]
---

{3-5 paragraphs of ~400 words total}

## {Subheading}

{1-2 more paragraphs}

- {Bullet list with 3-5 items}

{Optional: fenced code block with ~10 lines}
```

Content should be:
- **Reproducible** — use a fixed seed for the random generator
- **Realistic** — not lorem ipsum. Use a Markov chain or LLM-generated text corpus that reads like real blog posts. This matters because markdown parsers may perform differently on realistic vs synthetic text.
- **Varied** — different post lengths, with/without code blocks, with/without lists. Real blogs have variance.
- **Regenerated for each run** — prevents filesystem cache effects from favoring whoever ran last

### Benchmark Tiers

| Tier | Pages | Label |
|------|-------|-------|
| Small | 100 | Personal blog |
| Medium | 1,000 | Active blog |
| Large | 5,000 | Documentation site |
| Very Large | 10,000 | Enterprise content |

The 4-tier structure (100, 1K, 5K, 10K) balances coverage with reasonable run time.

### Measurement Protocol

```bash
# Core build benchmark (per SSG, per tier)
hyperfine \
  --warmup 3 \
  --runs 30 \
  --prepare 'rm -rf dist/' \
  --export-json "results/${ssg}-${tier}.json" \
  --export-markdown "results/${ssg}-${tier}.md" \
  "${build_command}"

# Comparative benchmark (all SSGs at one tier)
hyperfine \
  --warmup 3 \
  --runs 30 \
  --prepare 'rm -rf output/' \
  --export-json "results/comparison-${tier}.json" \
  --export-markdown "results/comparison-${tier}.md" \
  'seite build' \
  'hugo --quiet' \
  'zola build' \
  'npx @11ty/eleventy --quiet' \
  'bundle exec jekyll build --quiet'
```

### Two Benchmark Modes

1. **Controlled (apples-to-apples):**
   - Minimal template per SSG (just renders markdown body into `<html><body>{{ content }}</body></html>`)
   - No syntax highlighting, no image processing, no RSS, no sitemap
   - Tests raw markdown → HTML throughput

2. **Real-world (out-of-the-box):**
   - Each SSG's recommended starter with typical features enabled
   - RSS feed, sitemap, tag pages, collection indexes
   - Shows what a real user gets when they follow the getting-started guide

### Additional Benchmarks (Novel, Filling Gaps)

3. **Incremental rebuild:**
   - Build site, modify one file, measure rebuild time
   - Use `hyperfine --prepare 'touch content/posts/post-0500.md'`

4. **Memory usage:**
   - Use `/usr/bin/time -v` to capture peak RSS
   - Report at each tier

5. **Image processing (separate):**
   - Kodak 24-image set + 10 high-res images (3000×2000)
   - Only seite, Hugo, and Zola (others lack built-in image processing)
   - Measure resize + WebP + AVIF pipeline

### SSG Version Pinning

Pin exact versions. Use the latest stable release of each SSG at benchmark time. Document all versions. Re-run when major versions are released.

### CI Integration

- GitHub Actions workflow runs on every seite release tag
- Acknowledges CI hardware is noisy — canonical results from dedicated hardware
- Consider [Bencher](https://bencher.dev/) for continuous benchmark tracking
- Store results in the repo for historical comparison

---

## Key Sources

- [seancdavis/ssg-build-performance-tests](https://github.com/seancdavis/ssg-build-performance-tests) — Retired, but the original reference
- [zachleat/bench-framework-markdown](https://github.com/zachleat/bench-framework-markdown) — Most careful methodology
- [lumeland/benchmark](https://github.com/lumeland/benchmark) — Best structured active benchmark (100/1K/10K tiers)
- [grego/ssg-bench](https://github.com/grego/ssg-bench) — Honest small-scale Rust SSG comparison
- [sharkdp/hyperfine](https://github.com/sharkdp/hyperfine) — Gold standard CLI benchmarking tool
- [Easyperf: Consistent Benchmarking on Linux](https://easyperf.net/blog/2019/08/02/Perf-measurement-environment-on-Linux) — System controls guide
- [CloudCannon: Top 5 SSGs for 2025](https://cloudcannon.com/blog/the-top-five-static-site-generators-for-2025-and-when-to-use-them/) — Market landscape
- [Kodak Lossless True Color Image Suite](https://www.kaggle.com/datasets/sherylmehta/kodak-dataset) — Standard image benchmark set
- [Principles for Automated and Reproducible Benchmarking (ACM 2023)](https://dl.acm.org/doi/fullHtml/10.1145/3624062.3624133) — Formal methodology
- [Georges et al., OOPSLA 2007: Statistically Rigorous Java Performance Evaluation](https://dri.es/files/oopsla07-georges.pdf) — Statistical methodology
- [CSS-Tricks: Comparing SSG Build Times](https://css-tricks.com/comparing-static-site-generator-build-times/) — Original benchmark article
