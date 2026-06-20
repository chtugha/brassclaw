#!/usr/bin/env bash
# Standalone build script for the optional WeChat-voice SILK decoder helper.
#
# This crate is intentionally excluded from the BrassClaw workspace so the
# main `cargo build` does not require libclang. It is built separately:
#
#     ./crates/brassclaw_silk_decoder/build.sh
#
# After building, the binary lands in `target/release/brassclaw-silk-decoder`
# (relative to this crate). Install it next to your `brassclaw` binary, on
# `$PATH`, or point the `BRASSCLAW_SILK_DECODER` environment variable at it.

set -euo pipefail

cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo not found on PATH" >&2
    exit 1
fi

echo "Building brassclaw-silk-decoder (requires libclang + a C toolchain)..."
cargo build --release

OUT_BIN="target/release/brassclaw-silk-decoder"

if [ ! -f "$OUT_BIN" ]; then
    echo "Error: build did not produce $OUT_BIN" >&2
    exit 1
fi

echo ""
echo "Built: $OUT_BIN ($(du -h "$OUT_BIN" | cut -f1))"
echo ""
echo "To install (one of the following):"
echo "  cp $OUT_BIN \"\$(dirname \"\$(command -v brassclaw)\")/\"   # sibling install"
echo "  cp $OUT_BIN /usr/local/bin/                                  # system PATH"
echo "  export BRASSCLAW_SILK_DECODER=\"\$(pwd)/$OUT_BIN\"          # explicit path"
