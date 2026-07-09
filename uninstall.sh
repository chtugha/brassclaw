#!/usr/bin/env bash
# BrassClaw uninstall script.
# Removes the binary and (if present) the systemd service.
# Config/data is preserved by default; a prompt offers removal.
#
# Usage:
#   bash uninstall.sh               # removes user-local binary (~/.local/bin)
#   sudo bash uninstall.sh          # removes system binary + systemd service

set -euo pipefail

# The canonical installed binary name (same as install.sh uses)
BINARY_NAME="brassclaw-reborn"
SERVICE_NAME="brassclaw"
SYSTEMD_DIR="/etc/systemd/system"
CONFIG_DIR="${BRASSCLAW_REBORN_HOME:-$HOME/.brassclaw/reborn}"
# Parent brassclaw data dir (offered for removal separately)
DATA_DIR="${HOME}/.brassclaw"

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
echo -e "              ${RED}$INSTALL_DIR/$BINARY_NAME.bak${NC} (if present)"
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
    else
        # Kill any lingering process even without a unit file
        if pgrep -x "$BINARY_NAME" &>/dev/null; then
            log_step "Killing lingering $BINARY_NAME process..."
            pkill -x "$BINARY_NAME" 2>/dev/null || true
        fi
    fi
fi

# ── remove binaries ───────────────────────────────────────────────────────────
log_step "Removing binaries from $INSTALL_DIR..."
removed_any=0
for candidate in "$INSTALL_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME.bak"; do
    if [[ -f "$candidate" ]]; then
        rm -f "$candidate"
        log_info "Removed: $candidate"
        removed_any=1
    fi
done
if [[ $removed_any -eq 0 ]]; then
    log_warn "No binaries found at $INSTALL_DIR/$BINARY_NAME[.bak]"
fi

# ── optional config/data removal ─────────────────────────────────────────────
echo ""
if [[ -d "$CONFIG_DIR" ]]; then
    echo -e "Reborn config dir ${BLUE}$CONFIG_DIR${NC} contains your data and will be kept."
    read -rp "Remove reborn config dir? [y/N] " reply
    if [[ "$reply" =~ ^[Yy]$ ]]; then
        log_step "Removing reborn config directory..."
        rm -rf "$CONFIG_DIR"
        log_info "Removed: $CONFIG_DIR"
    else
        log_info "Config preserved at: $CONFIG_DIR"
    fi
fi

# Offer to remove the parent ~/.brassclaw dir if now empty or at user request
if [[ -d "$DATA_DIR" ]]; then
    echo ""
    echo -e "Parent data dir ${BLUE}$DATA_DIR${NC} may contain logs, notes, and skill files."
    read -rp "Remove entire $DATA_DIR too? [y/N] " reply
    if [[ "$reply" =~ ^[Yy]$ ]]; then
        log_step "Removing $DATA_DIR..."
        rm -rf "$DATA_DIR"
        log_info "Removed: $DATA_DIR"
    else
        log_info "Data preserved at: $DATA_DIR"
    fi
fi

echo ""
log_info "Uninstall complete."
