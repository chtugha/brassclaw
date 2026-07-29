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
#                                   # (binary, service, config, data, system user)
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

# Detect whether we have an interactive terminal for prompts.
# When stdin is a pipe (e.g. curl | bash), /dev/tty may be technically
# openable but read returns immediately empty under sudo with no controlling
# terminal.  We therefore only use /dev/tty when we can confirm it is truly
# interactive; otherwise we auto-yes so the script never silently cancels.
if [[ -t 0 ]]; then
    # stdin is a real terminal — prompt on stdin
    TTY_IN=/dev/stdin
elif [[ -t 2 ]] && [[ -c /dev/tty ]]; then
    # stderr is a terminal (common with `sudo bash`), /dev/tty is a char
    # device — open it and use it for prompts
    TTY_IN=/dev/tty
else
    # No usable terminal (pure pipe / cron / etc.) — proceed automatically.
    # The user can pass -y (preserve data) or --wipe (delete everything).
    TTY_IN=""
    YES=true
fi

# The canonical installed binary name (same as install.sh uses)
BINARY_NAME="brassclaw-reborn"
# Legacy binary name from installs prior to the brassclaw-reborn rename
LEGACY_BINARY_NAME="brassclaw"
SERVICE_NAME="brassclaw"
SERVICE_USER="brassclaw"
SYSTEMD_DIR="/etc/systemd/system"

# ── privilege / install mode ──────────────────────────────────────────────────
if [[ $EUID -eq 0 ]]; then
    INSTALL_DIR="/usr/local/bin"
    INSTALL_MODE="system"
else
    INSTALL_DIR="$HOME/.local/bin"
    INSTALL_MODE="user"
fi

# ── resolve all data dirs ─────────────────────────────────────────────────────
# In system mode the service may run as the 'brassclaw' system user whose home
# is /var/lib/brassclaw — not $HOME (which is /root when running sudo).
# Collect every candidate data root so --wipe removes them all.
WIPE_DIRS=()
if [[ $INSTALL_MODE == "system" ]]; then
    # Home of the dedicated system user (if it exists)
    if id "$SERVICE_USER" &>/dev/null; then
        svc_home=$(eval echo "~$SERVICE_USER" 2>/dev/null || true)
        if [[ -n "$svc_home" && -d "$svc_home" ]]; then
            WIPE_DIRS+=("$svc_home")
        fi
        # Also cover the fallback home used by install.sh
        if [[ -d "/var/lib/brassclaw" ]]; then
            WIPE_DIRS+=("/var/lib/brassclaw")
        fi
    fi
    # Also cover any root-owned data from early installs
    if [[ -d "/root/.brassclaw" ]]; then
        WIPE_DIRS+=("/root/.brassclaw")
    fi
    # Cover SUDO_USER's home if invoked via sudo
    if [[ -n "${SUDO_USER:-}" ]] && [[ "$SUDO_USER" != "root" ]]; then
        sudo_home=$(eval echo "~$SUDO_USER" 2>/dev/null || true)
        if [[ -n "$sudo_home" && -d "$sudo_home/.brassclaw" ]]; then
            WIPE_DIRS+=("$sudo_home/.brassclaw")
        fi
    fi
else
    # User-local install
    CONFIG_DIR="${BRASSCLAW_REBORN_HOME:-$HOME/.brassclaw/reborn}"
    DATA_DIR="$HOME/.brassclaw"
    WIPE_DIRS=("$DATA_DIR")
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
    for d in "${WIPE_DIRS[@]+"${WIPE_DIRS[@]}"}"; do
        echo -e "              ${RED}$d${NC} (data — wipe mode)"
    done
    if [[ $INSTALL_MODE == "system" ]] && id "$SERVICE_USER" &>/dev/null; then
        echo -e "              ${RED}system user '$SERVICE_USER'${NC} (wipe mode)"
    fi
fi
echo ""
if [[ "$YES" == "true" ]]; then
    if [[ "$WIPE" == "true" ]]; then
        echo "Proceeding non-interactively (--wipe: all data will be deleted)"
    else
        echo "Proceeding non-interactively (-y: config/data preserved)"
    fi
else
    reply=""
    if [[ -z "$TTY_IN" ]]; then
        reply="y"
    else
        read -rp "Continue? [y/N] " reply <"$TTY_IN" || reply=""
    fi
    if [[ ! "$reply" =~ ^[Yy]$ ]]; then
        log_info "Cancelled.  Re-run with -y to proceed non-interactively,"
        log_info "or with --wipe to also delete all data."
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

# ── wipe data dirs ────────────────────────────────────────────────────────────
echo ""
if [[ "$WIPE" == "true" ]]; then
    for d in "${WIPE_DIRS[@]+"${WIPE_DIRS[@]}"}"; do
        if [[ -d "$d" ]]; then
            log_step "Removing $d ..."
            rm -rf "$d"
            log_info "Removed: $d"
        fi
    done
    # Remove the /var/lib/brassclaw parent itself if it exists
    if [[ $INSTALL_MODE == "system" ]] && [[ -d "/var/lib/brassclaw" ]]; then
        rm -rf "/var/lib/brassclaw"
        log_info "Removed: /var/lib/brassclaw"
    fi
    # Delete the dedicated system user and its home
    if [[ $INSTALL_MODE == "system" ]] && id "$SERVICE_USER" &>/dev/null; then
        log_step "Removing system user '$SERVICE_USER'..."
        userdel -r "$SERVICE_USER" 2>/dev/null || userdel "$SERVICE_USER" 2>/dev/null || true
        log_info "System user '$SERVICE_USER' removed."
    fi
else
    # Interactive / -y: offer to keep or remove
    for d in "${WIPE_DIRS[@]+"${WIPE_DIRS[@]}"}"; do
        if [[ -d "$d" ]]; then
            echo -e "Data dir ${BLUE}$d${NC} contains your config/data and will be kept."
            if [[ "$YES" == "true" ]]; then
                log_info "Data preserved at: $d (use --wipe to remove)"
            else
                read -rp "Remove $d? [y/N] " reply <"${TTY_IN:-/dev/tty}"
                if [[ "$reply" =~ ^[Yy]$ ]]; then
                    log_step "Removing $d..."
                    rm -rf "$d"
                    log_info "Removed: $d"
                else
                    log_info "Preserved: $d"
                fi
            fi
        fi
    done
fi

echo ""
log_info "Uninstall complete."
