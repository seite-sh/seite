#!/usr/bin/env bash
#
# seite SSG benchmark
#
# Methodology (follows lumeland/benchmark pattern):
#   - Scaffold site with `seite init`
#   - Generate N markdown files with realistic frontmatter + body
#   - Run 10 cold builds (clean dist/ each time)
#   - Report median, min, max, mean build time
#   - Test at: 100, 500, 1000, 2000, 5000, 10000 pages
#
# Usage:
#   ./bench/run.sh                              # Run all tiers
#   ./bench/run.sh 1000                         # Run only 1000-page tier
#   ./bench/run.sh 1000 5                       # 1000 pages, 5 runs
#   ./bench/run.sh --with-images 1000 5         # With image processing
#   ./bench/run.sh --with-math 1000 5           # With math/KaTeX rendering
#   ./bench/run.sh --with-images --with-math    # Both, all tiers
#
# Comparison targets (1k pages, cold build, median):
#   Zola (Rust):     ~0.35s
#   Hugo (Go):       ~1.0s
#   Jekyll (Ruby):   ~2.4s
#   Eleventy (Node): ~2.9s

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SEITE_BIN="$PROJECT_DIR/target/release/seite"
BENCH_DIR="$PROJECT_DIR/bench"
RESULTS_FILE="$BENCH_DIR/results.txt"
TIMINGS_FILE="$BENCH_DIR/results-timings.json"

# Defaults
WITH_IMAGES=false
WITH_MATH=false
IMAGE_COUNT=10

# Parse named flags first, then positional args
POSITIONAL=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --with-images)  WITH_IMAGES=true; shift ;;
    --with-math)    WITH_MATH=true; shift ;;
    --images)       WITH_IMAGES=true; IMAGE_COUNT="$2"; shift 2 ;;
    *) POSITIONAL+=("$1"); shift ;;
  esac
done

# Configuration
TIERS="${POSITIONAL[0]:-100 500 1000 2000 5000 10000}"
RUNS="${POSITIONAL[1]:-10}"

# Lorem ipsum paragraphs for content generation
LOREM_PARAGRAPHS=(
  "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur."
  "Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris."
  "Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula ut dictum pharetra, nisi nunc fringilla magna, in commodo elit erat nec turpis. Ut pharetra auctor nunc."
  "Praesent dapibus, neque id cursus faucibus, tortor neque egestas augue, eu vulputate magna eros eu erat. Aliquam erat volutpat. Nam dui mi, tincidunt quis, accumsan porttitor, facilisis luctus, metus. Phasellus ultrices nulla quis nibh. Quisque a lectus."
  "Pellentesque habitant morbi tristique senectus et netus et malesuada fames ac turpis egestas. Proin pharetra nonummy pede. Mauris et orci. Aenean nec lorem. In porttitor. Donec laoreet nonummy augue. Suspendisse dui purus, scelerisque at, vulputate vitae, pretium mattis, nunc."
  "Mauris eget neque at sem venenatis eleifend. Ut nonummy. Fusce aliquet pede non pede. Suspendisse dapibus lorem pellentesque magna. Integer nulla. Donec blandit feugiat ligula. Donec hendrerit, felis et imperdiet euismod, purus ipsum pretium metus, in lacinia nulla nisl eget sapien."
)

TAGS=("rust" "web" "programming" "tutorial" "guide" "performance" "design" "architecture" "testing" "deployment" "security" "api" "database" "frontend" "backend" "devops" "cloud" "ai" "ml" "data")

# Utility: get N random tags (comma-separated for YAML array)
rand_tags() {
  local n=$1
  local result=""
  for ((i=0; i<n; i++)); do
    local tag="${TAGS[$((RANDOM % ${#TAGS[@]}))]}"
    if [ -z "$result" ]; then
      result="$tag"
    else
      result="$result, $tag"
    fi
  done
  echo "$result"
}

# Generate N markdown files in a posts collection
generate_content() {
  local site_dir="$1"
  local count="$2"

  mkdir -p "$site_dir/content/posts"

  for ((i=1; i<=count; i++)); do
    local year=$((2020 + (i % 5)))
    local month
    month=$(printf "%02d" $(( (i % 12) + 1 )))
    local day
    day=$(printf "%02d" $(( (i % 28) + 1 )))
    local date="${year}-${month}-${day}"
    local slug
    slug="post-$(printf "%05d" $i)"
    local tags
    tags=$(rand_tags $(( (RANDOM % 3) + 1 )))

    # Pick 3-5 paragraphs
    local num_paras=$(( (RANDOM % 3) + 3 ))
    local body=""
    for ((p=0; p<num_paras; p++)); do
      local para="${LOREM_PARAGRAPHS[$((RANDOM % ${#LOREM_PARAGRAPHS[@]}))]}"
      body="${body}${para}

"
    done

    cat > "$site_dir/content/posts/${date}-${slug}.md" <<MDEOF
---
title: "Benchmark Post ${i}: Exploring Topic $(( (i % 50) + 1 ))"
date: ${date}
description: "This is benchmark post number ${i} for performance testing of the page static site generator"
tags: [${tags}]
---

## Introduction to Post ${i}

${body}

## Technical Details

This section covers the implementation details of topic ${i}. The approach involves
several key considerations that affect overall system performance and reliability.

### Code Example

\`\`\`rust
fn benchmark_function_${i}(input: &str) -> Result<String, Error> {
    let processed = input.trim().to_lowercase();
    let result = format!("Processed: {}", processed);
    Ok(result)
}
\`\`\`

## Conclusion

Post ${i} demonstrates the core concepts discussed above. Further reading can be found
in the related documentation and source code references.
MDEOF
  done
}

# Generate test PNG images using Python (no external deps, fast bytearray approach)
generate_images() {
  local site_dir="$1" count="$2"
  mkdir -p "$site_dir/static/images"

  python3 -c "
import struct, zlib, os, sys

def create_png(width, height, r, g, b, path):
    row_data = bytearray([r, g, b] * width)
    raw = bytearray()
    for y in range(height):
        raw.append(0)
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
    (180, 80, 120), (60, 150, 60),
]

for i in range(int(sys.argv[1])):
    r, g, b = colors[i % len(colors)]
    create_png(1200, 800, r, g, b, os.path.join(sys.argv[2], f'image-{i+1:03d}.png'))
" "$count" "$site_dir/static/images"
}

# Create a benchmark site using `seite init` for scaffolding
create_bench_site() {
  local site_dir="$1"
  local count="$2"
  local parent_dir
  parent_dir="$(dirname "$site_dir")"
  local site_name
  site_name="$(basename "$site_dir")"

  rm -rf "$site_dir"
  mkdir -p "$parent_dir"

  # Scaffold with seite init
  cd "$parent_dir"
  "$SEITE_BIN" init "$site_name" \
    --title "Benchmark Site" \
    --description "Performance benchmark" \
    --deploy-target github-pages \
    --collections posts >/dev/null 2>&1
  cd "$PROJECT_DIR"

  # Remove the sample post that seite init creates
  rm -f "$site_dir"/content/posts/*.md

  # Generate benchmark content
  generate_content "$site_dir" "$count"

  # Optional: generate images (seite init already creates [images] section)
  if [ "$WITH_IMAGES" = true ]; then
    generate_images "$site_dir" "$IMAGE_COUNT"
    # Reference images from some posts
    local img_posts=$((count < IMAGE_COUNT * 3 ? count : IMAGE_COUNT * 3))
    for ((i=1; i<=img_posts && i<=count; i++)); do
      local img_num=$(( (i % IMAGE_COUNT) + 1 ))
      local img_name
      img_name=$(printf "image-%03d.png" "$img_num")
      local post_file
      post_file=$(find "$site_dir/content/posts/" -maxdepth 1 -name "*-post-$(printf "%05d" "$i").md" -print -quit 2>/dev/null)
      if [ -n "$post_file" ]; then
        echo "" >> "$post_file"
        echo "![Test image ${img_num}](/images/${img_name})" >> "$post_file"
      fi
    done
  fi

  # Optional: enable math rendering — insert after output_dir in [build] section
  if [ "$WITH_MATH" = true ]; then
    sed -i '' '/^output_dir/a\
math = true
' "$site_dir/seite.toml"
  fi
}

# Run a single build, return time in seconds (fractional)
# If CAPTURE_TIMINGS_FILE is set, appends step timings to that file
time_build() {
  local site_dir="$1"

  # Clean output
  rm -rf "$site_dir/dist"

  # Time the build — capture both stdout (timings) and stderr (errors)
  local start end elapsed outfile errfile
  outfile=$(mktemp)
  errfile=$(mktemp)
  start=$(python3 -c 'import time; print(f"{time.time():.6f}")')
  "$SEITE_BIN" build >"$outfile" 2>"$errfile" || {
    echo "BUILD FAILED:" >&2
    cat "$errfile" >&2
    rm -f "$outfile" "$errfile"
    echo "ERROR"
    return 1
  }
  end=$(python3 -c 'import time; print(f"{time.time():.6f}")')

  # Extract step timings from build output if requested
  if [ -n "${CAPTURE_TIMINGS_FILE:-}" ]; then
    grep -E '^\s{4}\S' "$outfile" >> "$CAPTURE_TIMINGS_FILE" 2>/dev/null || true
    echo "---" >> "$CAPTURE_TIMINGS_FILE"
  fi

  rm -f "$outfile" "$errfile"

  elapsed=$(python3 -c "print(f'{$end - $start:.3f}')")
  echo "$elapsed"
}

# Convert captured step timings to JSON
# Input: file with lines like "    Step Name: 123.4ms" separated by "---"
save_timings_json() {
  local timings_file="$1" tier="$2" output_file="$3"
  python3 -c "
import json, sys, re

timings_file = sys.argv[1]
tier = int(sys.argv[2])
output_file = sys.argv[3]

# Read existing data
try:
    with open(output_file) as f:
        data = json.load(f)
except (FileNotFoundError, json.JSONDecodeError):
    data = []

# Parse step timings (last run only — after the last '---' separator)
runs = []
current_run = {}
with open(timings_file) as f:
    for line in f:
        line = line.strip()
        if line == '---':
            if current_run:
                runs.append(current_run)
            current_run = {}
            continue
        m = re.match(r'^(.+?):\s+(?:([0-9.]+)ms|<1ms)$', line)
        if m:
            name = m.group(1)
            ms = float(m.group(2)) if m.group(2) else 0.5
            current_run[name] = ms
    if current_run:
        runs.append(current_run)

# Use last run's timings as representative
if runs:
    data.append({'pages': tier, 'step_timings': runs[-1]})

with open(output_file, 'w') as f:
    json.dump(data, f, indent=2)
" "$timings_file" "$tier" "$output_file"
}

# Calculate median from a file of numbers
calc_median() {
  sort -n | awk '{a[NR]=$1} END {if(NR%2==1) print a[(NR+1)/2]; else print (a[NR/2]+a[NR/2+1])/2}'
}

calc_mean() {
  awk '{s+=$1} END {printf "%.3f\n", s/NR}'
}

calc_min() {
  sort -n | head -1
}

calc_max() {
  sort -n | tail -1
}

# Print header
print_header() {
  echo "═══════════════════════════════════════════════════════════════"
  echo "  seite SSG Performance Benchmark"
  echo "═══════════════════════════════════════════════════════════════"
  echo ""
  if [ "$(uname)" = "Darwin" ]; then
    echo "  Hardware: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo 'unknown')"
    echo "  RAM:      $(sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.0f GB", $1/1024/1024/1024}' || echo 'unknown')"
    echo "  OS:       $(sw_vers -productName 2>/dev/null) $(sw_vers -productVersion 2>/dev/null)"
  else
    echo "  Hardware: $(lscpu 2>/dev/null | grep 'Model name' | sed 's/.*: *//' || echo 'unknown')"
    echo "  RAM:      $(free -h 2>/dev/null | awk '/Mem:/ {print $2}' || echo 'unknown')"
    echo "  OS:       $(uname -sr)"
  fi
  echo "  Binary:   $SEITE_BIN"
  echo "  Runs:     $RUNS per tier"
  echo "  Images:   $([ "$WITH_IMAGES" = true ] && echo "${IMAGE_COUNT} (WebP enabled)" || echo "none")"
  echo "  Math:     $([ "$WITH_MATH" = true ] && echo "enabled" || echo "disabled")"
  echo "  Date:     $(date '+%Y-%m-%d %H:%M:%S')"
  echo ""
  echo "  Reference (1k pages, cold build, median):"
  echo "    Zola (Rust):     ~0.35s"
  echo "    Hugo (Go):       ~1.0s"
  echo "    Jekyll (Ruby):   ~2.4s"
  echo "    Eleventy (Node): ~2.9s"
  echo ""
  echo "═══════════════════════════════════════════════════════════════"
}

# Main
main() {
  if [ ! -f "$SEITE_BIN" ]; then
    echo "ERROR: Release binary not found at $SEITE_BIN"
    echo "Run: cargo build --release"
    exit 1
  fi

  mkdir -p "$BENCH_DIR"

  # Initialize step timings JSON
  echo '[]' > "$TIMINGS_FILE"

  print_header | tee "$RESULTS_FILE"

  for tier in $TIERS; do
    echo "" | tee -a "$RESULTS_FILE"
    echo "───────────────────────────────────────────────────────────" | tee -a "$RESULTS_FILE"
    echo "  Generating ${tier} pages..." | tee -a "$RESULTS_FILE"

    local site_dir="$BENCH_DIR/site-${tier}"
    create_bench_site "$site_dir" "$tier"

    local content_size
    content_size=$(du -sh "$site_dir/content" | awk '{print $1}')
    echo "  Content size: ${content_size}" | tee -a "$RESULTS_FILE"
    if [ "$WITH_IMAGES" = true ] && [ -d "$site_dir/static" ]; then
      local static_size
      static_size=$(du -sh "$site_dir/static" | awk '{print $1}')
      echo "  Static size: ${static_size}" | tee -a "$RESULTS_FILE"
    fi
    echo "  Running ${RUNS} cold builds..." | tee -a "$RESULTS_FILE"

    # Capture step timings from builds
    local step_timings_capture
    step_timings_capture=$(mktemp)
    export CAPTURE_TIMINGS_FILE="$step_timings_capture"

    # Verify first build works
    cd "$site_dir"
    local first_build
    first_build=$(time_build "$site_dir")
    if [ "$first_build" = "ERROR" ]; then
      echo "  SKIPPING — build failed for ${tier} pages" | tee -a "$RESULTS_FILE"
      unset CAPTURE_TIMINGS_FILE
      rm -f "$step_timings_capture"
      cd "$PROJECT_DIR"
      continue
    fi

    local times_file
    times_file=$(mktemp)
    echo "$first_build" >> "$times_file"
    printf "    Run %2d: %ss\n" "1" "$first_build" | tee -a "$RESULTS_FILE"

    for ((r=2; r<=RUNS; r++)); do
      local t
      t=$(time_build "$site_dir")
      echo "$t" >> "$times_file"
      printf "    Run %2d: %ss\n" "$r" "$t" | tee -a "$RESULTS_FILE"
    done

    unset CAPTURE_TIMINGS_FILE

    # Save step timings from last run to JSON
    save_timings_json "$step_timings_capture" "$tier" "$TIMINGS_FILE"
    rm -f "$step_timings_capture"

    cd "$PROJECT_DIR"

    local median mean min max
    median=$(calc_median < "$times_file")
    mean=$(calc_mean < "$times_file")
    min=$(calc_min < "$times_file")
    max=$(calc_max < "$times_file")

    # Calculate pages/sec
    local pages_per_sec
    pages_per_sec=$(python3 -c "print(f'{$tier / $median:.0f}')")

    echo "" | tee -a "$RESULTS_FILE"
    echo "  ┌─────────────────────────────────────┐" | tee -a "$RESULTS_FILE"
    printf "  │ %5d pages  │  median: %7ss    │\n" "$tier" "$median" | tee -a "$RESULTS_FILE"
    printf "  │              │  mean:   %7ss    │\n" "$mean" | tee -a "$RESULTS_FILE"
    printf "  │              │  min:    %7ss    │\n" "$min" | tee -a "$RESULTS_FILE"
    printf "  │              │  max:    %7ss    │\n" "$max" | tee -a "$RESULTS_FILE"
    printf "  │              │  pages/s: %6s     │\n" "$pages_per_sec" | tee -a "$RESULTS_FILE"
    echo "  └─────────────────────────────────────┘" | tee -a "$RESULTS_FILE"

    # Check output from last run (dist still exists)
    local output_files=0
    if [ -d "$site_dir/dist" ]; then
      output_files=$(find "$site_dir/dist" -name "*.html" | wc -l | tr -d ' ')
    fi
    echo "  Output: ${output_files} HTML files" | tee -a "$RESULTS_FILE"

    rm -f "$times_file"
  done

  echo "" | tee -a "$RESULTS_FILE"
  echo "═══════════════════════════════════════════════════════════════" | tee -a "$RESULTS_FILE"
  echo "  Benchmark complete." | tee -a "$RESULTS_FILE"
  echo "  Results:  $RESULTS_FILE" | tee -a "$RESULTS_FILE"
  echo "  Timings:  $TIMINGS_FILE" | tee -a "$RESULTS_FILE"
  echo "═══════════════════════════════════════════════════════════════" | tee -a "$RESULTS_FILE"

  # Cleanup generated sites
  for tier in $TIERS; do
    rm -rf "$BENCH_DIR/site-${tier}"
  done
}

main
