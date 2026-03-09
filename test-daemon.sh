#!/usr/bin/env bash
set -e

AUTHD=~/.local/bin/brightwing-authd
DATA_DIR="$HOME/Library/Application Support/com.brightwing.mcp-manager"
SOCK="$DATA_DIR/authd.sock"
PID_FILE="$DATA_DIR/authd.pid"

GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

pass() { echo -e "  ${GREEN}PASS${NC} $1"; }
fail() { echo -e "  ${RED}FAIL${NC} $1"; exit 1; }
info() { echo -e "${CYAN}▸${NC} $1"; }

cleanup() {
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE" 2>/dev/null)
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            sleep 0.5
        fi
    fi
    rm -f "$SOCK" "$PID_FILE"
}

trap cleanup EXIT

# Clean slate
cleanup

echo ""
echo "════════════════════════════════════════════════"
echo "  Brightwing Daemon Phase 4 Smoke Test"
echo "════════════════════════════════════════════════"
echo ""

# 1. Start daemon
info "Starting daemon..."
$AUTHD &
sleep 1

if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    pass "PID file created (PID: $PID)"
else
    fail "PID file not created"
fi

if [ -S "$SOCK" ]; then
    pass "Unix socket created at $SOCK"
else
    fail "Unix socket not found"
fi

if kill -0 "$PID" 2>/dev/null; then
    pass "Daemon process alive"
else
    fail "Daemon process not running"
fi

# 2. Duplicate detection
info "Testing duplicate instance detection..."
$AUTHD 2>/tmp/authd-dup.log &
DUP_PID=$!
sleep 0.5
if ! kill -0 "$DUP_PID" 2>/dev/null; then
    pass "Duplicate instance exited (as expected)"
else
    kill "$DUP_PID" 2>/dev/null || true
    fail "Duplicate instance should have exited"
fi

# 3. Ping/Pong
info "Testing Ping/Pong health check..."
PONG=$(echo '{"action":"ping"}' | socat - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
if echo "$PONG" | grep -q '"pong"'; then
    UPTIME=$(echo "$PONG" | python3 -c "import sys,json; print(json.load(sys.stdin).get('uptime_secs','?'))" 2>/dev/null || echo "?")
    VERSION=$(echo "$PONG" | python3 -c "import sys,json; print(json.load(sys.stdin).get('daemon_version','?'))" 2>/dev/null || echo "?")
    pass "Pong received (uptime: ${UPTIME}s, version: v${VERSION})"
else
    fail "No pong response: $PONG"
fi

# 4. Handshake
info "Testing version handshake..."
HANDSHAKE=$(echo '{"action":"handshake","client":"gui","version":"1.0.0"}' | socat - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
if echo "$HANDSHAKE" | grep -q '"handshake_ok"'; then
    pass "Handshake OK"
else
    fail "Handshake failed: $HANDSHAKE"
fi

# 5. Register server + store + retrieve credentials
info "Testing credential round-trip..."
REGISTER=$(echo '{"action":"register_server","server_id":"test-srv","display_name":"Test","auth_type":"api_key"}' | socat - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
if echo "$REGISTER" | grep -q '"ok"'; then
    pass "Server registered"
else
    fail "Register failed: $REGISTER"
fi

STORE=$(echo '{"action":"store_credentials","server_id":"test-srv","credential":{"type":"api_key","env":{"TOKEN":"secret123"}}}' | socat - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
if echo "$STORE" | grep -q '"ok"'; then
    pass "Credentials stored"
else
    fail "Store failed: $STORE"
fi

GET=$(echo '{"action":"get_credentials","server_id":"test-srv"}' | socat - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
if echo "$GET" | grep -q '"secret123"'; then
    pass "Credentials retrieved correctly"
else
    fail "Get credentials failed: $GET"
fi

# 6. List servers
info "Testing list servers..."
LIST=$(echo '{"action":"list_servers"}' | socat - UNIX-CONNECT:"$SOCK" 2>/dev/null | head -1)
if echo "$LIST" | grep -q '"test-srv"'; then
    pass "Server appears in list"
else
    fail "List servers failed: $LIST"
fi

# 7. Graceful shutdown
info "Testing graceful shutdown (SIGTERM)..."
kill "$PID"
sleep 1

if kill -0 "$PID" 2>/dev/null; then
    fail "Daemon still running after SIGTERM"
else
    pass "Daemon exited cleanly"
fi

if [ ! -S "$SOCK" ]; then
    pass "Socket cleaned up"
else
    fail "Socket still exists after shutdown"
fi

if [ ! -f "$PID_FILE" ]; then
    pass "PID file cleaned up"
else
    fail "PID file still exists after shutdown"
fi

echo ""
echo "════════════════════════════════════════════════"
echo -e "  ${GREEN}All tests passed!${NC}"
echo "════════════════════════════════════════════════"
echo ""
