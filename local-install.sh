#!/bin/sh
# local-install.sh — Build and install page from source
#
# Usage:
#   ./local-install.sh
#
# Options (via environment variables):
#   INSTALL_DIR   Override install location (default: ~/.local/bin)

set -eu

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
BINARY="page"

cargo build --release

mkdir -p "$INSTALL_DIR"
cp "target/release/${BINARY}" "${INSTALL_DIR}/${BINARY}"
chmod +x "${INSTALL_DIR}/${BINARY}"

echo "Installed: ${INSTALL_DIR}/${BINARY}"
"${INSTALL_DIR}/${BINARY}" --version
