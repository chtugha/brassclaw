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

The original PG connection is implemented. L-01/L-02 repair its transition in place:
`RebornBuildInput::into_postgres_with_reborn_home` in Composition `src/input.rs`
changes only LocalDev storage, preserving the resolved policy, both mount overrides
and all service extras. Missing services/policy fail before server acquisition;
failed pool/input preparation stops only the server owned by this call.

The CLI does not run the general migrations itself. `build_reborn_runtime` calls
`build_reborn_services`, whose Postgres branch owns the general application-schema
runner before the first schema consumer. Serve retains its earlier migration for
the ACL seed that precedes runtime construction. The current run transition is:

```rust
#[cfg(feature = "postgres")]
async fn start_postgres_for_run(
    mut input: RebornRuntimeInput,
    boot_config: &RebornBootConfig,
) -> anyhow::Result<(
    RebornRuntimeInput,
    Option<brassclaw_embedded_postgres::ManagedPostgres>,
)> {
    use brassclaw_embedded_postgres::{EmbeddedPostgresConfig, ManagedPostgres};
    use brassclaw_pg::pool::build_pool;
    // SecretMaterial is secrecy::SecretString — use directly to avoid
    // depending on brassclaw_secrets as a direct CLI dep.
    use secrecy::SecretString as SecretMaterial;

    // Fail before acquiring a server when no resolved service contract exists.
    // The storage change must preserve the CLI's actual policy and host mounts.
    let services = input
        .services
        .take()
        .context("run Postgres transition requires an existing services input")?;
    services
        .runtime_policy()
        .context("run Postgres transition requires an already resolved runtime policy")?;

    // If the operator supplied an external PG URL, use it directly.
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

    // Keep fallible pool/input preparation in this scope so its handles are
    // released before an owned server is stopped after an error.
    let upgraded = build_pool(&pg_url)
        .map_err(|e| anyhow::anyhow!("failed to build Postgres connection pool: {e}"))
        .and_then(|pool| {
            services
                .into_postgres_with_reborn_home(
                    pool,
                    SecretMaterial::from(pg_url),
                    boot_config.home().path().to_path_buf(),
                )
                .map(|services| services.with_tenant_id(input.identity.tenant_id.clone()))
                .context("failed to retain the resolved services input during Postgres transition")
        });
    match upgraded {
        Ok(services) => {
            input.services = Some(services);
            Ok((input, managed_pg))
        }
        Err(error) => {
            if let Some(pg) = managed_pg
                && let Err(shutdown) = pg.shutdown().await
            {
                std::mem::forget(pg);
                return Err(error.context(format!(
                    "embedded Postgres cleanup after input transition failure also failed; owned instance requires recovery: {shutdown}"
                )));
            }
            Err(error)
        }
    }
}
```

### Step R.2 — Update `execute()` to call `start_postgres_for_run` before `build_reborn_runtime`

In `execute()`, inside the `rt.block_on(async move { ... })`:

The implemented order is: acquire the PG input, then call `build_reborn_runtime`
(which owns application-schema migration through `build_reborn_services`). The
fallible input handoff and runtime construction are scoped together. Construction
failure explicitly shuts down only the `ManagedPostgres` owned by this invocation,
retaining both the primary error and any cleanup failure. No immediate-Drop path
is used as normal error handling.

The current implementation in `crates/brassclaw_reborn_cli/src/runtime/mod.rs`
is authoritative. Successful startup drains the instance MCP listener and runtime
before stopping owned PostgreSQL, and reports shutdown failures as errors; do not
restore the historical `runtime.shutdown().await?`/debug-only PG cleanup example.
See `docs/plans/subplan_step_l01_startup_cleanup_of_bob1.md` for the exact adjacent
failure case and its native acceptance.


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
