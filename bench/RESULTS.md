# SSG Benchmark Results — seite 0.21

Measured 2026-11-05 on one machine. These numbers replace the July 2026 results for seite 0.3.2, which came from a different machine (Linux, 21 GB RAM), different SSG versions, and timed deleting the previous output as part of each build: nothing below can be compared with them.

## Headline

- **seite 0.21.2 is the fastest at 1,000 pages and above**: Zola 0.23.6 takes 1.14× seite's median time at 1,000 pages, 1.20× at 5,000 and 1.17× at 10,000. At 100 pages the two are within noise (Zola 22 ms, seite 23 ms). The 1,000-page lead varies between runs: in an earlier quiet run the same evening it was 4%.
- **0.21.2 builds 1.5–2.6× faster than 0.21.1** (10,000 pages: 1.84 s → 712 ms; 0.21.1 took 2.2× Zola's time) with byte-identical output. See [Why Zola was faster](#why-zola-was-faster) for what changed.
- **Hugo 0.169** takes 1.8–2.1× seite 0.21.2's time.
- seite's peak memory is less than half of Hugo's at every tier and up to a quarter above Zola's. 0.21.2 uses up to 14% more peak memory than 0.21.1 on the minimal templates (1,000 pages: 58 vs 51 MiB; 10,000 pages: 301 vs 303 MiB) and 13% more with the default theme (667 vs 592 MiB), because pages stay in memory until post-processing.
- Jekyll, Eleventy and Astro trail at every size. Astro's fixed startup cost (about 0.8 s) dominates small sites.
- Every SSG built every tier. The Zola segfault at ~8,000 pages reported for Zola 0.20.0 in July did not occur with Zola 0.23.6.

## Machine and versions

| | |
|:--|:--|
| Machine | Apple M4 Pro (14 cores), 48 GB RAM |
| OS | macOS 27.2 (Darwin 27.2.0) |
| hyperfine | 2.0.0 |
| seite | 0.21.2 (this branch) and 0.21.1 (the release on main), both `cargo build --release` |
| Hugo | 0.169.0+extended (Homebrew) |
| Zola | 0.23.6 (Homebrew) |
| Eleventy | 3.1.8 (Node.js 20.18.2) |
| Jekyll | 4.4.1 (Ruby 3.4.1 from Homebrew, gems bundled into `bench/sites/jekyll/vendor`) |
| Astro | 5.18.2 (Node.js 20.18.2); the script now installs 7.4.7, which has fixes for 11 Astro advisories and needs Node.js >= 22.23.4 |

## Methodology

- **Content**: the same generated posts for every SSG: YAML frontmatter (TOML for Zola), 1–3 random tags from 20, 3–5 Lorem Ipsum paragraphs and one fenced Rust code block. Content is regenerated per tier.
- **Templates**: a minimal template per SSG: `<h1>` + rendered content for a post, `<h1>` + a list of links for the index. seite uses `bench/skeletons/seite/templates/` (until now the seite skeleton fell back to seite's full default theme, unlike every other SSG).
- **Measurement**: `hyperfine --shell bash --warmup 1 --runs 5`. Cold builds: the output directory is deleted in hyperfine's `--prepare` step, which is not timed. (Earlier versions of this script deleted it inside the timed command; at 10,000 pages that deletion took 1.0 s for seite and 1.5 s for Zola, as long as the builds themselves.) Node SSGs run from `node_modules/.bin` (not `npx`, which adds resolution time to every run); Jekyll via `bundle exec`.
- **One run**: all four tiers and all seven commands ran back to back in one invocation, 22:14–22:29, after waiting for the machine to be idle (no Ollama model loaded, 1-minute load average under 5 for 3 minutes). Load during the run: median 2.9, max 5.7.
- **Medians**: the tables are sorted by median wall time; mean ± σ, min and max are shown too.
- **Desktop machine, not an isolated runner**: CPU frequency was not pinned and other apps were open. Treat differences under ~10% as noise.

### What each SSG does beyond the template (not equalized)

The minimal templates equalize page markup, not the work each SSG does by default:

- **seite** syntax-highlights code blocks (inline styles), checks every internal link, and also writes a `.md` copy of every page, `llms.txt`, `llms-full.txt`, `search-index.json`, RSS + Atom feeds, `sitemap.xml`, `sitemap.md`, `robots.txt`, `404.html` and tag pages. The skeleton `seite.toml` is deliberately minimal (pre-tracking format, no `[og]`, no `search = "full"`): `seite check` passes and only warns that `base.html` lacks the SEO head partial.
- **Hugo** writes RSS, `sitemap.xml` and tag + category pages; highlighting is off (`codeFences = false`).
- **Zola** writes `sitemap.xml`, `robots.txt` and `404.html`; no taxonomies configured, highlighting off.
- **Jekyll** (`highlighter: none`), **Eleventy** and **Astro** write the posts plus an index only (Eleventy: no index).

All of this costs seite time, so the comparison does not favour it.

## Results (minimal templates)

### 100 pages (5 runs)

| SSG | Median | Mean ± σ | Min | Max | Peak RSS | vs fastest | vs seite 0.21.2 |
|:----|-----:|-----:|----:|----:|-----:|-----:|-----:|
| zola | 22 ms | 22 ms ± 1 ms | 21 ms | 23 ms | 31 MiB | 1.00× | 0.94× |
| **seite 0.21.2** | **23 ms** | 23 ms ± 1 ms | 22 ms | 24 ms | 27 MiB | 1.06× | 1.00× |
| seite 0.21.1 | 34 ms | 34 ms ± 1 ms | 33 ms | 35 ms | 25 MiB | 1.57× | 1.48× |
| hugo | 42 ms | 43 ms ± 1 ms | 42 ms | 44 ms | 74 MiB | 1.95× | 1.84× |
| eleventy | 198 ms | 199 ms ± 4 ms | 195 ms | 206 ms | 101 MiB | 9.12× | 8.59× |
| jekyll | 219 ms | 220 ms ± 1 ms | 218 ms | 222 ms | 52 MiB | 10.08× | 9.50× |
| astro | 833 ms | 831 ms ± 6 ms | 824 ms | 839 ms | 300 MiB | 38.32× | 36.12× |

### 1,000 pages (5 runs)

| SSG | Median | Mean ± σ | Min | Max | Peak RSS | vs fastest | vs seite 0.21.2 |
|:----|-----:|-----:|----:|----:|-----:|-----:|-----:|
| **seite 0.21.2** | **83 ms** | 84 ms ± 3 ms | 81 ms | 90 ms | 58 MiB | 1.00× | 1.00× |
| zola | 94 ms | 93 ms ± 3 ms | 89 ms | 96 ms | 52 MiB | 1.14× | 1.14× |
| hugo | 148 ms | 148 ms ± 2 ms | 145 ms | 151 ms | 134 MiB | 1.79× | 1.79× |
| seite 0.21.1 | 178 ms | 182 ms ± 11 ms | 176 ms | 201 ms | 51 MiB | 2.15× | 2.15× |
| eleventy | 462 ms | 466 ms ± 7 ms | 462 ms | 479 ms | 138 MiB | 5.59× | 5.59× |
| jekyll | 484 ms | 485 ms ± 4 ms | 483 ms | 493 ms | 71 MiB | 5.85× | 5.85× |
| astro | 1.29 s | 1.28 s ± 0.00 s | 1.28 s | 1.29 s | 451 MiB | 15.54× | 15.54× |

### 5,000 pages (5 runs)

| SSG | Median | Mean ± σ | Min | Max | Peak RSS | vs fastest | vs seite 0.21.2 |
|:----|-----:|-----:|----:|----:|-----:|-----:|-----:|
| **seite 0.21.2** | **344 ms** | 347 ms ± 5 ms | 343 ms | 354 ms | 172 MiB | 1.00× | 1.00× |
| zola | 412 ms | 412 ms ± 2 ms | 410 ms | 415 ms | 138 MiB | 1.20× | 1.20× |
| hugo | 640 ms | 641 ms ± 6 ms | 634 ms | 649 ms | 372 MiB | 1.86× | 1.86× |
| seite 0.21.1 | 846 ms | 853 ms ± 19 ms | 834 ms | 881 ms | 162 MiB | 2.46× | 2.46× |
| jekyll | 1.56 s | 1.56 s ± 0.01 s | 1.54 s | 1.58 s | 156 MiB | 4.53× | 4.53× |
| eleventy | 1.77 s | 1.78 s ± 0.01 s | 1.77 s | 1.79 s | 239 MiB | 5.15× | 5.15× |
| astro | 3.10 s | 3.10 s ± 0.02 s | 3.07 s | 3.11 s | 1,057 MiB | 9.00× | 9.00× |

### 10,000 pages (5 runs)

| SSG | Median | Mean ± σ | Min | Max | Peak RSS | vs fastest | vs seite 0.21.2 |
|:----|-----:|-----:|----:|----:|-----:|-----:|-----:|
| **seite 0.21.2** | **712 ms** | 713 ms ± 5 ms | 708 ms | 719 ms | 301 MiB | 1.00× | 1.00× |
| zola | 833 ms | 831 ms ± 3 ms | 828 ms | 834 ms | 248 MiB | 1.17× | 1.17× |
| hugo | 1.49 s | 1.50 s ± 0.02 s | 1.49 s | 1.53 s | 692 MiB | 2.10× | 2.10× |
| seite 0.21.1 | 1.84 s | 1.83 s ± 0.02 s | 1.80 s | 1.85 s | 303 MiB | 2.58× | 2.58× |
| jekyll | 2.93 s | 2.94 s ± 0.05 s | 2.91 s | 3.02 s | 269 MiB | 4.11× | 4.11× |
| eleventy | 4.21 s | 4.21 s ± 0.02 s | 4.19 s | 4.24 s | 357 MiB | 5.91× | 5.91× |
| astro | 5.32 s | 5.36 s ± 0.10 s | 5.28 s | 5.48 s | 1,629 MiB | 7.46× | 7.46× |

## Scaling (median wall time)

```
Pages    seite 0.21.2  Zola      Hugo      seite 0.21.1
  100     23 ms         22 ms     42 ms     34 ms
1,000     83 ms         94 ms    148 ms    178 ms
5,000    344 ms        412 ms    640 ms    846 ms
10,000   712 ms        833 ms    1.49 s    1.84 s
```

## Why Zola was faster

Profiling seite 0.21.1 on the 10,000-page site (samply, release build with symbols) showed about two thirds of its CPU time in file-system calls: `open` 39%, `read` 10%, `close` 10%, `write` 8%, plus 9% in `malloc`. Markdown, syntax highlighting (7.5%), Tera rendering and related-post scoring were small by comparison. On this Mac, creating a file costs about 60 µs and barely parallelizes (20,000 files: 1.2 s on one thread, 0.8 s on 14), whether the files share one directory or each has its own.

seite 0.21.1 did about two and a half times Zola's file operations (about 50,000 opens for 10,000 pages: read each source, write the page, read it back, rewrite it, write the `.md` copy; Zola reads and writes once each). It wrote each page, read it back and wrote it again in post-processing (code-copy buttons, analytics, image `srcset`, link checks), and wrote the pages and their `.md` copies one at a time on one thread while rendering ran in parallel. 0.21.2 keeps rendered pages in memory until post-processing, writes each page once, writes the `.md` copies in the background while indexes and feeds render, renders tag pages in parallel, and writes generated files in parallel:

| Stage, 10,000 pages (`seite build -v`, mean of 3) | 0.21.1 | 0.21.2 |
|:--|--:|--:|
| Render pages (now render only) | 489 ms | 29 ms |
| Output markdown copies (now written in the background; this is the remaining wait) | 485 ms | 89 ms |
| Post-process HTML (now also the only page write) | 507 ms | 239 ms |
| Process collections (read, parse, Markdown, highlighting) | 158 ms | 161 ms |
| Render indexes, tags (tags now in parallel) | 97 ms | 70 ms |

On macOS seite now writes with 4 threads: file creation there is kernel-bound, and 8–14 writer threads were slower than 4 because they contend in the kernel.

With that, seite 0.21.2 (712 ms) is ahead of Zola (833 ms) at 10,000 pages, even though it still writes the 10,000 `.md` copies next to the HTML and highlights code and checks links, which the Zola skeleton does not. At 100 pages, where fixed startup costs dominate, the two are even.

## seite with its default theme

The seite skeleton's minimal templates are what the tables above use. With seite's bundled default theme instead (33 KB post pages with CSS, SEO head, JSON-LD and the search script; 425 MB of output at 10,000 pages):

| 10,000 pages, default theme | Median | Mean ± σ | Peak RSS |
|:--|--:|--:|--:|
| seite 0.21.2 | 963 ms | 983 ms ± 56 ms | 667 MiB |
| seite 0.21.1 | 2.33 s | 2.38 s ± 0.13 s | 592 MiB |

Same machine and content, hyperfine 10 runs, 1 warmup, output deleted in `--prepare`, measured right after the main run.

## Reproducing

```bash
cargo build --release
brew install hyperfine hugo zola        # plus Node.js and Ruby + Bundler
./bench/run-comparison.sh --tiers "100 1000 5000 10000" --runs 5 --warmup 1
```

`SEITE_BIN=/path/to/seite` benchmarks another seite binary, `SEITE_BASELINE_BIN=/path/to/seite` adds a second one to the same run (this run: `SEITE_BIN` = 0.21.2, `SEITE_BASELINE_BIN` = 0.21.1), and `--skip <ssg>` drops an SSG. The script installs pinned versions of Eleventy, Astro and Jekyll (`ELEVENTY_VERSION`, `ASTRO_VERSION`, `JEKYLL_VERSION` override them) into `bench/sites/`, writing their `package.json` / `Gemfile` there rather than committing them, and writes hyperfine JSON and Markdown to `bench/results/`; both directories are gitignored.
