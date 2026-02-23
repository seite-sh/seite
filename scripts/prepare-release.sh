#!/usr/bin/env bash
# prepare-release.sh — Scaffold a changelog entry for the current Cargo.toml
# version and regenerate releases.md.
#
# Usage:
#   scripts/prepare-release.sh                 # Create entry for current version
#   scripts/prepare-release.sh --dry-run       # Preview without writing
#   scripts/prepare-release.sh --version 0.2.0 # Override version
#   scripts/prepare-release.sh --date 2026-03-01 # Override date

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

CHANGELOG_DIR="$PROJECT_DIR/seite-sh/content/changelog"
GENERATE_SCRIPT="$SCRIPT_DIR/generate-release-docs.sh"

VERSION=""
DATE=""
DRY_RUN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)   VERSION="$2"; shift 2 ;;
    --date)      DATE="$2"; shift 2 ;;
    --dry-run)   DRY_RUN=true; shift ;;
    -h|--help)
      echo "Usage: $0 [--version <ver>] [--date <YYYY-MM-DD>] [--dry-run]"
      echo ""
      echo "  (no flags)         Scaffold changelog for Cargo.toml version"
      echo "  --version <ver>    Override version (e.g., 0.2.0)"
      echo "  --date <date>      Override date (default: today)"
      echo "  --dry-run          Print what would be created without writing"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

# Resolve version from Cargo.toml if not overridden.
if [ -z "$VERSION" ]; then
  VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
fi

# Resolve date.
if [ -z "$DATE" ]; then
  DATE=$(date '+%Y-%m-%d')
fi

# Convert version to filename slug: 0.1.7 -> 0-1-7
VERSION_SLUG=$(echo "$VERSION" | tr '.' '-')

# Check if an entry already exists for this version.
EXISTING=""
for f in "$CHANGELOG_DIR"/*-v"${VERSION_SLUG}".md; do
  if [ -f "$f" ]; then
    EXISTING="$f"
    break
  fi
done

if [ -n "$EXISTING" ]; then
  echo "Changelog entry already exists: $EXISTING"
  echo "Regenerating releases.md..."
  if [ "$DRY_RUN" = true ]; then
    echo "(dry-run: would run generate-release-docs.sh)"
  else
    bash "$GENERATE_SCRIPT"
  fi
  exit 0
fi

FILENAME="${DATE}-v${VERSION_SLUG}.md"
FILEPATH="$CHANGELOG_DIR/$FILENAME"

# Gather git log since last tag for reference.
GIT_LOG=""
LAST_TAG=$(git -C "$PROJECT_DIR" describe --tags --abbrev=0 2>/dev/null || echo "")
if [ -n "$LAST_TAG" ]; then
  GIT_LOG=$(git -C "$PROJECT_DIR" log --oneline "${LAST_TAG}..HEAD" 2>/dev/null || echo "")
else
  GIT_LOG=$(git -C "$PROJECT_DIR" log --oneline -20 2>/dev/null || echo "")
fi

# Build the template content.
TEMPLATE="---
title: v${VERSION}
description: \"\"
date: ${DATE}
tags:
- new
---"

# Add git log as a reference comment if available.
if [ -n "$GIT_LOG" ]; then
  TEMPLATE="${TEMPLATE}

<!--
Git log since last release (for reference — delete before committing):

${GIT_LOG}
-->"
fi

TEMPLATE="${TEMPLATE}

## Build Pipeline

-

## Content

-

## Themes

-

## AI Integration

-

## Deploy

-

## Developer Experience

-
"

if [ "$DRY_RUN" = true ]; then
  echo "Would create: $FILEPATH"
  echo ""
  echo "--- content ---"
  echo "$TEMPLATE"
  echo "--- end ---"
  echo ""
  echo "(dry-run: would also run generate-release-docs.sh)"
else
  mkdir -p "$CHANGELOG_DIR"
  echo "$TEMPLATE" > "$FILEPATH"
  echo "Created: $FILEPATH"
  echo "Regenerating releases.md..."
  bash "$GENERATE_SCRIPT"
fi
