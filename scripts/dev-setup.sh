#!/usr/bin/env bash
# Developer setup script for BrassClaw.
#
# Gets a fresh checkout ready for development without requiring
# Docker, PostgreSQL, or any external services.
#
# Usage:
#   ./scripts/dev-setup.sh
#
# After running, you can:
#   cargo check           # default features (postgres + libsql)
#   cargo test            # default test suite (uses libsql temp DB)
#   cargo test --all-features         # full test suite

set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== BrassClaw Developer Setup ==="
echo ""

# 1. Check rustup
if ! command -v rustup &>/dev/null; then
    echo "ERROR: rustup not found. Install from https://rustup.rs"
    exit 1
fi
echo "[1/4] rustup found: $(rustup --version 2>/dev/null | head -1)"

# 2. Verify the project compiles
echo "[2/4] Running cargo check..."
cargo check

# 3. Run tests using libsql temp DB (no Docker/external DB needed)
echo "[3/4] Running tests (no external DB required)..."
cargo test

# 4. Install git hooks
echo "[4/4] Installing git hooks..."
HOOKS_DIR=$(git rev-parse --git-path hooks 2>/dev/null) || true
if [ -n "$HOOKS_DIR" ]; then
    mkdir -p "$HOOKS_DIR"
    SCRIPTS_ABS="$(cd "$(dirname "$0")" && pwd)"
    ln -sf "$SCRIPTS_ABS/commit-msg-regression.sh" "$HOOKS_DIR/commit-msg"
    echo "  commit-msg hook installed (regression test enforcement)"
    ln -sf "$SCRIPTS_ABS/pre-commit-safety.sh" "$HOOKS_DIR/pre-commit"
    echo "  pre-commit hook installed (UTF-8, case-sensitivity, /tmp, redaction checks)"
    REPO_ROOT="$(git rev-parse --show-toplevel)"
    ln -sf "$REPO_ROOT/.githooks/pre-push" "$HOOKS_DIR/pre-push"
    echo "  pre-push hook installed (quality gate + optional delta lint)"
else
    echo "  Skipped: not a git repository"
fi

echo ""
echo "=== Setup complete ==="
echo ""
echo "Quick start:"
echo "  cargo run                            # Run with default features"
echo "  cargo test                           # Test suite (libsql temp DB)"
echo "  cargo test --all-features            # Full test suite"
echo "  cargo clippy --all-features          # Lint all code"
