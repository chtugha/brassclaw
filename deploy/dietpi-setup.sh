#!/usr/bin/env bash
set -euo pipefail

BRASSCLAW_DIR="/opt/brassclaw"
BRASSCLAW_REPO="https://github.com/chtugha/brassclaw.git"
VLLM_MODEL="Qwen/Qwen2.5-7B-Instruct-AWQ"
export VLLM_HOST="${VLLM_HOST}"
export VLLM_PORT="${VLLM_PORT}"
#VLLM_HOST="${VLLM_HOST:-localhost}"
#VLLM_PORT="${VLLM_PORT:-8000}"

echo "=== BrassClaw DietPi Setup ==="
echo "This script sets up BrassClaw on a DietPi system."
echo ""
echo "vLLM endpoint: http://${VLLM_HOST}:${VLLM_PORT}/v1"
echo "  Set VLLM_HOST / VLLM_PORT env vars to override (e.g. remote GPU server)."
echo ""

echo "[1/7] Removing old ironclaw+brassclaw remnants and Ollama..."
systemctl stop ironclaw 2>/dev/null || true
systemctl disable ironclaw 2>/dev/null || true
rm -f /etc/systemd/system/ironclaw.service
rm -rf /etc/systemd/system/ironclaw.service.d
systemctl stop brassclaw 2>/dev/null || true
systemctl disable brassclaw 2>/dev/null || true
rm -f /etc/systemd/system/brassclaw.service
rm -rf /etc/systemd/system/brassclaw.service.d
systemctl stop ollama 2>/dev/null || true
systemctl disable ollama 2>/dev/null || true
apt-get remove -y ollama 2>/dev/null || true
rm -f /etc/systemd/system/ollama.service
rm -rf /etc/systemd/system/ollama.service.d
rm -rf /usr/local/bin/ollama /usr/share/ollama ~/.ollama
rm -rf /opt/ironclaw ~/.ironclaw
rm -rf /root/.ironclaw
rm -rf /root/.brassclaw
rm -rf /root/brassclaw-workspace
systemctl daemon-reload
echo "  Done."

echo "[2/7] Installing system dependencies..."
apt-get update -qq
apt-get install -y -qq build-essential pkg-config libssl-dev git curl python3 python3-pip python3-venv
echo "  Done."

echo "[3/7] Installing Rust toolchain..."
if ! command -v rustup &>/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env"
fi
rustup update stable
echo "  Done."


echo "[4/7] Skipping local vLLM install (using remote: ${VLLM_HOST}:${VLLM_PORT})..."
echo "[5/7] Verifying remote vLLM connectivity..."
if curl -s --connect-timeout 5 "http://${VLLM_HOST}:${VLLM_PORT}/v1/models" >/dev/null 2>&1; then
    echo "  Remote vLLM is reachable."
else
    echo "  WARNING: Cannot reach vLLM at ${VLLM_HOST}:${VLLM_PORT}. Ensure it is running."
fi
echo "  Done."


echo "[6/7] Cloning and building BrassClaw..."
cd /opt
rm -rf "$BRASSCLAW_DIR"
git clone "$BRASSCLAW_REPO" "$BRASSCLAW_DIR"
cd "$BRASSCLAW_DIR"
cargo build --release -p brassclaw_reborn_cli --bin brassclaw-reborn
ln -sf "$BRASSCLAW_DIR/target/release/brassclaw-reborn" /usr/local/bin/brassclaw-reborn
echo "  Done."

echo "[7/7] Configuring BrassClaw..."
mkdir -p ~/.brassclaw/reborn
mkdir -p ~/brassclaw-workspace
WEBUI_TOKEN=$(head -c 24 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 32)
cat > ~/.brassclaw/reborn/config.toml << EOF
[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
api_key_env = "BRASSCLAW_VLLM_KEY"
base_url = "http://${VLLM_HOST}:${VLLM_PORT}/v1"

[boot]
profile = "local-dev-yolo"
EOF

if [ "$VLLM_HOST" = "localhost" ] || [ "$VLLM_HOST" = "127.0.0.1" ]; then
    VLLM_DEPS="Requires=vllm.service"
    VLLM_AFTER="After=network-online.target vllm.service"
else
    VLLM_DEPS=""
    VLLM_AFTER="After=network-online.target"
fi

cat > /etc/systemd/system/brassclaw.service << SVCEOF
[Unit]
Description=BrassClaw AI Assistant
${VLLM_AFTER}
Wants=network-online.target
${VLLM_DEPS}

[Service]
Type=simple
User=root
WorkingDirectory=/root/brassclaw-workspace
Environment=BRASSCLAW_REBORN_LOG=debug
Environment=BRASSCLAW_REBORN_WEBUI_TOKEN=${WEBUI_TOKEN}
Environment=BRASSCLAW_REBORN_WEBUI_USER_ID=brassclaw-admin
Environment=BRASSCLAW_REBORN_HOME=/root/.brassclaw/reborn
Environment=BRASSCLAW_REBORN_PROFILE=local-dev-yolo
Environment=LLM_BACKEND=openai_compatible
Environment=LLM_BASE_URL=http://${VLLM_HOST}:${VLLM_PORT}/v1
Environment=LLM_API_KEY=none
Environment=LLM_MODEL=Qwen/Qwen2.5-7B-Instruct-AWQ
Environment=BRASSCLAW_VLLM_KEY=none
ExecStart=/usr/local/bin/brassclaw-reborn serve --host 0.0.0.0 --port 3000
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
SVCEOF

systemctl daemon-reload
systemctl enable brassclaw
echo "  Done."

echo ""
echo "=== Setup Complete ==="
echo ""
echo "WebUI bearer token: $WEBUI_TOKEN"
echo "(save this — needed to authenticate API requests)"
echo ""
echo "Services registered:"
if [ "$VLLM_HOST" = "localhost" ] || [ "$VLLM_HOST" = "127.0.0.1" ]; then
    echo "  - vllm.service  (model: $VLLM_MODEL, port ${VLLM_PORT})"
fi
echo "  - brassclaw.service (vLLM at ${VLLM_HOST}:${VLLM_PORT})"
echo ""
echo "To register in dietpi-services, add to /boot/dietpi/dietpi-services_include_exclude:"
if [ "$VLLM_HOST" = "localhost" ] || [ "$VLLM_HOST" = "127.0.0.1" ]; then
    echo "  + vllm"
fi
echo "  + brassclaw"
echo ""
echo "Start BrassClaw:"
echo "  systemctl start brassclaw"
echo ""
echo "Interactive mode:"
echo "  BRASSCLAW_REBORN_HOME=/root/.brassclaw/reborn \\"
echo "  LLM_BACKEND=openai_compatible \\"
echo "  LLM_BASE_URL=http://${VLLM_HOST}:${VLLM_PORT}/v1 \\"
echo "  LLM_API_KEY=none \\"
echo "  LLM_MODEL=Qwen/Qwen2.5-7B-Instruct-AWQ \\"
echo "  brassclaw-reborn repl"
