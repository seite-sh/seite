#!/bin/sh
# install.sh — Install the page static site generator
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/sanchezomar/page/main/install.sh | sh
#
# Works on macOS, Linux, and WSL (Windows Subsystem for Linux).
# For native Windows, use install.ps1 instead:
#   irm https://raw.githubusercontent.com/sanchezomar/page/main/install.ps1 | iex
#
# Options (via environment variables):
#   VERSION     Pin to a specific release (e.g., VERSION=v0.1.0)
#   INSTALL_DIR Override install location (default: ~/.local/bin)

set -eu

REPO="sanchezomar/page"
BINARY="page"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# --- Colors (only when stdout is a terminal) ---

if [ -t 1 ]; then
  BOLD='\033[1m'
  GREEN='\033[0;32m'
  YELLOW='\033[0;33m'
  RED='\033[0;31m'
  RESET='\033[0m'
else
  BOLD=''
  GREEN=''
  YELLOW=''
  RED=''
  RESET=''
fi

info()  { printf "${BOLD}${GREEN}info${RESET}  %s\n" "$1"; }
warn()  { printf "${BOLD}${YELLOW}warn${RESET}  %s\n" "$1"; }
error() { printf "${BOLD}${RED}error${RESET} %s\n" "$1" >&2; }

# --- Platform detection ---

detect_platform() {
  OS=$(uname -s)
  ARCH=$(uname -m)

  case "$OS" in
    Darwin) OS_TRIPLE="apple-darwin" ;;
    Linux)  OS_TRIPLE="unknown-linux-gnu" ;;
    *)
      error "Unsupported operating system: $OS"
      echo "  Install from source instead: cargo install page"
      exit 1
      ;;
  esac

  case "$ARCH" in
    x86_64|amd64)    ARCH_TRIPLE="x86_64" ;;
    aarch64|arm64)   ARCH_TRIPLE="aarch64" ;;
    *)
      error "Unsupported architecture: $ARCH"
      echo "  Install from source instead: cargo install page"
      exit 1
      ;;
  esac

  TARGET="${ARCH_TRIPLE}-${OS_TRIPLE}"
}

# --- Download helper (curl preferred, wget fallback) ---

download() {
  URL="$1"
  OUTPUT="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$URL" -o "$OUTPUT"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$OUTPUT" "$URL"
  else
    error "Neither curl nor wget found. Install one and try again."
    exit 1
  fi
}

# --- Checksum helper ---

compute_sha256() {
  FILE="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$FILE" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$FILE" | cut -d' ' -f1
  else
    warn "No SHA256 tool found — skipping checksum verification"
    echo ""
  fi
}

# --- Resolve version ---

resolve_version() {
  if [ -n "${VERSION:-}" ]; then
    # Ensure the version starts with 'v'
    case "$VERSION" in
      v*) ;;
      *)  VERSION="v$VERSION" ;;
    esac
    echo "$VERSION"
    return
  fi

  info "Fetching latest release..."
  LATEST_URL="https://api.github.com/repos/${REPO}/releases/latest"

  TMPFILE=$(mktemp)
  download "$LATEST_URL" "$TMPFILE"
  TAG=$(grep '"tag_name"' "$TMPFILE" | head -1 | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
  rm -f "$TMPFILE"

  if [ -z "$TAG" ]; then
    error "Could not determine latest release version."
    echo "  Try pinning a version: VERSION=v0.1.0 curl -fsSL ... | sh"
    exit 1
  fi

  echo "$TAG"
}

# --- Main ---

main() {
  detect_platform

  VERSION_TAG=$(resolve_version)
  ARCHIVE="page-${TARGET}.tar.gz"
  DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION_TAG}/${ARCHIVE}"
  CHECKSUMS_URL="https://github.com/${REPO}/releases/download/${VERSION_TAG}/checksums-sha256.txt"

  info "Installing page ${VERSION_TAG} for ${TARGET}"

  # Create temp directory with cleanup trap
  TMPDIR=$(mktemp -d)
  trap 'rm -rf "$TMPDIR"' EXIT

  # Download archive and checksums
  info "Downloading ${ARCHIVE}..."
  download "$DOWNLOAD_URL" "${TMPDIR}/${ARCHIVE}"
  download "$CHECKSUMS_URL" "${TMPDIR}/checksums-sha256.txt"

  # Verify checksum
  ACTUAL=$(compute_sha256 "${TMPDIR}/${ARCHIVE}")
  if [ -n "$ACTUAL" ]; then
    EXPECTED=$(grep "${ARCHIVE}" "${TMPDIR}/checksums-sha256.txt" | cut -d' ' -f1)
    if [ "$ACTUAL" != "$EXPECTED" ]; then
      error "Checksum verification failed!"
      echo "  Expected: $EXPECTED"
      echo "  Actual:   $ACTUAL"
      exit 1
    fi
    info "Checksum verified"
  fi

  # Extract binary
  tar xzf "${TMPDIR}/${ARCHIVE}" -C "${TMPDIR}"

  # Install
  mkdir -p "$INSTALL_DIR"
  mv "${TMPDIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
  chmod +x "${INSTALL_DIR}/${BINARY}"

  info "Installed page to ${INSTALL_DIR}/${BINARY}"

  # Check PATH
  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      warn "${INSTALL_DIR} is not in your PATH"
      echo ""
      echo "  Add it to your shell profile:"
      echo ""
      echo "    # bash"
      echo "    echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc"
      echo ""
      echo "    # zsh"
      echo "    echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc"
      echo ""
      echo "    # fish"
      echo "    fish_add_path ${INSTALL_DIR}"
      echo ""
      echo "  Then restart your shell or run:"
      echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
      echo ""
      ;;
  esac

  # Verify
  if command -v "${INSTALL_DIR}/${BINARY}" >/dev/null 2>&1; then
    INSTALLED_VERSION=$("${INSTALL_DIR}/${BINARY}" --version 2>/dev/null || echo "unknown")
    info "Done! ${INSTALLED_VERSION}"
  else
    info "Done! Run 'page --version' to verify."
  fi
}

main
