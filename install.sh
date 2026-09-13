#!/usr/bin/env bash
# BrassClaw install script.
# Downloads the latest (or pinned) release binary from GitHub and installs it.
# Works on Linux (amd64) and macOS (arm64, amd64).
#
# The binary is self-contained: PostgreSQL 16 binaries AND the pgvector
# extension are compiled in at build time. No Postgres installation, no gcc,
# no make, no network access beyond downloading the binary itself.
#
# Run as root for a system install with a systemd service; run as a normal user
# for a user-local install without a service.
#
# Usage:
#   bash install.sh               # latest release, auto-detect arch
#   bash install.sh -v 0.9.0     # pin to a specific version
#   sudo bash install.sh          # system install + systemd service

set -euo pipefail

# ── configurable ──────────────────────────────────────────────────────────────
GITHUB_REPO="chtugha/brassclaw"
BINARY_NAME="brassclaw-reborn"
LEGACY_BINARY_NAME="brassclaw"
SERVICE_NAME="brassclaw"
SYSTEMD_DIR="/etc/systemd/system"
# ─────────────────────────────────────────────────────────────────────────────

# ── parse flags ───────────────────────────────────────────────────────────────
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
        Linux/x86_64)   echo "brassclaw-linux-amd64" ;;
        Darwin/arm64)   echo "brassclaw-macos-arm64" ;;
        Darwin/x86_64)  echo "brassclaw-macos-amd64" ;;
        Linux/aarch64)
            log_error "Linux ARM64 pre-built binaries are not in the release matrix yet." >&2
            log_info  "To build from source:" >&2
            log_info  "  cargo build --release --bin brassclaw" >&2
            log_info  "  sudo cp target/release/brassclaw /usr/local/bin/brassclaw-reborn" >&2
            log_info  "Then re-run: sudo bash install.sh (it will find the binary already installed)" >&2
            exit 1 ;;
        *)
            log_error "Unsupported platform: $os/$arch" >&2
            log_info  "Build from source: cargo build --release --bin brassclaw" >&2
            exit 1 ;;
    esac
}

# ── resolve version ───────────────────────────────────────────────────────────
resolve_version() {
    if [[ -n "$PINNED_VERSION" ]]; then
        echo "$PINNED_VERSION"
        return
    fi
    log_step "Fetching latest release version from GitHub..." >&2
    local latest
    latest=$(curl -fsSL "https://api.github.com/repos/$GITHUB_REPO/releases/latest" \
        | grep '"tag_name"' \
        | sed 's/.*"tag_name": *"v\([^"]*\)".*/\1/' \
        | tr -d '[:space:]')
    if [[ -z "$latest" ]]; then
        log_error "Could not determine latest version. Use -v to pin a version." >&2
        log_info  "Example: bash install.sh -v 0.9.0" >&2
        exit 1
    fi
    echo "$latest"
}

# ── checksum verification ─────────────────────────────────────────────────────
sha256_check() {
    local file="$1" expected_file="$2"
    local hash
    hash=$(awk '{print $1}' "$expected_file")
    if command -v sha256sum &>/dev/null; then
        echo "$hash  $file" | sha256sum -c - >/dev/null
    elif command -v shasum &>/dev/null; then
        echo "$hash  $file" | shasum -a 256 -c - >/dev/null
    else
        log_warn "No sha256 tool found — skipping checksum verification." >&2
    fi
}

# ── download binary ───────────────────────────────────────────────────────────
download_binary() {
    local version="$1" artifact="$2"
    local base_url="https://github.com/$GITHUB_REPO/releases/download/v$version"
    local tmp_dir
    tmp_dir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp_dir'" EXIT

    log_step "Downloading $artifact v$version..."
    if ! curl -fsSL --retry 3 --retry-connrefused \
            -o "$tmp_dir/$artifact" "$base_url/$artifact"; then
        log_error "Download failed: $base_url/$artifact" >&2
        log_info  "Check available releases: https://github.com/$GITHUB_REPO/releases/tag/v$version" >&2
        exit 1
    fi

    if curl -fsSL --retry 3 -o "$tmp_dir/$artifact.sha256" \
            "$base_url/$artifact.sha256" 2>/dev/null; then
        log_step "Verifying checksum..."
        if sha256_check "$tmp_dir/$artifact" "$tmp_dir/$artifact.sha256"; then
            log_info "Checksum OK"
        else
            log_error "Checksum mismatch — the download may be corrupt. Aborting." >&2
            exit 1
        fi
    else
        log_warn "No checksum file found for this release — skipping verification."
    fi

    mkdir -p "$INSTALL_DIR"
    chmod +x "$tmp_dir/$artifact"

    # Back up current binary (single .bak, not accumulating timestamped copies).
    if [[ -f "$INSTALL_DIR/$BINARY_NAME" ]]; then
        log_step "Backing up existing binary to $INSTALL_DIR/$BINARY_NAME.bak"
        cp "$INSTALL_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME.bak"
    fi

    mv "$tmp_dir/$artifact" "$INSTALL_DIR/$BINARY_NAME"
    log_info "Installed: $INSTALL_DIR/$BINARY_NAME"

    # Remove legacy binaries left behind by older installs.
    for legacy in \
        "$INSTALL_DIR/$LEGACY_BINARY_NAME" \
        "$INSTALL_DIR/$LEGACY_BINARY_NAME.bak"; do
        if [[ -f "$legacy" ]]; then
            rm -f "$legacy"
            log_info "Removed legacy binary: $legacy"
        fi
    done

    trap - EXIT
    rm -rf "$tmp_dir"
}

# ── config dir (user-mode only) ───────────────────────────────────────────────
create_config_dir() {
    [[ $INSTALL_MODE == "system" ]] && return 0
    local config_dir="${BRASSCLAW_REBORN_HOME:-$HOME/.brassclaw/reborn}"
    if [[ ! -d "$config_dir" ]]; then
        log_step "Creating config directory: $config_dir"
        mkdir -p "$config_dir"
    else
        log_info "Config directory already exists: $config_dir"
    fi
}

# ── systemd service (system-mode only) ───────────────────────────────────────
create_systemd_service() {
    [[ $INSTALL_MODE != "system" ]] && return 0
    if ! command -v systemctl &>/dev/null; then
        log_warn "systemctl not found — skipping service install."
        log_info  "To start manually: $INSTALL_DIR/$BINARY_NAME serve --host 0.0.0.0 --port 3000"
        return 0
    fi

    # Use the real user who invoked sudo.  Fall back to a dedicated 'brassclaw'
    # system account.  initdb refuses to run as root, so root is never allowed.
    local service_user="${SUDO_USER:-}"
    if [[ -z "$service_user" || "$service_user" == "root" ]]; then
        service_user="brassclaw"
        if ! id "$service_user" &>/dev/null; then
            log_step "Creating system user '$service_user'..."
            useradd --system --no-create-home --shell /usr/sbin/nologin "$service_user"
        fi
    fi

    # Resolve the service user's home directory.
    local home_dir
    home_dir=$(eval echo "~$service_user" 2>/dev/null || true)
    # System users created with --no-create-home have no real home; use /var/lib/brassclaw.
    if [[ -z "$home_dir" || "$home_dir" == "~$service_user" || ! -d "$home_dir" ]]; then
        home_dir="/var/lib/brassclaw"
        mkdir -p "$home_dir"
        chown "$service_user:$service_user" "$home_dir"
    fi

    local reborn_home="${home_dir}/.brassclaw/reborn"
    local service_file="$SYSTEMD_DIR/$SERVICE_NAME.service"

    # On upgrade: preserve the existing WebUI token and user_id so operator
    # bookmarks and client config continue to work after a version update.
    local webui_token="" webui_user_id="" is_upgrade=false
    if [[ -f "$service_file" ]]; then
        is_upgrade=true
        webui_token=$(grep -oP '(?<=Environment=BRASSCLAW_REBORN_WEBUI_TOKEN=)\S+' \
            "$service_file" 2>/dev/null || true)
        webui_user_id=$(grep -oP '(?<=Environment=BRASSCLAW_REBORN_WEBUI_USER_ID=)\S+' \
            "$service_file" 2>/dev/null || true)
    fi
    if [[ -z "$webui_token" ]]; then
        webui_token=$(LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | dd bs=40 count=1 2>/dev/null || \
                      LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | fold -w 40 | head -n 1)
    fi
    if [[ -z "$webui_user_id" ]]; then
        webui_user_id="brassclaw-admin"
    fi

    # Stop a running instance so the old process releases port 3000 before
    # systemd starts the new one.
    if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
        log_step "Stopping running service for upgrade..."
        systemctl stop "$SERVICE_NAME"
        sleep 1
    fi

    # Create the data directory before writing the service file.
    if [[ ! -d "$reborn_home" ]]; then
        log_step "Creating data directory: $reborn_home"
        mkdir -p "$reborn_home"
    fi
    chown -R "$service_user:$service_user" "$reborn_home"

    # NOTE: No Postgres download, extraction, or pgvector build steps here.
    # The binary is fully self-contained: PostgreSQL 16 binaries and the
    # pgvector extension are embedded in the binary at compile time (build.rs
    # + include_bytes!).  They are extracted from the binary on first boot
    # into $BRASSCLAW_REBORN_HOME/postgres/bin — no network access, no gcc,
    # no make required.

    log_step "Writing $service_file"
    cat > "$service_file" <<EOF
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
Environment=BRASSCLAW_RUNTIME_PROFILE=local_dev
Environment=BRASSCLAW_REBORN_WEBUI_TOKEN=$webui_token
Environment=BRASSCLAW_REBORN_WEBUI_USER_ID=$webui_user_id
# Embedded Postgres listens on 127.0.0.1:5434 by default (loopback only).
# To change the port:    Environment=BRASSCLAW_EMBEDDED_PG_PORT=5434
# To allow LAN access (first-boot only — written once to postgresql.conf by initdb):
#                        Environment=BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES=0.0.0.0
#   After first boot edit \$BRASSCLAW_REBORN_HOME/postgres/data/postgresql.conf directly.
# To use an external Postgres instead of the embedded one:
#                        Environment=BRASSCLAW_PG_URL=postgresql://user:pass@host:5432/brassclaw
ExecStart=$INSTALL_DIR/$BINARY_NAME serve --host 0.0.0.0 --port 3000
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=$reborn_home $home_dir /tmp

[Install]
WantedBy=multi-user.target
EOF
    chmod 640 "$service_file"

    systemctl daemon-reload
    systemctl enable "$SERVICE_NAME"
    systemctl start "$SERVICE_NAME"

    # Wait up to 45 s for the service to become active.  On first boot the binary
    # extracts the embedded Postgres archive from itself (~40 MB decompression),
    # runs initdb, starts the server, runs migrations, and then starts the web
    # server — all without any network access.
    log_step "Waiting for service to start (first boot extracts embedded Postgres)..."
    local i=0
    while [[ $i -lt 45 ]]; do
        if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
            break
        fi
        if systemctl is-failed --quiet "$SERVICE_NAME" 2>/dev/null; then
            break
        fi
        sleep 1
        (( i++ )) || true
    done

    if systemctl is-active --quiet "$SERVICE_NAME"; then
        log_info "Service started successfully"
    else
        log_error "Service failed to start — check: journalctl -u $SERVICE_NAME -n 50" >&2
        log_info  "Common causes:" >&2
        log_info  "  - BRASSCLAW_REBORN_WEBUI_TOKEN not set (already set in service file)" >&2
        log_info  "  - Port 3000 already in use (change with --port in ExecStart)" >&2
        log_info  "  - Disk full (binary extracts ~200 MB of Postgres data on first boot)" >&2
        exit 1
    fi

    echo ""
    if [[ "$is_upgrade" == "true" ]]; then
        echo -e "${GREEN}✓  Upgraded — existing WebUI token preserved:${NC}"
        echo -e "   ${GREEN}$webui_token${NC}"
    else
        echo -e "${YELLOW}⚠  Save your WebUI token (needed to log in):${NC}"
        echo -e "   ${GREEN}$webui_token${NC}"
    fi
    echo "   (also stored in $service_file)"
    echo ""
}

# ── post-install summary ──────────────────────────────────────────────────────
print_summary() {
    local version="$1"
    local config_dir
    echo ""
    echo -e "${BLUE}══════════════════════════════════════════${NC}"
    echo -e "${GREEN}  BrassClaw v$version installed!${NC}"
    echo -e "${BLUE}══════════════════════════════════════════${NC}"
    echo -e "  Binary:   $INSTALL_DIR/$BINARY_NAME"

    if [[ $INSTALL_MODE == "system" ]]; then
        # Resolve the service user's data dir for display.
        local svc_home="/var/lib/brassclaw"
        if id "$SERVICE_NAME" &>/dev/null; then
            local _h; _h=$(eval echo "~$SERVICE_NAME" 2>/dev/null || true)
            [[ -n "$_h" && -d "$_h" ]] && svc_home="$_h"
        fi
        config_dir="$svc_home/.brassclaw/reborn"
        echo -e "  Data:     $config_dir"
        echo -e "  Service:  systemctl {start|stop|restart|status} $SERVICE_NAME"
        echo -e "  Logs:     journalctl -u $SERVICE_NAME -f"
        local ip
        ip=$(hostname -I 2>/dev/null | awk '{print $1}' || echo "localhost")
        echo -e "  WebUI:    http://${ip}:3000"
    else
        config_dir="${BRASSCLAW_REBORN_HOME:-$HOME/.brassclaw/reborn}"
        echo -e "  Data:     $config_dir"
        echo ""
        echo -e "${BLUE}To start:${NC}"
        echo -e "  BRASSCLAW_REBORN_WEBUI_TOKEN=<token> \\"
        echo -e "  BRASSCLAW_REBORN_WEBUI_USER_ID=me \\"
        echo -e "  $BINARY_NAME serve"
        if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
            echo ""
            echo -e "${YELLOW}Add to PATH:${NC}"
            echo -e "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
            echo -e "  source ~/.bashrc"
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

    echo -e "${BLUE}BrassClaw installer — v$version ($artifact → $BINARY_NAME, $INSTALL_MODE mode)${NC}"
    echo ""

    download_binary "$version" "$artifact"
    create_config_dir
    create_systemd_service
    print_summary "$version"
}

main
