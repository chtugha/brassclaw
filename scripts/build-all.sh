#!/usr/bin/env bash
# Build BrassClaw and all bundled channels.
#
# Run this before release or when channel sources have changed.

set -euo pipefail

cd "$(dirname "$0")/.."

echo "Building bundled channels..."
if [ -d "channels-src/telegram" ]; then
    ./channels-src/telegram/build.sh
fi

echo ""
echo "Building BrassClaw..."
cargo build --release

echo ""
echo "Done. Binary: target/release/brassclaw"
