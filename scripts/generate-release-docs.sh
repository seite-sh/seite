#!/usr/bin/env bash
# generate-release-docs.sh — Generate seite-sh/content/docs/releases.md from
# changelog entries in seite-sh/content/changelog/.
#
# Usage:
#   scripts/generate-release-docs.sh            # Write releases.md
#   scripts/generate-release-docs.sh --stdout    # Print to stdout
#   scripts/generate-release-docs.sh --check     # Exit 1 if releases.md is stale

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

CHANGELOG_DIR="$PROJECT_DIR/seite-sh/content/changelog"
OUTPUT_FILE="$PROJECT_DIR/seite-sh/content/docs/releases.md"

MODE="write"   # write | stdout | check

while [[ $# -gt 0 ]]; do
  case "$1" in
    --stdout)  MODE="stdout"; shift ;;
    --check)   MODE="check";  shift ;;
    -h|--help)
      echo "Usage: $0 [--stdout | --check]"
      echo ""
      echo "  (no flags)  Write releases.md"
      echo "  --stdout    Print generated content to stdout"
      echo "  --check     Exit 1 if releases.md is out of sync"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

# --- helpers ---

# Extract title from YAML frontmatter.
get_title() {
  local file="$1"
  awk '
    BEGIN { count=0 }
    /^---$/ { count++; if (count==2) exit; next }
    count==1 && /^title:/ {
      sub(/^title:[[:space:]]*/, "")
      gsub(/^"|"$/, "")
      print
      exit
    }
  ' "$file"
}

# Extract body: everything after the second "---" line.
get_body() {
  local file="$1"
  awk '
    BEGIN { count=0; found=0 }
    /^---$/ { count++; if (count==2) { found=1; next } }
    found { print }
  ' "$file"
}

# --- main ---

# Static frontmatter for releases.md
HEADER='---
title: "Releases"
description: "Release history and changelog for seite."
weight: 12
---'

generate() {
  echo "$HEADER"

  if [ ! -d "$CHANGELOG_DIR" ]; then
    echo ""
    echo "No releases documented yet."
    return
  fi

  # Collect changelog files, sorted reverse-alphabetically (newest first).
  mapfile -t files < <(
    find "$CHANGELOG_DIR" -maxdepth 1 -name '*.md' -printf '%f\n' \
      | sort -r \
      | while read -r name; do echo "$CHANGELOG_DIR/$name"; done
  )

  if [ "${#files[@]}" -eq 0 ]; then
    echo ""
    echo "No releases documented yet."
    return
  fi

  for file in "${files[@]}"; do
    title="$(get_title "$file")"
    if [ -z "$title" ]; then
      # Derive title from filename: 2026-02-20-v0-1-0.md -> v0.1.0
      basename_no_ext="$(basename "$file" .md)"
      # Strip date prefix (YYYY-MM-DD-)
      version_slug="${basename_no_ext#????-??-??-}"
      # Convert v0-1-0 -> v0.1.0 (replace digit-hyphen with digit-dot)
      title="$version_slug"
      while [[ "$title" =~ ([0-9])- ]]; do
        title="${title/"${BASH_REMATCH[0]}"/"${BASH_REMATCH[1]}."}"
      done
    fi

    body="$(get_body "$file")"

    echo ""
    echo "## $title"
    # Trim leading blank lines from body, preserve the rest
    echo "$body" | sed '/./,$!d'
  done
}

case "$MODE" in
  write)
    generate > "$OUTPUT_FILE"
    echo "Generated $OUTPUT_FILE"
    ;;
  stdout)
    generate
    ;;
  check)
    generated="$(generate)"
    if [ ! -f "$OUTPUT_FILE" ]; then
      echo "releases.md does not exist. Run: scripts/generate-release-docs.sh" >&2
      exit 1
    fi
    committed="$(< "$OUTPUT_FILE")"
    if [ "$generated" != "$committed" ]; then
      echo "releases.md is out of sync with changelog entries." >&2
      echo "Run: scripts/generate-release-docs.sh" >&2
      diff --unified <(echo "$committed") <(echo "$generated") >&2 || true
      exit 1
    fi
    echo "releases.md is up to date."
    ;;
esac
