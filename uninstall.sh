#!/usr/bin/env bash
# BrassClaw uninstall script.
# Removes the binary and (if present) the systemd service.
# Config/data is preserved by default; a prompt offers removal.
#
# Usage:
#   bash uninstall.sh               # removes user-local binary (~/.local/bin)
#   sudo bash uninstall.sh          # removes system binary + systemd service
#   sudo bash uninstall.sh -y       # non-interactive, preserves config/data
#   sudo bash uninstall.sh --wipe   # non-interactive, deletes EVERYTHING
#                                   # (binary, service, config, and data dirs)
#
# Pipe-friendly (curl | sudo bash -s -- --wipe):
#   curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/uninstall.sh \
#     | sudo bash -s -- --wipe

set -euo pipefail

# ── parse flags ───────────────────────────────────────────────────────────────
YES=false
WIPE=false
for arg in "$@"; do
    case "$arg" in
        -y|--yes)  YES=true  ;;
        --wipe)    WIPE=true; YES=true ;;
    esac
done

# When stdin is not a terminal (e.g. curl | bash), open /dev/tty for prompts.
# If /dev/tty is also unavailable (no controlling terminal at all) we fall back
# to auto-yes so the script does not silently cancel instead of running.
if [[ -t 0 ]]; then
    TTY_IN=/dev/stdin
elif [[ -e /dev/tty ]]; then
    TTY_IN=/dev/tty
else
    TTY_IN=""          # no terminal — force non-interactive yes
    YES=true
fi

# The canonical installed binary name (same as install.sh uses)
BINARY_NAME="brassclaw-reborn"
# Legacy binary name from installs prior to the brassclaw-reborn rename
LEGACY_BINARY_NAME="brassclaw"
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
echo -e "              ${RED}$INSTALL_DIR/$LEGACY_BINARY_NAME${NC} (legacy, if present)"
if [[ $INSTALL_MODE == "system" ]] && [[ -f "$SYSTEMD_DIR/$SERVICE_NAME.service" ]]; then
    echo -e "              ${RED}$SYSTEMD_DIR/$SERVICE_NAME.service${NC}"
fi
if [[ "$WIPE" == "true" ]]; then
    echo -e "              ${RED}$CONFIG_DIR${NC} (config — wipe mode)"
    echo -e "              ${RED}$DATA_DIR${NC} (all data — wipe mode)"
fi
echo ""
if [[ "$YES" == "true" ]]; then
    if [[ "$WIPE" == "true" ]]; then
        echo "Proceeding non-interactively (--wipe: all data will be deleted)"
    else
        echo "Proceeding non-interactively (-y: config/data preserved)"
    fi
else
    if [[ -z "$TTY_IN" ]]; then
        reply="y"
    else
        read -rp "Continue? [y/N] " reply <"$TTY_IN"
    fi
    if [[ ! "$reply" =~ ^[Yy]$ ]]; then
        log_info "Cancelled."
        exit 0
    fi
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
        for proc in "$BINARY_NAME" "$LEGACY_BINARY_NAME"; do
            if pgrep -x "$proc" &>/dev/null; then
                log_step "Killing lingering $proc process..."
                pkill -x "$proc" 2>/dev/null || true
            fi
        done
    fi
fi

# ── remove binaries ───────────────────────────────────────────────────────────
log_step "Removing binaries from $INSTALL_DIR..."
removed_any=0
for candidate in "$INSTALL_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME.bak" \
                 "$INSTALL_DIR/$LEGACY_BINARY_NAME" "$INSTALL_DIR/$LEGACY_BINARY_NAME.bak"; do
    if [[ -f "$candidate" ]]; then
        rm -f "$candidate"
        log_info "Removed: $candidate"
        removed_any=1
    fi
done
if [[ $removed_any -eq 0 ]]; then
    log_warn "No binaries found at $INSTALL_DIR/$BINARY_NAME[.bak] or $INSTALL_DIR/$LEGACY_BINARY_NAME[.bak]"
fi

# ── optional config/data removal ─────────────────────────────────────────────
echo ""
if [[ -d "$CONFIG_DIR" ]]; then
    if [[ "$WIPE" == "true" ]]; then
        log_step "Removing reborn config directory (wipe mode)..."
        rm -rf "$CONFIG_DIR"
        log_info "Removed: $CONFIG_DIR"
    else
        echo -e "Reborn config dir ${BLUE}$CONFIG_DIR${NC} contains your data and will be kept."
        if [[ "$YES" == "true" ]]; then
            log_info "Config preserved at: $CONFIG_DIR (use --wipe to remove)"
        else
            read -rp "Remove reborn config dir? [y/N] " reply <"${TTY_IN:-/dev/tty}"
            if [[ "$reply" =~ ^[Yy]$ ]]; then
                log_step "Removing reborn config directory..."
                rm -rf "$CONFIG_DIR"
                log_info "Removed: $CONFIG_DIR"
            else
                log_info "Config preserved at: $CONFIG_DIR"
            fi
        fi
    fi
fi

# Offer to remove the parent ~/.brassclaw dir if now empty or at user request
if [[ -d "$DATA_DIR" ]]; then
    echo ""
    if [[ "$WIPE" == "true" ]]; then
        log_step "Removing parent data directory (wipe mode)..."
        rm -rf "$DATA_DIR"
        log_info "Removed: $DATA_DIR"
    else
        echo -e "Parent data dir ${BLUE}$DATA_DIR${NC} may contain logs, notes, and skill files."
        if [[ "$YES" == "true" ]]; then
            log_info "Data preserved at: $DATA_DIR (use --wipe to remove)"
        else
            read -rp "Remove entire $DATA_DIR too? [y/N] " reply <"${TTY_IN:-/dev/tty}"
            if [[ "$reply" =~ ^[Yy]$ ]]; then
                log_step "Removing $DATA_DIR..."
                rm -rf "$DATA_DIR"
                log_info "Removed: $DATA_DIR"
            else
                log_info "Data preserved at: $DATA_DIR"
            fi
        fi
    fi
fi

echo ""
log_info "Uninstall complete."
