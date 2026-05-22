#!/usr/bin/env bash
# Fetch the pinned osquery release for the host platform and place its
# entire signed .app bundle (mac) / executable (win) in src-tauri/binaries/.
#
# Why the .app bundle on mac (not just the binary):
#   The official osqueryd has the `com.apple.developer.endpoint-security.client`
#   entitlement, and macOS requires its code signature to verify against the
#   surrounding Contents/Info.plist + Contents/_CodeSignature/. Copying the
#   bare binary out breaks the signature, drops the entitlement, and gets
#   the process SIGKILLed once we try to use ES. So we ship the whole .app.
#
# Usage:
#   ./scripts/fetch-osqueryd.sh          # current host platform
#   FORCE=1 ./scripts/fetch-osqueryd.sh  # re-download / re-extract

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VERSION="$(awk '/^[0-9]/ { print; exit }' "$ROOT/osquery-version.txt" | tr -d '[:space:]')"
if [[ -z "$VERSION" ]]; then
  echo "[fetch-osqueryd] could not parse version from osquery-version.txt" >&2
  exit 1
fi
echo "[fetch-osqueryd] target version: $VERSION"

BIN_DIR="$ROOT/src-tauri/binaries"
mkdir -p "$BIN_DIR"

OS="$(uname -s)"
case "$OS" in
  Darwin)
    DEST="$BIN_DIR/osquery.app"
    ASSET="osquery-${VERSION}.pkg"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    DEST="$BIN_DIR/osqueryd-win.exe"
    ARCH="$(uname -m)"
    if [[ "$ARCH" == *aarch64* || "$ARCH" == *arm64* ]]; then
      ASSET="osquery-${VERSION}.windows_arm64.zip"
    else
      ASSET="osquery-${VERSION}.windows_x86_64.zip"
    fi
    ;;
  *)
    echo "unsupported OS: $OS" >&2
    exit 1
    ;;
esac

if [[ -e "$DEST" && "${FORCE:-0}" != "1" ]]; then
  echo "[fetch-osqueryd] $DEST already exists (set FORCE=1 to re-fetch)"
  exit 0
fi

URL="https://github.com/osquery/osquery/releases/download/${VERSION}/${ASSET}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "[fetch-osqueryd] downloading ${ASSET} ..."
curl -fL --progress-bar -o "$TMP/$ASSET" "$URL"

case "$ASSET" in
  *.pkg)
    echo "[fetch-osqueryd] expanding .pkg ..."
    pkgutil --expand-full "$TMP/$ASSET" "$TMP/unpacked"
    APP_SRC="$(find "$TMP/unpacked" -name 'osquery.app' -type d | head -1)"
    if [[ -z "$APP_SRC" ]]; then
      echo "[fetch-osqueryd] osquery.app not found in .pkg" >&2
      exit 1
    fi
    rm -rf "$DEST"
    cp -R "$APP_SRC" "$DEST"
    echo "[fetch-osqueryd] verifying signature ..."
    codesign --verify --verbose=2 "$DEST" 2>&1
    echo "[fetch-osqueryd] installed at $DEST"
    "$DEST/Contents/MacOS/osqueryd" --version
    ;;
  *.zip)
    unzip -q "$TMP/$ASSET" -d "$TMP/extracted"
    FOUND="$(find "$TMP/extracted" -name 'osqueryd.exe' | head -1)"
    if [[ -z "$FOUND" ]]; then
      echo "[fetch-osqueryd] osqueryd.exe not found in zip" >&2
      exit 1
    fi
    cp "$FOUND" "$DEST"
    chmod +x "$DEST"
    echo "[fetch-osqueryd] installed at $DEST"
    ;;
esac
