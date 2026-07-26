# Subplan: Step 2 — Wire PgExtensionInstallationStore in serve path ✅ DONE

## Problem

`build_local_dev()` constructs `FilesystemExtensionInstallationStore` at line ~862
in factory.rs. On the hybrid path (`brassclaw serve` = LocalDev + Postgres),
extension install records are lost on process restart because the filesystem store
writes to the VFS rather than the PG `brassclaw_extension_manifests` table.

`PgExtensionInstallationStore` is fully implemented in
`crates/brassclaw_extensions/src/pg_store.rs` but never used in composition.

## Constraints

- `build_local_dev()` takes `RebornBuildInput` which contains only local-dev
  filesystem paths — no Postgres pool.
- The PG pool is available in `build_reborn_services()` at the call site of
  `build_local_dev()` (line ~580) as `pool: deadpool_postgres::Pool`.
- We cannot change `RebornBuildInput` (public type, many callers).
- After `build_local_dev()` returns, `extension_management` is inside
  `Arc<RebornLocalRuntimeServices>` and cannot be replaced via `Arc::get_mut`
  because the Arc may already be aliased internally.

## Chosen approach

Add an optional `#[cfg(feature = "postgres")]` parameter to the internal
`build_local_dev()` function:

```rust
async fn build_local_dev(
    input: RebornBuildInput,
    #[cfg(feature = "postgres")] pg_pool: Option<Arc<deadpool_postgres::Pool>>,
) -> Result<RebornServices, RebornBuildError>
```

All internal callers (tests etc.) pass `None`. The hybrid path in
`build_reborn_services()` passes `Some(pg_pool_arc)`.

Inside `build_local_dev()`, after `pool` is `Some`, replace
`FilesystemExtensionInstallationStore::load(...)` with
`PgExtensionInstallationStore::new(Arc::clone(&pool), "default")`.

## Steps

### Step 2a — Add `pg_pool` parameter to `build_local_dev()`

In `factory.rs`:
- Add `#[cfg(feature = "postgres")] pg_pool: Option<Arc<deadpool_postgres::Pool>>`
  as second parameter to `build_local_dev()`.
- All non-hybrid callers of `build_local_dev` (plain `build_local_dev(input)`)
  must be updated to `build_local_dev(input, #[cfg(feature = "postgres")] None)`.
  Find all call sites with grep.

### Step 2b — Conditionally use `PgExtensionInstallationStore`

At the point where `extension_installation_store` is constructed (factory.rs ~862):

```rust
#[cfg(feature = "postgres")]
let extension_installation_store: Arc<dyn ExtensionInstallationStore> =
    if let Some(pool) = pg_pool.as_ref() {
        Arc::new(brassclaw_extensions::PgExtensionInstallationStore::new(
            Arc::clone(pool),
            "default",
        ))
    } else {
        Arc::new(
            FilesystemExtensionInstallationStore::load(extension_filesystem.clone())
                .await
                .map_err(|error| RebornBuildError::InvalidConfig {
                    reason: format!("extension installation state could not be loaded: {error}"),
                })?,
        )
    };
#[cfg(not(feature = "postgres"))]
let extension_installation_store: Arc<dyn ExtensionInstallationStore> = Arc::new(
    FilesystemExtensionInstallationStore::load(extension_filesystem.clone())
        .await
        .map_err(|error| RebornBuildError::InvalidConfig {
            reason: format!("extension installation state could not be loaded: {error}"),
        })?,
);
```

Note: `PgExtensionInstallationStore::new()` is sync and infallible (no `load()`
needed) — it just wraps the pool. No `await` needed.

### Step 2c — Update call site in `build_reborn_services()`

In the hybrid path (factory.rs ~581), change:
```rust
let mut services = build_local_dev(local_input).await?;
```
to:
```rust
let mut services = build_local_dev(
    local_input,
    #[cfg(feature = "postgres")] Some(Arc::clone(&pg_pool_arc)),
).await?;
```

### Step 2d — Update all other `build_local_dev()` call sites

Find every other call to `build_local_dev` and add `None` for the pool:
```rust
build_local_dev(input, #[cfg(feature = "postgres")] None).await?
```

Expected callers (from grep results):
- `build_reborn_services()` line ~591: `build_local_dev(input).await` (non-Postgres branch)
- Test helpers in `factory.rs` (~lines 2944, 3047, 3072, 3148, 3262, 3323, 3374, 3456, 3498, 3560)

### Step 2e — Run clippy + tests

```bash
cargo clippy -p brassclaw_reborn_composition --all-targets --all-features -- -D warnings
cargo test -p brassclaw_reborn_composition -p brassclaw_extensions
```

### Step 2f — Mark Step 2 done in final_gaps_pre-v3.md, commit + push

## Files to touch

- `crates/brassclaw_reborn_composition/src/factory.rs` — all changes
- `final_gaps_pre-v3.md` — mark Step 2 DONE
