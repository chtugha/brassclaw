#!/bin/sh
# BrassClaw Uninstallation Script
# Removes BrassClaw binary, configuration, data, and systemd services

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BINARY_NAME="brassclaw"
INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.brassclaw"
DATA_DIR="${HOME}/.local/share/brassclaw"
SERVICE_DIR="${HOME}/.config/systemd/user"
SERVICE_FILE="${SERVICE_DIR}/brassclaw.service"

# Parse arguments
FORCE=false
for arg in "$@"; do
    case $arg in
        --force|-f)
            FORCE=true
            shift
            ;;
    esac
done

# Confirmation prompt
confirm_uninstall() {
    if [ "$FORCE" = true ]; then
        return 0
    fi
    
    echo "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo "${BLUE}  BrassClaw Uninstaller${NC}"
    echo "${BLUE}═══════════════════════════════════════════════════${NC}"
    echo ""
    echo "${YELLOW}This will remove:${NC}"
    echo "  • Binary: ${INSTALL_DIR}/${BINARY_NAME}"
    echo "  • Config: ${CONFIG_DIR}"
    echo "  • Data: ${DATA_DIR}"
    echo "  • Systemd service (if installed)"
    echo ""
    printf "${RED}Are you sure you want to uninstall BrassClaw? (y/N): ${NC}"
    read -r response
    
    if [ "$response" != "y" ] && [ "$response" != "Y" ]; then
        echo "${YELLOW}Uninstallation cancelled${NC}"
        exit 0
    fi
}

# Stop and disable systemd service
remove_systemd_service() {
    if [ ! -f "$SERVICE_FILE" ]; then
        return 0
    fi
    
    echo "${BLUE}Removing systemd service...${NC}"
    
    # Check if systemctl is available
    if command -v systemctl > /dev/null 2>&1; then
        # Stop service if running
        if systemctl --user is-active --quiet brassclaw 2>/dev/null; then
            echo "  Stopping service..."
            systemctl --user stop brassclaw || true
        fi
        
        # Disable service if enabled
        if systemctl --user is-enabled --quiet brassclaw 2>/dev/null; then
            echo "  Disabling service..."
            systemctl --user disable brassclaw || true
        fi
        
        # Remove service file
        rm -f "$SERVICE_FILE"
        
        # Reload systemd
        systemctl --user daemon-reload || true
        
        echo "${GREEN}✓ Systemd service removed${NC}"
    else
        # Just remove the file if systemctl not available
        rm -f "$SERVICE_FILE"
        echo "${GREEN}✓ Service file removed${NC}"
    fi
}

# Remove binary
remove_binary() {
    if [ -f "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        echo "${BLUE}Removing binary...${NC}"
        rm -f "${INSTALL_DIR}/${BINARY_NAME}"
        echo "${GREEN}✓ Binary removed${NC}"
    else
        echo "${YELLOW}Binary not found (already removed?)${NC}"
    fi
}

# Remove configuration
remove_config() {
    if [ -d "$CONFIG_DIR" ]; then
        echo "${BLUE}Removing configuration...${NC}"
        rm -rf "$CONFIG_DIR"
        echo "${GREEN}✓ Configuration removed${NC}"
    else
        echo "${YELLOW}Configuration directory not found${NC}"
    fi
}

# Remove data
remove_data() {
    if [ -d "$DATA_DIR" ]; then
        echo "${BLUE}Removing data...${NC}"
        rm -rf "$DATA_DIR"
        echo "${GREEN}✓ Data removed${NC}"
    else
        echo "${YELLOW}Data directory not found${NC}"
    fi
}

# Main uninstallation
main() {
    confirm_uninstall
    
    echo ""
    echo "${BLUE}Starting uninstallation...${NC}"
    echo ""
    
    remove_systemd_service
    remove_binary
    remove_config
    remove_data
    
    echo ""
    echo "${GREEN}═══════════════════════════════════════════════════${NC}"
    echo "${GREEN}  BrassClaw has been uninstalled${NC}"
    echo "${GREEN}═══════════════════════════════════════════════════${NC}"
    echo ""
    echo "Thank you for using BrassClaw!"
    echo ""
}

main "$@"

# Made with Bob
