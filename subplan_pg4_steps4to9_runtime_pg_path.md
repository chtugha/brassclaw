# Subplan: PG-4 Steps 4–9 — Wire `build_reborn_runtime` for the pure-postgres path

## Status: ✅ IMPLEMENTED

## Goal

Remove the early-return "not yet wired" gate in `build_reborn_runtime` and replace
every `local_runtime.X` reference with a `RuntimeSubstrate` enum that covers both
the local-dev path and the pure-postgres path.

## Approach

Instead of rewriting the whole function body we introduce a thin
`RuntimeSubstrate` enum that carries either `&RebornLocalRuntimeServices` or
`&PgRuntimeStores`. We add getter methods for each field that is used from
`local_runtime`. That keeps diffs minimal and lets us switch paths by building
the right variant.

```
enum RuntimeSubstrate<'a> {
    LocalDev(&'a RebornLocalRuntimeServices),
    Postgres(&'a PgRuntimeStores),
}
```

The getters return `Arc<dyn ...>` so callers need no `cfg` gates.

## Reference: local_runtime usages to eliminate

| Line(s) | Field accessed | PG substitute |
|---------|---------------|---------------|
| 1596 | `turn_state` | `pg.turn_state` |
| 1597 | `checkpoint_state_store` | `pg.checkpoint_state_store` |
| 1598 | `loop_checkpoint_store` | `pg.loop_checkpoint_store` |
| 1609 | `thread_service` (fallback) | `PgSessionThreadService::new(pool,"default")` |
| 1722-1723 | passed to `local_dev_filesystem_skill_context_source(local_runtime,...)` | guard with `if let Some(lr) = services.local_runtime` |
| 1911 | `broadcast_budget_event_sink` | `pg.broadcast_budget_event_sink` |
| 1914 | `resource_governor` | `pg.resource_governor` |
| 1916 | `budget_gate_store` | `pg.budget_gate_store` |
| 1931 | `event_log` | `pg.event_log` |
| 1932 | `audit_log` | `pg.audit_log` |
| 1986 | passed to `local_dev::capability_wiring(services,...)` — returns `None` if no `local_runtime` | already returns `None` if `services.local_runtime.is_none()`; but the `.ok_or(HostRuntimeUnavailable)` on the result would fail — guard it |
| 2006 | `content_cache_slot` | `CurrentCacheBridgeSlot::default()` |
| 2035 | `extension_filesystem` → `ThirdPartyDiscoveryInput.filesystem` | `None` |
| 2175-2176 | `local_dev_storage_root`, `default_system_prompt_path` | `pg.local_dev_storage_root`, `pg.default_system_prompt_path` |
| 2239 | `approval_requests` | `pg.approval_requests` |
| 2245 | `approval_requests` | `pg.approval_requests` |
| 2255-2257 | `workspace_mounts`, `skill_mounts`, `memory_mounts` | `MountView::empty()` |
| 2303 | passed to `build_trigger_poller_services(local_runtime,...)` | refactor signature |
| 2321 | `trigger_repository` | `pg.trigger_repository` |
| 2356-2363 | `services.local_runtime.as_ref()` for budget projection | always build from substrate |
| 2369-2376 | `services.local_runtime.as_ref()` for plan library | `None` on PG path |

## Step-by-step

### A — Add `RuntimeSubstrate` helper to `factory.rs`

Add (inside `#[cfg(feature = "postgres")]`):

```rust
pub(crate) enum RuntimeSubstrate<'a> {
    LocalDev(&'a RebornLocalRuntimeServices),
    #[cfg(feature = "postgres")]
    Postgres(&'a PgRuntimeStores),
}
```

Methods:
- `turn_state() -> Arc<LocalDevTurnStateStore>` / `Arc<PgTurnStateStore>` — but
  both consumers need `Arc<LocalDevTurnStateStore>` for `build_webui_auth_interaction_service`.
  So for that one keep using `services.local_runtime` directly.
- Actually: define substrate getters that return `Arc<dyn Trait>` where possible.
  The exceptions (typed Arc required): `turn_state_store` which must be `Arc<LocalDevTurnStateStore>`
  for `build_webui_auth_interaction_service` — keep that function local-dev only.

### B — Build the `RuntimeSubstrate` at the start of `build_reborn_runtime`

Replace the early-return gate with:
```rust
let pg_stores: Option<PgRuntimeStores>;
#[cfg(feature = "postgres")]
let substrate: RuntimeSubstrate = if let Some(lr) = services.local_runtime.as_ref() {
    pg_stores = None;
    RuntimeSubstrate::LocalDev(lr)
} else {
    let pool = services.pg_pool.as_ref()
        .ok_or_else(|| RebornRuntimeError::InvalidArgument { ... })?;
    let stores = build_pg_runtime_stores(Arc::clone(pool), ...).await?;
    pg_stores = Some(stores);
    RuntimeSubstrate::Postgres(pg_stores.as_ref().unwrap())
};
```

### C — Replace each `local_runtime.X` reference with `substrate.X()`

Walk the reference table above line by line.

### D — Refactor `build_trigger_poller_services`

Change signature to take `trigger_repository` directly instead of `local_runtime`.

### E — Fix `build_webui_auth_interaction_service`

Its `turn_state_store: Arc<LocalDevTurnStateStore>` param makes it local-dev only.
On the PG path skip wiring it (return `UnavailableAuthInteractionService`) — the
WebUI auth flow is not available on the headless postgres path anyway.

### F — Remove `#[allow(dead_code)]` from `PgRuntimeStores`, `build_pg_runtime_stores`

### G — Clippy + tests

```bash
cargo clippy -p brassclaw_reborn_composition --all-targets --all-features -- -D warnings
cargo test -p brassclaw_reborn_composition
```

## Files to touch

- `crates/brassclaw_reborn_composition/src/factory.rs` — add `RuntimeSubstrate` enum
- `crates/brassclaw_reborn_composition/src/runtime.rs` — main body changes
