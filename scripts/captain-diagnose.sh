#!/usr/bin/env bash
# Captain Agent diagnostic — checks every layer of the pipeline and emits a
# report you can paste back when something breaks. Designed to be a fix-it-
# yourself flowchart: each FAIL line is followed by a HINT pointing to the
# action.
#
# Run as root (most checks need it):
#   sudo ./scripts/captain-diagnose.sh

set -uo pipefail

PASS="${T_GREEN:-✓}"
FAIL="${T_RED:-✗}"
WARN="${T_YELLOW:-⚠}"

okay()  { echo "[ ✓ ] $*"; }
fail()  { echo "[ ✗ ] $*"; }
warn()  { echo "[ ⚠ ] $*"; }
hint()  { echo "       → $*"; }
header(){ echo; echo "── $* ──"; }

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  warn "not running as root — some checks (TCC.db, /var/lib/captain-helper/) will be skipped"
  hint "re-run with: sudo $0"
fi
ROOT_OK=$([[ "${EUID:-$(id -u)}" -eq 0 ]] && echo 1 || echo 0)

# ── 1. Binaries on disk ────────────────────────────────────────
header "1. Installation artifacts"

if [[ -x /usr/local/libexec/captain-helper ]]; then
  okay "captain-helper installed at /usr/local/libexec/"
  /usr/local/libexec/captain-helper --version 2>/dev/null | head -1 || true
else
  fail "captain-helper missing"
  hint "run: sudo $(dirname "$0")/install-helper.sh"
fi

OSQ_PATH="/Library/Application Support/com.captainagent.helper/osquery.app/Contents/MacOS/osqueryd"
if [[ -x "$OSQ_PATH" ]]; then
  okay "osquery.app installed"
  "$OSQ_PATH" --version 2>/dev/null | head -1
else
  fail "osquery.app missing"
  hint "run: $(dirname "$0")/fetch-osqueryd.sh && sudo $(dirname "$0")/install-helper.sh"
fi

if [[ -f /Library/LaunchDaemons/com.captainagent.helper.plist ]]; then
  okay "LaunchDaemon plist installed"
else
  fail "plist missing — daemon won't auto-start"
  hint "sudo $(dirname "$0")/install-helper.sh"
fi

# ── 2. Daemon liveness ─────────────────────────────────────────
header "2. Daemon process"
if launchctl print system/com.captainagent.helper 2>/dev/null | grep -q "state = running"; then
  okay "LaunchDaemon state = running"
  PID=$(launchctl print system/com.captainagent.helper 2>/dev/null | awk -F'=' '/^\tpid/ {print $2}' | tr -d ' ')
  [[ -n "$PID" ]] && echo "       pid=$PID, uptime $(ps -o etime= -p $PID 2>/dev/null | xargs)"
else
  fail "LaunchDaemon not running"
  hint "sudo launchctl bootstrap system /Library/LaunchDaemons/com.captainagent.helper.plist"
fi

# ── 3. UDS socket ──────────────────────────────────────────────
header "3. UDS socket"
SOCK=/var/run/captain-helper.sock
if [[ -S "$SOCK" ]]; then
  PERM=$(stat -f '%Op' "$SOCK")
  if [[ "${PERM: -3}" == "666" ]]; then
    okay "UDS at $SOCK (perms 0666 — user-mode UI can connect)"
  else
    warn "UDS perms are ${PERM: -3}, expected 666"
    hint "this can happen if helper crashed mid-init; sudo launchctl kickstart -k system/com.captainagent.helper"
  fi
else
  fail "UDS socket missing"
  hint "daemon probably crashed at startup — check /var/log/captain-helper.err"
fi

# ── 4. RPC sanity ───────────────────────────────────────────────
header "4. IPC RPC"
REPLY=$( (echo '{"type":"ping"}'; sleep 0.3) | nc -U "$SOCK" 2>/dev/null | head -1 )
if [[ "$REPLY" == *'"pong"'* ]]; then
  okay "Ping → Pong"
else
  fail "Ping RPC failed (reply: $REPLY)"
  hint "daemon probably hung; sudo launchctl kickstart -k system/com.captainagent.helper"
fi

REPLY=$( (echo '{"type":"get_status"}'; sleep 0.3) | nc -U "$SOCK" 2>/dev/null | head -1 )
if [[ "$REPLY" == *'"events_emitted_total"'* ]]; then
  EVENTS=$(echo "$REPLY" | python3 -c 'import sys,json;print(json.load(sys.stdin).get("events_emitted_total","?"))' 2>/dev/null)
  UPTIME=$(echo "$REPLY" | python3 -c 'import sys,json;print(json.load(sys.stdin).get("uptime_seconds","?"))' 2>/dev/null)
  okay "helper status: $EVENTS events emitted, ${UPTIME}s uptime"
else
  fail "GetStatus failed"
fi

REPLY=$( (echo '{"type":"list_alive_pids"}'; sleep 0.3) | nc -U "$SOCK" 2>/dev/null | head -1 )
if [[ "$REPLY" == *'"alive_pids"'* ]]; then
  N=$(echo "$REPLY" | python3 -c 'import sys,json;print(len(json.load(sys.stdin).get("pids",[])))' 2>/dev/null)
  okay "ListAlivePids → $N alive PIDs"
else
  warn "ListAlivePids returned: $REPLY"
  hint "helper might be from a pre-Slice-4 build; rebuild + reinstall"
fi

# ── 5. ES publisher state ─────────────────────────────────────
header "5. Endpoint Security publishers"
LOG=/var/log/captain-helper.err
if [[ -r "$LOG" ]]; then
  ES_DISABLED=$(tail -30 "$LOG" 2>/dev/null | grep -c 'endpointsecurity[^_]*: EndpointSecurity client lacks user TCC permissions' || true)
  FIM_DISABLED=$(tail -30 "$LOG" 2>/dev/null | grep -c 'endpointsecurity_fim: EndpointSecurity client lacks user TCC permissions' || true)
  if [[ "$ES_DISABLED" -gt 0 ]]; then
    fail "endpointsecurity publisher missing TCC permission"
    hint "Settings → Privacy & Security → Full Disk Access → toggle osqueryd OFF then ON, then: sudo launchctl kickstart -k system/com.captainagent.helper"
  else
    okay "endpointsecurity publisher init OK"
  fi
  if [[ "$FIM_DISABLED" -gt 0 ]]; then
    fail "endpointsecurity_fim publisher missing TCC permission (file events won't fire)"
    hint "same fix as above. Note: FIM uses a separate TCC sub-evaluation that can fail even when process events work."
  else
    okay "endpointsecurity_fim publisher init OK"
  fi
else
  warn "$LOG not readable (run with sudo for log access)"
fi

# ── 6. TCC.db grants ──────────────────────────────────────────
header "6. TCC database grants"
if [[ "$ROOT_OK" -eq 1 ]]; then
  ROWS=$(sqlite3 "/Library/Application Support/com.apple.TCC/TCC.db" \
    "SELECT service, auth_value, client FROM access WHERE client LIKE '%osquery%' OR client LIKE '%captain%';" 2>/dev/null)
  if [[ -n "$ROWS" ]]; then
    okay "Grants found:"
    echo "$ROWS" | sed 's/^/       /'
  else
    fail "No TCC grants for osquery/captain in system DB"
    hint "Settings → Privacy & Security → Full Disk Access → add osqueryd from /Library/Application Support/com.captainagent.helper/osquery.app/Contents/MacOS/"
  fi
else
  warn "skipped (root required)"
fi

# ── 7. Event flow in SQLite ───────────────────────────────────
header "7. Event ingestion (last 5 min)"
DB="$HOME/Library/Application Support/com.captainagent.app/captain.sqlite"
if [[ -f "$DB" ]]; then
  FIVE_MIN_AGO=$(( ($(date +%s) - 300) * 1000000000 ))
  PROC=$(sqlite3 "$DB" "SELECT COUNT(*) FROM events WHERE ts > $FIVE_MIN_AGO AND kind='process_spawn';" 2>/dev/null)
  WRITE=$(sqlite3 "$DB" "SELECT COUNT(*) FROM events WHERE ts > $FIVE_MIN_AGO AND kind='file_write';" 2>/dev/null)
  echo "       process_spawn: ${PROC:-?}"
  echo "       file_write:    ${WRITE:-?}"
  if [[ "${PROC:-0}" -gt 0 ]]; then
    okay "process events flowing"
  else
    warn "no process events captured in last 5 min"
    hint "if Tauri UI isn't running, no events get persisted (helper has them but doesn't store)"
  fi
  if [[ "${WRITE:-0}" -gt 0 ]]; then
    okay "file events flowing"
  else
    warn "no file events — FIM may be broken (see check 5)"
  fi
else
  warn "captain.sqlite missing — has the Tauri UI ever been started?"
fi

header "Done"
echo "If something is FAIL or WARN, follow the hint. If everything passes but UI"
echo "still looks wrong, restart Tauri dev: pnpm tauri dev (in project root)."
