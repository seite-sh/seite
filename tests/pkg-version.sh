#!/usr/bin/env bash
# Tests for scripts/pkg-version.sh (deploy-docs.yml release detection).
#
# Usage:
#   bash tests/pkg-version.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PKG_VERSION="$PROJECT_DIR/scripts/pkg-version.sh"

PASSED=0
FAILED=0

pass() { PASSED=$((PASSED + 1)); echo "  PASS: $1"; }
fail() { FAILED=$((FAILED + 1)); echo "  FAIL: $1 — $2"; }

# expect NAME EXPECTED ACTUAL
expect() {
  if [ "$3" = "$2" ]; then
    pass "$1"
  else
    fail "$1" "expected '$2', got '$3'"
  fi
}

version_of() { printf '%s' "$1" | bash "$PKG_VERSION"; }
is_release() { bash "$PKG_VERSION" changed "$(version_of "$1")" "$(version_of "$2")"; }

BASE='[package]
name = "seite"
version = "0.21.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }

[dependencies.toml]
version = "0.8"
'

echo "=== pkg-version.sh tests ==="
echo ""

echo "Version line:"
expect "[package] version" 'version = "0.21.0"' "$(version_of "$BASE")"
expect "[workspace] before [package]" 'version = "1.2.3"' \
  "$(version_of '[workspace]
members = ["a"]

[workspace.package]
version = "9.9.9"

[package]
name = "x"
version = "1.2.3"
')"
expect "no [package] table" '' "$(version_of '[dependencies.toml]
version = "0.8"
')"
expect "empty input" '' "$(version_of '')"
REAL="$(bash "$PKG_VERSION" < "$PROJECT_DIR/Cargo.toml")"
case "$REAL" in
  'version = "'*'"') pass "this repository's Cargo.toml: $REAL" ;;
  *) fail "this repository's Cargo.toml" "got '$REAL'" ;;
esac

echo ""
echo "Release detection:"
DEP_BUMP="${BASE/version = \"0.8\"/version = \"0.9\"}"
expect "dependency table version change is not a release" false "$(is_release "$BASE" "$DEP_BUMP")"
INLINE_DEP_BUMP="${BASE/version = \"1\"/version = \"2\"}"
expect "inline dependency version change is not a release" false "$(is_release "$BASE" "$INLINE_DEP_BUMP")"
DEP_FIRST='[dependencies.toml]
version = "0.8"

[package]
name = "seite"
version = "0.21.0"
'
expect "dependency table before [package]: its version change is not a release" false \
  "$(is_release "$DEP_FIRST" "${DEP_FIRST/version = \"0.8\"/version = \"0.9\"}")"
PKG_BUMP="${BASE/version = \"0.21.0\"/version = \"0.21.1\"}"
expect "[package] version change is a release" true "$(is_release "$BASE" "$PKG_BUMP")"
expect "no change is not a release" false "$(is_release "$BASE" "$BASE")"
expect "missing previous Cargo.toml is not a release" false "$(is_release '' "$PKG_BUMP")"
expect "missing current Cargo.toml is not a release" false "$(is_release "$BASE" '')"
expect "both missing is not a release" false "$(bash "$PKG_VERSION" changed '' '')"
if bash "$PKG_VERSION" changed only-one >/dev/null 2>&1; then
  fail "changed with one argument" "exited 0"
else
  pass "changed with one argument is a usage error"
fi

echo ""
echo "=== Results: $PASSED passed, $FAILED failed ==="

if [ "$FAILED" -gt 0 ]; then
  exit 1
fi
