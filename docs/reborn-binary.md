# `brassclaw-reborn` standalone binary

`brassclaw-reborn` is the standalone executable boundary for Reborn. It is separate from the current `brassclaw` binary so Reborn boot, config, state, and runtime composition can evolve without accidentally invoking v1 runtime paths.

This binary is available as the workspace package `brassclaw_reborn_cli` and builds the executable named `brassclaw-reborn`.

## Current status

`brassclaw-reborn` is an early operator/testing surface, not the default BrassClaw runtime.

It currently supports:

```bash
brassclaw-reborn --help
brassclaw-reborn channels list
brassclaw-reborn channels list --json
brassclaw-reborn channels list --verbose
brassclaw-reborn completion --shell bash
brassclaw-reborn completion --shell zsh
brassclaw-reborn config path
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
brassclaw-reborn profile list
brassclaw-reborn profile list --json
brassclaw-reborn repl
brassclaw-reborn run
brassclaw-reborn run --confirm-host-access
brassclaw-reborn serve
brassclaw-reborn serve --confirm-host-access
brassclaw-reborn skills list
brassclaw-reborn skills list --json
brassclaw-reborn skills list --verbose
```

It intentionally does not yet support:

- replacing `brassclaw` behavior;
- daemon/service installation;
- web gateway/UI startup;
- v1 config, DB, settings, or secrets migration;
- production extension/tool execution;
- long-lived Reborn runtime services.

## Commands

### `channels list`

Reports configured Reborn channels without resolving Reborn home, reading v1 channel config, or creating directories.

The Reborn channel registry is not wired yet, so the command currently reports an explicit empty surface:

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- channels list
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- channels list --json
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- channels list --verbose
```

Expected fields include:

- `configured: 0`
- `status: not-wired`
- `v1_state: not-used`

### `extension`

Searches and manages local-dev Reborn extensions through the same lifecycle facade exposed to product surfaces. Available extension packages are read from `/system/extensions`, which maps to `<reborn-home>/local-dev/system/extensions` for the local-dev profile.

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- extension search github
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- extension search github --json
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- extension install github-mcp
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- extension activate github-mcp
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- extension remove github-mcp
```

The commands are scoped to Reborn boot/config resolution and do not create or read v1 state directories.

Expected fields include:

- `phase`
- `package_ref.id` for package-specific commands
- `payload.kind`
- `payload.count` and `payload.extensions[].package_ref.id` for search
- `payload.installed`, `payload.activated`, or `payload.removed` for lifecycle mutations

### `completion`

Generates shell completion scripts without resolving Reborn home, reading v1 state, or creating directories.

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- completion --shell zsh > brassclaw-reborn.zsh
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- completion --shell bash > brassclaw-reborn.bash
```

The zsh output keeps the v1 CLI guard around `compdef` so the generated script is safe when zsh completion functions are not loaded yet.

### `config path`

Shows the resolved Reborn state root, its source, selected profile, and explicit v1-state status without creating directories.

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- config path
```

Expected fields include:

- `reborn_home`
- `home_source`
- `profile`
- `v1_state: not-used`

### `doctor`

Validates and reports Reborn boot configuration without creating state directories or starting runtime services.

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- doctor
```

Expected fields include:

- `reborn_home`
- `home_source`
- `profile`
- `v1_state: not-used`
- `driver_registry: initialized`

### `hooks list`

Reports configured Reborn hooks without resolving Reborn home, reading v1 hook config, or creating directories.

The Reborn hook registry is not wired yet, so the command currently reports an explicit empty surface:

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- hooks list
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- hooks list --json
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- hooks list --verbose
```

Expected fields include:

- `configured: 0`
- `status: not-wired`
- `v1_state: not-used`

### `logs`

Reports Reborn log availability without resolving Reborn home, reading v1 gateway logs, or creating directories.

The Reborn log source is not wired yet, so the command currently reports an explicit empty surface:

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- logs
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- logs --json
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- logs --verbose
```

Expected fields include:

- `entries: 0`
- `status: not-wired`
- `v1_state: not-used`

### `models list` / `models status`

Shows Reborn model purpose slots and route status without resolving Reborn home, reading v1 provider settings, or creating directories.

Routes are not configurable through Reborn CLI yet, so the command currently reports `not-configured` routes for built-in slots:

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- models list
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- models list --json
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- models status
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- models status --json
```

Expected fields include:

- `default`
- `mission`
- `routes: not-configured`
- `v1_state: not-used`

### `profile list`

Lists the supported Reborn boot profiles without resolving Reborn home, reading v1 state, or creating directories.

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- profile list
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- profile list --json
```

Supported profiles:

- `local-dev` (default)
- `local-dev-yolo`
- `production`
- `migration-dry-run`

Select a profile with `BRASSCLAW_REBORN_PROFILE=<profile>`.

### `run`

Starts the standalone Reborn runtime and reads messages from stdin. The no-profile path targets the planned AgentLoop runtime (`reborn-planned-default`). Without model provider environment variables, the runtime still starts but messages fail cleanly because no LLM gateway is wired.

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- run
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- run --message "hello"
```

Use `--dry-run` for the side-effect-free readiness snapshot:

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- run --dry-run
```

Expected fields include:

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

For `BRASSCLAW_REBORN_PROFILE=local-dev-yolo`, `run`, `repl`, and `serve` require `--confirm-host-access` before the runtime receives trusted-laptop host access. Confirmed access mounts the host home through `/host`; Unix-style raw home aliases are also accepted when they can be represented as scoped mount aliases.

When `serve --confirm-host-access` grants trusted-laptop access, `serve` refuses non-loopback listeners such as `0.0.0.0`. Bind to `127.0.0.1` or `::1`, or use a less privileged profile for non-loopback test listeners.

### `skills list`

Reports configured Reborn local-dev skills from `<reborn-home>/local-dev/skills`
and `<reborn-home>/local-dev/system/skills` through the Reborn composition
skill listing function. It does not read v1 skill discovery paths, and a missing
local-dev storage root is reported as an empty skill list without creating
directories.

```bash
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- skills list
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- skills list --json
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- skills list --verbose
```

Expected fields include:

- `configured: <count>`
- `source: reborn-local-dev`
- per-skill `name`, `source`, and `description` in text output
- per-skill `name`, `version`, `description`, `source`, `keywords`, `tags`,
  and `requires_skills` in JSON output

`--verbose` adds the resolved `profile`, `reborn_home`, `local_dev_root`, and
`owner_id`; text output also includes per-skill `version`, `keywords`, `tags`,
and `requires_skills` when present. `skills list` currently supports
`local-dev` and `local-dev-yolo` profiles and rejects `production` /
`migration-dry-run` until those catalog backends are wired.

## State and config root

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
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- doctor
```

## Local smoke checks

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
  cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- run
cargo run -q -p brassclaw_reborn_cli --bin brassclaw-reborn -- skills list
```

## Adding commands

Future commands should follow the crate-local agent contract in:

```text
crates/brassclaw_reborn_cli/AGENTS.md
```

Short version:

1. add one command module under `crates/brassclaw_reborn_cli/src/commands/`;
2. register it in `commands::Command`;
3. resolve and pass `RebornCliContext` from dispatch only when the command needs boot config;
4. keep pure commands independent from Reborn home resolution;
5. add a binary smoke test through `env!("CARGO_BIN_EXE_brassclaw-reborn")`;
6. avoid v1 runtime imports and v1 state mutation unless explicitly scoped and guarded.

Do not port the current `src/cli/*` command tree wholesale. Port commands one at a time, starting with Reborn-owned or read-only surfaces.

## Release packaging decision

`brassclaw-reborn` is **not yet included in cargo-dist release artifacts**.

Current `dist plan --output-format=json` with `crates/brassclaw_reborn_cli` marked `dist = false` emits only the root `brassclaw` package artifacts. Removing `dist = false` alone is not enough to ship `brassclaw-reborn` in the existing `brassclaw-v*` release workflow because that workflow is shaped around the root `brassclaw` package tag. Enabling a standalone `brassclaw_reborn_cli` release also requires cargo-dist WiX metadata/template work and an explicit tag/versioning decision.

Follow-up issue: #3483 tracks packaging `brassclaw-reborn` in release artifacts.

Until #3483 is resolved, keep:

```toml
[package.metadata.dist]
dist = false
```

in `crates/brassclaw_reborn_cli/Cargo.toml` so releases do not silently claim to ship an unverified Reborn binary package.
