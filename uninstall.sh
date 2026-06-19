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
LOG_DIR="${HOME}/.local/state/brassclaw"
STATE_DIR="${HOME}/.local/state/brassclaw"

# Service/daemon locations
LAUNCHD_PLIST="${HOME}/Library/LaunchAgents/com.brassclaw.agent.plist"
SYSTEMD_USER_SERVICE="${HOME}/.config/systemd/user/brassclaw.service"
SYSTEMD_SYSTEM_SERVICE="/etc/systemd/system/brassclaw.service"

# Repository/installation locations (optional, for development/testing)
REPO_DIR="/opt/brassclaw"
ALT_INSTALL_DIR="/root/brassclaw"

# Database locations
POSTGRES_DB_NAME="brassclaw"
SQLITE_DB="${DATA_DIR}/brassclaw.db"
LIBSQL_DB="${DATA_DIR}/brassclaw.sqld"

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
    
    # Check for systemd user service
    if [ -f "$SYSTEMD_USER_SERVICE" ]; then
        echo "Stopping systemd user service..."
        systemctl --user stop brassclaw.service 2>/dev/null || true
        systemctl --user disable brassclaw.service 2>/dev/null || true
    fi
    
    # Check for systemd system service (requires root)
    if [ -f "$SYSTEMD_SYSTEM_SERVICE" ]; then
        echo "Stopping systemd system service..."
        if [ "$(id -u)" -eq 0 ]; then
            systemctl stop brassclaw.service 2>/dev/null || true
            systemctl disable brassclaw.service 2>/dev/null || true
        else
            echo "${YELLOW}Warning: System service found but requires root to stop${NC}"
            echo "Run with sudo to remove system service: sudo $0 $@"
        fi
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

# Function to clean up database
cleanup_database() {
    echo "${YELLOW}Checking for databases...${NC}"
    
    # Check for PostgreSQL database
    if command -v psql > /dev/null 2>&1; then
        if psql -lqt 2>/dev/null | cut -d \| -f 1 | grep -qw "$POSTGRES_DB_NAME"; then
            echo "Found PostgreSQL database: $POSTGRES_DB_NAME"
            printf "Do you want to drop the PostgreSQL database? [y/N] "
            read -r response
            case "$response" in
                [yY][eE][sS]|[yY])
                    echo "Dropping PostgreSQL database..."
                    dropdb "$POSTGRES_DB_NAME" 2>/dev/null || echo "${YELLOW}Failed to drop database (may require different user)${NC}"
                    ;;
                *)
                    echo "Skipping PostgreSQL database removal"
                    ;;
            esac
        fi
    fi
    
    # SQLite/LibSQL databases are in DATA_DIR and will be removed with it
    if [ -f "$SQLITE_DB" ] || [ -f "$LIBSQL_DB" ]; then
        echo "SQLite/LibSQL databases will be removed with data directory"
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
    echo "  - Logs: ${LOG_DIR}"
    echo "  - State: ${STATE_DIR}"
    
    if [ -f "$LAUNCHD_PLIST" ]; then
        echo "  - LaunchAgent: ${LAUNCHD_PLIST}"
    fi
    
    if [ -f "$SYSTEMD_USER_SERVICE" ]; then
        echo "  - Systemd user service: ${SYSTEMD_USER_SERVICE}"
    fi
    
    if [ -f "$SYSTEMD_SYSTEM_SERVICE" ]; then
        echo "  - Systemd system service: ${SYSTEMD_SYSTEM_SERVICE}"
    fi
    
    if [ -d "$REPO_DIR" ]; then
        echo "  - Repository: ${REPO_DIR} (optional)"
    fi
    
    if [ -d "$ALT_INSTALL_DIR" ]; then
        echo "  - Alt installation: ${ALT_INSTALL_DIR} (optional)"
    fi
    
    echo ""
    echo "${YELLOW}Note: PostgreSQL database (if exists) will be handled separately${NC}"
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
    
    # Clean up database
    if [ "$1" != "--force" ] && [ "$1" != "-f" ]; then
        cleanup_database
        echo ""
    fi
    
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
    
    # Remove logs
    remove_path "$LOG_DIR" "log directory"
    echo ""
    
    # Remove state
    if [ "$STATE_DIR" != "$LOG_DIR" ]; then
        remove_path "$STATE_DIR" "state directory"
        echo ""
    fi
    
    # Remove service files
    if [ -f "$LAUNCHD_PLIST" ]; then
        remove_path "$LAUNCHD_PLIST" "LaunchAgent plist"
        echo ""
    fi
    
    if [ -f "$SYSTEMD_USER_SERVICE" ]; then
        remove_path "$SYSTEMD_USER_SERVICE" "systemd user service"
        echo ""
    fi
    
    if [ -f "$SYSTEMD_SYSTEM_SERVICE" ]; then
        if [ "$(id -u)" -eq 0 ]; then
            remove_path "$SYSTEMD_SYSTEM_SERVICE" "systemd system service"
            systemctl daemon-reload 2>/dev/null || true
        else
            echo "${YELLOW}Skipping system service (requires root)${NC}"
        fi
        echo ""
    fi
    
    # Optional: Remove repository and alt installation
    if [ "$1" != "--force" ] && [ "$1" != "-f" ]; then
        if [ -d "$REPO_DIR" ]; then
            printf "Remove repository directory ${REPO_DIR}? [y/N] "
            read -r response
            case "$response" in
                [yY][eE][sS]|[yY])
                    remove_path "$REPO_DIR" "repository directory"
                    ;;
                *)
                    echo "Keeping repository directory"
                    ;;
            esac
            echo ""
        fi
        
        if [ -d "$ALT_INSTALL_DIR" ]; then
            printf "Remove alt installation ${ALT_INSTALL_DIR}? [y/N] "
            read -r response
            case "$response" in
                [yY][eE][sS]|[yY])
                    remove_path "$ALT_INSTALL_DIR" "alt installation directory"
                    ;;
                *)
                    echo "Keeping alt installation directory"
                    ;;
            esac
            echo ""
        fi
    fi
    
    # Clean up temporary files
    echo "Cleaning up temporary files..."
    rm -rf /tmp/brassclaw* 2>/dev/null || true
    rm -rf /tmp/tmp.*/brassclaw* 2>/dev/null || true
    echo ""
    
    # Check for any remaining processes
    if pgrep -x "$BINARY_NAME" > /dev/null; then
        echo "${RED}Warning: BrassClaw processes are still running${NC}"
        echo "You may need to manually kill them or restart your system."
        echo ""
    fi
    
    echo "${GREEN}✓ BrassClaw has been successfully uninstalled!${NC}"
    echo ""
    echo "Cleanup summary:"
    echo "  ✓ Binary and executables removed"
    echo "  ✓ Configuration and data removed"
    echo "  ✓ Cache and temporary files removed"
    echo "  ✓ Services stopped and disabled"
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