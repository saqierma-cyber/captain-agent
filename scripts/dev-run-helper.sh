#!/usr/bin/env bash
# Run captain-helper in the foreground for development iteration.
# Uses the in-tree osquery.app and a temp state dir.
#
# Usage:
#   ./scripts/dev-run-helper.sh           # asks for sudo password
#
# The UDS is written to /var/run/captain-helper.sock (root-owned, world-
# accessible). Once running, launch the Tauri UI as your normal user in
# another terminal:
#   pnpm tauri dev
# It will pick up the same default socket and connect.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BIN="$ROOT/target/debug/captain-helper"
OSQ="$ROOT/src-tauri/binaries/osquery.app/Contents/MacOS/osqueryd"
STATE="/tmp/captain-helper-dev"

[[ -x "$BIN" ]] || { echo "missing $BIN (build first: cargo build -p captain-helper)" >&2; exit 1; }
[[ -x "$OSQ" ]] || { echo "missing $OSQ (fetch first: ./scripts/fetch-osqueryd.sh)" >&2; exit 1; }

mkdir -p "$STATE"
echo "[dev] running captain-helper as root from in-tree build"
echo "[dev] state dir: $STATE"
echo "[dev] socket:    /var/run/captain-helper.sock"
echo

exec sudo \
  CAPTAIN_OSQUERYD="$OSQ" \
  CAPTAIN_HELPER_STATE="$STATE" \
  CAPTAIN_HELPER_SOCK="/var/run/captain-helper.sock" \
  RUST_LOG="captain_helper=debug" \
  "$BIN"
