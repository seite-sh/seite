#!/usr/bin/env bash
# Refresh the pinned SEOMachine release that `seite skill install seomachine`
# downloads (src/cli/seomachine.pin). Run by a maintainer before a seite
# release, review the diff, and commit it:
#
#   scripts/refresh-seomachine-pin.sh [<commit-sha>]   # default: head of main
#
# The pin file lists one `<sha256>  <path>` line per downloaded file. The
# script keeps the path list (add a new file by appending a line with just its
# path; drop a file by deleting its line), resolves the commit, downloads each
# file from raw.githubusercontent.com at that exact SHA, and rewrites the
# hashes. Needs gh, curl, and shasum or sha256sum.
set -euo pipefail

REPO="TheCraigHewitt/seomachine"
PIN="$(cd "$(dirname "$0")/.." && pwd)/src/cli/seomachine.pin"
REQ="${1:-}"

if [ -z "$REQ" ]; then
  REQ="$(gh api "repos/$REPO/commits/main" --jq .sha)"
fi
SHA="$(gh api "repos/$REPO/commits/$REQ" --jq .sha)"
DATE="$(gh api "repos/$REPO/commits/$SHA" --jq .commit.committer.date)"
LICENSE="$(gh api "repos/$REPO/license" --jq .license.spdx_id)"

if command -v sha256sum >/dev/null 2>&1; then
  HASH() { sha256sum | cut -d' ' -f1; }
else
  HASH() { shasum -a 256 | cut -d' ' -f1; }
fi

PATHS="$(awk '!/^#/ && NF { print $NF }' "$PIN")"
OUT="$(mktemp)"
{
  echo "# repo $REPO"
  echo "# commit $SHA"
  echo "# date $DATE"
  echo "# licence $LICENSE"
  echo "# Refreshed by scripts/refresh-seomachine-pin.sh. Format: <sha256>  <path>."
  for p in $PATHS; do
    h="$(curl -fsSL "https://raw.githubusercontent.com/$REPO/$SHA/$p" | HASH)"
    echo "$h  $p"
  done
} > "$OUT"
mv "$OUT" "$PIN"
echo "pinned $REPO at $SHA ($DATE), $(echo "$PATHS" | wc -l | tr -d ' ') files, licence $LICENSE"
