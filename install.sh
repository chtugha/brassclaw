#!/usr/bin/env bash
# BrassClaw install script.
# Downloads the latest (or pinned) release binary from GitHub and installs it.
# Works on Linux (amd64, arm64) and macOS (arm64, amd64).
# Run as root for a system install with a systemd service; run as a normal user
# for a user-local install without a service.
#
# Usage:
#   bash install.sh               # latest release, auto-detect arch
#   bash install.sh -v 0.30.6    # pin to a specific version
#   sudo bash install.sh          # system install + systemd service

set -euo pipefail

# ── configurable ──────────────────────────────────────────────────────────────
GITHUB_REPO="chtugha/brassclaw"
BINARY_NAME="brassclaw"
CONFIG_DIR="${BRASSCLAW_REBORN_HOME:-$HOME/.brassclaw/reborn}"
SERVICE_NAME="brassclaw"
SYSTEMD_DIR="/etc/systemd/system"
# ─────────────────────────────────────────────────────────────────────────────

# ── parse flags ──────────────────────────────────────────────────────────────
PINNED_VERSION=""
while getopts "v:" opt; do
    case $opt in
        v) PINNED_VERSION="$OPTARG" ;;
        *) echo "Usage: $0 [-v version]"; exit 1 ;;
    esac
done

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
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }
log_step()  { echo -e "${BLUE}[STEP]${NC}  $*"; }

# ── detect platform ───────────────────────────────────────────────────────────
detect_artifact() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$os/$arch" in
        Linux/x86_64)       echo "brassclaw-linux-amd64" ;;
        Linux/aarch64)      echo "brassclaw-linux-arm64" ;;
        Darwin/arm64)       echo "brassclaw-macos-arm64" ;;
        Darwin/x86_64)      echo "brassclaw-macos-amd64" ;;
        *)
            log_error "Unsupported platform: $os/$arch"
            log_info  "Build from source: cargo build --release --bin brassclaw"
            exit 1 ;;
    esac
}

# ── resolve version ───────────────────────────────────────────────────────────
resolve_version() {
    if [[ -n "$PINNED_VERSION" ]]; then
        echo "$PINNED_VERSION"
        return
    fi
    log_step "Fetching latest release version from GitHub..."
    local latest
    latest=$(curl -fsSL "https://api.github.com/repos/$GITHUB_REPO/releases/latest" \
        | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"v\([^"]*\)".*/\1/')
    if [[ -z "$latest" ]]; then
        log_error "Could not determine latest version. Use -v to pin a version."
        exit 1
    fi
    echo "$latest"
}

# ── checksum tool ─────────────────────────────────────────────────────────────
sha256_check() {
    local file="$1" expected_file="$2"
    if command -v sha256sum &>/dev/null; then
        # expected_file contains "<hash>  <filename>" — rewrite filename to match
        local hash
        hash=$(awk '{print $1}' "$expected_file")
        echo "$hash  $file" | sha256sum -c - >/dev/null
    elif command -v shasum &>/dev/null; then
        local hash
        hash=$(awk '{print $1}' "$expected_file")
        echo "$hash  $file" | shasum -a 256 -c - >/dev/null
    else
        log_warn "No sha256 tool found — skipping checksum verification."
        return 0
    fi
}

# ── download ──────────────────────────────────────────────────────────────────
download_binary() {
    local version="$1" artifact="$2"
    local base_url="https://github.com/$GITHUB_REPO/releases/download/v$version"
    local tmp_dir
    tmp_dir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf $tmp_dir" EXIT

    log_step "Downloading $artifact v$version..."
    if ! curl -fsSL -o "$tmp_dir/$artifact" "$base_url/$artifact"; then
        log_error "Download failed: $base_url/$artifact"
        log_info  "Check releases at: https://github.com/$GITHUB_REPO/releases/tag/v$version"
        exit 1
    fi

    if curl -fsSL -o "$tmp_dir/$artifact.sha256" "$base_url/$artifact.sha256" 2>/dev/null; then
        log_step "Verifying checksum..."
        if sha256_check "$tmp_dir/$artifact" "$tmp_dir/$artifact.sha256"; then
            log_info "Checksum OK"
        else
            log_error "Checksum mismatch — aborting."
            exit 1
        fi
    else
        log_warn "No checksum file available for this release — skipping verification."
    fi

    mkdir -p "$INSTALL_DIR"
    chmod +x "$tmp_dir/$artifact"

    # Backup existing binary with a single .bak file (not timestamped accumulation)
    if [[ -f "$INSTALL_DIR/$BINARY_NAME" ]]; then
        log_step "Backing up existing binary to $INSTALL_DIR/$BINARY_NAME.bak"
        cp "$INSTALL_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME.bak"
    fi

    mv "$tmp_dir/$artifact" "$INSTALL_DIR/$BINARY_NAME"
    log_info "Installed to $INSTALL_DIR/$BINARY_NAME"
    trap - EXIT
    rm -rf "$tmp_dir"
}

# ── config dir ────────────────────────────────────────────────────────────────
create_config_dir() {
    if [[ ! -d "$CONFIG_DIR" ]]; then
        log_step "Creating config directory: $CONFIG_DIR"
        mkdir -p "$CONFIG_DIR"
    else
        log_info "Config directory already exists: $CONFIG_DIR"
    fi
}

# ── systemd service ───────────────────────────────────────────────────────────
create_systemd_service() {
    [[ $INSTALL_MODE != "system" ]] && return 0
    command -v systemctl &>/dev/null || { log_warn "systemctl not found — skipping service install."; return 0; }

    local service_user="${SUDO_USER:-root}"
    local home_dir
    home_dir=$(eval echo "~$service_user")
    local reborn_home="${home_dir}/.brassclaw/reborn"
    local webui_token
    webui_token=$(LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 40)

    log_step "Writing $SYSTEMD_DIR/$SERVICE_NAME.service"
    cat > "$SYSTEMD_DIR/$SERVICE_NAME.service" <<EOF
[Unit]
Description=BrassClaw AI Agent
Documentation=https://github.com/$GITHUB_REPO
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$service_user
WorkingDirectory=$reborn_home
Environment=BRASSCLAW_REBORN_HOME=$reborn_home
Environment=BRASSCLAW_REBORN_WEBUI_TOKEN=$webui_token
Environment=BRASSCLAW_REBORN_WEBUI_USER_ID=brassclaw-admin
ExecStart=$INSTALL_DIR/$BINARY_NAME serve --host 0.0.0.0 --port 3000
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=$reborn_home

[Install]
WantedBy=multi-user.target
EOF
    chmod 644 "$SYSTEMD_DIR/$SERVICE_NAME.service"

    # Stop running instance before daemon-reload
    if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
        log_step "Stopping running service for upgrade..."
        systemctl stop "$SERVICE_NAME"
    fi

    systemctl daemon-reload
    systemctl enable "$SERVICE_NAME"
    systemctl start "$SERVICE_NAME"
    sleep 2

    if systemctl is-active --quiet "$SERVICE_NAME"; then
        log_info "Service started successfully"
    else
        log_error "Service failed to start — check: journalctl -u $SERVICE_NAME -n 50"
        exit 1
    fi

    echo ""
    echo -e "${YELLOW}⚠  SAVE YOUR WEBUI TOKEN:${NC}"
    echo -e "   ${GREEN}$webui_token${NC}"
    echo "   (also in $SYSTEMD_DIR/$SERVICE_NAME.service)"
    echo ""
}

# ── post-install summary ──────────────────────────────────────────────────────
print_summary() {
    local version="$1"
    echo ""
    echo -e "${BLUE}══════════════════════════════════════════${NC}"
    echo -e "${GREEN}  BrassClaw v$version installed!${NC}"
    echo -e "${BLUE}══════════════════════════════════════════${NC}"
    echo -e "  Binary:   $INSTALL_DIR/$BINARY_NAME"
    echo -e "  Config:   $CONFIG_DIR"
    if [[ $INSTALL_MODE == "system" ]]; then
        echo -e "  Service:  systemctl {start|stop|restart|status} $SERVICE_NAME"
        echo -e "  Logs:     journalctl -u $SERVICE_NAME -f"
    else
        echo ""
        echo -e "${BLUE}Run:${NC}"
        echo -e "  BRASSCLAW_REBORN_WEBUI_TOKEN=<token> \\"
        echo -e "  BRASSCLAW_REBORN_WEBUI_USER_ID=me \\"
        echo -e "  $BINARY_NAME serve"
        if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
            echo ""
            echo -e "${YELLOW}Add to PATH:${NC}"
            echo -e "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
        fi
    fi
    echo -e "${BLUE}══════════════════════════════════════════${NC}"
    echo ""
}

# ── main ──────────────────────────────────────────────────────────────────────
main() {
    local version artifact
    version=$(resolve_version)
    artifact=$(detect_artifact)

    echo -e "${BLUE}BrassClaw installer — v$version ($artifact, $INSTALL_MODE mode)${NC}"
    echo ""

    download_binary "$version" "$artifact"
    create_config_dir
    create_systemd_service
    print_summary "$version"
}

main
