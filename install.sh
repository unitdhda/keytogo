#!/usr/bin/env sh
# keytogo installer — downloads the latest release binary for your architecture.
# Usage: curl -fsSL https://raw.githubusercontent.com/unitf90/keytogo/main/install.sh | sh

set -e

REPO="unitf90/keytogo"
BIN="keytogo"
INSTALL_DIR="/usr/local/bin"

# ── Detect architecture ────────────────────────────────────────────────────────

ARCH="$(uname -m)"
case "$ARCH" in
  arm64)  TARGET="aarch64-apple-darwin" ;;
  x86_64) TARGET="x86_64-apple-darwin" ;;
  *)
    echo "error: unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

# ── Resolve latest release tag ─────────────────────────────────────────────────

echo "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' \
  | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "error: could not determine latest release tag" >&2
  exit 1
fi

echo "Latest release: $LATEST"

# ── Download and install ───────────────────────────────────────────────────────

ARCHIVE="${BIN}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${LATEST}/${ARCHIVE}"
TMP="$(mktemp -d)"

echo "Downloading $ARCHIVE..."
curl -fsSL "$URL" -o "${TMP}/${ARCHIVE}"

tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"

# ── Place binary ───────────────────────────────────────────────────────────────

if [ -w "$INSTALL_DIR" ]; then
  mv "${TMP}/${BIN}" "${INSTALL_DIR}/${BIN}"
else
  echo "Installing to ${INSTALL_DIR} (requires sudo)..."
  sudo mv "${TMP}/${BIN}" "${INSTALL_DIR}/${BIN}"
fi

chmod +x "${INSTALL_DIR}/${BIN}"
rm -rf "$TMP"

echo ""
echo "keytogo ${LATEST} installed to ${INSTALL_DIR}/${BIN}"
echo ""
echo "Next steps:"
echo "  1. Grant Accessibility permission:"
echo "     System Settings → Privacy & Security → Accessibility → add keytogo"
echo "  2. Install as a login daemon:"
echo "     keytogo --install"
echo "  3. Or run once:"
echo "     keytogo"
