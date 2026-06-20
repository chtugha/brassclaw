#!/bin/bash
# BrassClaw P0.2/P0.3 Testing Script
# Run this on test machine: 192.168.10.219
# Location: /root/brassclaw-build

set -e  # Exit on error

echo "=========================================="
echo "BrassClaw P0.2/P0.3 Testing Script"
echo "=========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
LLM_ENDPOINT="http://192.168.10.223"
LLM_MODEL="Qwen/Qwen2.5-7B-Instruct-AWQ"
BUILD_DIR="/root/brassclaw-build"

echo -e "${YELLOW}Step 1: Verify we're in the build directory${NC}"
cd "$BUILD_DIR" || { echo -e "${RED}Failed to cd to $BUILD_DIR${NC}"; exit 1; }
pwd
echo ""

echo -e "${YELLOW}Step 2: Check LLM endpoint connectivity${NC}"
if curl -s --connect-timeout 5 "$LLM_ENDPOINT/v1/models" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ LLM endpoint is reachable${NC}"
else
    echo -e "${RED}✗ LLM endpoint is NOT reachable${NC}"
    echo "  Trying to get more info..."
    curl -v "$LLM_ENDPOINT/v1/models" 2>&1 | head -20
fi
echo ""

echo -e "${YELLOW}Step 3: Configure LLM environment variables${NC}"
export LLM_BACKEND="provider"
export PROVIDER_BASE_URL="$LLM_ENDPOINT"
export PROVIDER_MODEL="$LLM_MODEL"
export PROVIDER_API_KEY="not-needed"

# Also create .env file for persistence
cat > .env << EOF
LLM_BACKEND=provider
PROVIDER_BASE_URL=$LLM_ENDPOINT
PROVIDER_MODEL=$LLM_MODEL
PROVIDER_API_KEY=not-needed
EOF

echo "Environment variables set:"
env | grep -E '(LLM|PROVIDER)' || echo "No LLM/PROVIDER vars found"
echo ""
echo ".env file created:"
cat .env
echo ""

echo -e "${YELLOW}Step 4: Check BrassClaw binary${NC}"
if [ -f "$HOME/.local/bin/brassclaw" ]; then
    echo -e "${GREEN}✓ Binary exists at $HOME/.local/bin/brassclaw${NC}"
    ls -lh "$HOME/.local/bin/brassclaw"
else
    echo -e "${RED}✗ Binary not found${NC}"
fi
echo ""

echo -e "${YELLOW}Step 5: Test cargo build (quick check)${NC}"
if cargo check --release 2>&1 | tail -5; then
    echo -e "${GREEN}✓ Cargo check passed${NC}"
else
    echo -e "${RED}✗ Cargo check failed${NC}"
fi
echo ""

echo -e "${YELLOW}Step 6: Run BrassClaw doctor (configuration check)${NC}"
echo "This will check if LLM configuration is valid..."
timeout 30 cargo run --release -- doctor 2>&1 | tee doctor_output.log || {
    echo -e "${YELLOW}Doctor command timed out or failed (this is expected if DB not configured)${NC}"
}
echo ""

echo -e "${YELLOW}Step 7: Test models status${NC}"
echo "Checking current LLM provider configuration..."
timeout 30 cargo run --release -- models status 2>&1 | tee models_status.log || {
    echo -e "${YELLOW}Models status command timed out or failed${NC}"
}
echo ""

echo -e "${YELLOW}Step 8: Test simple message with auto-approval${NC}"
echo "Testing tool execution with auto-approval enabled..."
echo "Command: cargo run --release -- -m 'What is 2+2?' --auto-approve --no-db"
timeout 60 cargo run --release -- -m "What is 2+2?" --auto-approve --no-db 2>&1 | tee simple_test.log || {
    echo -e "${YELLOW}Simple test timed out or failed${NC}"
}
echo ""

echo -e "${YELLOW}Step 9: Check for auto-approval logs${NC}"
echo "Looking for 'Auto-approving tool execution' messages..."
if grep -i "auto-approving" simple_test.log 2>/dev/null; then
    echo -e "${GREEN}✓ Found auto-approval log messages${NC}"
else
    echo -e "${YELLOW}No auto-approval messages found (may not have triggered tool use)${NC}"
fi
echo ""

echo -e "${YELLOW}Step 10: Test file operation (if LLM is working)${NC}"
echo "Testing: List files in current directory"
timeout 60 cargo run --release -- -m "List the files in the current directory" --auto-approve --no-db 2>&1 | tee file_test.log || {
    echo -e "${YELLOW}File test timed out or failed${NC}"
}
echo ""

echo -e "${YELLOW}Step 11: Check skills${NC}"
echo "Listing available skills..."
timeout 30 cargo run --release -- skills list 2>&1 | tee skills_list.log || {
    echo -e "${YELLOW}Skills list command timed out or failed${NC}"
}
echo ""

echo "=========================================="
echo "Testing Complete!"
echo "=========================================="
echo ""
echo "Log files created:"
echo "  - doctor_output.log"
echo "  - models_status.log"
echo "  - simple_test.log"
echo "  - file_test.log"
echo "  - skills_list.log"
echo ""
echo "Next steps:"
echo "1. Review the log files for errors"
echo "2. Check if LLM responded to queries"
echo "3. Verify auto-approval messages in logs"
echo "4. Test more complex tool operations if basic tests passed"
echo ""
echo "To run interactive REPL:"
echo "  cargo run --release -- run --auto-approve --cli-only"
echo ""

# Made with Bob
