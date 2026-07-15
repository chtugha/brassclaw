#!/bin/bash
set -euo pipefail

BINARY_NAME="tomedo-monitor"
INSTALL_DIR="$HOME/.local/bin"
PLIST_DIR="$HOME/Library/LaunchAgents"
PLIST_NAME="de.brassclaw.tomedo-monitor.plist"
PORT="${TOMEDO_MONITOR_PORT:-49152}"
PATTERN="${TOMEDO_MONITOR_PATTERN:-}"
LOG_DIR="$HOME/Library/Logs/brassclaw"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_PATH="$SCRIPT_DIR/target/release/$BINARY_NAME"

if [ ! -f "$BINARY_PATH" ]; then
    echo "Building $BINARY_NAME..."
    cd "$SCRIPT_DIR"
    cargo build --release
fi

mkdir -p "$INSTALL_DIR"
mkdir -p "$PLIST_DIR"
mkdir -p "$LOG_DIR"

cp "$BINARY_PATH" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"
echo "Installed binary: $INSTALL_DIR/$BINARY_NAME"

PATTERN_ARG=""
if [ -n "$PATTERN" ]; then
    PATTERN_ARG="<string>--pattern</string>
        <string>$PATTERN</string>"
fi

cat > "$PLIST_DIR/$PLIST_NAME" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>de.brassclaw.tomedo-monitor</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/$BINARY_NAME</string>
        <string>--port</string>
        <string>$PORT</string>
        $PATTERN_ARG
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOG_DIR/tomedo-monitor.log</string>
    <key>StandardErrorPath</key>
    <string>$LOG_DIR/tomedo-monitor.error.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
</dict>
</plist>
EOF

echo "Created LaunchAgent plist: $PLIST_DIR/$PLIST_NAME"

if launchctl list | grep -q "de.brassclaw.tomedo-monitor" 2>/dev/null; then
    launchctl unload "$PLIST_DIR/$PLIST_NAME" 2>/dev/null || true
fi
launchctl load "$PLIST_DIR/$PLIST_NAME"
echo "LaunchAgent loaded — tomedo-monitor will start automatically at login."

echo ""
echo "Monitor is running on http://127.0.0.1:$PORT"
echo ""
echo "IMPORTANT: Grant Accessibility access in:"
echo "  System Settings → Privacy & Security → Accessibility"
echo "  Add: $INSTALL_DIR/$BINARY_NAME"
echo ""
echo "Test with: curl http://127.0.0.1:$PORT/health"
echo "Get patient: curl http://127.0.0.1:$PORT/tomedo"
