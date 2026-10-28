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

OUT="$(mktemp)"
trap 'rm -f "$OUT"' EXIT

if [ -z "$REQ" ]; then
  REQ="$(gh api "repos/$REPO/commits/main" --jq .sha)"
fi
SHA="$(gh api "repos/$REPO/commits/$REQ" --jq .sha)"
DATE="$(gh api "repos/$REPO/commits/$SHA" --jq .commit.committer.date)"
LICENSE="$(gh api "repos/$REPO/license" --jq .license.spdx_id)"

if command -v sha256sum >/dev/null 2>&1; then
  hash_stdin() { sha256sum | cut -d' ' -f1; }
else
  hash_stdin() { shasum -a 256 | cut -d' ' -f1; }
fi

{
  echo "# repo $REPO"
  echo "# commit $SHA"
  echo "# date $DATE"
  echo "# licence $LICENSE"
  echo "# Refreshed by scripts/refresh-seomachine-pin.sh. Format: <sha256>  <path>."
} > "$OUT"

COUNT=0
while read -r path; do
  # Pin paths are repo-relative: no absolute paths, no dot segments, no
  # characters beyond a conservative set (the Rust parser rejects the rest).
  case "$path" in
    /* | *..* | *//* | *[!A-Za-z0-9._/-]*)
      echo "refusing unsafe path in pin: $path" >&2
      exit 1
      ;;
  esac
  h="$(curl -fsSL "https://raw.githubusercontent.com/$REPO/$SHA/$path" | hash_stdin)"
  echo "$h  $path" >> "$OUT"
  COUNT=$((COUNT + 1))
done < <(awk '!/^#/ && NF { print $NF }' "$PIN")

if [ "$COUNT" -eq 0 ]; then
  echo "pin file lists no paths" >&2
  exit 1
fi

# Only now touch the pin: a failure above leaves it unchanged.
cat "$OUT" > "$PIN"
echo "pinned $REPO at $SHA ($DATE), $COUNT files, licence $LICENSE"
