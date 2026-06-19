#!/bin/sh
# BrassClaw Uninstallation Script
# Removes BrassClaw binary, configuration, and data directories

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
BINARY_NAME="brassclaw"
INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.brassclaw"
DATA_DIR="${HOME}/.local/share/brassclaw"
CACHE_DIR="${HOME}/.cache/brassclaw"

# Service/daemon locations (if applicable)
LAUNCHD_PLIST="${HOME}/Library/LaunchAgents/com.brassclaw.agent.plist"
SYSTEMD_SERVICE="${HOME}/.config/systemd/user/brassclaw.service"

echo "${YELLOW}BrassClaw Uninstaller${NC}"
echo "====================="
echo ""

# Function to stop running services
stop_services() {
    echo "${YELLOW}Checking for running services...${NC}"
    
    # Check for macOS LaunchAgent
    if [ -f "$LAUNCHD_PLIST" ]; then
        echo "Stopping macOS LaunchAgent..."
        launchctl unload "$LAUNCHD_PLIST" 2>/dev/null || true
    fi
    
    # Check for systemd service
    if [ -f "$SYSTEMD_SERVICE" ]; then
        echo "Stopping systemd service..."
        systemctl --user stop brassclaw.service 2>/dev/null || true
        systemctl --user disable brassclaw.service 2>/dev/null || true
    fi
    
    # Kill any running brassclaw processes
    if pgrep -x "$BINARY_NAME" > /dev/null; then
        echo "Stopping running BrassClaw processes..."
        pkill -x "$BINARY_NAME" || true
        sleep 2
        # Force kill if still running
        if pgrep -x "$BINARY_NAME" > /dev/null; then
            pkill -9 -x "$BINARY_NAME" || true
        fi
    fi
}

# Function to remove files/directories
remove_path() {
    local path="$1"
    local description="$2"
    
    if [ -e "$path" ]; then
        echo "Removing ${description}: ${path}"
        rm -rf "$path"
        echo "${GREEN}✓ Removed${NC}"
    else
        echo "Not found: ${path} (skipping)"
    fi
}

# Main uninstallation
main() {
    echo "This will remove BrassClaw and all its data from your system."
    echo ""
    echo "The following will be removed:"
    echo "  - Binary: ${INSTALL_DIR}/${BINARY_NAME}"
    echo "  - Configuration: ${CONFIG_DIR}"
    echo "  - Data: ${DATA_DIR}"
    echo "  - Cache: ${CACHE_DIR}"
    
    if [ -f "$LAUNCHD_PLIST" ]; then
        echo "  - LaunchAgent: ${LAUNCHD_PLIST}"
    fi
    
    if [ -f "$SYSTEMD_SERVICE" ]; then
        echo "  - Systemd service: ${SYSTEMD_SERVICE}"
    fi
    
    echo ""
    
    # Ask for confirmation unless --force flag is provided
    if [ "$1" != "--force" ] && [ "$1" != "-f" ]; then
        printf "Do you want to continue? [y/N] "
        read -r response
        case "$response" in
            [yY][eE][sS]|[yY]) 
                echo ""
                ;;
            *)
                echo "${YELLOW}Uninstallation cancelled.${NC}"
                exit 0
                ;;
        esac
    fi
    
    # Stop services first
    stop_services
    echo ""
    
    # Remove binary
    remove_path "${INSTALL_DIR}/${BINARY_NAME}" "binary"
    echo ""
    
    # Remove configuration
    remove_path "$CONFIG_DIR" "configuration directory"
    echo ""
    
    # Remove data
    remove_path "$DATA_DIR" "data directory"
    echo ""
    
    # Remove cache
    remove_path "$CACHE_DIR" "cache directory"
    echo ""
    
    # Remove service files
    if [ -f "$LAUNCHD_PLIST" ]; then
        remove_path "$LAUNCHD_PLIST" "LaunchAgent plist"
        echo ""
    fi
    
    if [ -f "$SYSTEMD_SERVICE" ]; then
        remove_path "$SYSTEMD_SERVICE" "systemd service"
        echo ""
    fi
    
    # Check for any remaining processes
    if pgrep -x "$BINARY_NAME" > /dev/null; then
        echo "${RED}Warning: BrassClaw processes are still running${NC}"
        echo "You may need to manually kill them or restart your system."
    fi
    
    echo "${GREEN}✓ BrassClaw has been successfully uninstalled!${NC}"
    echo ""
    echo "Note: If you added ${INSTALL_DIR} to your PATH, you may want to remove it from your shell profile."
}

# Show help
show_help() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -f, --force    Skip confirmation prompt"
    echo "  -h, --help     Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0              # Interactive uninstallation"
    echo "  $0 --force      # Uninstall without confirmation"
}

# Parse arguments
case "$1" in
    -h|--help)
        show_help
        exit 0
        ;;
    -f|--force)
        main --force
        ;;
    "")
        main
        ;;
    *)
        echo "${RED}Error: Unknown option '$1'${NC}"
        echo ""
        show_help
        exit 1
        ;;
esac

# Made with Bob