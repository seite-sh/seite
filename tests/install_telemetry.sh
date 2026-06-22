#!/bin/sh
set -eu
# Source install.sh without running main(), then verify telemetry honors opt-out.
INSTALL_SH_SKIP_MAIN=1 . ./install.sh

# Required vars for send_telemetry (normally set by detect_platform/resolve_version).
VERSION_TAG="v0.0.0-test"; OS="TestOS"; ARCH="testarch"

fail() { echo "FAIL: $1" >&2; exit 1; }

# 1) DO_NOT_TRACK disables: no HTTP tool should be invoked. We stub curl/wget on PATH.
STUB=$(mktemp -d)
printf '#!/bin/sh\necho called >> "%s/hits"\n' "$STUB" > "$STUB/curl"
cp "$STUB/curl" "$STUB/wget"; chmod +x "$STUB/curl" "$STUB/wget"
: > "$STUB/hits"
PATH="$STUB:$PATH" DO_NOT_TRACK=1 send_telemetry
[ ! -s "$STUB/hits" ] || fail "DO_NOT_TRACK should suppress the ping"
unset DO_NOT_TRACK

# 2) SEITE_TELEMETRY=0 disables.
: > "$STUB/hits"
PATH="$STUB:$PATH" SEITE_TELEMETRY=0 send_telemetry
[ ! -s "$STUB/hits" ] || fail "SEITE_TELEMETRY=0 should suppress the ping"
unset SEITE_TELEMETRY

# 3) Default enabled: the stubbed curl is invoked.
: > "$STUB/hits"
PATH="$STUB:$PATH" send_telemetry
[ -s "$STUB/hits" ] || fail "default should invoke the HTTP tool"

rm -rf "$STUB"
echo "PASS"
