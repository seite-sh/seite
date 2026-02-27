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
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCH_DIR="$SCRIPT_DIR"
SITES_DIR="$BENCH_DIR/sites"
RESULTS_DIR="$BENCH_DIR/results"
SKELETONS_DIR="$BENCH_DIR/skeletons"
SEITE_BIN="$PROJECT_DIR/target/release/seite"

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

setup_eleventy() {
  local s="$SITES_DIR/eleventy"
  if [ ! -d "$s/node_modules" ]; then
    rm -rf "$s"
    cp -r "$SKELETONS_DIR/eleventy" "$s"
    (cd "$s" && npm install --silent 2>/dev/null) || true
  fi
}

setup_jekyll() {
  local s="$SITES_DIR/jekyll"
  if [ ! -f "$s/Gemfile.lock" ]; then
    rm -rf "$s"
    cp -r "$SKELETONS_DIR/jekyll" "$s"
    (cd "$s" && bundle install --quiet 2>/dev/null) || true
  fi
}

setup_astro() {
  local s="$SITES_DIR/astro"
  if [ ! -d "$s/node_modules" ]; then
    rm -rf "$s"
    cp -r "$SKELETONS_DIR/astro" "$s"
    (cd "$s" && npm install --silent 2>/dev/null) || true
  fi
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
  echo "  hugo:     $(hugo version 2>/dev/null | head -c 60)"
  echo "  zola:     $(zola --version 2>/dev/null)"
  ! should_skip "eleventy" && echo "  eleventy: $(cd "$SITES_DIR/eleventy" && npx @11ty/eleventy --version 2>/dev/null || echo 'unknown')"
  echo "  jekyll:   $(jekyll --version 2>/dev/null)"
  ! should_skip "astro" && echo "  astro:    $(cd "$SITES_DIR/astro" && npx astro --version 2>/dev/null | tr -d ' ' || echo 'unknown')"
  echo ""
  echo "Hardware:"
  if [ "$(uname)" = "Darwin" ]; then
    echo "  CPU: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo 'unknown')"
    echo "  RAM: $(sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.0f GB", $1/1024/1024/1024}')"
  else
    echo "  CPU: $(lscpu 2>/dev/null | grep 'Model name' | sed 's/.*: *//' || echo 'unknown')"
    echo "  RAM: $(free -h 2>/dev/null | awk '/Mem:/ {print $2}' || echo 'unknown')"
    echo "  OS:  $(uname -sr)"
  fi

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
      echo "done ($(find "$SITES_DIR/$ssg" -name '*.md' -not -path '*/node_modules/*' | wc -l | tr -d ' ') .md files)"
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
        zola)     (cd "$SITES_DIR/zola"     && zola build) >/dev/null 2>&1 || build_ok=false ;;
        eleventy) (cd "$SITES_DIR/eleventy" && npx @11ty/eleventy --quiet) >/dev/null 2>&1 || build_ok=false ;;
        jekyll)   (cd "$SITES_DIR/jekyll"   && bundle exec jekyll build --quiet) >/dev/null 2>&1 || build_ok=false ;;
        astro)    (cd "$SITES_DIR/astro"    && npx astro build --silent) >/dev/null 2>&1 || build_ok=false ;;
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
      --warmup "$WARMUP"
      --runs "$RUNS"
      --export-json "$RESULTS_DIR/comparison-${tier}.json"
      --export-markdown "$RESULTS_DIR/comparison-${tier}.md"
    )

    for ssg in "${active[@]}"; do
      case "$ssg" in
        seite)    hf_args+=(-n seite    "cd $SITES_DIR/seite    && rm -rf dist   && $SEITE_BIN build 2>/dev/null") ;;
        hugo)     hf_args+=(-n hugo     "cd $SITES_DIR/hugo     && rm -rf public && hugo --quiet 2>/dev/null") ;;
        zola)     hf_args+=(-n zola     "cd $SITES_DIR/zola     && rm -rf public && zola build 2>/dev/null") ;;
        eleventy) hf_args+=(-n eleventy "cd $SITES_DIR/eleventy && rm -rf _site  && npx @11ty/eleventy --quiet 2>/dev/null") ;;
        jekyll)   hf_args+=(-n jekyll   "cd $SITES_DIR/jekyll   && rm -rf _site  && bundle exec jekyll build --quiet 2>/dev/null") ;;
        astro)    hf_args+=(-n astro    "cd $SITES_DIR/astro    && rm -rf dist   && npx astro build --silent 2>/dev/null") ;;
      esac
    done

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
