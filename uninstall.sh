#!/usr/bin/env bash
set -euo pipefail

# BrassClaw Reborn Uninstallation Script
# Removes binary, systemd service, and optionally configuration/data

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

# Check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root to remove systemd service"
        log_info "Please run: sudo $0"
        exit 1
    fi
}

# Confirm uninstallation
confirm_uninstall() {
    echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  BrassClaw Reborn Uninstallation${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${YELLOW}This will remove:${NC}"
    echo -e "  • Binary: ${RED}$INSTALL_DIR/$BINARY_NAME${NC}"
    echo -e "  • Systemd service: ${RED}$SYSTEMD_DIR/$SERVICE_NAME.service${NC}"
    echo ""
    echo -e "${YELLOW}Configuration and data will be preserved by default.${NC}"
    echo ""
    read -p "Continue with uninstallation? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Uninstallation cancelled"
        exit 0
    fi
}

# Stop and disable service
stop_and_disable_service() {
    if [[ -f "$SYSTEMD_DIR/$SERVICE_NAME.service" ]]; then
        log_step "Managing systemd service..."
        
        # Stop service if running
        if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
            log_info "Stopping $SERVICE_NAME service..."
            systemctl stop "$SERVICE_NAME"
        fi
        
        # Disable service if enabled
        if systemctl is-enabled --quiet "$SERVICE_NAME" 2>/dev/null; then
            log_info "Disabling $SERVICE_NAME service..."
            systemctl disable "$SERVICE_NAME"
        fi
        
        log_info "Service stopped and disabled"
    else
        log_info "Systemd service not found (already removed or never installed)"
    fi
}

# Remove service file
remove_service_file() {
    if [[ -f "$SYSTEMD_DIR/$SERVICE_NAME.service" ]]; then
        log_step "Removing systemd service file..."
        rm "$SYSTEMD_DIR/$SERVICE_NAME.service"
        systemctl daemon-reload
        log_info "Service file removed"
    fi
}

# Remove binary
remove_binary() {
    if [[ -f "$INSTALL_DIR/$BINARY_NAME" ]]; then
        log_step "Removing binary..."
        rm "$INSTALL_DIR/$BINARY_NAME"
        log_info "Binary removed"
    else
        log_warn "Binary not found at $INSTALL_DIR/$BINARY_NAME"
    fi
    
    # Remove backup files if they exist
    local backup_count=$(find "$INSTALL_DIR" -name "$BINARY_NAME.backup.*" 2>/dev/null | wc -l)
    if [[ $backup_count -gt 0 ]]; then
        log_step "Removing backup files..."
        rm -f "$INSTALL_DIR/$BINARY_NAME.backup."*
        log_info "Removed $backup_count backup file(s)"
    fi
}

# Ask about config removal
remove_config_prompt() {
    echo ""
    echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
    echo -e "${YELLOW}  Configuration and Data${NC}"
    echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
    echo ""
    
    if [[ -d "$CONFIG_DIR" ]]; then
        echo -e "Configuration directory: ${BLUE}$CONFIG_DIR${NC}"
        echo ""
        echo "This directory may contain:"
        echo "  • Configuration files"
        echo "  • Database files"
        echo "  • Logs and other data"
        echo ""
        read -p "Remove configuration directory? [y/N] " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            log_step "Removing configuration directory..."
            rm -rf "$CONFIG_DIR"
            log_info "Configuration directory removed"
        else
            log_info "Configuration directory preserved at: $CONFIG_DIR"
        fi
    else
        log_info "Configuration directory not found (already removed or never created)"
    fi
}

# Print completion message
print_completion() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Uninstallation Complete!${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo ""
    
    if [[ -d "$CONFIG_DIR" ]]; then
        echo -e "${YELLOW}Note:${NC} Configuration preserved at: ${BLUE}$CONFIG_DIR${NC}"
        echo -e "      To remove manually: ${RED}rm -rf $CONFIG_DIR${NC}"
        echo ""
    fi
    
    echo "Thank you for using BrassClaw Reborn!"
    echo ""
}

# Main uninstallation flow
main() {
    check_root
    confirm_uninstall
    
    echo ""
    log_info "Starting uninstallation..."
    echo ""
    
    stop_and_disable_service
    remove_service_file
    remove_binary
    remove_config_prompt
    
    print_completion
}

# Run main function
main "$@"

# Made with Bob
