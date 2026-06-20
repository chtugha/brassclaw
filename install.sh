#!/usr/bin/env bash
set -euo pipefail

# BrassClaw Reborn Installation Script
# Supports both fresh installation and updates
# Preserves configuration and database during updates

VERSION="0.29.2"
GITHUB_REPO="chtugha/brassclaw"
BINARY_NAME="brassclaw-reborn"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="$HOME/.brassclaw/reborn"
SERVICE_NAME="brassclaw-reborn"
SYSTEMD_DIR="/etc/systemd/system"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# Check if running as root for systemd service
check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root for systemd service installation"
        log_info "Please run: sudo $0"
        exit 1
    fi
}

# Detect if this is an update
is_update() {
    if [[ -f "$INSTALL_DIR/$BINARY_NAME" ]]; then
        return 0
    else
        return 1
    fi
}

# Get installed version
get_installed_version() {
    if [[ -f "$INSTALL_DIR/$BINARY_NAME" ]]; then
        "$INSTALL_DIR/$BINARY_NAME" --version 2>/dev/null | grep -oP '\d+\.\d+\.\d+' || echo "unknown"
    else
        echo "none"
    fi
}

# Stop service if running
stop_service() {
    if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
        log_step "Stopping $SERVICE_NAME service..."
        systemctl stop "$SERVICE_NAME"
        log_info "Service stopped"
    fi
}

# Backup existing binary
backup_binary() {
    if [[ -f "$INSTALL_DIR/$BINARY_NAME" ]]; then
        local backup_file="$INSTALL_DIR/$BINARY_NAME.backup.$(date +%Y%m%d_%H%M%S)"
        log_step "Backing up existing binary to $backup_file..."
        cp "$INSTALL_DIR/$BINARY_NAME" "$backup_file"
        log_info "Backup created"
    fi
}

# Download binary from GitHub release
download_binary() {
    local download_url="https://github.com/$GITHUB_REPO/releases/download/v$VERSION/brassclaw-reborn-linux-amd64"
    local checksum_url="https://github.com/$GITHUB_REPO/releases/download/v$VERSION/brassclaw-reborn-linux-amd64.sha256"
    local temp_dir=$(mktemp -d)
    local download_name="brassclaw-reborn-linux-amd64"
    
    log_step "Downloading brassclaw-reborn v$VERSION..."
    
    if ! curl -L -f -o "$temp_dir/$download_name" "$download_url" 2>&1 >/dev/null; then
        log_error "Failed to download binary from $download_url"
        log_info "Please check if the release exists at: https://github.com/$GITHUB_REPO/releases/tag/v$VERSION"
        rm -rf "$temp_dir"
        exit 1
    fi
    
    log_info "Binary downloaded successfully"
    
    log_step "Downloading checksum..."
    if ! curl -L -f -o "$temp_dir/$download_name.sha256" "$checksum_url" 2>&1 >/dev/null; then
        log_warn "Checksum file not available, skipping verification"
    else
        log_step "Verifying checksum..."
        cd "$temp_dir"
        if sha256sum -c "$download_name.sha256" >/dev/null 2>&1; then
            log_info "Checksum verification passed"
        else
            log_error "Checksum verification failed"
            cd - > /dev/null
            rm -rf "$temp_dir"
            exit 1
        fi
        cd - > /dev/null
    fi
    
    log_step "Installing binary to $INSTALL_DIR..."
    chmod +x "$temp_dir/$download_name"
    mv "$temp_dir/$download_name" "$INSTALL_DIR/$BINARY_NAME"
    log_info "Binary installed successfully"
    
    rm -rf "$temp_dir"
}

# Create config directory if it doesn't exist
create_config_dir() {
    if [[ ! -d "$CONFIG_DIR" ]]; then
        log_step "Creating configuration directory at $CONFIG_DIR..."
        mkdir -p "$CONFIG_DIR"
        # Set ownership to the user who invoked sudo
        if [[ -n "${SUDO_USER:-}" ]]; then
            chown -R "$SUDO_USER:$SUDO_USER" "$CONFIG_DIR"
        fi
        log_info "Configuration directory created"
    else
        log_info "Configuration directory already exists (preserving existing data)"
    fi
}

# Create systemd service file
create_systemd_service() {
    log_step "Creating systemd service at $SYSTEMD_DIR/$SERVICE_NAME.service..."
    
    # Determine the user to run the service as
    local service_user="${SUDO_USER:-$USER}"
    local service_group="${SUDO_USER:-$USER}"
    
    cat > "$SYSTEMD_DIR/$SERVICE_NAME.service" <<EOF
[Unit]
Description=BrassClaw Reborn AI Agent
Documentation=https://github.com/chtugha/brassclaw
After=network.target network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$service_user
Group=$service_group
WorkingDirectory=$CONFIG_DIR
ExecStart=$INSTALL_DIR/$BINARY_NAME serve --host 0.0.0.0
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

# Environment variables (customize as needed)
Environment="BRASSCLAW_REBORN_WEBUI_TOKEN=change-me-$(openssl rand -hex 16)"
Environment="BRASSCLAW_REBORN_WEBUI_USER_ID=default-user"

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=$CONFIG_DIR

[Install]
WantedBy=multi-user.target
EOF

    chmod 644 "$SYSTEMD_DIR/$SERVICE_NAME.service"
    log_info "Systemd service file created"
}

# Reload systemd and enable service
enable_service() {
    log_step "Reloading systemd daemon..."
    systemctl daemon-reload
    
    log_step "Enabling $SERVICE_NAME service..."
    systemctl enable "$SERVICE_NAME"
    log_info "Service enabled (will start on boot)"
}

# Start service
start_service() {
    log_step "Starting $SERVICE_NAME service..."
    systemctl start "$SERVICE_NAME"
    
    sleep 2
    
    if systemctl is-active --quiet "$SERVICE_NAME"; then
        log_info "Service started successfully"
    else
        log_error "Service failed to start"
        log_info "Check logs with: journalctl -u $SERVICE_NAME -n 50"
        exit 1
    fi
}

# Show status
show_status() {
    echo ""
    log_step "Service status:"
    systemctl status "$SERVICE_NAME" --no-pager -l || true
}

# Print post-installation instructions
print_instructions() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Installation Complete!${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${BLUE}Installation Details:${NC}"
    echo -e "  Binary:        ${GREEN}$INSTALL_DIR/$BINARY_NAME${NC}"
    echo -e "  Config Dir:    ${GREEN}$CONFIG_DIR${NC}"
    echo -e "  Service:       ${GREEN}$SERVICE_NAME${NC}"
    echo -e "  Service File:  ${GREEN}$SYSTEMD_DIR/$SERVICE_NAME.service${NC}"
    echo ""
    echo -e "${BLUE}Useful Commands:${NC}"
    echo -e "  ${GREEN}sudo systemctl status $SERVICE_NAME${NC}     # Check service status"
    echo -e "  ${GREEN}sudo systemctl restart $SERVICE_NAME${NC}    # Restart service"
    echo -e "  ${GREEN}sudo systemctl stop $SERVICE_NAME${NC}       # Stop service"
    echo -e "  ${GREEN}sudo systemctl start $SERVICE_NAME${NC}      # Start service"
    echo -e "  ${GREEN}sudo journalctl -u $SERVICE_NAME -f${NC}     # View live logs"
    echo -e "  ${GREEN}sudo journalctl -u $SERVICE_NAME -n 50${NC}  # View last 50 log lines"
    echo ""
    echo -e "${YELLOW}⚠ IMPORTANT SECURITY NOTICE:${NC}"
    echo -e "  The service has been created with a random authentication token."
    echo -e "  To customize authentication and other settings:"
    echo ""
    echo -e "  1. Edit the service file:"
    echo -e "     ${GREEN}sudo nano $SYSTEMD_DIR/$SERVICE_NAME.service${NC}"
    echo ""
    echo -e "  2. Update the Environment variables as needed"
    echo ""
    echo -e "  3. Reload and restart the service:"
    echo -e "     ${GREEN}sudo systemctl daemon-reload${NC}"
    echo -e "     ${GREEN}sudo systemctl restart $SERVICE_NAME${NC}"
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo ""
}

# Main installation flow
main() {
    echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  BrassClaw Reborn Installation Script${NC}"
    echo -e "${BLUE}  Version: $VERSION${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo ""
    
    check_root
    
    local installed_version=$(get_installed_version)
    
    if is_update; then
        echo -e "${YELLOW}Existing installation detected${NC}"
        echo -e "  Current version: ${BLUE}$installed_version${NC}"
        echo -e "  New version:     ${BLUE}$VERSION${NC}"
        echo ""
        log_info "Performing update (configuration and data will be preserved)"
        echo ""
        stop_service
        backup_binary
    else
        log_info "Performing fresh installation"
        echo ""
    fi
    
    download_binary
    create_config_dir
    create_systemd_service
    enable_service
    start_service
    
    print_instructions
    show_status
}

# Run main function
main "$@"

# Made with Bob
