#!/usr/bin/env bash
# Remove the captain-helper LaunchDaemon and clean up its files.
#
# Usage:
#   sudo ./scripts/uninstall-helper.sh         # leave state dir + logs
#   sudo PURGE=1 ./scripts/uninstall-helper.sh # also remove /var/lib + logs

set -euo pipefail

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "must run as root: sudo $0" >&2
  exit 1
fi

LABEL="com.captainagent.helper"
PLIST_DST="/Library/LaunchDaemons/$LABEL.plist"
BIN_DST="/usr/local/libexec/captain-helper"
APP_DST="/Library/Application Support/com.captainagent.helper/osquery.app"
STATE_DIR="/var/lib/captain-helper"
SOCK="/var/run/captain-helper.sock"

if launchctl print "system/$LABEL" >/dev/null 2>&1; then
  echo "[uninstall] booting out $LABEL ..."
  launchctl bootout "system/$LABEL"
else
  echo "[uninstall] $LABEL was not loaded"
fi

rm -f "$PLIST_DST" "$BIN_DST" "$SOCK"
rm -rf "$APP_DST"
rmdir "$(dirname "$APP_DST")" 2>/dev/null || true

if [[ "${PURGE:-0}" == "1" ]]; then
  echo "[uninstall] PURGE=1 — also removing state dir + logs"
  rm -rf "$STATE_DIR" /var/log/captain-helper.log /var/log/captain-helper.err
fi

echo "[uninstall] done"
