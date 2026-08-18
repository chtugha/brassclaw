# Goals Pre-V3 Review — Audit & Fix Plan

> **Purpose:** Audit and fix all remaining gaps for Goal 1 (no installation profiles) and Goal 2 (no postgres-less design) before the v3 plan begins.
> **Working Rules:** Do steps one by one. After each step: mark it `[DONE]`, commit + push to `origin/main`, continue.
> **Never suppress findings — always resolve until clean.**

---

## Goal 1: No Installation Profiles

The `RebornCompositionProfile` enum has been removed. The hard-error on `BRASSCLAW_REBORN_PROFILE` is implemented in code. However, **deployment files and documentation still reference the old env var name**, and some type/function names still contain "local-dev" (which is acceptable — those are implementation names, not profile enum variants).

## Goal 2: No Postgres-less Design

Postgres is mandatory. The `default = ["postgres"]` feature is set. However, **multiple production code paths still silently fall back to in-memory or filesystem backends when the Postgres pool is unavailable at runtime**. These are not safe because the pool can be `None` during boot races or misconfigurations — and the system silently continues with non-durable state instead of failing with a clear error.

Additionally, `RamSource` (a DB-less retrieval backend), `DbLessFallback` intent resolution variant, and `retrieval_dbless.rs` still exist as active code paths, contradicting the "postgres is mandatory" goal.

---

## Steps

### Step 1 — Fix Deployment Files: `BRASSCLAW_REBORN_PROFILE` → `BRASSCLAW_RUNTIME_PROFILE`

**Status:** [DONE]

**Files:**
- `deploy/brassclaw.service` line 20: `Environment=BRASSCLAW_REBORN_PROFILE=local-dev` → `Environment=BRASSCLAW_RUNTIME_PROFILE=local_dev`
- `deploy/brassclaw.service` lines 16-19: Update comment to explain the new env var, valid values, and remove Phase 11 reference
- `deploy/dietpi-setup.sh` line 120: `Environment=BRASSCLAW_REBORN_PROFILE=local-dev-yolo` → `Environment=BRASSCLAW_RUNTIME_PROFILE=local_yolo`

**Fix:**
1. In `deploy/brassclaw.service`: Replace the env var and update its documentation comment.
2. In `deploy/dietpi-setup.sh`: Replace the env var name and value.

---

### Step 2 — Fix Documentation: Remove `BRASSCLAW_REBORN_PROFILE` References

**Status:** [DONE]

**Files:**
- `README.md` lines ~225-245: References `BRASSCLAW_REBORN_PROFILE` as active profile env var and has a "Profiles" section — remove or update to `BRASSCLAW_RUNTIME_PROFILE`
- `docs/reborn-binary.md` multiple lines: References to `BRASSCLAW_REBORN_PROFILE=local-dev-yolo`, `BRASSCLAW_REBORN_PROFILE=production` — update to `BRASSCLAW_RUNTIME_PROFILE` with correct values
- `docs/brassclaw-architecture.md` multiple lines: References `BRASSCLAW_REBORN_PROFILE`, `local-dev` profile selection — update

**Fix:** Update all documentation to use `BRASSCLAW_RUNTIME_PROFILE` and the new profile values (`local_dev`, `local_yolo`, `local_safe`, `hosted_safe`). Remove or update the "Profiles" section in README.md.

---

### Step 3 — Eliminate In-Memory Subagent Goal Store Fallback (`runtime.rs`)

**Status:** [ ] Pending

**File:** `crates/brassclaw_reborn_composition/src/runtime.rs` lines ~2343-2355

**Current code:**
```rust
#[cfg(feature = "postgres")]
let subagent_goal_store: Arc<dyn brassclaw_reborn::runtime::RuntimeSubagentGoalStore> =
    if let Some(pool) = services.pg_pool.as_ref() {
        Arc::new(PgSubagentGoalStore::new((**pool).clone()))
    } else {
        Arc::new(InMemoryBoundedSubagentGoalStore::new())  // ← SILENT FALLBACK
    };
```

**Problem:** When `pg_pool` is `None` (e.g., boot race, misconfiguration), subagent goals silently use in-memory storage. Goals survive only until the next process restart. This is a silent data-loss path.

**Fix:** Remove the `else` branch. If the pool is not available, bail with a clear error:
```rust
#[cfg(feature = "postgres")]
let subagent_goal_store: Arc<dyn brassclaw_reborn::runtime::RuntimeSubagentGoalStore> = {
    let pool = services.pg_pool.as_ref().ok_or_else(|| RebornRuntimeError::InvalidArgument {
        reason: "Postgres pool required for subagent goal store (postgres is mandatory)".to_string(),
    })?;
    Arc::new(PgSubagentGoalStore::new((**pool).clone()))
};
```

---

### Step 4 — Eliminate In-Memory Outbound State Store Fallback (`runtime.rs`)

**Status:** [ ] Pending

**File:** `crates/brassclaw_reborn_composition/src/runtime.rs` lines ~2360-2374

**Current code:**
```rust
#[cfg(feature = "postgres")]
let outbound_store: Arc<dyn brassclaw_outbound::OutboundStateStore> =
    if let Some(pool) = services.pg_pool.as_ref() {
        Arc::new(brassclaw_outbound::PgOutboundStateStore::new((**pool).clone()))
    } else {
        Arc::new(brassclaw_outbound::InMemoryOutboundStateStore::default())  // ← SILENT FALLBACK
    };
```

**Problem:** Same as Step 3 — silent non-durable fallback. Notification targets do not survive process restart when pool is unavailable.

**Fix:** Remove the `else` branch. Fail with a clear error if pool is unavailable.

---

### Step 5 — Eliminate Filesystem Secret Store Fallback (`factory.rs`)

**Status:** [ ] Pending

**File:** `crates/brassclaw_reborn_composition/src/factory.rs` lines ~804-823

**Current code:**
```rust
#[cfg(feature = "postgres")]
let secret_store: Arc<dyn SecretStore> = if let Some(pool) = pg_pool.as_ref() {
    Arc::new(brassclaw_secrets::PgSecretStore::new(...))
} else {
    let local_dev_secret_store =
        build_local_dev_secret_store(&root, ...)?;
    local_dev_secret_store as Arc<dyn SecretStore>  // ← FILESYSTEM FALLBACK
};
```

**Problem:** When the pool is `None`, secrets silently fall back to the filesystem secret store. This could occur in production if the pool initialization is delayed or fails.

**Context:** This is in `factory.rs` which builds LOCAL DEV services. The production path goes through `build_postgres_production_host_runtime_services`. Investigate whether this factory function is ONLY called for local dev, in which case the fallback may be intentional (local dev without embedded PG during tests). If this is truly only for local dev testing (not production serve), document it clearly with a `#[cfg(test)]` gate or a clear code comment. If it can be hit in production serve, fix by removing the fallback.

**Action:**
1. Verify which callers call this factory function (trace `build_local_dev_host_runtime_services` or similar)
2. If only test code: add a comment explaining this is intentional for unit tests only
3. If production reachable: remove the else branch and fail hard

---

### Step 6 — Eliminate MemoryDoc Recipe Store Fallback (`webui.rs`)

**Status:** [ ] Pending

**File:** `crates/brassclaw_reborn_composition/src/webui.rs` lines ~209-231

**Current code:**
```rust
#[cfg(feature = "postgres")]
if let Some(pool) = services.pg_pool.as_ref() {
    // PgRecipeStoreFacade wired
} else if let Some(memory_doc_store) = services.pg_memory_doc_store.clone() {
    // Non-postgres fallback: keep old MemoryDoc-backed store
    // ← SILENT FALLBACK TO LEGACY STORE
}
```

**Problem:** When no PG pool, falls back to the legacy MemoryDoc-backed recipe store. This contradicts the "postgres is mandatory" goal.

**Fix:** Remove the `else if` fallback. If the pool is `None`, do NOT wire a recipe store (leave it unwired — the API layer returns "unavailable") or fail hard. The MemoryDoc fallback path belongs only in tests.

---

### Step 7 — Eliminate MemoryDoc Recipe Library Fallback (`runtime.rs`)

**Status:** [ ] Pending

**File:** `crates/brassclaw_reborn_composition/src/runtime.rs` lines ~2523-2541

**Current code:**
```rust
#[cfg(feature = "postgres")]
let recipe_lookup = services.pg_pool.as_ref()
    .map(|pool| Arc::new(PgRecipeLibrary::local_dev(Arc::clone(pool))) ...)
    .or_else(|| {
        services.pg_memory_doc_store.as_ref().map(|store| {
            // Fallback: MemoryDoc-backed store (retained until PG-8 cleanup)
            Arc::new(RecipeLibrary::new(dyn_store)) ...  // ← LEGACY FALLBACK
        })
    });
```

**Problem:** Falls back to the legacy `RecipeLibrary` (MemoryDoc-backed) when pool is unavailable. This continues serving old-format recipes silently.

**Fix:** Remove the `.or_else` branch. If pool is unavailable, `recipe_lookup = None` (which disables recipe lookup, causing Tier 2 fallback in RecipeStage — acceptable and explicit).

---

### Step 8 — Remove or Gate `RamSource` (DB-less retrieval backend)

**Status:** [ ] Pending

**File:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`

**Problem:** `RamSource` is a DB-less retrieval backend that reads from an in-memory keyword store and an optional static file (`BRASSCLAW_FALLBACK_CONTENT_FILE` env var). It is still compiled and exported as part of `RetrievalSource`. Its existence allows code paths to bypass Postgres.

The plan (Phase K) explicitly removes `RamSource`. However, until Phase K ships, we must at minimum ensure `RamSource` is not reachable from production serve paths.

**Investigation needed:**
- Find all call sites that construct a `RamSource`
- Determine if any production serve path uses `RamSource`
- If production-reachable: replace with `PostgresSource` or hard-error
- If test-only: gate with `#[cfg(test)]`

**Note:** Phase K will delete `RamSource` entirely. This step ensures it is not silently used in production before that.

---

### Step 9 — Address `DbLessFallback` Intent Resolution Variant

**Status:** [ ] Pending

**File:** `crates/brassclaw_engine/src/memory/intent_system.rs`

**Current code:**
```rust
pub enum IntentResolution {
    Match { component_id: Uuid, component_class_code: i32 },
    Disambiguation { candidates: Vec<IntentCandidate> },
    NoMatch,
    DbLessFallback,  // ← "The intent system is in DB-less mode"
}
```

**File:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`
```rust
Ok(IntentResolution::NoMatch) | Ok(IntentResolution::DbLessFallback) | Err(_) => {
    // Fall back to full UNION ALL
```

**Problem:** `DbLessFallback` is an explicit "we have no DB" code path. With Postgres mandatory, this variant should never be returned. Its presence allows the intent system to silently continue without a DB.

**Investigation needed:** Under what conditions does `resolve_intent` return `DbLessFallback`? (It likely checks if `pool` is `None` and returns this instead of erroring.)

**Fix:**
1. Find where `DbLessFallback` is returned in `intent_system.rs`
2. Replace the condition with an error: if no pool, return `Err(IntentSystemError::NoDatabaseConnection)` instead
3. Update `retrieval_source.rs` to handle this as an error (propagate up, don't silently fall through to UNION ALL)
4. Remove the `DbLessFallback` variant from the enum

---

### Step 10 — Verify and Document Remaining `if pg_pool.is_some()` Branches in `runtime.rs`

**Status:** [ ] Pending

**File:** `crates/brassclaw_reborn_composition/src/runtime.rs`

After completing Steps 3, 4, and 7, audit the file for any remaining `if let Some(pool)` branches that could silently degrade behavior. Any branch that falls back to something non-durable should either:
1. Be removed (replaced with a hard error if pool is missing)
2. Be an intentional `None`-means-disabled pattern with a clear code comment explaining why silence is correct for this specific case

Document findings and either fix or clearly comment each remaining branch.

---

### Step 11 — Fix README.md Profiles Table and Boot Config Example

**Status:** [ ] Pending  

**File:** `README.md`

The README still references `BRASSCLAW_REBORN_PROFILE` in the env var table and has a `[boot]\nprofile = "local-dev"` config block. These must be updated:

1. Remove or update the `[boot]\nprofile = "local-dev"` config file example
2. In the env var table: replace `BRASSCLAW_REBORN_PROFILE` with `BRASSCLAW_RUNTIME_PROFILE`, update description
3. Remove the "Profiles" section or update it to reflect the current runtime-profile list

---

## Summary Table

| Step | Area | Type | Severity |
|------|------|------|----------|
| 1 | deploy/brassclaw.service, deploy/dietpi-setup.sh | Deployment | High |
| 2 | README.md, docs/ | Documentation | Medium |
| 3 | runtime.rs SubagentGoalStore | Code — Goal 2 | High |
| 4 | runtime.rs OutboundStore | Code — Goal 2 | High |
| 5 | factory.rs SecretStore | Code — Goal 2 | Medium (verify scope first) |
| 6 | webui.rs RecipeStore | Code — Goal 2 | High |
| 7 | runtime.rs RecipeLibrary | Code — Goal 2 | High |
| 8 | retrieval_source.rs RamSource | Code — Goal 2 | Medium (Phase K will delete; gate now) |
| 9 | intent_system.rs DbLessFallback | Code — Goal 2 | Medium |
| 10 | runtime.rs remaining fallbacks | Code — Goal 2 | Medium |
| 11 | README.md profiles section | Documentation | Low |
