<p align="center">
  <img src="brassclaw.png?v=2" alt="BrassClaw" width="200"/>
</p>

<h1 align="center">BrassClaw</h1>

<p align="center">
  <strong>Your secure personal AI assistant — runs entirely on your hardware</strong>
</p>

<p align="center">
  <a href="https://github.com/chtugha/brassclaw/releases/latest"><img src="https://img.shields.io/github/v/release/chtugha/brassclaw?label=latest%20release" alt="Latest Release" /></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache%202.0-blue.svg" alt="License: MIT OR Apache-2.0" /></a>
</p>

---

## Quick Start

**Requirements:** any machine with ~8 GB RAM, 64-bit CPU, ~4 GB free disk space.

### Option A: Linux — one-line install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/install.sh | sudo bash
```

Or review first:

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/install.sh -o install.sh
less install.sh
sudo bash install.sh
```

Pin to a specific version:

```bash
sudo bash install.sh -v 0.9.4
```

### Option B: macOS — manual binary

```bash
# Apple Silicon (M1/M2/M3):
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-arm64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn

# Intel Mac:
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-amd64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn
```

### Option C: Build from source

Requires [Rust 1.94+](https://rustup.rs).

```bash
git clone https://github.com/chtugha/brassclaw.git
cd brassclaw
cargo build --release --bin brassclaw
# Binary: target/release/brassclaw
sudo cp target/release/brassclaw /usr/local/bin/brassclaw-reborn
```

---

## Configuration

Set the required environment variables then start the server:

```bash
export BRASSCLAW_REBORN_WEBUI_TOKEN=your-secret-token
export BRASSCLAW_REBORN_WEBUI_USER_ID=your-user-id
brassclaw-reborn serve --host 0.0.0.0 --port 3000
```

Open `http://localhost:3000/v2` in your browser and log in with the token.

### Minimal `~/.brassclaw/reborn/config.toml`

```toml
[identity]
tenant = "my-instance"

[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
base_url = "http://localhost:8000/v1"
# api_key_env = "MY_LLM_KEY"   # set if your LLM endpoint requires auth
```

### Environment variables

**Bootstrap tier** (set before startup — use systemd `Environment=` or a `.env` file):

| Variable | Default | Description |
|----------|---------|-------------|
| `BRASSCLAW_REBORN_WEBUI_TOKEN` | _(required)_ | Bearer token for WebUI login |
| `BRASSCLAW_REBORN_WEBUI_USER_ID` | _(required)_ | User identity for all sessions |
| `BRASSCLAW_REBORN_HOME` | `~/.brassclaw/reborn` | State directory |
| `BRASSCLAW_RUNTIME_PROFILE` | `local_dev` | Security profile: `local_dev`, `local_safe`, `local_yolo`, `hosted_safe` |
| `BRASSCLAW_REBORN_LOG` | _(off)_ | Log filter, e.g. `brassclaw=debug` |
| `BRASSCLAW_PG_URL` | _(embedded)_ | External Postgres URL (omit to use embedded) |

### LLM setup

**vLLM** (GPU servers):
```bash
pip install vllm
vllm serve Qwen/Qwen2.5-7B-Instruct-AWQ --host 0.0.0.0 --port 8000
```

**Ollama** (CPU/home):
```bash
ollama serve
ollama pull qwen2.5:7b
```
Then set `provider_id = "ollama"` and `model = "qwen2.5:7b"` in `config.toml`.

### First-run checklist

1. Start the server and open the WebUI at `/v2`
2. Navigate to **Settings → Providers** and verify your LLM is listed
3. Navigate to **Settings → Prefix Cache** and click **Generate** to build the knowledge bundle
4. Start chatting

---

## Uninstall

```bash
# Remove binary and service, keep data:
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/uninstall.sh | sudo bash

# Wipe everything (binary, service, config, all data):
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/uninstall.sh | sudo bash -s -- --wipe
```

---

## Documentation

Full system documentation is in [`docs/agents-v3/`](docs/agents-v3/):

- [`01-architecture-overview.md`](docs/agents-v3/01-architecture-overview.md) — Three-layer model
- [`03-recipe-system.md`](docs/agents-v3/03-recipe-system.md) — Recipes and intent routing
- [`05-skills-system.md`](docs/agents-v3/05-skills-system.md) — Skills and ToolSkills
- [`06-tools-system.md`](docs/agents-v3/06-tools-system.md) — Built-in tools
- [`10-prefix-base-prompt.md`](docs/agents-v3/10-prefix-base-prompt.md) — Prefix cache / base prompt
- [`15-component-catalog.md`](docs/agents-v3/15-component-catalog.md) — Component class codes (0–23)

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
