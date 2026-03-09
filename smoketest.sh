#!/usr/bin/env bash
set -e

APP="/Applications/Brightwing MCP Manager.app"
MACOS="$APP/Contents/MacOS"
AUTHD="$MACOS/brightwing-authd"
DATA_DIR="$HOME/Library/Application Support/com.brightwing.mcp-manager"
SOCK="$DATA_DIR/authd.sock"
PID_FILE="$DATA_DIR/authd.pid"

GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

PASS=0
FAIL=0

pass() { ((PASS++)); echo -e "  ${GREEN}✓${NC} $1"; }
fail() { ((FAIL++)); echo -e "  ${RED}✗${NC} $1"; }
info() { echo -e "\n${CYAN}${BOLD}▸ $1${NC}"; }

cleanup() {
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE" 2>/dev/null)
        [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
        sleep 0.3
    fi
    rm -f "$SOCK" "$PID_FILE"
}
trap cleanup EXIT

# Clean slate
cleanup

echo ""
echo "═══════════════════════════════════════════════════"
echo "  Brightwing MCP Manager — Smoke Test"
echo "═══════════════════════════════════════════════════"

# ─── 1. App bundle ───────────────────────────────────
info "App bundle"

[ -d "$APP" ] && pass ".app exists in /Applications" || fail ".app not found"

for bin in brightwing-mcp-manager brightwing-authd brightwing-proxy bw; do
    [ -x "$MACOS/$bin" ] && pass "$bin bundled & executable" || fail "$bin missing"
done

# ─── 2. Daemon start ────────────────────────────────
info "Daemon lifecycle"

"$AUTHD" &
sleep 0.8

[ -f "$PID_FILE" ] && pass "PID file created ($(cat "$PID_FILE"))" || fail "No PID file"
[ -S "$SOCK" ]     && pass "Socket created"                        || fail "No socket"
kill -0 "$(cat "$PID_FILE")" 2>/dev/null && pass "Process alive"   || fail "Process dead"

# ─── 3. Duplicate blocked ───────────────────────────
info "Duplicate detection"

"$AUTHD" 2>/dev/null &
DUP=$!; sleep 0.4
kill -0 "$DUP" 2>/dev/null && { fail "Duplicate still running"; kill "$DUP" 2>/dev/null; } \
                           || pass "Duplicate exited"

# ─── 4. IPC: Ping ───────────────────────────────────
info "IPC — Ping/Pong"

PONG=$(echo '{"action":"ping"}' | socat -t2 - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
echo "$PONG" | grep -q '"pong"' && pass "Pong received" || fail "No pong: $PONG"

UPTIME=$(echo "$PONG" | python3 -c "import sys,json;print(json.load(sys.stdin).get('uptime_secs','?'))" 2>/dev/null)
VER=$(echo "$PONG"    | python3 -c "import sys,json;print(json.load(sys.stdin).get('daemon_version','?'))" 2>/dev/null)
echo -e "    uptime=${UPTIME}s  version=v${VER}"

# ─── 5. IPC: Handshake ──────────────────────────────
info "IPC — Handshake"

HS=$(echo '{"action":"handshake","client":"gui","version":"1.0.0"}' | socat -t2 - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
echo "$HS" | grep -q '"handshake_ok"' && pass "Handshake OK" || fail "Handshake failed: $HS"

# ─── 6. IPC: Credential round-trip ──────────────────
info "IPC — Credentials"

REG=$(echo '{"action":"register_server","server_id":"smoke-test","display_name":"Smoke","auth_type":"api_key"}' \
    | socat -t2 - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
echo "$REG" | grep -q '"ok"' && pass "Server registered" || fail "Register: $REG"

STORE=$(echo '{"action":"store_credentials","server_id":"smoke-test","credential":{"type":"api_key","env":{"TOKEN":"s3cret"}}}' \
    | socat -t2 - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
echo "$STORE" | grep -q '"ok"' && pass "Credentials stored" || fail "Store: $STORE"

GET=$(echo '{"action":"get_credentials","server_id":"smoke-test"}' \
    | socat -t2 - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
echo "$GET" | grep -q '"s3cret"' && pass "Credentials retrieved" || fail "Get: $GET"

# ─── 7. IPC: List servers ───────────────────────────
info "IPC — List servers"

LIST=$(echo '{"action":"list_servers"}' | socat -t2 - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
echo "$LIST" | grep -q '"smoke-test"' && pass "Server in list" || fail "List: $LIST"

# ─── 8. bw CLI ────────────────────────────────────
info "bw CLI"

BW="$MACOS/bw"
BW_LIST=$("$BW" list 2>&1) || true
echo "$BW_LIST" | grep -qi "server\|connected\|no servers" && pass "bw list runs" || fail "bw list: $BW_LIST"

BW_LIST_SERVER=$("$BW" list smoke-test 2>&1) || true
echo "$BW_LIST_SERVER" | grep -qi "tool\|no tool\|smoke-test" && pass "bw list <server> runs" || fail "bw list server: $BW_LIST_SERVER"

# ─── 9. IPC: Unregister ─────────────────────────────
info "IPC — Cleanup"

UNREG=$(echo '{"action":"unregister_server","server_id":"smoke-test"}' \
    | socat -t2 - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
echo "$UNREG" | grep -q '"ok"' && pass "Server unregistered" || fail "Unregister: $UNREG"

GONE=$(echo '{"action":"get_credentials","server_id":"smoke-test"}' \
    | socat -t2 - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
echo "$GONE" | grep -q '"not_found"' && pass "Credentials gone" || fail "Still there: $GONE"

# ─── 9. Graceful shutdown ───────────────────────────
info "Graceful shutdown"

kill "$(cat "$PID_FILE")"
sleep 0.5

[ ! -S "$SOCK" ]     && pass "Socket removed"   || fail "Socket lingers"
[ ! -f "$PID_FILE" ] && pass "PID file removed"  || fail "PID file lingers"

# ─── Summary ────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════"
if [ "$FAIL" -eq 0 ]; then
    echo -e "  ${GREEN}${BOLD}All $PASS tests passed${NC}"
else
    echo -e "  ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}"
fi
echo "═══════════════════════════════════════════════════"
echo ""

[ "$FAIL" -eq 0 ] || exit 1
