#!/bin/sh
# BrassClaw Installation Script
# Detects platform and architecture, downloads and installs the appropriate binary

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
REPO="chtugha/brassclaw"
BINARY_NAME="brassclaw"
INSTALL_DIR="${HOME}/.local/bin"

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux";;
        Darwin*)    echo "macos";;
        MINGW*|MSYS*|CYGWIN*) echo "windows";;
        *)          echo "unknown";;
    esac
}

# Detect Architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   echo "x86_64";;
        aarch64|arm64)  echo "aarch64";;
        *)              echo "unknown";;
    esac
}

# Get latest release tag
get_latest_release() {
    curl -s "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install
install_brassclaw() {
    OS=$(detect_os)
    ARCH=$(detect_arch)
    
    echo "${GREEN}Detecting platform...${NC}"
    echo "OS: ${OS}"
    echo "Architecture: ${ARCH}"
    
    if [ "$OS" = "unknown" ] || [ "$ARCH" = "unknown" ]; then
        echo "${RED}Error: Unsupported platform${NC}"
        echo "OS: $OS, Architecture: $ARCH"
        exit 1
    fi
    
    # Get latest release or use provided version
    if [ -z "$VERSION" ]; then
        echo "${GREEN}Fetching latest release...${NC}"
        VERSION=$(get_latest_release)
        if [ -z "$VERSION" ]; then
            echo "${RED}Error: Could not determine latest version${NC}"
            exit 1
        fi
    fi
    
    echo "${GREEN}Installing BrassClaw ${VERSION}...${NC}"
    
    # Construct download URL based on platform
    case "$OS" in
        linux)
            if [ "$ARCH" = "x86_64" ]; then
                ARCHIVE="brassclaw-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
            elif [ "$ARCH" = "aarch64" ]; then
                ARCHIVE="brassclaw-${VERSION}-aarch64-unknown-linux-gnu.tar.gz"
            fi
            ;;
        macos)
            if [ "$ARCH" = "x86_64" ]; then
                ARCHIVE="brassclaw-${VERSION}-x86_64-apple-darwin.tar.gz"
            elif [ "$ARCH" = "aarch64" ]; then
                ARCHIVE="brassclaw-${VERSION}-aarch64-apple-darwin.tar.gz"
            fi
            ;;
        windows)
            if [ "$ARCH" = "x86_64" ]; then
                ARCHIVE="brassclaw-${VERSION}-x86_64-pc-windows-msvc.zip"
            fi
            ;;
    esac
    
    if [ -z "$ARCHIVE" ]; then
        echo "${RED}Error: No binary available for ${OS}-${ARCH}${NC}"
        exit 1
    fi
    
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
    
    echo "${GREEN}Attempting to download pre-compiled binary...${NC}"
    echo "URL: ${DOWNLOAD_URL}"
    
    # Create temporary directory
    TMP_DIR=$(mktemp -d)
    cd "$TMP_DIR"
    
    # Try to download pre-compiled binary
    BINARY_AVAILABLE=0
    if curl -L -f -o "$ARCHIVE" "$DOWNLOAD_URL" 2>/dev/null; then
        BINARY_AVAILABLE=1
        echo "${GREEN}✓ Pre-compiled binary downloaded${NC}"
    else
        echo "${YELLOW}⚠ Pre-compiled binary not available${NC}"
        echo "${GREEN}Falling back to building from source...${NC}"
        
        # Check for required build tools
        if ! command -v cargo > /dev/null 2>&1; then
            echo "${RED}Error: cargo not found. Please install Rust from https://rustup.rs/${NC}"
            rm -rf "$TMP_DIR"
            exit 1
        fi
        
        # Clone and build from source
        echo "${GREEN}Cloning repository...${NC}"
        if ! git clone --depth 1 --branch "${VERSION}" "https://github.com/${REPO}.git" brassclaw-src; then
            echo "${RED}Error: Failed to clone repository${NC}"
            rm -rf "$TMP_DIR"
            exit 1
        fi
        
        cd brassclaw-src
        echo "${GREEN}Building BrassClaw (this may take a few minutes)...${NC}"
        if ! cargo build --release; then
            echo "${RED}Error: Build failed${NC}"
            cd ..
            rm -rf "$TMP_DIR"
            exit 1
        fi
        
        # Copy built binary to temp directory
        cp target/release/brassclaw ../"$BINARY_NAME"
        cd ..
        BINARY_AVAILABLE=2  # Built from source
    fi
    
    if [ "$BINARY_AVAILABLE" -eq 0 ]; then
        echo "${RED}Error: Could not obtain binary${NC}"
        rm -rf "$TMP_DIR"
        exit 1
    fi
    
    # Extract archive if we downloaded one
    if [ "$BINARY_AVAILABLE" -eq 1 ]; then
        echo "${GREEN}Extracting archive...${NC}"
        case "$ARCHIVE" in
            *.tar.gz)
                tar -xzf "$ARCHIVE"
                ;;
            *.zip)
                unzip -q "$ARCHIVE"
                ;;
        esac
        
        # Find binary in extracted archive
        BINARY_PATH=$(find . -name "$BINARY_NAME" -o -name "${BINARY_NAME}.exe" | head -n 1)
        
        if [ -z "$BINARY_PATH" ]; then
            echo "${RED}Error: Binary not found in archive${NC}"
            rm -rf "$TMP_DIR"
            exit 1
        fi
    else
        # Binary was built from source, already in place
        BINARY_PATH="./$BINARY_NAME"
        if [ ! -f "$BINARY_PATH" ]; then
            echo "${RED}Error: Built binary not found${NC}"
            rm -rf "$TMP_DIR"
            exit 1
        fi
    fi
    
    # Create install directory if it doesn't exist
    mkdir -p "$INSTALL_DIR"
    
    # Install binary
    echo "${GREEN}Installing to ${INSTALL_DIR}...${NC}"
    cp "$BINARY_PATH" "$INSTALL_DIR/"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    
    # Cleanup
    cd - > /dev/null
    rm -rf "$TMP_DIR"
    
    echo "${GREEN}✓ BrassClaw ${VERSION} installed successfully!${NC}"
    echo ""
    echo "Binary location: ${INSTALL_DIR}/${BINARY_NAME}"
    echo ""
    
    # Check if install directory is in PATH
    case ":$PATH:" in
        *":${INSTALL_DIR}:"*) 
            echo "${GREEN}✓ ${INSTALL_DIR} is already in your PATH${NC}"
            ;;
        *)
            echo "${YELLOW}⚠ ${INSTALL_DIR} is not in your PATH${NC}"
            echo ""
            echo "Add it to your PATH by adding this line to your shell profile:"
            echo "  export PATH=\"\$PATH:${INSTALL_DIR}\""
            echo ""
            case "$(basename "$SHELL")" in
                bash)
                    echo "For bash, add to ~/.bashrc or ~/.bash_profile"
                    ;;
                zsh)
                    echo "For zsh, add to ~/.zshrc"
                    ;;
                fish)
                    echo "For fish, run: fish_add_path ${INSTALL_DIR}"
                    ;;
            esac
            ;;
    esac
    
    echo ""
    echo "Run 'brassclaw --help' to get started!"
}

# Main execution
main() {
    echo "${GREEN}BrassClaw Installer${NC}"
    echo "===================="
    echo ""
    
    # Check for required commands
    for cmd in curl tar; do
        if ! command -v $cmd > /dev/null 2>&1; then
            echo "${RED}Error: Required command '$cmd' not found${NC}"
            exit 1
        fi
    done
    
    # Note: git and cargo are checked later if needed for building from source
    
    install_brassclaw
}

main "$@"

# Made with Bob
