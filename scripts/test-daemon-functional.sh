#!/usr/bin/env bash
# test-daemon-functional.sh - LNXDrive daemon functional tests
#
# This script runs INSIDE the Podman container as testuser.
# It verifies the daemon lifecycle: start, D-Bus registration,
# journal logging, and graceful shutdown.
#
# Expected environment:
#   XDG_RUNTIME_DIR=/run/user/<uid>
#   DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/<uid>/bus
#
# Exit codes:
#   0 - All checks passed
#   1 - One or more checks failed

set -euo pipefail

# --- Colors and formatting ---------------------------------------------------

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m' # No Color

PASS_COUNT=0
FAIL_COUNT=0
TOTAL_CHECKS=8

pass() {
    echo -e "  ${GREEN}[PASS]${NC} $1"
    PASS_COUNT=$((PASS_COUNT + 1))
}

fail() {
    echo -e "  ${RED}[FAIL]${NC} $1"
    FAIL_COUNT=$((FAIL_COUNT + 1))
}

info() {
    echo -e "  ${YELLOW}[INFO]${NC} $1"
}

# --- Pre-flight checks -------------------------------------------------------

echo ""
echo -e "${BOLD}LNXDrive Daemon Functional Tests${NC}"
echo "================================="
echo ""

# Verify we are running as testuser
if [ "$(whoami)" != "testuser" ]; then
    echo -e "${RED}ERROR: This script must run as testuser${NC}"
    exit 1
fi

# Verify XDG_RUNTIME_DIR is set
if [ -z "${XDG_RUNTIME_DIR:-}" ]; then
    echo -e "${RED}ERROR: XDG_RUNTIME_DIR is not set${NC}"
    exit 1
fi

info "User: $(whoami)"
info "XDG_RUNTIME_DIR: ${XDG_RUNTIME_DIR}"
info "DBUS_SESSION_BUS_ADDRESS: ${DBUS_SESSION_BUS_ADDRESS:-not set}"
echo ""

# Wait briefly for user session to be fully ready
sleep 2

# --- Check 1: Start the service ----------------------------------------------

echo -e "${BOLD}Check 1/8: Start service${NC}"
if systemctl --user start lnxdrive 2>/dev/null; then
    pass "Service started successfully"
else
    fail "Service failed to start"
    info "Trying to get service status for diagnostics..."
    systemctl --user status lnxdrive --no-pager 2>&1 || true
fi

# Give the daemon time to initialize
sleep 3

# --- Check 2: Service is active ----------------------------------------------

echo -e "${BOLD}Check 2/8: Service is active${NC}"
STATUS=$(systemctl --user is-active lnxdrive 2>/dev/null || echo "inactive")
if [ "$STATUS" = "active" ]; then
    pass "Service status is 'active'"
else
    fail "Service status is '$STATUS' (expected 'active')"
fi

# --- Check 3: Process is running ----------------------------------------------

echo -e "${BOLD}Check 3/8: Daemon process running${NC}"
if pgrep -x lnxdrived >/dev/null 2>&1; then
    DAEMON_PID=$(pgrep -x lnxdrived)
    pass "lnxdrived is running (PID: $DAEMON_PID)"
else
    fail "lnxdrived process not found"
fi

# --- Check 4: D-Bus name registered ------------------------------------------

echo -e "${BOLD}Check 4/8: D-Bus name registered${NC}"
if busctl --user list 2>/dev/null | grep -q "com.enigmora.LNXDrive"; then
    pass "D-Bus name 'com.enigmora.LNXDrive' is registered"
else
    fail "D-Bus name 'com.enigmora.LNXDrive' not found on session bus"
    info "Current D-Bus names:"
    busctl --user list 2>&1 | head -20 || true
fi

# --- Check 5: CLI responds ---------------------------------------------------

echo -e "${BOLD}Check 5/8: CLI daemon status${NC}"
if lnxdrive daemon status >/dev/null 2>&1; then
    CLI_OUTPUT=$(lnxdrive daemon status 2>&1)
    pass "CLI 'daemon status' responded"
    info "Output: $CLI_OUTPUT"
else
    fail "CLI 'daemon status' failed (exit code: $?)"
    info "Output: $(lnxdrive daemon status 2>&1 || true)"
fi

# --- Check 6: Database file created ------------------------------------------

echo -e "${BOLD}Check 6/8: Database file created${NC}"
DB_PATH="${HOME}/.local/share/lnxdrive/lnxdrive.db"
if [ -f "$DB_PATH" ]; then
    DB_SIZE=$(stat -c%s "$DB_PATH" 2>/dev/null || echo "unknown")
    pass "Database exists at $DB_PATH ($DB_SIZE bytes)"
else
    fail "Database not found at $DB_PATH"
    info "Contents of ~/.local/share/lnxdrive/:"
    ls -la "${HOME}/.local/share/lnxdrive/" 2>&1 || true
fi

# --- Check 7: Journal logs ---------------------------------------------------

echo -e "${BOLD}Check 7/8: Journal contains daemon logs${NC}"
JOURNAL_OUTPUT=$(journalctl --user -u lnxdrive --no-pager -n 50 2>/dev/null || echo "")
if echo "$JOURNAL_OUTPUT" | grep -qi "lnxdrive\|starting\|daemon"; then
    pass "Journal contains LNXDrive log entries"
    info "Last 5 log lines:"
    echo "$JOURNAL_OUTPUT" | tail -5 | while IFS= read -r line; do
        info "  $line"
    done
else
    fail "No LNXDrive entries found in user journal"
    info "Journal output (last 10 lines):"
    journalctl --user --no-pager -n 10 2>&1 || true
fi

# --- Check 8: Graceful stop --------------------------------------------------

echo -e "${BOLD}Check 8/8: Graceful shutdown${NC}"
if systemctl --user stop lnxdrive 2>/dev/null; then
    # Wait for process to terminate
    sleep 2
    if ! pgrep -x lnxdrived >/dev/null 2>&1; then
        pass "Service stopped and process terminated"
    else
        fail "Service stopped but lnxdrived process still running"
    fi
else
    fail "Failed to stop service"
fi

# --- Summary ------------------------------------------------------------------

echo ""
echo "========================================="
echo -e "${BOLD}Results: ${PASS_COUNT}/${TOTAL_CHECKS} checks passed${NC}"
echo "========================================="

if [ "$FAIL_COUNT" -eq 0 ]; then
    echo -e "${GREEN}All checks passed.${NC}"
    echo ""
    exit 0
else
    echo -e "${RED}${FAIL_COUNT} check(s) failed.${NC}"
    echo ""
    exit 1
fi
