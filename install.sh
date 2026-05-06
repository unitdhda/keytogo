#!/usr/bin/env sh
# keytogo installer — downloads a release binary for your architecture.

set -e

REPO="unitdhda/keytogo"
BIN="keytogo"
INSTALL_DIR="${HOME}/.local/bin"

# ── Parse args ────────────────────────────────────────────────────────────────

VERSION=""

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

# ── Validate version ──────────────────────────────────────────────────────────

if [ -n "$VERSION" ]; then
  echo "$VERSION" | grep -Eq '^v[0-9]+(\.[0-9]+){0,3}$' || {
    echo "error: invalid version format: $VERSION" >&2
    exit 1
  }
fi

# ── Detect OS + architecture ───────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) OS_TARGET="apple-darwin" ;;
  *)
    echo "error: unsupported OS: $OS" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  arm64|aarch64) ARCH_TARGET="aarch64" ;;
  x86_64|amd64)  ARCH_TARGET="x86_64" ;;
  *)
    echo "error: unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

TARGET="${ARCH_TARGET}-${OS_TARGET}"

# ── Resolve version ───────────────────────────────────────────────────────────

if [ -n "$VERSION" ]; then
  LATEST="$VERSION"
  echo "Using specified version: $LATEST"
else
  echo "Fetching latest release (including prereleases)..."

  LATEST="$(
    curl -fsSL "https://api.github.com/repos/${REPO}/releases?per_page=100" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' \
    | grep -E '^v[0-9]+(\.[0-9]+){0,3}$' \
    | head -n 1
  )"

  if [ -z "$LATEST" ]; then
    echo "error: could not determine latest release tag" >&2
    exit 1
  fi

  echo "Latest release: $LATEST"
fi

# ── Download ──────────────────────────────────────────────────────────────────

ARCHIVE="${BIN}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${LATEST}/${ARCHIVE}"

TMP="$(mktemp -d)"

echo "Downloading $ARCHIVE..."
curl -fL "$URL" -o "${TMP}/${ARCHIVE}"

echo "Extracting..."
tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"

FOUND_BIN="$(find "$TMP" -type f -name "$BIN" | head -n 1)"

if [ -z "$FOUND_BIN" ]; then
  echo "error: binary not found in archive" >&2
  exit 1
fi

# ── Install (user-level) ──────────────────────────────────────────────────────

mkdir -p "$INSTALL_DIR"
DEST="${INSTALL_DIR}/${BIN}"

mv "$FOUND_BIN" "$DEST"
chmod +x "$DEST"
rm -rf "$TMP"

echo ""
echo "keytogo ${LATEST} installed to ${DEST}"

# ── PATH hint ─────────────────────────────────────────────────────────────────

case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    ;;
  *)
    echo ""
    echo "⚠️  ${INSTALL_DIR} is not in your PATH."
    echo ""
    echo "Add it by running:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    echo ""
    echo "And add that line to your shell config:"
    echo "  ~/.zshrc (zsh) or ~/.bashrc (bash)"
    ;;
esac

echo ""
echo "Next steps:"
echo "  1. Grant Accessibility permission:"
echo "     System Settings → Privacy & Security → Accessibility → add keytogo"
echo "  2. Install as a login daemon:"
echo "     keytogo --install"
echo "  3. Or run once:"
echo "     keytogo"
