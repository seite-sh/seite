#!/usr/bin/env bash
#
# Multi-SSG Benchmark Comparison
#
# Benchmarks seite against Hugo, Zola, Eleventy, Jekyll, and Astro
# using hyperfine for statistically rigorous measurements.
#
# Usage:
#   ./bench/run-comparison.sh                    # All tiers, all SSGs
#   ./bench/run-comparison.sh --tiers "100 1000" # Specific tiers
#   ./bench/run-comparison.sh --runs 10          # Fewer runs
#   ./bench/run-comparison.sh --skip astro       # Skip an SSG
#   SEITE_BIN=/path/to/seite ./bench/run-comparison.sh  # Other seite binary
#   SEITE_BASELINE_BIN=/path/to/old/seite ./bench/run-comparison.sh  # Also time a second seite
#
# Timed: the build only; deleting the previous output runs in --prepare.
#
# Requires: hyperfine, hugo, zola, Node.js + npm (Eleventy; Astro needs
# Node.js >= 22.23.4), Ruby + Bundler (Jekyll). Everything it installs or
# generates lives under bench/sites/ and bench/results/ (both gitignored).
#
# The Node and Ruby SSGs are pinned below and their package.json / Gemfile
# are written into bench/sites/ at setup, never committed: the repo carries
# no manifest for tools it only measures (no Dependabot alerts or PRs for
# them). Override a pin with e.g. ASTRO_VERSION=7.4.7.
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCH_DIR="$SCRIPT_DIR"
SITES_DIR="$BENCH_DIR/sites"
RESULTS_DIR="$BENCH_DIR/results"
SKELETONS_DIR="$BENCH_DIR/skeletons"
SEITE_BIN="${SEITE_BIN:-$PROJECT_DIR/target/release/seite}"

# Exact versions of the SSGs installed into bench/sites/.
ELEVENTY_VERSION="${ELEVENTY_VERSION:-3.1.8}"
JEKYLL_VERSION="${JEKYLL_VERSION:-4.4.1}"
ASTRO_VERSION="${ASTRO_VERSION:-7.4.7}"

# Defaults
TIERS="100 1000 5000 10000"
RUNS=10
WARMUP=2
SKIP=""

while [[ $# -gt 0 ]]; do
  case $1 in
    --tiers)  TIERS="$2"; shift 2 ;;
    --runs)   RUNS="$2"; shift 2 ;;
    --warmup) WARMUP="$2"; shift 2 ;;
    --skip)   SKIP="$SKIP $2"; shift 2 ;;
    *)        echo "Unknown arg: $1"; exit 1 ;;
  esac
done

# ── Content ──────────────────────────────────────────────────────────

LOREM=(
  "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur."
  "Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris."
  "Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula ut dictum pharetra, nisi nunc fringilla magna, in commodo elit erat nec turpis. Ut pharetra auctor nunc."
  "Praesent dapibus, neque id cursus faucibus, tortor neque egestas augue, eu vulputate magna eros eu erat. Aliquam erat volutpat. Nam dui mi, tincidunt quis, accumsan porttitor, facilisis luctus, metus. Phasellus ultrices nulla quis nibh. Quisque a lectus."
  "Pellentesque habitant morbi tristique senectus et netus et malesuada fames ac turpis egestas. Proin pharetra nonummy pede. Mauris et orci. Aenean nec lorem. In porttitor. Donec laoreet nonummy augue. Suspendisse dui purus, scelerisque at, vulputate vitae, pretium mattis, nunc."
  "Mauris eget neque at sem venenatis eleifend. Ut nonummy. Fusce aliquet pede non pede. Suspendisse dapibus lorem pellentesque magna. Integer nulla. Donec blandit feugiat ligula. Donec hendrerit, felis et imperdiet euismod, purus ipsum pretium metus, in lacinia nulla nisl eget sapien."
)

TAGS=("rust" "web" "programming" "tutorial" "guide" "performance" "design" "architecture" "testing" "deployment" "security" "api" "database" "frontend" "backend" "devops" "cloud" "ai" "ml" "data")

should_skip() { [[ " $SKIP " == *" $1 "* ]]; }

rand_tags_yaml() {
  local n=$1 r=""
  for ((i=0; i<n; i++)); do r="${r:+$r, }${TAGS[$((RANDOM % ${#TAGS[@]}))]}"; done
  echo "$r"
}

body_paragraphs() {
  local n=$(( (RANDOM % 3) + 3 )) body=""
  for ((p=0; p<n; p++)); do body="${body}${LOREM[$((RANDOM % ${#LOREM[@]}))]}

"; done
  echo "$body"
}

# Generate YAML-frontmatter markdown (seite, hugo, eleventy, jekyll)
gen_yaml_content() {
  local dir="$1" count="$2"
  mkdir -p "$dir"
  for ((i=1; i<=count; i++)); do
    local y=$((2020 + (i % 5)))
    local m; m=$(printf "%02d" $(( (i % 12) + 1 )))
    local d; d=$(printf "%02d" $(( (i % 28) + 1 )))
    local tags; tags=$(rand_tags_yaml $(( (RANDOM % 3) + 1 )))
    local body; body=$(body_paragraphs)
    cat > "$dir/${y}-${m}-${d}-post-$(printf "%05d" $i).md" <<MDEOF
---
title: "Benchmark Post ${i}: Exploring Topic $(( (i % 50) + 1 ))"
date: ${y}-${m}-${d}
description: "Benchmark post number ${i} for performance testing"
tags: [${tags}]
layout: post
---

${body}

## Code Example

\`\`\`rust
fn process_${i}(input: &str) -> String {
    input.trim().to_lowercase()
}
\`\`\`
MDEOF
  done
}

# Generate TOML-frontmatter markdown (zola)
gen_toml_content() {
  local dir="$1" count="$2"
  mkdir -p "$dir"
  for ((i=1; i<=count; i++)); do
    local y=$((2020 + (i % 5)))
    local m; m=$(printf "%02d" $(( (i % 12) + 1 )))
    local d; d=$(printf "%02d" $(( (i % 28) + 1 )))
    local body; body=$(body_paragraphs)
    cat > "$dir/${y}-${m}-${d}-post-$(printf "%05d" $i).md" <<MDEOF
+++
title = "Benchmark Post ${i}: Exploring Topic $(( (i % 50) + 1 ))"
date = ${y}-${m}-${d}
description = "Benchmark post number ${i} for performance testing"
[taxonomies]
+++

${body}

## Code Example

\`\`\`rust
fn process_${i}(input: &str) -> String {
    input.trim().to_lowercase()
}
\`\`\`
MDEOF
  done
}

# ── Setup (one-time per SSG) ─────────────────────────────────────────

setup_seite() {
  local s="$SITES_DIR/seite"
  rm -rf "$s"
  cp -r "$SKELETONS_DIR/seite" "$s"
}

setup_hugo() {
  local s="$SITES_DIR/hugo"
  rm -rf "$s"
  cp -r "$SKELETONS_DIR/hugo" "$s"
}

setup_zola() {
  local s="$SITES_DIR/zola"
  rm -rf "$s"
  cp -r "$SKELETONS_DIR/zola" "$s"
  mkdir -p "$s/content/posts"
  cat > "$s/content/posts/_index.md" <<'EOF'
+++
sort_by = "date"
+++
EOF
}

# Copy a skeleton into bench/sites/<ssg> and write its manifest there, unless
# that site is already installed from the same manifest. $1 = ssg, $2 =
# manifest file name, $3 = manifest contents, $4 = install command.
setup_installed() {
  local ssg="$1" manifest="$2" contents="$3" install="$4"
  local s="$SITES_DIR/$ssg"
  if [ -f "$s/.installed" ] && [ "$(cat "$s/$manifest" 2>/dev/null)" = "$contents" ]; then
    return
  fi
  rm -rf "$s"
  cp -r "$SKELETONS_DIR/$ssg" "$s"
  printf '%s\n' "$contents" > "$s/$manifest"
  if (cd "$s" && eval "$install"); then
    touch "$s/.installed"
  else
    echo "  warning: installing $ssg failed; its runs will fail" >&2
  fi
}

# $1 = ssg, $2 = package, $3 = version, $4 = extra top-level JSON line (optional)
npm_manifest() {
  printf '{\n  "name": "bench-%s",\n  "private": true,\n%s  "dependencies": {\n    "%s": "%s"\n  }\n}' "$1" "${4:+  $4$'\n'}" "$2" "$3"
}

NPM_INSTALL="npm install --silent --no-audit --no-fund --save-exact >/dev/null 2>&1"

setup_eleventy() {
  setup_installed eleventy package.json "$(npm_manifest eleventy @11ty/eleventy "$ELEVENTY_VERSION")" "$NPM_INSTALL"
}

setup_jekyll() {
  setup_installed jekyll Gemfile "$(printf 'source "https://rubygems.org"\ngem "jekyll", "%s"' "$JEKYLL_VERSION")" \
    "bundle config set --local path vendor/bundle >/dev/null && bundle install --quiet >/dev/null 2>&1"
}

setup_astro() {
  setup_installed astro package.json "$(npm_manifest astro astro "$ASTRO_VERSION" '"type": "module",')" "$NPM_INSTALL"
}

# ── Fill content for a tier ──────────────────────────────────────────

fill_seite()    { rm -rf "$SITES_DIR/seite/content/posts";      gen_yaml_content "$SITES_DIR/seite/content/posts" "$1"; }
fill_hugo()     { rm -rf "$SITES_DIR/hugo/content/posts";       gen_yaml_content "$SITES_DIR/hugo/content/posts" "$1"; }
fill_zola()     {
  rm -rf "$SITES_DIR/zola/content/posts"
  mkdir -p "$SITES_DIR/zola/content/posts"
  cat > "$SITES_DIR/zola/content/posts/_index.md" <<'EOF'
+++
sort_by = "date"
+++
EOF
  gen_toml_content "$SITES_DIR/zola/content/posts" "$1"
}
fill_eleventy() { rm -rf "$SITES_DIR/eleventy/posts";           gen_yaml_content "$SITES_DIR/eleventy/posts" "$1"; }
fill_jekyll()   { rm -rf "$SITES_DIR/jekyll/_posts";            gen_yaml_content "$SITES_DIR/jekyll/_posts" "$1"; }
fill_astro()    { rm -rf "$SITES_DIR/astro/src/content/posts";  gen_yaml_content "$SITES_DIR/astro/src/content/posts" "$1"; }

# ── Main ─────────────────────────────────────────────────────────────

main() {
  echo "═══════════════════════════════════════════════════════════════"
  echo "  Multi-SSG Benchmark Comparison"
  echo "═══════════════════════════════════════════════════════════════"
  echo ""
  echo "  Date:    $(date '+%Y-%m-%d %H:%M:%S')"
  echo "  Tiers:   $TIERS"
  echo "  Runs:    $RUNS per command"
  echo "  Warmup:  $WARMUP runs"

  if [ ! -f "$SEITE_BIN" ]; then
    echo "ERROR: seite binary not found. Run: cargo build --release"
    exit 1
  fi

  mkdir -p "$SITES_DIR" "$RESULTS_DIR"

  local ssgs=()
  for ssg in seite hugo zola eleventy jekyll astro; do
    if should_skip "$ssg"; then
      echo "  Skipping: $ssg"
    else
      ssgs+=("$ssg")
    fi
  done
  echo "  SSGs:    ${ssgs[*]}"

  # One-time setup
  echo ""
  echo "── Setting up SSG projects ─────────────────────────────────────"
  for ssg in "${ssgs[@]}"; do
    echo -n "  $ssg... "
    "setup_$ssg"
    echo "done"
  done

  # Print versions
  echo ""
  echo "SSG Versions:"
  echo "  seite:    $("$SEITE_BIN" --version 2>/dev/null || echo 'unknown')"
  [ -n "${SEITE_BASELINE_BIN:-}" ] && echo "  seite-baseline: $("$SEITE_BASELINE_BIN" --version 2>/dev/null || echo 'unknown')"
  should_skip hugo     || echo "  hugo:     $(hugo version 2>/dev/null | cut -d' ' -f2 || echo 'unknown')"
  should_skip zola     || echo "  zola:     $(zola --version 2>/dev/null || echo 'unknown')"
  should_skip eleventy || echo "  eleventy: $("$SITES_DIR/eleventy/node_modules/.bin/eleventy" --version 2>/dev/null || echo 'unknown')"
  should_skip jekyll   || echo "  jekyll:   $(cd "$SITES_DIR/jekyll" && bundle exec jekyll --version 2>/dev/null || echo 'unknown') (ruby $(ruby -e 'print RUBY_VERSION' 2>/dev/null))"
  should_skip astro    || echo "  astro:    $("$SITES_DIR/astro/node_modules/.bin/astro" --version 2>/dev/null | tr -d ' ' || echo 'unknown')"
  echo "  node:     $(node --version 2>/dev/null || echo 'none')"
  echo "  hyperfine: $(hyperfine --version 2>/dev/null || echo 'none')"
  echo ""
  echo "Hardware:"
  if [ "$(uname)" = "Darwin" ]; then
    echo "  CPU: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo 'unknown')"
    echo "  RAM: $(sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.0f GB", $1/1024/1024/1024}')"
  else
    echo "  CPU: $(lscpu 2>/dev/null | grep 'Model name' | sed 's/.*: *//' || echo 'unknown')"
    echo "  RAM: $(free -h 2>/dev/null | awk '/Mem:/ {print $2}' || echo 'unknown')"
  fi
  echo "  OS:  $(uname -sr)"

  # Run per tier
  for tier in $TIERS; do
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "  Benchmark: ${tier} pages"
    echo "═══════════════════════════════════════════════════════════════"

    echo "  Generating content..."
    for ssg in "${ssgs[@]}"; do
      echo -n "    $ssg... "
      "fill_$ssg" "$tier"
      echo "done ($(find "$SITES_DIR/$ssg" -name '*.md' -not -path '*/node_modules/*' -not -path '*/vendor/*' | wc -l | tr -d ' ') .md files)"
    done

    # Verify builds
    echo "  Verifying builds..."
    local active=()
    for ssg in "${ssgs[@]}"; do
      echo -n "    $ssg... "
      local out_dir
      case "$ssg" in
        seite)    out_dir="$SITES_DIR/seite/dist" ;;
        hugo)     out_dir="$SITES_DIR/hugo/public" ;;
        zola)     out_dir="$SITES_DIR/zola/public" ;;
        eleventy) out_dir="$SITES_DIR/eleventy/_site" ;;
        jekyll)   out_dir="$SITES_DIR/jekyll/_site" ;;
        astro)    out_dir="$SITES_DIR/astro/dist" ;;
      esac
      rm -rf "$out_dir"

      local build_ok=true
      case "$ssg" in
        seite)    (cd "$SITES_DIR/seite"    && "$SEITE_BIN" build) >/dev/null 2>&1 || build_ok=false ;;
        hugo)     (cd "$SITES_DIR/hugo"     && hugo --quiet) >/dev/null 2>&1 || build_ok=false ;;
        zola)     (cd "$SITES_DIR/zola" && zola build) >/dev/null 2>&1 || build_ok=false ;;
        eleventy) (cd "$SITES_DIR/eleventy" && node_modules/.bin/eleventy --quiet) >/dev/null 2>&1 || build_ok=false ;;
        jekyll)   (cd "$SITES_DIR/jekyll"   && bundle exec jekyll build --quiet) >/dev/null 2>&1 || build_ok=false ;;
        astro)    (cd "$SITES_DIR/astro"    && node_modules/.bin/astro build --silent) >/dev/null 2>&1 || build_ok=false ;;
      esac

      if $build_ok; then
        local html_count
        html_count=$(find "$out_dir" -name "*.html" 2>/dev/null | wc -l | tr -d ' ')
        echo "OK ($html_count HTML files)"
        active+=("$ssg")
      else
        echo "FAILED — skipping"
      fi
    done

    if [ ${#active[@]} -eq 0 ]; then
      echo "  No SSGs built. Skipping tier."
      continue
    fi

    # Build hyperfine command
    local hf_args=(
      hyperfine
      --shell bash
      --warmup "$WARMUP"
      --runs "$RUNS"
      --export-json "$RESULTS_DIR/comparison-${tier}.json"
      --export-markdown "$RESULTS_DIR/comparison-${tier}.md"
    )

    # Cold builds: each SSG's output dir is deleted in --prepare, which
    # hyperfine does not time (deleting 10,000+ files takes about as long as
    # building them, and differs per output layout).
    local cmd_args=()
    for ssg in "${active[@]}"; do
      local dir out cmd
      dir="$SITES_DIR/$ssg"
      case "$ssg" in
        seite)    out=dist;   cmd="'$SEITE_BIN' build" ;;
        hugo)     out=public; cmd="hugo --quiet" ;;
        zola)     out=public; cmd="zola build" ;;
        eleventy) out=_site;  cmd="node_modules/.bin/eleventy --quiet" ;;
        jekyll)   out=_site;  cmd="bundle exec jekyll build --quiet" ;;
        astro)    out=dist;   cmd="node_modules/.bin/astro build --silent" ;;
      esac
      hf_args+=(--prepare "rm -rf '$dir/$out'")
      cmd_args+=(-n "$ssg" "cd '$dir' && $cmd 2>/dev/null")
    done
    # Optional second seite binary on the same site (e.g. a release vs. a branch)
    if [ -n "${SEITE_BASELINE_BIN:-}" ] && [[ " ${active[*]} " == *" seite "* ]]; then
      hf_args+=(--prepare "rm -rf '$SITES_DIR/seite/dist'")
      cmd_args+=(-n seite-baseline "cd '$SITES_DIR/seite' && '$SEITE_BASELINE_BIN' build 2>/dev/null")
    fi
    hf_args+=("${cmd_args[@]}")

    echo ""
    echo "  Running hyperfine ($RUNS runs, $WARMUP warmup)..."
    echo ""

    "${hf_args[@]}" 2>&1 || true

    if [ -f "$RESULTS_DIR/comparison-${tier}.md" ]; then
      echo ""
      cat "$RESULTS_DIR/comparison-${tier}.md"
    fi
  done

  echo ""
  echo "═══════════════════════════════════════════════════════════════"
  echo "  Benchmark complete! Results in: $RESULTS_DIR/"
  echo "═══════════════════════════════════════════════════════════════"
  ls -la "$RESULTS_DIR/"
}

main "$@"
