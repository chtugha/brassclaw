#!/usr/bin/env bash
# BrassClaw install script.
# Downloads the latest (or pinned) release binary from GitHub and installs it.
# Works on Linux (amd64) and macOS (arm64, amd64).
# Run as root for a system install with a systemd service; run as a normal user
# for a user-local install without a service.
#
# Usage:
#   bash install.sh               # latest release, auto-detect arch
#   bash install.sh -v 0.41.2    # pin to a specific version
#   sudo bash install.sh          # system install + systemd service

set -euo pipefail

# ── configurable ──────────────────────────────────────────────────────────────
GITHUB_REPO="chtugha/brassclaw"
# The installed binary is always named "brassclaw-reborn" — that is the Reborn
# CLI binary.
BINARY_NAME="brassclaw-reborn"
# Legacy binary name used by older installs; cleaned up automatically on upgrade.
LEGACY_BINARY_NAME="brassclaw"
CONFIG_DIR="${BRASSCLAW_REBORN_HOME:-$HOME/.brassclaw/reborn}"
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
        # CI produces only linux-amd64, macos-arm64, macos-amd64 artifacts.
        # Linux/aarch64 is not in the build matrix; direct users to build from source.
        Linux/x86_64)       echo "brassclaw-linux-amd64" ;;
        Darwin/arm64)       echo "brassclaw-macos-arm64" ;;
        Darwin/x86_64)      echo "brassclaw-macos-amd64" ;;
        Linux/aarch64)
            log_error "Linux ARM64 pre-built binaries are not available yet." >&2
            log_info  "Build from source: cargo build --release --bin brassclaw" >&2
            log_info  "See: https://github.com/$GITHUB_REPO#building-from-source" >&2
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
        | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"v\([^"]*\)".*/\1/')
    if [[ -z "$latest" ]]; then
        log_error "Could not determine latest version. Use -v to pin a version." >&2
        exit 1
    fi
    echo "$latest"
}

# ── checksum tool ─────────────────────────────────────────────────────────────
sha256_check() {
    local file="$1" expected_file="$2"
    if command -v sha256sum &>/dev/null; then
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

    # Backup existing binary with a single .bak file (not timestamped accumulation).
    # Back up under the installed name (brassclaw-reborn), not the artifact name.
    if [[ -f "$INSTALL_DIR/$BINARY_NAME" ]]; then
        log_step "Backing up existing binary to $INSTALL_DIR/$BINARY_NAME.bak"
        cp "$INSTALL_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME.bak"
    fi

    # Install the artifact under the canonical binary name
    mv "$tmp_dir/$artifact" "$INSTALL_DIR/$BINARY_NAME"
    log_info "Installed to $INSTALL_DIR/$BINARY_NAME"

    # Remove the legacy binary if present (left behind by older installs)
    for legacy in "$INSTALL_DIR/$LEGACY_BINARY_NAME" "$INSTALL_DIR/$LEGACY_BINARY_NAME.bak"; do
        if [[ -f "$legacy" ]]; then
            rm -f "$legacy"
            log_info "Removed legacy binary: $legacy"
        fi
    done

    trap - EXIT
    rm -rf "$tmp_dir"
}

# ── config dir ────────────────────────────────────────────────────────────────
create_config_dir() {
    # In system mode the config dir is created (with correct ownership) inside
    # create_systemd_service once we know the service user; skip here.
    [[ $INSTALL_MODE == "system" ]] && return 0
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

    # Prefer the real user who called sudo. Fall back to a dedicated system
    # account because initdb (embedded Postgres) refuses to run as root.
    local service_user="${SUDO_USER:-}"
    if [[ -z "$service_user" || "$service_user" == "root" ]]; then
        service_user="brassclaw"
        if ! id "$service_user" &>/dev/null; then
            log_step "Creating system user '$service_user'..."
            useradd --system --no-create-home --shell /usr/sbin/nologin "$service_user"
        fi
    fi
    local home_dir
    home_dir=$(eval echo "~$service_user")
    # If the home doesn't exist (system user with --no-create-home), use /var/lib/brassclaw
    if [[ ! -d "$home_dir" ]] || [[ "$home_dir" == "~$service_user" ]]; then
        home_dir="/var/lib/brassclaw"
        mkdir -p "$home_dir"
        chown "$service_user:$service_user" "$home_dir"
    fi
    local reborn_home="${home_dir}/.brassclaw/reborn"
    local existing_service="$SYSTEMD_DIR/$SERVICE_NAME.service"

    # On upgrade: preserve the existing token and user_id so the operator
    # does not need to reconfigure their browser or bookmark after every
    # version update.  Only generate a fresh token on a first-time install.
    local webui_token="" webui_user_id="" is_upgrade=false
    if [[ -f "$existing_service" ]]; then
        is_upgrade=true
        webui_token=$(grep -oP '(?<=Environment=BRASSCLAW_REBORN_WEBUI_TOKEN=)\S+' "$existing_service" 2>/dev/null || true)
        webui_user_id=$(grep -oP '(?<=Environment=BRASSCLAW_REBORN_WEBUI_USER_ID=)\S+' "$existing_service" 2>/dev/null || true)
    fi
    if [[ -z "$webui_token" ]]; then
        webui_token=$(LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 40 || true)
    fi
    if [[ -z "$webui_user_id" ]]; then
        webui_user_id="brassclaw-admin"
    fi

    # Stop running instance before writing the new service file so the old
    # process releases port 3000 cleanly before systemd starts the new one.
    if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
        log_step "Stopping running service for upgrade..."
        systemctl stop "$SERVICE_NAME"
        sleep 1
    fi

    # Create the reborn config/data dir with correct ownership before writing
    # the service file (the service won't start if the dir doesn't exist).
    if [[ ! -d "$reborn_home" ]]; then
        log_step "Creating config directory: $reborn_home"
        mkdir -p "$reborn_home"
    fi

    # Pre-download and extract the pinned PostgreSQL 16 binaries during install
    # so the service does not need to fetch ~40 MB from GitHub on first boot
    # (which races against the startup timeout and always loses on a clean install).
    #
    # The archive layout from theseus-rs is:
    #   postgresql-16.4.0-x86_64-unknown-linux-gnu/bin/initdb
    #   postgresql-16.4.0-x86_64-unknown-linux-gnu/lib/...
    # --strip-components=1 flattens that top-level dir so bin/ lands directly
    # in pg_bin_dst, matching the path the binary looks for (bin_cache_dir/bin/initdb).
    local pg_bin_dst="$reborn_home/postgres/bin"
    local pg_version="16.4.0"
    local pg_artifact="postgresql-${pg_version}-x86_64-unknown-linux-gnu.tar.gz"
    local pg_url="https://github.com/theseus-rs/postgresql-binaries/releases/download/${pg_version}/${pg_artifact}"

    # Check for an existing valid cache first (upgrade path)
    local found_cache=""
    for candidate_home in /root /home/* /var/lib/brassclaw; do
        local candidate_cache="$candidate_home/.brassclaw/reborn/postgres/bin"
        if [[ -d "$candidate_cache/bin" ]] && [[ -x "$candidate_cache/bin/initdb" ]]; then
            found_cache="$candidate_cache"
            break
        fi
    done

    if [[ -x "$pg_bin_dst/bin/initdb" ]]; then
        log_info "Postgres binaries already present at $pg_bin_dst"
    elif [[ -n "$found_cache" ]]; then
        log_step "Copying cached Postgres binaries from $found_cache ..."
        mkdir -p "$pg_bin_dst"
        cp -a "$found_cache/." "$pg_bin_dst/"
    else
        log_step "Downloading PostgreSQL $pg_version binaries (~40 MB)..."
        local tmp_pg
        tmp_pg=$(mktemp -d)
        # shellcheck disable=SC2064
        trap "rm -rf $tmp_pg" EXIT
        if curl -fsSL -o "$tmp_pg/$pg_artifact" "$pg_url"; then
            mkdir -p "$pg_bin_dst"
            tar -xzf "$tmp_pg/$pg_artifact" -C "$pg_bin_dst" --strip-components=1
            log_info "PostgreSQL $pg_version extracted to $pg_bin_dst"
        else
            log_warn "Could not download PostgreSQL binaries — the service will attempt the download on first boot."
        fi
        trap - EXIT
        rm -rf "$tmp_pg"
    fi

    chown -R "$service_user:$service_user" "$reborn_home"

    # Build pgvector from source if not already present and gcc/make are available.
    # pgvector is required by the V0__shared_triggers migration (CREATE EXTENSION vector).
    local vector_so="$pg_bin_dst/lib/vector.so"
    local vector_ctl="$pg_bin_dst/share/extension/vector.control"
    if [[ ! -f "$vector_so" ]] || [[ ! -f "$vector_ctl" ]]; then
        if command -v gcc &>/dev/null && command -v make &>/dev/null; then
            log_step "Building pgvector extension for embedded PostgreSQL..."
            local tmp_vec
            tmp_vec=$(mktemp -d)
            # shellcheck disable=SC2064
            trap "rm -rf $tmp_vec" EXIT
            local pgvector_ver="0.8.0"
            local pgvector_url="https://github.com/pgvector/pgvector/archive/refs/tags/v${pgvector_ver}.tar.gz"
            if curl -fsSL -o "$tmp_vec/pgvector.tar.gz" "$pgvector_url"; then
                tar -xzf "$tmp_vec/pgvector.tar.gz" -C "$tmp_vec" --strip-components=1
                # Build against the embedded PG headers; PG_CONFIG must point to pg_config
                make -C "$tmp_vec" PG_CONFIG="$pg_bin_dst/bin/pg_config" 2>&1 | tail -5
                make -C "$tmp_vec" PG_CONFIG="$pg_bin_dst/bin/pg_config" install 2>&1 | tail -3
                chown -R "$service_user:$service_user" "$pg_bin_dst/lib" "$pg_bin_dst/share/extension" 2>/dev/null || true
                log_info "pgvector ${pgvector_ver} installed"
            else
                log_warn "Could not download pgvector — CREATE EXTENSION vector will fail."
            fi
            trap - EXIT
            rm -rf "$tmp_vec"
        else
            log_warn "gcc/make not found — skipping pgvector build. CREATE EXTENSION vector will fail."
        fi
    else
        log_info "pgvector already present at $pg_bin_dst"
    fi

    log_step "Writing $SYSTEMD_DIR/$SERVICE_NAME.service"
    cat > "$existing_service" <<EOF
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
# Embedded Postgres is managed by the brassclaw service.
# Default port: 5434 (loopback only). To override:
#   - Port:            Environment=BRASSCLAW_EMBEDDED_PG_PORT=5434
#   - Listen address:  Environment=BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES=0.0.0.0
#   Note: listen_addresses is a first-boot setting (written to postgresql.conf
#   by initdb). To change it on an existing cluster, edit
#   $reborn_home/postgres/data/postgresql.conf and restart the service.
# To use an external Postgres instead of the embedded one:
#   Environment=BRASSCLAW_PG_URL=postgresql://user:pass@host:5432/brassclaw
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
    chmod 644 "$existing_service"

    systemctl daemon-reload
    systemctl enable "$SERVICE_NAME"
    systemctl start "$SERVICE_NAME"

    # Wait up to 30 s for the service to become active. The first boot needs to
    # download and extract the embedded Postgres binaries from GitHub (~40 MB),
    # which takes longer than the previous hard-coded 3 s sleep.
    local i=0
    while [[ $i -lt 30 ]]; do
        if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
            break
        fi
        # Abort early if the service has already failed (not just still starting).
        if systemctl is-failed --quiet "$SERVICE_NAME" 2>/dev/null; then
            break
        fi
        sleep 1
        (( i++ )) || true
    done

    if systemctl is-active --quiet "$SERVICE_NAME"; then
        log_info "Service started successfully"
    else
        log_error "Service failed to start — check: journalctl -u $SERVICE_NAME -n 50"
        exit 1
    fi

    echo ""
    if [[ "$is_upgrade" == "true" ]]; then
        echo -e "${GREEN}✓  Token preserved from previous install:${NC}"
        echo -e "   ${GREEN}$webui_token${NC}"
    else
        echo -e "${YELLOW}⚠  SAVE YOUR WEBUI TOKEN:${NC}"
        echo -e "   ${GREEN}$webui_token${NC}"
    fi
    echo "   (also in $existing_service)"
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
    if [[ $INSTALL_MODE == "system" ]]; then
        # In system mode the actual config is under the service user's home,
        # not $HOME (which is root when running sudo).
        local svc_home="/var/lib/brassclaw"
        if id "$SERVICE_NAME" &>/dev/null; then
            local _h; _h=$(eval echo "~$SERVICE_NAME" 2>/dev/null || true)
            [[ -n "$_h" && -d "$_h" ]] && svc_home="$_h"
        fi
        echo -e "  Config:   $svc_home/.brassclaw/reborn"
    else
        echo -e "  Config:   $CONFIG_DIR"
    fi
    if [[ $INSTALL_MODE == "system" ]]; then
        echo -e "  Service:  systemctl {start|stop|restart|status} $SERVICE_NAME"
        echo -e "  Logs:     journalctl -u $SERVICE_NAME -f"
        echo -e "  WebUI:    http://$(hostname -I | awk '{print $1}'):3000"
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

    echo -e "${BLUE}BrassClaw installer — v$version ($artifact → $BINARY_NAME, $INSTALL_MODE mode)${NC}"
    echo ""

    download_binary "$version" "$artifact"
    create_config_dir
    create_systemd_service
    print_summary "$version"
}

main
