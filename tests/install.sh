#!/usr/bin/env bash
# Tests for install.sh — validates platform detection, checksum, and version logic
#
# Usage:
#   bash tests/install.sh
#
# These tests source individual functions from install.sh without running main().

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
INSTALL_SCRIPT="$PROJECT_DIR/install.sh"

PASSED=0
FAILED=0

pass() { PASSED=$((PASSED + 1)); echo "  PASS: $1"; }
fail() { FAILED=$((FAILED + 1)); echo "  FAIL: $1 — $2"; }

# Source install.sh without executing main()
export INSTALL_SH_SKIP_MAIN=1
# shellcheck source=../install.sh
. "$INSTALL_SCRIPT"

# Disable color output in tests (used by sourced install.sh functions)
export BOLD='' GREEN='' YELLOW='' RED='' RESET=''

echo "=== install.sh tests ==="
echo ""

# --- Platform detection ---
echo "Platform detection:"

detect_platform
OS_ACTUAL=$(uname -s)
ARCH_ACTUAL=$(uname -m)

case "$OS_ACTUAL" in
  Darwin) EXPECTED_OS="apple-darwin" ;;
  Linux)  EXPECTED_OS="unknown-linux-gnu" ;;
  *)      EXPECTED_OS="unsupported" ;;
esac

case "$ARCH_ACTUAL" in
  x86_64|amd64)  EXPECTED_ARCH="x86_64" ;;
  aarch64|arm64) EXPECTED_ARCH="aarch64" ;;
  *)             EXPECTED_ARCH="unsupported" ;;
esac

if [ "$OS_TRIPLE" = "$EXPECTED_OS" ]; then
  pass "OS detection: $OS_TRIPLE"
else
  fail "OS detection" "expected $EXPECTED_OS, got $OS_TRIPLE"
fi

if [ "$ARCH_TRIPLE" = "$EXPECTED_ARCH" ]; then
  pass "ARCH detection: $ARCH_TRIPLE"
else
  fail "ARCH detection" "expected $EXPECTED_ARCH, got $ARCH_TRIPLE"
fi

if [ "$TARGET" = "${EXPECTED_ARCH}-${EXPECTED_OS}" ]; then
  pass "TARGET triple: $TARGET"
else
  fail "TARGET triple" "expected ${EXPECTED_ARCH}-${EXPECTED_OS}, got $TARGET"
fi

echo ""

# --- Checksum verification ---
echo "Checksum verification:"

TMPFILE=$(mktemp)
echo "hello world" > "$TMPFILE"
EXPECTED_HASH="a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447"
ACTUAL_HASH=$(compute_sha256 "$TMPFILE")

if [ "$ACTUAL_HASH" = "$EXPECTED_HASH" ]; then
  pass "SHA256 matches expected hash"
else
  fail "SHA256" "expected $EXPECTED_HASH, got $ACTUAL_HASH"
fi

# Verify same file gives same hash
ACTUAL_HASH2=$(compute_sha256 "$TMPFILE")
if [ "$ACTUAL_HASH" = "$ACTUAL_HASH2" ]; then
  pass "SHA256 is deterministic"
else
  fail "SHA256 determinism" "two runs gave different hashes"
fi

# Verify different content gives different hash
echo "different content" > "$TMPFILE"
ACTUAL_HASH3=$(compute_sha256 "$TMPFILE")
if [ "$ACTUAL_HASH" != "$ACTUAL_HASH3" ]; then
  pass "SHA256 differs for different content"
else
  fail "SHA256 collision" "different content gave same hash"
fi

rm -f "$TMPFILE"

echo ""

# --- Version resolution ---
echo "Version resolution:"

# Pinned version with v prefix (VERSION is read by resolve_version from install.sh)
export VERSION="v1.2.3"
RESOLVED=$(resolve_version 2>/dev/null)
if [ "$RESOLVED" = "v1.2.3" ]; then
  pass "Pinned version with v prefix: $RESOLVED"
else
  fail "Pinned version v prefix" "expected v1.2.3, got $RESOLVED"
fi

# Pinned version without v prefix
export VERSION="1.2.3"
RESOLVED=$(resolve_version 2>/dev/null)
if [ "$RESOLVED" = "v1.2.3" ]; then
  pass "Pinned version adds v prefix: $RESOLVED"
else
  fail "Pinned version auto-prefix" "expected v1.2.3, got $RESOLVED"
fi

unset VERSION

echo ""

# --- Summary ---
echo "=== Results: $PASSED passed, $FAILED failed ==="

if [ "$FAILED" -gt 0 ]; then
  exit 1
fi
