# `brassclaw-reborn` standalone binary

`brassclaw-reborn` is the standalone executable boundary for Reborn. It is separate from the legacy `brassclaw` binary so Reborn boot, config, state, and runtime composition can evolve without accidentally invoking v1 runtime paths.

This binary is available as the workspace package `brassclaw_reborn_cli` and builds the executable named `brassclaw-reborn`.

## Build

```bash
cargo build --release -p brassclaw_reborn_cli --bin brassclaw-reborn 
```

Without ``, the binary builds without the embedded WebUI v2 assets. The `serve` command still starts but the `/v2` route returns a placeholder.

## Current status

`brassclaw-reborn` is an early operator and developer surface, not the default BrassClaw runtime. It currently supports:

```bash
brassclaw-reborn --help
brassclaw-reborn channels list
brassclaw-reborn channels list --json
brassclaw-reborn channels list --verbose
brassclaw-reborn completion --shell bash
brassclaw-reborn completion --shell zsh
brassclaw-reborn config path
brassclaw-reborn config init
brassclaw-reborn doctor
brassclaw-reborn extension search github
brassclaw-reborn extension search github --json
brassclaw-reborn extension install github-mcp
brassclaw-reborn extension activate github-mcp
brassclaw-reborn extension remove github-mcp
brassclaw-reborn hooks list
brassclaw-reborn hooks list --json
brassclaw-reborn hooks list --verbose
brassclaw-reborn logs
brassclaw-reborn logs --json
brassclaw-reborn logs --verbose
brassclaw-reborn models list
brassclaw-reborn models list --json
brassclaw-reborn models status
brassclaw-reborn models status --json
brassclaw-reborn models set-provider
brassclaw-reborn profile list
brassclaw-reborn profile list --json
brassclaw-reborn repl
brassclaw-reborn run
brassclaw-reborn run --confirm-host-access
brassclaw-reborn serve
brassclaw-reborn serve --host 0.0.0.0 --port 3000
brassclaw-reborn serve --confirm-host-access
brassclaw-reborn skills list
brassclaw-reborn skills list --json
brassclaw-reborn skills list --verbose
```

It intentionally does not yet support:

- replacing `brassclaw` behavior
- daemon/service installation
- v1 config, DB, settings, or secrets migration
- production extension/tool execution in long-lived services

## Commands

### `repl`

Starts an interactive REPL session. Reads messages from stdin and writes responses to stdout. Requires LLM provider environment variables to be set; without them, messages fail cleanly because no LLM gateway is wired.

```bash
brassclaw-reborn repl
```

### `run`

Starts the standalone Reborn runtime and reads a single message from stdin or from `--message`. Without model provider environment variables, the runtime still starts but messages fail cleanly.

```bash
brassclaw-reborn run
brassclaw-reborn run --message "hello"
brassclaw-reborn run --dry-run
```

Use `--dry-run` for a side-effect-free readiness snapshot. Expected fields in the output include:

- `binary: brassclaw-reborn`
- `version`
- `reborn_home`
- `home_source`
- `profile`
- `v1_state: not-used`
- `runtime_driver: planned-agent-loop`
- `driver_registry: initialized`
- `local_runtime_shell_readiness: ready`
- `planned_default_profile: available`

For `BRASSCLAW_REBORN_PROFILE=local-dev-yolo`, `run`, `repl`, and `serve` require `--confirm-host-access` before the runtime receives trusted-laptop host access. Confirmed access mounts the host home through `/host`.

### `serve`

Starts the HTTP server with WebUI v2 and the Reborn runtime. Default bind is `127.0.0.1:3000`. The WebUI v2 SPA is served at `/v2`.

```bash
brassclaw-reborn serve
brassclaw-reborn serve --host 127.0.0.1 --port 3000
brassclaw-reborn serve --host 0.0.0.0 --port 3000   # non-loopback: requires non-yolo profile
brassclaw-reborn serve --confirm-host-access          # required for local-dev-yolo profile
```

When `serve --confirm-host-access` grants trusted-laptop access, `serve` refuses non-loopback listeners such as `0.0.0.0`. Bind to `127.0.0.1` or `::1`, or use a less privileged profile for non-loopback test listeners.

### `config path`

Shows the resolved Reborn state root, its source, selected profile, and v1 state status without creating directories.

```bash
brassclaw-reborn config path
```

Expected fields include:

- `reborn_home`
- `home_source`
- `profile`
- `v1_state: not-used`

### `config init`

Initializes the Reborn state root at the resolved home path, creating required directories and writing default configuration.

```bash
brassclaw-reborn config init
```

### `models list` / `models status`

Shows Reborn model purpose slots and route status. Routes are configured through LLM provider environment variables or through `models set-provider`.

```bash
brassclaw-reborn models list
brassclaw-reborn models list --json
brassclaw-reborn models status
brassclaw-reborn models status --json
```

Expected fields include:

- `default`
- `mission`
- `routes`
- `v1_state: not-used`

### `models set-provider`

Sets the active LLM provider for the current profile.

```bash
brassclaw-reborn models set-provider
```

### `doctor`

Validates and reports Reborn boot configuration without creating state directories or starting runtime services.

```bash
brassclaw-reborn doctor
```

Expected fields include:

- `reborn_home`
- `home_source`
- `profile`
- `v1_state: not-used`
- `driver_registry: initialized`

### `skills list`

Reports configured Reborn local-dev skills from `<reborn-home>/local-dev/skills` and `<reborn-home>/local-dev/system/skills`. A missing local-dev storage root is reported as an empty skill list without creating directories.

```bash
brassclaw-reborn skills list
brassclaw-reborn skills list --json
brassclaw-reborn skills list --verbose
```

Expected fields include:

- `configured: <count>`
- `source: reborn-local-dev`
- per-skill `name`, `source`, and `description` in text output
- per-skill `name`, `version`, `description`, `source`, `keywords`, `tags`, and `requires_skills` in JSON output

`--verbose` adds the resolved `profile`, `reborn_home`, `local_dev_root`, and `owner_id`.

`skills list` supports `local-dev` and `local-dev-yolo` profiles and rejects `production` / `migration-dry-run` until those catalog backends are wired.

### `extension`

Searches and manages local-dev Reborn extensions through the same lifecycle facade exposed to product surfaces. Available extension packages are read from `/system/extensions`, which maps to `<reborn-home>/local-dev/system/extensions` for the local-dev profile.

```bash
brassclaw-reborn extension search github
brassclaw-reborn extension search github --json
brassclaw-reborn extension install github-mcp
brassclaw-reborn extension activate github-mcp
brassclaw-reborn extension remove github-mcp
```

Expected fields include:

- `phase`
- `package_ref.id` for package-specific commands
- `payload.kind`
- `payload.count` and `payload.extensions[].package_ref.id` for search
- `payload.installed`, `payload.activated`, or `payload.removed` for lifecycle mutations

### `channels list`

Reports configured Reborn channels. The Reborn channel registry is not fully wired yet, so the command currently reports an explicit empty surface.

```bash
brassclaw-reborn channels list
brassclaw-reborn channels list --json
brassclaw-reborn channels list --verbose
```

Expected fields include:

- `configured: 0`
- `status: not-wired`
- `v1_state: not-used`

### `hooks list`

Reports configured Reborn hooks. The Reborn hook registry is not wired yet, so the command currently reports an explicit empty surface.

```bash
brassclaw-reborn hooks list
brassclaw-reborn hooks list --json
brassclaw-reborn hooks list --verbose
```

Expected fields include:

- `configured: 0`
- `status: not-wired`
- `v1_state: not-used`

### `logs`

Reports Reborn log availability. The Reborn log source is not wired yet, so the command currently reports an explicit empty surface.

```bash
brassclaw-reborn logs
brassclaw-reborn logs --json
brassclaw-reborn logs --verbose
```

Expected fields include:

- `entries: 0`
- `status: not-wired`
- `v1_state: not-used`

### `profile list`

Lists the supported Reborn boot profiles without resolving Reborn home, reading v1 state, or creating directories.

```bash
brassclaw-reborn profile list
brassclaw-reborn profile list --json
```

Supported profiles:

- `local-dev` (default)
- `local-dev-yolo`
- `production`
- `migration-dry-run`

### `completion`

Generates shell completion scripts without resolving Reborn home, reading v1 state, or creating directories.

```bash
brassclaw-reborn completion --shell zsh > brassclaw-reborn.zsh
brassclaw-reborn completion --shell bash > brassclaw-reborn.bash
```

The zsh output keeps the v1 CLI guard around `compdef` so the generated script is safe when zsh completion functions are not loaded.

## State and Config Root

Reborn must not use the current v1 BrassClaw state root by default.

Home resolution precedence:

1. `BRASSCLAW_REBORN_HOME`
2. `~/.brassclaw/reborn`

The resolver rejects unsafe or misleading homes, including empty paths, relative paths, filesystem root, parent-directory components, and known v1 state-root aliases such as `$HOME/.brassclaw` or `BRASSCLAW_BASE_DIR`.

## Profiles

Use `BRASSCLAW_REBORN_PROFILE` to select the boot profile.

Supported values:

- `local-dev` (default)
- `local-dev-yolo`
- `production`
- `migration-dry-run`

Example:

```bash
BRASSCLAW_REBORN_HOME="$PWD/.reborn-home" \
BRASSCLAW_REBORN_PROFILE=production \
brassclaw-reborn doctor
```

## WebUI v2

WebUI v2 is a React SPA served at `/v2` when the binary is built with ``.

Start the server:

```bash
brassclaw-reborn serve
brassclaw-reborn serve --host 127.0.0.1 --port 3000
```

Open `http://127.0.0.1:3000/v2` in a browser. Bearer-token authentication is required. The token is printed to stdout on first start or can be retrieved with `config path`.

For non-loopback access:

- Do not use `local-dev-yolo` with `--confirm-host-access` and `--host 0.0.0.0` together; that combination is rejected.
- Use `local-dev` or `production` profile for non-loopback test listeners.

## Local Smoke Checks

Run these before changing Reborn CLI behavior:

```bash
cargo fmt --all -- --check
cargo test -p brassclaw_reborn_cli
cargo test -p brassclaw_reborn_config
cargo test -p brassclaw_reborn model_slots_are_exposed_in_cli_display_order
cargo test -p brassclaw_architecture reborn
cargo clippy -p brassclaw_reborn_cli --all-targets -- -D warnings
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- --help
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- channels list
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- completion --shell zsh >/tmp/brassclaw-reborn.zsh
BRASSCLAW_REBORN_HOME="$(mktemp -d)/reborn-home" \
  cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- config path
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- hooks list
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- logs
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- models status
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- profile list
BRASSCLAW_REBORN_HOME="$(mktemp -d)/reborn-home" \
  cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- run --dry-run
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- skills list
```

## Adding Commands

New commands follow the crate-local agent contract in `crates/brassclaw_reborn_cli/AGENTS.md`.

Short version:

1. Add one command module under `crates/brassclaw_reborn_cli/src/commands/`.
2. Register it in `commands::Command`.
3. Resolve and pass `RebornCliContext` from dispatch only when the command needs boot config.
4. Keep pure commands independent from Reborn home resolution.
5. Add a binary smoke test through `env!("CARGO_BIN_EXE_brassclaw-reborn")`.
6. Avoid v1 runtime imports and v1 state mutation unless explicitly scoped and guarded.

Do not port the current `src/cli/*` command tree wholesale. Port commands one at a time, starting with Reborn-owned or read-only surfaces.

## Release Packaging

`brassclaw-reborn` is not yet included in cargo-dist release artifacts.

Until issue #3483 is resolved, `crates/brassclaw_reborn_cli/Cargo.toml` keeps:

```toml
[package.metadata.dist]
dist = false
```

Removing `dist = false` alone is not sufficient to ship `brassclaw-reborn` in the existing release workflow, which is shaped around the root `brassclaw` package tag. Enabling a standalone release also requires cargo-dist WiX metadata work and an explicit tag/versioning decision.
