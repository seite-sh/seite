#!/usr/bin/env bash
# Release detection for .github/workflows/deploy-docs.yml.
#
# Usage:
#   scripts/pkg-version.sh < Cargo.toml
#       Print the `version = ...` line of the [package] table (a dependency
#       table's `version =` line is not the package version). Prints nothing
#       when there is none (or stdin is empty).
#   scripts/pkg-version.sh changed OLD NEW
#       Print `true` when OLD and NEW (lines printed by the form above) are
#       both non-empty and differ, else `false`. An empty side (e.g. the
#       previous commit couldn't be fetched after a force-push) is never a
#       release.
#
# Tested by tests/pkg-version.sh.

set -eu

if [ "${1:-}" = "changed" ]; then
  if [ "$#" -ne 3 ]; then
    echo "usage: $0 changed OLD NEW" >&2
    exit 2
  fi
  if [ -n "$2" ] && [ -n "$3" ] && [ "$2" != "$3" ]; then
    echo true
  else
    echo false
  fi
  exit 0
fi

awk '/^\[/{p=($0=="[package]")} p&&/^version *=/{print; exit}'
