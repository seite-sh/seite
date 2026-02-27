#!/usr/bin/env bash
#
# seite build pipeline profiler
#
# Generates a realistic test site (text + images + math) and runs
# `seite build` under `samply` for flamegraph analysis.
#
# Usage:
#   ./bench/profile.sh                           # Default: 500 pages, 10 images, math enabled
#   ./bench/profile.sh --pages 1000              # 1000 pages
#   ./bench/profile.sh --pages 500 --images 20   # 500 pages, 20 images
#   ./bench/profile.sh --no-images               # Text-only (no image processing)
#   ./bench/profile.sh --no-math                 # Skip math/KaTeX content
#   ./bench/profile.sh --timings-only            # Just build and show step timings (no samply)
#
# Prerequisites:
#   cargo install samply    (unless using --timings-only)
#   Python 3                (for test image generation)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SEITE_BIN="$PROJECT_DIR/target/release/seite"
PROFILE_SITE="$SCRIPT_DIR/profile-site"

# Defaults
PAGES=500
IMAGES=10
MATH=true
TIMINGS_ONLY=false

# ── Parse arguments ──────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case "$1" in
    --pages)      PAGES="$2"; shift 2 ;;
    --images)     IMAGES="$2"; shift 2 ;;
    --no-images)  IMAGES=0; shift ;;
    --no-math)    MATH=false; shift ;;
    --math)       MATH=true; shift ;;
    --timings-only) TIMINGS_ONLY=true; shift ;;
    -h|--help)
      echo "Usage: $0 [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --pages N        Number of markdown posts (default: 500)"
      echo "  --images N       Number of test images (default: 10)"
      echo "  --no-images      Skip image generation"
      echo "  --math           Include math expressions (default)"
      echo "  --no-math        Skip math content"
      echo "  --timings-only   Just build and show step timings (no samply)"
      echo "  -h, --help       Show this help"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

# ── Preflight checks ────────────────────────────────────────────────

if [ ! -f "$SEITE_BIN" ]; then
  echo "ERROR: Release binary not found at $SEITE_BIN"
  echo "Run: cargo build --release"
  exit 1
fi

if [ "$TIMINGS_ONLY" = false ] && ! command -v samply &>/dev/null; then
  echo "ERROR: samply not found. Install with: cargo install samply"
  exit 1
fi

# ── Content generation ───────────────────────────────────────────────

LOREM_PARAGRAPHS=(
  "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."
  "Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio."
  "Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor."
  "Praesent dapibus, neque id cursus faucibus, tortor neque egestas augue, eu vulputate magna eros eu erat. Aliquam erat volutpat."
  "Pellentesque habitant morbi tristique senectus et netus et malesuada fames ac turpis egestas. Proin pharetra nonummy pede."
)

TAGS=("rust" "web" "programming" "tutorial" "guide" "performance" "design" "architecture" "testing" "deployment")

MATH_BLOCKS=(
  'The quadratic formula is $x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}$ which solves any quadratic equation.'
  '$$\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}$$'
  'Consider the matrix $A = \begin{pmatrix} a & b \\ c & d \end{pmatrix}$ with determinant $\det(A) = ad - bc$.'
  '$$\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}$$'
  'Euler identity: $e^{i\pi} + 1 = 0$, connecting five fundamental constants.'
  '$$\nabla \times \mathbf{E} = -\frac{\partial \mathbf{B}}{\partial t}$$'
  'The probability density function is $f(x) = \frac{1}{\sigma\sqrt{2\pi}} e^{-\frac{(x-\mu)^2}{2\sigma^2}}$.'
  '$$\lim_{n \to \infty} \left(1 + \frac{1}{n}\right)^n = e$$'
)

rand_tags() {
  local n=$1 result=""
  for ((i=0; i<n; i++)); do
    local tag="${TAGS[$((RANDOM % ${#TAGS[@]}))]}"
    result="${result:+$result, }$tag"
  done
  echo "$result"
}

generate_posts() {
  local site_dir="$1" count="$2"
  mkdir -p "$site_dir/content/posts"

  for ((i=1; i<=count; i++)); do
    local year=$((2020 + (i % 5)))
    local month day date slug tags
    month=$(printf "%02d" $(( (i % 12) + 1 )))
    day=$(printf "%02d" $(( (i % 28) + 1 )))
    date="${year}-${month}-${day}"
    slug="post-$(printf "%05d" $i)"
    tags=$(rand_tags $(( (RANDOM % 3) + 1 )))

    # Build body: 3-5 paragraphs
    local body=""
    local num_paras=$(( (RANDOM % 3) + 3 ))
    for ((p=0; p<num_paras; p++)); do
      body="${body}${LOREM_PARAGRAPHS[$((RANDOM % ${#LOREM_PARAGRAPHS[@]}))]}"$'\n\n'
    done

    # Add math expressions to ~30% of posts
    local math_section=""
    if [ "$MATH" = true ] && (( i % 3 == 0 )); then
      local expr1="${MATH_BLOCKS[$((RANDOM % ${#MATH_BLOCKS[@]}))]}"
      local expr2="${MATH_BLOCKS[$((RANDOM % ${#MATH_BLOCKS[@]}))]}"
      math_section="
## Mathematical Analysis

${expr1}

${expr2}
"
    fi

    # Add a code block to every post (exercises syntect)
    cat > "$site_dir/content/posts/${date}-${slug}.md" <<MDEOF
---
title: "Benchmark Post ${i}: Exploring Topic $(( (i % 50) + 1 ))"
date: ${date}
description: "Benchmark post number ${i} for profiling the seite build pipeline"
tags: [${tags}]
---

## Introduction

${body}
${math_section}
## Code Example

\`\`\`rust
fn process_${i}(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut output = Vec::with_capacity(input.len());
    for chunk in input.chunks(1024) {
        output.extend_from_slice(chunk);
    }
    Ok(output)
}
\`\`\`

## Conclusion

Post ${i} demonstrates the core concepts discussed above.
MDEOF
  done
}

generate_images() {
  local site_dir="$1" count="$2"
  mkdir -p "$site_dir/static/images"

  echo "  Generating ${count} test images (1200x800 PNG)..."

  # Fast PNG generation: create solid-color rows using bytearray (no per-pixel loop)
  python3 -c "
import struct, zlib, os, sys

def create_png(width, height, r, g, b, path):
    # Build raw image data using fast bytearray operations
    row_data = bytearray([r, g, b] * width)
    raw = bytearray()
    for y in range(height):
        raw.append(0)  # filter byte
        # Slight vertical gradient: darken toward bottom
        factor = 1.0 - 0.3 * (y / height)
        if y == 0:
            raw.extend(row_data)
        else:
            row = bytearray(len(row_data))
            row[0::3] = bytes([max(0, min(255, int(r * factor)))] * width)
            row[1::3] = bytes([max(0, min(255, int(g * factor)))] * width)
            row[2::3] = bytes([max(0, min(255, int(b * factor)))] * width)
            raw.extend(row)

    def chunk(ctype, data):
        c = ctype + data
        return struct.pack('>I', len(data)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)

    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n')
        f.write(chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)))
        f.write(chunk(b'IDAT', zlib.compress(bytes(raw), 1)))
        f.write(chunk(b'IEND', b''))

colors = [
    (220, 50, 50), (50, 130, 220), (50, 180, 80), (200, 150, 30),
    (150, 50, 200), (30, 170, 170), (220, 100, 50), (100, 100, 180),
    (180, 80, 120), (60, 150, 60), (200, 200, 50), (80, 80, 80),
    (255, 128, 0), (0, 128, 255), (128, 0, 255), (255, 0, 128),
    (0, 255, 128), (128, 255, 0), (64, 64, 192), (192, 64, 64),
]

count = int(sys.argv[1])
out_dir = sys.argv[2]

for i in range(count):
    r, g, b = colors[i % len(colors)]
    path = os.path.join(out_dir, f'image-{i+1:03d}.png')
    create_png(1200, 800, r, g, b, path)
    print(f'    Created {path}')
" "$count" "$site_dir/static/images"
}

# ── Site scaffolding ─────────────────────────────────────────────────

create_profile_site() {
  rm -rf "$PROFILE_SITE"
  mkdir -p "$(dirname "$PROFILE_SITE")"

  local site_name
  site_name="$(basename "$PROFILE_SITE")"

  echo "  Scaffolding site with seite init..."
  cd "$(dirname "$PROFILE_SITE")"
  "$SEITE_BIN" init "$site_name" \
    --title "Profile Benchmark" \
    --description "Build pipeline profiling" \
    --deploy-target github-pages \
    --collections posts >/dev/null 2>&1
  cd "$PROJECT_DIR"

  # Remove sample post
  rm -f "$PROFILE_SITE"/content/posts/*.md

  # Generate posts
  echo "  Generating ${PAGES} posts..."
  generate_posts "$PROFILE_SITE" "$PAGES"

  # Generate images (seite init already creates [images] section with widths/webp)
  if [ "$IMAGES" -gt 0 ]; then
    generate_images "$PROFILE_SITE" "$IMAGES"
  fi

  # Enable math if requested — insert after output_dir line in [build] section
  if [ "$MATH" = true ]; then
    sed -i '' '/^output_dir/a\
math = true
' "$PROFILE_SITE/seite.toml"
  fi

  # Add some images to posts (reference them in markdown)
  if [ "$IMAGES" -gt 0 ]; then
    local img_posts=$((PAGES < IMAGES * 3 ? PAGES : IMAGES * 3))
    for ((i=1; i<=img_posts && i<=PAGES; i++)); do
      local img_num=$(( (i % IMAGES) + 1 ))
      local img_name
      img_name=$(printf "image-%03d.png" "$img_num")
      # Find and append image reference to the post
      local post_file
      post_file=$(ls "$PROFILE_SITE/content/posts/"*"-post-$(printf "%05d" $i).md" 2>/dev/null | head -1)
      if [ -n "$post_file" ]; then
        echo "" >> "$post_file"
        echo "![Test image ${img_num}](/images/${img_name})" >> "$post_file"
      fi
    done
  fi
}

# ── Step timing parser ───────────────────────────────────────────────

parse_step_timings() {
  # Parse seite's timing output from stdout
  # Format: "    Step Name: 123.4ms" or "    Step Name: <1ms"
  local output="$1"
  echo ""
  echo "  ┌─────────────────────────────────────────────────────┐"
  echo "  │  Step Timings                                       │"
  echo "  ├─────────────────────────────────────────────────────┤"

  echo "$output" | grep -E '^\s{4}\S' | while IFS= read -r line; do
    # Trim leading whitespace
    local trimmed
    trimmed=$(echo "$line" | sed 's/^[[:space:]]*//')
    printf "  │  %-49s │\n" "$trimmed"
  done

  echo "  └─────────────────────────────────────────────────────┘"

  # Also extract total build time
  local total
  total=$(echo "$output" | grep -oE '[0-9]+\.[0-9]+s' | head -1)
  if [ -n "$total" ]; then
    echo ""
    echo "  Total build time: $total"
  fi
}

# ── Main ─────────────────────────────────────────────────────────────

main() {
  echo "═══════════════════════════════════════════════════════════════"
  echo "  seite Build Pipeline Profiler"
  echo "═══════════════════════════════════════════════════════════════"
  echo ""
  echo "  Configuration:"
  echo "    Pages:   $PAGES"
  echo "    Images:  $IMAGES"
  echo "    Math:    $MATH"
  echo "    Mode:    $([ "$TIMINGS_ONLY" = true ] && echo 'timings only' || echo 'samply flamegraph')"
  echo ""

  # Step 1: Create test site
  echo "── Creating test site ──────────────────────────────────────────"
  create_profile_site

  local content_size
  content_size=$(du -sh "$PROFILE_SITE/content" | awk '{print $1}')
  local static_size="0B"
  if [ -d "$PROFILE_SITE/static" ]; then
    static_size=$(du -sh "$PROFILE_SITE/static" | awk '{print $1}')
  fi
  echo "  Content: ${content_size}, Static: ${static_size}"
  echo ""

  cd "$PROFILE_SITE"

  if [ "$TIMINGS_ONLY" = true ]; then
    # Just run the build and capture timings
    echo "── Running build (timings only) ──────────────────────────────"
    local build_output
    build_output=$("$SEITE_BIN" build 2>&1) || {
      echo "BUILD FAILED:"
      echo "$build_output"
      cd "$PROJECT_DIR"
      exit 1
    }
    echo "$build_output" | head -1
    parse_step_timings "$build_output"
  else
    # First do a timings-only build to show step breakdown
    echo "── Dry-run build (step timings) ───────────────────────────────"
    rm -rf "$PROFILE_SITE/dist"
    local build_output
    build_output=$("$SEITE_BIN" build 2>&1) || {
      echo "BUILD FAILED:"
      echo "$build_output"
      cd "$PROJECT_DIR"
      exit 1
    }
    echo "$build_output" | head -1
    parse_step_timings "$build_output"

    # Now run under samply
    echo ""
    echo "── Running under samply (flamegraph) ─────────────────────────"
    echo "  This will open Firefox Profiler in your browser."
    echo ""
    rm -rf "$PROFILE_SITE/dist"
    samply record "$SEITE_BIN" build
  fi

  cd "$PROJECT_DIR"

  echo ""
  echo "═══════════════════════════════════════════════════════════════"
  echo "  Profiling complete."
  echo ""
  echo "  Site left at: $PROFILE_SITE"
  echo "  Clean up with: rm -rf $PROFILE_SITE"
  echo "═══════════════════════════════════════════════════════════════"
}

main
