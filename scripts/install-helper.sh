#!/usr/bin/env bash
# Install captain-helper as a system LaunchDaemon.
#
# Layout after install:
#   /usr/local/libexec/captain-helper                              # binary
#   /Library/Application Support/com.captainagent.helper/osquery.app  # osquery
#   /Library/LaunchDaemons/com.captainagent.helper.plist           # plist
#   /var/run/captain-helper.sock                                   # UDS (created by daemon at boot)
#   /var/lib/captain-helper/                                       # state dir (osquery.db etc.)
#   /var/log/captain-helper.log + .err                             # daemon logs
#
# Usage:
#   sudo ./scripts/install-helper.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "must run as root: sudo $0" >&2
  exit 1
fi

LABEL="com.captainagent.helper"
PLIST_SRC="$ROOT/install/$LABEL.plist"
PLIST_DST="/Library/LaunchDaemons/$LABEL.plist"
BIN_SRC="$ROOT/target/debug/captain-helper"   # use --release for prod
BIN_DST="/usr/local/libexec/captain-helper"
APP_SRC="$ROOT/src-tauri/binaries/osquery.app"
APP_DST="/Library/Application Support/com.captainagent.helper/osquery.app"
STATE_DIR="/var/lib/captain-helper"

echo "[install] verifying source files ..."
[[ -f "$PLIST_SRC" ]] || { echo "missing $PLIST_SRC" >&2; exit 1; }
[[ -f "$BIN_SRC" ]] || { echo "missing $BIN_SRC (build with: cargo build -p captain-helper)" >&2; exit 1; }
[[ -d "$APP_SRC" ]] || { echo "missing $APP_SRC (fetch with: ./scripts/fetch-osqueryd.sh)" >&2; exit 1; }

# If a previous version is running, bootout first (idempotent reinstall).
if launchctl print "system/$LABEL" >/dev/null 2>&1; then
  echo "[install] booting out existing $LABEL ..."
  launchctl bootout "system/$LABEL" || true
  sleep 1
fi

echo "[install] checking helper binary ..."
mkdir -p "$(dirname "$BIN_DST")"
# Preserve TCC grant on the helper binary: only copy if bytes differ.
# Each rebuild produces a new ad-hoc-signed binary; replacing the on-disk
# copy invalidates the macOS Endpoint Security entitlement attached to
# the old code identity even though the path stays the same.
if [[ -f "$BIN_DST" ]] && cmp -s "$BIN_SRC" "$BIN_DST"; then
  echo "[install] captain-helper unchanged (preserving TCC grant)"
else
  echo "[install] captain-helper bytes differ — copying (TCC may need re-grant)"
  cp "$BIN_SRC" "$BIN_DST"
  chown root:wheel "$BIN_DST"
  chmod 0755 "$BIN_DST"
fi

echo "[install] checking osquery.app ..."
mkdir -p "$(dirname "$APP_DST")"
# Preserve TCC grants: only replace osquery.app if the inner osqueryd
# binary actually changed. Each rm/cp creates new inodes which can
# invalidate macOS Full Disk Access entitlement we already gave the user.
if [[ -d "$APP_DST" ]] && cmp -s "$APP_SRC/Contents/MacOS/osqueryd" "$APP_DST/Contents/MacOS/osqueryd"; then
  echo "[install] osquery.app already installed with identical osqueryd (preserving FDA grant)"
else
  echo "[install] copying osquery.app (binary differs — FDA will need to be re-granted)"
  rm -rf "$APP_DST"
  cp -R "$APP_SRC" "$APP_DST"
fi
# osquery.app's own signature must remain — don't chown/chmod inside it.

echo "[install] creating state + log dirs ..."
mkdir -p "$STATE_DIR"
chown root:wheel "$STATE_DIR"
chmod 0700 "$STATE_DIR"
touch /var/log/captain-helper.log /var/log/captain-helper.err
chown root:wheel /var/log/captain-helper.log /var/log/captain-helper.err

echo "[install] installing plist ..."
cp "$PLIST_SRC" "$PLIST_DST"
chown root:wheel "$PLIST_DST"
chmod 0644 "$PLIST_DST"

echo "[install] bootstrapping LaunchDaemon ..."
launchctl bootstrap system "$PLIST_DST"

# Wait briefly for the socket to appear so we can sanity-check.
for i in 1 2 3 4 5 6 7 8 9 10; do
  if [[ -e /var/run/captain-helper.sock ]]; then
    echo "[install] UDS appeared after ${i} attempt(s)"
    break
  fi
  sleep 0.5
done

if [[ -e /var/run/captain-helper.sock ]]; then
  echo "[install] OK — helper is up at /var/run/captain-helper.sock"
else
  echo "[install] WARNING — UDS did not appear within ~5s. Check logs:"
  echo "         tail -20 /var/log/captain-helper.err"
fi

echo
echo "Inspect:           launchctl print system/$LABEL"
echo "Logs:              tail -f /var/log/captain-helper.log /var/log/captain-helper.err"
echo "Uninstall:         sudo ./scripts/uninstall-helper.sh"
