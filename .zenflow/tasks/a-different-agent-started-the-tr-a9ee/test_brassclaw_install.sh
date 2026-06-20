#!/bin/bash
# BrassClaw Installation Test Script
# Run this on the test machine: root@192.168.10.219

set -e

echo "=========================================="
echo "BrassClaw Installation Test Script"
echo "=========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Step 1: Platform Detection
echo -e "${YELLOW}Step 1: Platform Detection${NC}"
echo "----------------------------"
OS=$(uname -s)
ARCH=$(uname -m)
KERNEL=$(uname -r)

echo "OS: $OS"
echo "Architecture: $ARCH"
echo "Kernel: $KERNEL"
echo ""

# Normalize architecture
if [ "$ARCH" = "arm64" ]; then
    ARCH_NORMALIZED="aarch64"
else
    ARCH_NORMALIZED="$ARCH"
fi

# Normalize OS
case "$OS" in
    Linux*)     OS_NORMALIZED="linux";;
    Darwin*)    OS_NORMALIZED="apple-darwin";;
    *)          OS_NORMALIZED="unknown";;
esac

echo "Normalized: ${ARCH_NORMALIZED}-${OS_NORMALIZED}"
echo ""

# Step 2: Check for existing BrassClaw installation
echo -e "${YELLOW}Step 2: Check Existing Installation${NC}"
echo "-------------------------------------"
if command -v brassclaw &> /dev/null; then
    echo -e "${GREEN}✓${NC} BrassClaw found in PATH"
    echo "Location: $(which brassclaw)"
    echo "Version: $(brassclaw --version 2>&1 || echo 'Unable to get version')"
else
    echo "✗ BrassClaw not found in PATH"
fi
echo ""

# Step 3: Check if we can access GitHub
echo -e "${YELLOW}Step 3: Check GitHub Connectivity${NC}"
echo "-----------------------------------"
if command -v curl &> /dev/null; then
    if curl -s --head https://github.com | head -n 1 | grep "HTTP/[12].[01] [23].." > /dev/null; then
        echo -e "${GREEN}✓${NC} GitHub is accessible"
    else
        echo -e "${RED}✗${NC} Cannot reach GitHub"
    fi
else
    echo "curl not found, skipping connectivity check"
fi
echo ""

# Step 4: Download the install script
echo -e "${YELLOW}Step 4: Download Install Script${NC}"
echo "---------------------------------"
INSTALL_SCRIPT_URL="https://raw.githubusercontent.com/yourusername/brassclaw/v2.0.0-migration-complete/install.sh"
echo "Attempting to download from: $INSTALL_SCRIPT_URL"
echo "(Note: Update the URL with the actual GitHub repository)"
echo ""

# Step 5: Check what's in the GitHub release
echo -e "${YELLOW}Step 5: Check GitHub Release Assets${NC}"
echo "-------------------------------------"
if command -v gh &> /dev/null; then
    echo "Checking release v2.0.0-migration-complete..."
    gh release view v2.0.0-migration-complete --json assets --jq '.assets[] | {name: .name, size: .size}' 2>&1 || echo "Unable to check release (gh CLI may not be configured)"
else
    echo "GitHub CLI (gh) not found, skipping release check"
fi
echo ""

