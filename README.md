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

## Reborn v3 Recipe architecture

## Binding preloadable Skill interface (v3)

A Skill is one Tool-usage pattern with prose and explicitly associated PythonCode
exposing a preloadable function interface. Declare public names/signatures,
private helpers/constants and dependencies. Resolve one approved catalogue
snapshot; pin exact interface/code/association/artifact revisions and export
resolution. Load definitions in deterministic dependency-first order, rejecting
cycles/conflicts and effectful initializers. Invoke the pinned export on demand
with typed data; loading is not an invocation or a new Recipe effect step.

Preloaded code does not automatically enable a Tool. Its matching ToolSkill
binding and current kernel checks still apply before every actual dispatch.
Reusable code and immutable constants may be shared; mutable arrays, defaults,
closures, inputs and results remain isolated per task/attempt/invocation. Running
and resumed tasks keep their selected exports when new revisions activate.
Each real Skill has a canonical execution Recipe with a matching command.
MCP tools/list derives from available approved mcp-call-skill-recipes, not raw
Skill rows. Keep the server always running; Kohai connects/advertises to the
provider after final prefix addition just before sending a prompt, then
disconnects that request on the complete answer. Refresh discovery at
startup/restart and qualified Skill/Recipe catalogue changes. Existing calls
keep their advertised contract and normal chat task snapshot.
MCP tools/list gives its exact sentence, variable positions/types, escaping and
valid examples; the model sends the completed command for intent matching.
MCP accepts the completed listed command, opens a new ordinary chat, sends it
as a user message, forwards the correlated chat result and closes the chat.
It accepts no Python and has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](skills.md#preloadable-function-interface-binding-v3-target).
This is a binding target, not proof of implemented loader/store/runner support.
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


Recipes tell the orchestrator how to fulfill tasks using Rust Tools, many
ToolSkills, many Skills and many small reusable PythonCode components. IBS
assembles the steps and pins component versions for each task. See
[recipe.md](recipe.md) for the binding authoring contract and current runtime
limitations; this describes the target, not a completed v3 cutover.

## Quick Start

**Requirements:** any machine with ~8 GB RAM, 64-bit CPU, ~4 GB free disk space; Python 3.9+ for the installation/removal scripts.

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
sudo bash install.sh -v 1.7.0-rc.4
```

The installer selects the newest published release containing the complete
application/worker pair for your platform, which can be a prerelease (announced
explicitly). Use `-v` to choose a specific release. It verifies both executables
and their checksums before installation. Linux systemd upgrades preserve the existing service unit, data
location, credentials and operator overrides. The service stops before executable
replacement; both previous executables are retained as `.bak` files. Startup is
confirmed through the authenticated live Monty status endpoint, with a default
180-second timeout (`--startup-timeout seconds` to change it).

Fresh systemd installations bind to `127.0.0.1:3000` and use `local_dev`. The
`v1.7.0-rc.4` runtime's default `full` profile fails during PostgreSQL
startup; an operator profile set in `/etc/brassclaw/secrets.env` overrides the
service default. For a remote machine, use `ssh -L 3000:127.0.0.1:3000 your-host`
and open `http://localhost:3000/v2` locally.

If a new release fails readiness, the installer stops it and reports failure.
Check its journal and database migration compatibility before restoring an older
executable pair; automatic database downgrade is not performed. Manual/user-local
upgrades require stopping the running instance first.

### Option B: macOS — manual binary

```bash
# Apple Silicon (M1/M2/M3):
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-arm64
curl -fsSL -o monty_worker https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-arm64-monty-worker
chmod +x brassclaw-reborn monty_worker
sudo mv monty_worker /usr/local/bin/monty_worker
sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn

# Intel Mac:
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-amd64
curl -fsSL -o monty_worker https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-amd64-monty-worker
chmod +x brassclaw-reborn monty_worker
sudo mv monty_worker /usr/local/bin/monty_worker
sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn
```

### Option C: Build from source

Requires [Rust 1.96+](https://rustup.rs).

```bash
git clone https://github.com/chtugha/brassclaw.git
cd brassclaw
cargo build --release -p brassclaw -p brassclaw_monty_host --bins
# Binaries: target/release/brassclaw and target/release/monty_worker
sudo cp target/release/brassclaw /usr/local/bin/brassclaw-reborn
sudo cp target/release/monty_worker /usr/local/bin/monty_worker
```

---

## Configuration

Set the required environment variables then start the server:

```bash
export BRASSCLAW_REBORN_WEBUI_TOKEN=your-secret-token
export BRASSCLAW_REBORN_WEBUI_USER_ID=your-user-id
brassclaw-reborn serve --host 127.0.0.1 --port 3000
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

Requires Python 3.9+. The script lists the removal inventory, then asks for
confirmation: **Enter permanently wipes all discovered BrassClaw state**;
`keep` removes the application while retaining data; `cancel` stops.

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/uninstall.sh | sudo bash

# Inspect without stopping or removing anything:
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/uninstall.sh | sudo bash -s -- --dry-run

# Explicit unattended choices:
bash uninstall.sh --yes
bash uninstall.sh --keep-data
```

A full wipe removes the service, main and Monty worker binaries, backups,
embedded PostgreSQL, configuration, credentials, extensions, workspace, caches,
and identified BrassClaw downloads and temporary files. It removes dedicated
BrassClaw package/container resources and installation-owned system accounts.
Custom configured state directories are included when their ownership is safe.
Unresolved shared resources or external PostgreSQL require separate cleanup;
the script refuses to report a complete wipe while these remain unresolved.
Shared OS journals and shared system dependencies are preserved.

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
