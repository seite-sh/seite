# SSG Benchmark Results — seite 0.21

Measured 2026-11-05 on one machine. These numbers replace the July 2026 results for seite 0.3.2, which came from a different machine (Linux, 21 GB RAM), different SSG versions, and timed deleting the previous output as part of each build: nothing below can be compared with them.

## Headline

- **seite 0.21.2 is the fastest at 5,000 and 10,000 pages**: Zola 0.23.6 takes 1.16× and 1.15× seite's median time. At 1,000 pages the two are within noise (88 vs 92 ms); at 100 pages Zola is faster (22 vs 26 ms).
- **0.21.2 builds 1.3–2.5× faster than 0.21.1** (10,000 pages: 1.81 s → 718 ms; 0.21.1 was 2.2× Zola's time) with byte-identical output. See [Why Zola was faster](#why-zola-was-faster) for what changed.
- **Hugo 0.169** takes 1.65–2.06× seite 0.21.2's time.
- seite's peak memory is less than half of Hugo's at every tier and up to a quarter above Zola's. 0.21.2 uses 2–14% more peak memory than 0.21.1 on the minimal templates (10,000 pages: 311 vs 304 MiB) and 15% more with the default theme (683 vs 595 MiB), because pages stay in memory until post-processing.
- Jekyll, Eleventy and Astro trail at every size. Astro's fixed startup cost (about 0.9 s) dominates small sites.
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
| Astro | 5.18.2 (Node.js 20.18.2) |

## Methodology

- **Content**: the same generated posts for every SSG: YAML frontmatter (TOML for Zola), 1–3 random tags from 20, 3–5 Lorem Ipsum paragraphs and one fenced Rust code block. Content is regenerated per tier.
- **Templates**: a minimal template per SSG: `<h1>` + rendered content for a post, `<h1>` + a list of links for the index. seite uses `bench/skeletons/seite/templates/` (until now the seite skeleton fell back to seite's full default theme, unlike every other SSG).
- **Measurement**: `hyperfine --shell bash --warmup 1 --runs 5`. Cold builds: the output directory is deleted in hyperfine's `--prepare` step, which is not timed. (Earlier versions of this script deleted it inside the timed command; at 10,000 pages that deletion took 1.0 s for seite and 1.5 s for Zola, as long as the builds themselves.) Node SSGs run from `node_modules/.bin` (not `npx`, which adds resolution time to every run); Jekyll via `bundle exec`.
- **One run**: all four tiers and all seven commands ran back to back in one invocation, 18:46–19:02, after waiting for the machine to be idle (no Ollama model loaded, 1-minute load average under 5 for 3 minutes). Load during the run: median 2.8, max 7.5.
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
| zola | 22 ms | 22 ms ± 1 ms | 22 ms | 24 ms | 31 MiB | 1.00× | 0.83× |
| **seite 0.21.2** | **26 ms** | 26 ms ± 1 ms | 23 ms | 27 ms | 26 MiB | 1.20× | 1.00× |
| seite 0.21.1 | 34 ms | 34 ms ± 1 ms | 33 ms | 35 ms | 25 MiB | 1.55× | 1.29× |
| hugo | 43 ms | 43 ms ± 1 ms | 43 ms | 45 ms | 74 MiB | 1.98× | 1.65× |
| eleventy | 205 ms | 208 ms ± 5 ms | 203 ms | 215 ms | 102 MiB | 9.38× | 7.83× |
| jekyll | 236 ms | 243 ms ± 11 ms | 234 ms | 260 ms | 52 MiB | 10.79× | 9.00× |
| astro | 865 ms | 863 ms ± 3 ms | 859 ms | 867 ms | 297 MiB | 39.55× | 33.01× |

### 1,000 pages (5 runs)

| SSG | Median | Mean ± σ | Min | Max | Peak RSS | vs fastest | vs seite 0.21.2 |
|:----|-----:|-----:|----:|----:|-----:|-----:|-----:|
| **seite 0.21.2** | **88 ms** | 94 ms ± 13 ms | 82 ms | 112 ms | 58 MiB | 1.00× | 1.00× |
| zola | 92 ms | 92 ms ± 2 ms | 89 ms | 94 ms | 52 MiB | 1.04× | 1.04× |
| hugo | 151 ms | 151 ms ± 1 ms | 149 ms | 152 ms | 133 MiB | 1.70× | 1.70× |
| seite 0.21.1 | 186 ms | 187 ms ± 7 ms | 181 ms | 199 ms | 51 MiB | 2.10× | 2.10× |
| eleventy | 485 ms | 487 ms ± 7 ms | 481 ms | 498 ms | 137 MiB | 5.48× | 5.48× |
| jekyll | 523 ms | 521 ms ± 14 ms | 508 ms | 542 ms | 73 MiB | 5.91× | 5.91× |
| astro | 1.31 s | 1.31 s ± 0.02 s | 1.29 s | 1.34 s | 444 MiB | 14.84× | 14.84× |

### 5,000 pages (5 runs)

| SSG | Median | Mean ± σ | Min | Max | Peak RSS | vs fastest | vs seite 0.21.2 |
|:----|-----:|-----:|----:|----:|-----:|-----:|-----:|
| **seite 0.21.2** | **356 ms** | 357 ms ± 8 ms | 351 ms | 370 ms | 173 MiB | 1.00× | 1.00× |
| zola | 415 ms | 418 ms ± 16 ms | 406 ms | 446 ms | 138 MiB | 1.16× | 1.16× |
| hugo | 644 ms | 646 ms ± 8 ms | 639 ms | 658 ms | 379 MiB | 1.81× | 1.81× |
| seite 0.21.1 | 896 ms | 895 ms ± 25 ms | 857 ms | 927 ms | 162 MiB | 2.51× | 2.51× |
| jekyll | 1.59 s | 1.59 s ± 0.02 s | 1.58 s | 1.62 s | 162 MiB | 4.45× | 4.45× |
| eleventy | 1.79 s | 1.79 s ± 0.01 s | 1.78 s | 1.81 s | 239 MiB | 5.01× | 5.01× |
| astro | 3.12 s | 3.12 s ± 0.01 s | 3.11 s | 3.14 s | 1,058 MiB | 8.77× | 8.77× |

### 10,000 pages (5 runs)

| SSG | Median | Mean ± σ | Min | Max | Peak RSS | vs fastest | vs seite 0.21.2 |
|:----|-----:|-----:|----:|----:|-----:|-----:|-----:|
| **seite 0.21.2** | **718 ms** | 752 ms ± 56 ms | 707 ms | 840 ms | 311 MiB | 1.00× | 1.00× |
| zola | 823 ms | 819 ms ± 15 ms | 794 ms | 831 ms | 248 MiB | 1.15× | 1.15× |
| hugo | 1.48 s | 1.48 s ± 0.02 s | 1.46 s | 1.51 s | 698 MiB | 2.06× | 2.06× |
| seite 0.21.1 | 1.81 s | 1.81 s ± 0.01 s | 1.80 s | 1.83 s | 304 MiB | 2.52× | 2.52× |
| jekyll | 2.98 s | 2.99 s ± 0.03 s | 2.97 s | 3.05 s | 268 MiB | 4.14× | 4.14× |
| eleventy | 4.20 s | 4.22 s ± 0.03 s | 4.19 s | 4.26 s | 352 MiB | 5.85× | 5.85× |
| astro | 5.39 s | 5.43 s ± 0.10 s | 5.32 s | 5.56 s | 1,618 MiB | 7.51× | 7.51× |

## Scaling (median wall time)

```
Pages    seite 0.21.2  Zola      Hugo      seite 0.21.1
  100     26 ms         22 ms     43 ms     34 ms
1,000     88 ms         92 ms    151 ms    186 ms
5,000    356 ms        415 ms    644 ms    896 ms
10,000   718 ms        823 ms    1.48 s    1.81 s
```

## Why Zola was faster

Profiling seite 0.21.1 on the 10,000-page site (samply, release build with symbols) showed about two thirds of its CPU time in file-system calls: `open` 39%, `read` 10%, `close` 10%, `write` 8%, plus 9% in `malloc`. Markdown, syntax highlighting (7.5%), Tera rendering and related-post scoring were small by comparison. On this Mac, creating a file costs about 60 µs and barely parallelizes (20,000 files: 1.2 s on one thread, 0.8 s on 14), whether the files share one directory or each has its own.

seite 0.21.1 did about two and a half times Zola's file operations (about 50,000 opens for 10,000 pages: read each source, write the page, read it back, rewrite it, write the `.md` copy; Zola reads and writes once each). It wrote each page, read it back and wrote it again in post-processing (code-copy buttons, analytics, image `srcset`, link checks), and wrote the pages and their `.md` copies one at a time on one thread while rendering ran in parallel. 0.21.2 keeps rendered pages in memory until post-processing, writes each page once, writes the `.md` copies in the background while indexes and feeds render, renders tag pages in parallel, and writes generated files in parallel:

| Stage, 10,000 pages (`seite build -v`, mean of 3) | 0.21.1 | 0.21.2 |
|:--|--:|--:|
| Render pages (now render only) | 494 ms | 30 ms |
| Output markdown copies (now written in the background; this is the remaining wait) | 498 ms | 82 ms |
| Post-process HTML (now also the only page write) | 475 ms | 235 ms |
| Process collections (read, parse, Markdown, highlighting) | 160 ms | 159 ms |
| Render indexes, tags (tags now in parallel) | 98 ms | 71 ms |

On macOS seite now writes with 4 threads: file creation there is kernel-bound, and 8–14 writer threads were slower than 4 because they contend in the kernel.

With that, seite 0.21.2 (718 ms) is ahead of Zola (823 ms) at 10,000 pages, even though it still writes the 10,000 `.md` copies next to the HTML and highlights code and checks links, which the Zola skeleton does not. Zola keeps the lead at 100 pages, where fixed startup costs dominate.

## seite with its default theme

The seite skeleton's minimal templates are what the tables above use. With seite's bundled default theme instead (33 KB post pages with CSS, SEO head, JSON-LD and the search script; 425 MB of output at 10,000 pages):

| 10,000 pages, default theme | Median | Mean ± σ | Peak RSS |
|:--|--:|--:|--:|
| seite 0.21.2 | 882 ms | 909 ms ± 61 ms | 683 MiB |
| seite 0.21.1 | 2.39 s | 2.42 s ± 0.05 s | 595 MiB |

Same machine and content, hyperfine 10 runs, 1 warmup, output deleted in `--prepare`, measured right after the main run.

## Reproducing

```bash
cargo build --release
brew install hyperfine hugo zola        # plus Node.js and Ruby + Bundler
./bench/run-comparison.sh --tiers "100 1000 5000 10000" --runs 5 --warmup 1
```

`SEITE_BIN=/path/to/seite` benchmarks another seite binary, `SEITE_BASELINE_BIN=/path/to/seite` adds a second one to the same run (this run: `SEITE_BIN` = 0.21.2, `SEITE_BASELINE_BIN` = 0.21.1), and `--skip <ssg>` drops an SSG. The script installs Eleventy, Astro and Jekyll into `bench/sites/` and writes hyperfine JSON and Markdown to `bench/results/`; both directories are gitignored.
