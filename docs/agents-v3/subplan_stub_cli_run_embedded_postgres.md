# Subplan: Wire Embedded Postgres Startup into `brassclaw_reborn_cli` `execute()` path

**Status:** [x] Done
**Scope:** `crates/brassclaw_reborn_cli/src/runtime/mod.rs`  
**Triggered by:** 12 failing smoke tests (`repl_exit_command_exits_cleanly_without_touching_v1_state`, `run_help_command_prints_repl_commands_and_exits_on_quit`, `repl_piped_message_exits_nonzero_when_runtime_does_not_produce_reply`, `run_message_exits_nonzero_when_runtime_does_not_produce_reply`, `run_piped_stdin_exits_nonzero_when_runtime_does_not_produce_reply`, `repl_resolves_codex_auth_env_without_openai_api_key`, `repl_resolves_codex_api_key_auth_env_without_openai_api_key`, `run_rejects_codex_backend_when_auth_file_is_missing`, `repl_help_command_prints_repl_commands_and_exits_on_exit`, `run_help_command_prints_repl_commands_and_exits_on_quit`, plus 2 run tests)

---

## Problem

`execute()` in `runtime/mod.rs` calls `build_reborn_runtime(runtime_input)` but does NOT start
embedded Postgres before calling it. When `BRASSCLAW_PG_URL` is absent (as in the smoke tests
that use `env_clear()`), the Postgres pool is required but never started — the runtime panics
with:

```
Postgres pool is required for PgSubagentGoalStore (postgres is mandatory; in-memory fallback removed)
```

The `serve` subcommand correctly calls `start_postgres_and_upgrade_input()` (the `#[cfg(feature = "postgres")]` block at lines 320–326 of `serve.rs`) before calling `build_reborn_runtime`. 

The `execute()` path (used by `repl` and `run`) has a stub:

```rust
/// Wires the local trigger-fire access checker into `runtime_input` for the
/// `run` command. Currently a no-op until embedded PG is plumbed into the
/// local-dev run path (known tech-debt: wire pool after embedded-PG startup).
async fn with_run_local_trigger_fire_access_checker(
    runtime_input: RebornRuntimeInput,
    _config: &RebornBootConfig,
) -> anyhow::Result<RebornRuntimeInput> {
    Ok(runtime_input)
}
```

---

## Root Cause

Commit `7b2527a3` (B-3: Remove RebornCompositionProfile — always-Postgres) removed the 
fallback to in-memory / libSQL backends, making Postgres mandatory. The `serve.rs` path was
updated at that point, but the `execute()` path in `runtime/mod.rs` was left with just the
no-op stub comment.

---

## Fix (minimal)

### Step R.1 — Implement `start_postgres_for_run` in `runtime/mod.rs`

Add a new `#[cfg(feature = "postgres")]` function that mirrors `serve.rs`'s
`start_postgres_and_upgrade_input`:

```rust
#[cfg(feature = "postgres")]
async fn start_postgres_for_run(
    input: RebornRuntimeInput,
    boot_config: &brassclaw_reborn_config::RebornBootConfig,
) -> anyhow::Result<(RebornRuntimeInput, Option<brassclaw_embedded_postgres::ManagedPostgres>)> {
    use brassclaw_embedded_postgres::{EmbeddedPostgresConfig, ManagedPostgres};
    use brassclaw_pg::migrations;
    use brassclaw_pg::pool::build_pool;
    use brassclaw_reborn_composition::local_dev_runtime_policy;
    use secrecy::SecretString as SecretMaterial;

    let (pg_url, managed_pg) = if let Ok(url) = std::env::var("BRASSCLAW_PG_URL") {
        (url, None)
    } else {
        let config = EmbeddedPostgresConfig::from_reborn_home(boot_config.home().path());
        let managed = ManagedPostgres::start(config)
            .await
            .map_err(|e| anyhow::anyhow!("failed to start embedded Postgres: {e}"))?;
        let url = managed.connection_url();
        (url, Some(managed))
    };

    let pool = build_pool(&pg_url)
        .map_err(|e| anyhow::anyhow!("failed to build Postgres connection pool: {e}"))?;

    migrations::run_migrations(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Postgres schema migrations failed: {e}"))?;

    let owner_id = input
        .services
        .as_ref()
        .map(|s| s.owner_id().to_string())
        .unwrap_or_else(|| "reborn-cli".to_string());

    let reborn_home = boot_config.home().path().to_path_buf();
    let runtime_policy = local_dev_runtime_policy()
        .map_err(|e| anyhow::anyhow!("failed to resolve local-dev runtime policy: {e}"))?;
    let pg_input = brassclaw_reborn_composition::RebornBuildInput::postgres_with_reborn_home(
        owner_id,
        pool,
        SecretMaterial::from(pg_url),
        reborn_home,
    )
    .with_runtime_policy(runtime_policy);

    let mut upgraded = input;
    upgraded.services = Some(pg_input);
    Ok((upgraded, managed_pg))
}
```

### Step R.2 — Update `execute()` to call `start_postgres_for_run` before `build_reborn_runtime`

In `execute()`, inside the `rt.block_on(async move { ... })`:

```rust
// Before (stub):
let runtime_input =
    with_run_local_trigger_fire_access_checker(runtime_input, &boot_config).await?;
let runtime = build_reborn_runtime(runtime_input).await?;

// After:
#[cfg(feature = "postgres")]
let (runtime_input, managed_pg) =
    start_postgres_for_run(runtime_input, &boot_config).await?;
#[cfg(not(feature = "postgres"))]
let managed_pg: Option<brassclaw_embedded_postgres::ManagedPostgres> = None;

let runtime_input =
    with_run_local_trigger_fire_access_checker(runtime_input, &boot_config).await?;
let runtime = build_reborn_runtime(runtime_input).await?;

// ... existing run/repl logic ...

runtime.shutdown().await?;
// Shut down the embedded Postgres only after the runtime pool is dropped.
if let Some(pg) = managed_pg
    && let Err(error) = pg.shutdown().await
{
    tracing::debug!(%error, "embedded Postgres shutdown failed (run path)");
}
```

### Step R.3 — No-op the `with_run_local_trigger_fire_access_checker` stub

The stub remains but now correctly says it's permanently a no-op for the `run` path
(the trigger access checker needs a user identity — the `run` path has no WebUI user).
Update the comment.

### Step R.4 — Verify

Run `cargo test -p brassclaw_reborn_cli --test smoke` and confirm all 12 tests pass.

---

## Implementation steps

- [x] **R.1** — Add `start_postgres_for_run` in `runtime/mod.rs`
- [x] **R.2** — Update `execute()` to call it before `build_reborn_runtime`
- [x] **R.3** — Update stub comment in `with_run_local_trigger_fire_access_checker`
- [x] **R.4** — Run smoke tests to verify (75 passed, 4 ignored, 0 failed)
- [x] **R.5** — Fix port conflicts in parallel smoke tests (add `free_pg_port()` helper + `BRASSCLAW_EMBEDDED_PG_PORT` per test)
- [x] **R.6** — Fix stale `skills list` test assertions after Phase P.1 (system skills → DB only)
- [x] **R.7** — Fix stale `run_warns_when_falling_back_to_stub_gateway` assertion (VFS installer removed)
