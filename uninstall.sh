#!/usr/bin/env bash
# BrassClaw uninstall script.
# Removes the binary and (if present) the systemd service.
# Config/data is preserved by default; a prompt offers removal.
#
# Usage:
#   bash uninstall.sh               # removes user-local binary (~/.local/bin)
#   sudo bash uninstall.sh          # removes system binary + systemd service

set -euo pipefail

BINARY_NAME="brassclaw"
SERVICE_NAME="brassclaw"
SYSTEMD_DIR="/etc/systemd/system"
CONFIG_DIR="${BRASSCLAW_REBORN_HOME:-$HOME/.brassclaw/reborn}"

# ── privilege / install mode ──────────────────────────────────────────────────
if [[ $EUID -eq 0 ]]; then
    INSTALL_DIR="/usr/local/bin"
    INSTALL_MODE="system"
else
    INSTALL_DIR="$HOME/.local/bin"
    INSTALL_MODE="user"
fi

# ── colours ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_step()  { echo -e "${BLUE}[STEP]${NC}  $*"; }

# ── confirm ───────────────────────────────────────────────────────────────────
echo -e "${BLUE}BrassClaw uninstaller ($INSTALL_MODE mode)${NC}"
echo ""
echo -e "Will remove:  ${RED}$INSTALL_DIR/$BINARY_NAME${NC}"
if [[ $INSTALL_MODE == "system" ]] && [[ -f "$SYSTEMD_DIR/$SERVICE_NAME.service" ]]; then
    echo -e "              ${RED}$SYSTEMD_DIR/$SERVICE_NAME.service${NC}"
fi
echo ""
read -rp "Continue? [y/N] " reply
if [[ ! "$reply" =~ ^[Yy]$ ]]; then
    log_info "Cancelled."
    exit 0
fi
echo ""

# ── stop and remove systemd service (system mode only) ───────────────────────
if [[ $INSTALL_MODE == "system" ]] && command -v systemctl &>/dev/null; then
    if [[ -f "$SYSTEMD_DIR/$SERVICE_NAME.service" ]]; then
        log_step "Stopping and disabling service..."
        systemctl stop "$SERVICE_NAME" 2>/dev/null || true
        systemctl disable "$SERVICE_NAME" 2>/dev/null || true
        rm -f "$SYSTEMD_DIR/$SERVICE_NAME.service"
        systemctl daemon-reload
        log_info "Service removed."
    fi
fi

# ── remove binary ─────────────────────────────────────────────────────────────
if [[ -f "$INSTALL_DIR/$BINARY_NAME" ]]; then
    log_step "Removing binary..."
    rm -f "$INSTALL_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME.bak"
    log_info "Binary removed."
else
    log_warn "Binary not found at $INSTALL_DIR/$BINARY_NAME"
fi

# ── optional config removal ───────────────────────────────────────────────────
echo ""
if [[ -d "$CONFIG_DIR" ]]; then
    echo -e "Config directory ${BLUE}$CONFIG_DIR${NC} contains your data and will be kept."
    read -rp "Remove it too? [y/N] " reply
    if [[ "$reply" =~ ^[Yy]$ ]]; then
        log_step "Removing config directory..."
        rm -rf "$CONFIG_DIR"
        log_info "Config directory removed."
    else
        log_info "Config preserved at: $CONFIG_DIR"
    fi
fi

echo ""
log_info "Uninstall complete."
