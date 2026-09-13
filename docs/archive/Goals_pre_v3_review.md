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

**Status:** [DONE]

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

**Status:** [DONE]

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

**Status:** [DONE] — Verified safe. The `pg_pool = None` branch in `build_local_dev()` is only reachable from unit tests (comment at line ~592 of factory.rs explicitly states "used in unit tests only"). The filesystem fallback is intentional for test isolation. Added a code comment to document this invariant.

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

**Status:** [DONE]

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

**Status:** [DONE]

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

**Status:** [DONE — Documented, removal deferred to Phase K per plan]

**Findings:** `RamSource` is the **active production retrieval backend** today (wired in `crates/brassclaw_engine/src/runtime/manager.rs`). `PostgresSource` is exported but never instantiated in production. The plan's Phase K explicitly removes `RamSource` and wires `PostgresSource`. This step adds a clear `TODO(Phase K)` comment in `manager.rs` explaining the gap.

---

### Step 9 — Address `DbLessFallback` Intent Resolution Variant

**Status:** [DONE]

**Findings:** `DbLessFallback` was a design relic — `resolve_intent` takes a `&PgPool` directly so the variant could never actually be returned from the `PostgresSource` path. It was matched in the same arm as `NoMatch` at line 679 of `retrieval_source.rs`. Removed the variant from the enum and the dead match arm.

---

### Step 10 — Verify and Document Remaining `if pg_pool.is_some()` Branches in `runtime.rs`

**Status:** [DONE]

Remaining `if let Some(pool)` branches after Steps 3/4/7:
- `interceptor_store`: `None` when pool unavailable — correct (interceptor is optional, no data loss)
- `hooks_pg_pool`: `None` when pool unavailable — correct (hooks are optional, no data loss)
- These are intentional `None`-means-disabled patterns. All production-impacting silent fallbacks have been eliminated.

---

### Step 11 — Fix README.md Profiles Table and Boot Config Example

**Status:** [DONE] — Completed as part of Step 2.

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

---

## Independent Re-Audit — Additional Goal 2 Findings (Steps 12–15)

> **Context:** An independent re-audit of the codebase confirmed Steps 1–11 are genuinely
> implemented in code (not just marked `[DONE]`). **Goal 1 is fully accomplished.**
> **Goal 2 is only partially accomplished.** The first pass missed one literal Goal 2 target
> (a filesystem-based fallback for a postgres-less design) and left two related
> postgres-less paths undocumented. These are recorded here as Steps 12–15.
>
> **Key clarification discovered during re-audit:** In production `RamSource` is constructed
> with the engine `Store` that is backed by `PgMemoryDocStore` (`pg_memory_doc_store.rs:176`
> `impl Store for PgMemoryDocStore`, wired in `factory.rs`/`runtime.rs`). `PgMemoryDocStore` is
> **postgres-backed**. Therefore `RamSource` in production is **keyword-retrieval OVER postgres**
> — it is *not* a postgres-less backend. The only genuinely "filesystem-based fallback for a
> postgres-less design" is the optional `BRASSCLAW_FALLBACK_CONTENT_FILE` JSONL file loaded by
> `RamSource` for "fully offline / DB-less deployments" (`retrieval_source.rs:148,164-192`).
> This is exactly what Goal 2 targets ("All filesystem based fallback solutions for a
> postgres-less design should be annihilated").

### Step 12 — Remove `BRASSCLAW_FALLBACK_CONTENT_FILE` filesystem fallback

**Status:** [DONE — implemented]

**File:** `crates/brassclaw_engine/src/memory/retrieval_source.rs` (and `memory/mod.rs`)

**Problem:** `RamSource` supports a static filesystem fallback-content file
(`BRASSCLAW_FALLBACK_CONTENT_FILE` env var, JSONL format). Its own doc comment states this is
"for fully offline / DB-less deployments." This is a **filesystem-based fallback for a
postgres-less design** — the literal target of Goal 2. The env var is not set in any deploy
file or doc, so it is dormant in practice, but the code path must be annihilated per Goal 2,
not left dormant.

**Fix:** Remove `FALLBACK_CONTENT_FILE_ENV`, `FallbackEntry`, `load_fallback_file`,
`load_fallback_file_from_env`, `RamSource::new_with_fallback`, the `RamSource.fallback_entries`
field, `search_fallback_entries`, the fallback branch in `fetch_for_consumer`, and the
now-dead `doc_type_weight_by_class` (if unused after removal). Update `memory/mod.rs` exports.
Remove the tests that exercise the fallback path. `RamSource` keeps its store-backed keyword
retrieval (which is postgres-backed in production). Run `cargo fmt` +
`cargo clippy -p brassclaw_engine --all-targets -- -D warnings`.

**Implementation (done):** Removed from `retrieval_source.rs`: `FALLBACK_CONTENT_FILE_ENV`,
`FallbackEntry`, `load_fallback_file`, `load_fallback_file_from_env`, `search_fallback_entries`,
`RamSource::new_with_fallback`, the `RamSource.fallback_entries` field, and the fallback branch
in `fetch_for_consumer` (the store-empty branch now returns `Ok(vec![])`). Removed the now-dead
`doc_type_weight_by_class` from `retrieval_dbless.rs` — its only caller was `search_fallback_entries`
and no test covered it, so leaving it would trip `dead_code` under `-D warnings`. Updated
`memory/mod.rs` exports to drop the removed items. Removed the four fallback tests
(`ram_source_falls_back_to_file_when_store_empty`, `ram_source_prefers_live_store_over_fallback`,
`load_fallback_file_parses_jsonl`, `load_fallback_file_returns_empty_for_missing_path`) and the
`fallback_entries()` test helper. Updated doc comments in `retrieval_source.rs` and
`retrieval_dbless.rs` to record that the filesystem DB-less fallback is gone (Postgres mandatory)
and that the remaining keyword helpers are deferred to v3 Phase K together with
`RamSource`/`retrieval_dbless.rs`. `RamSource::new(store)` — the postgres-backed keyword path via
`RetrievalEngine::retrieve_context` — is unchanged and remains the production retrieval backend
until Phase K wires `PostgresSource`. Cross-impact noted for Task 2: `saved_plan_to_v3.md`
instructs adding class-code weight arms (`22 => 0.42`, `23 => 0.38`) to `doc_type_weight_by_class`;
those plan steps are now obsolete because the function is removed — the intent-driven
`PostgresSource` orders by `(class_code ASC, prompt_uid ASC)` and does not use these weights.
This is recorded for the Task 2 plan review.
**Validation:** `cargo fmt -p brassclaw_engine`; `cargo clippy -p brassclaw_engine --all-targets
-- -D warnings` (clean, 0 warnings); `cargo test -p brassclaw_engine --lib memory::` (84 passed,
0 failed). No other crate referenced the removed exports (self-contained to `brassclaw_engine`).

---

### Step 13 — Eliminate the postgres-less e2e/test runtime (`--no-default-features --features libsql`)

**Status:** [OPEN — subplan written: `subplan_step13_of_Goals_pre_v3_review.md`]

**Problem:** The e2e harness, canary scripts, and `Dockerfile.test` build the binary with
`--no-default-features --features libsql`. This turns the `postgres` feature **OFF**, so the
`#[cfg(not(feature = "postgres"))]` runtime blocks (27 in `runtime.rs`, across 8 files) compile
and run with **in-memory stores** — a postgres-less runtime. This contradicts "Postgres is
mandatory" / AGENTS.md ("In-memory backends are acceptable for unit tests only"; e2e are
integration tests). Note: `libsql` is now just a backward-compat alias for `migrate-from-libsql`
(a one-way migration read path, not a storage backend), so the e2e binary has no real storage
backend in this build — it runs entirely in-memory.

**Why deferred (not blindly executed):** The clean fix is to rework the e2e/canary/`Dockerfile.test`
build to use the `postgres` feature (embedded PG / testcontainers) and then delete the
`#[cfg(not(feature = "postgres"))]` in-memory runtime blocks. This is a large test-infra change
that cannot be safely executed or validated without running the full e2e suite, which is out of
scope for this review pass.

**Action — file-by-file rework (for a future task with e2e execution capability):**

A. Switch build commands from `--no-default-features --features libsql` to `--features postgres`
   and set `DATABASE_BACKEND=postgres` (embedded PG / testcontainers are already dev-deps):

   | File | Line(s) | Current | Target |
   |------|---------|---------|--------|
   | `Dockerfile.test` | 32 | `cargo build --release --no-default-features --features libsql --bin brassclaw` | `cargo build --release --features postgres --bin brassclaw` |
   | `Dockerfile.test` | 54 | `DATABASE_BACKEND=libsql` | `DATABASE_BACKEND=postgres` |
   | `tests/e2e/conftest.py` | ~323 | `["cargo","build","--no-default-features","--features","libsql"]` | `["cargo","build","--features","postgres"]` |
   | `tests/e2e/CLAUDE.md` | 78 | doc `--no-default-features --features libsql` | `--features postgres` |
   | `tests/e2e/README.md` | 24 | `--no-default-features --features libsql` | `--features postgres` |
   | `scripts/live_canary/common.py` | 91 | `--no-default-features --features libsql` | `--features postgres` |
   | `scripts/live-canary/upgrade-canary.sh` | 51,58 | `--no-default-features --features libsql` | `--features postgres` |
   | `scripts/auth_canary/README.md` | 119 | `--no-default-features --features libsql` | `--features postgres` |
   | `scripts/replay-snap.sh` | 51 | `--no-default-features ...` | `--features postgres` |
   | `docs/reborn/harness/e2e.md` | 133 | `--no-default-features --features libsql` | `--features postgres` |

   (Historical `docs/plans/2026-02-24-*.md` references are historical records — leave unchanged.)

B. Ensure the e2e harness boots embedded Postgres when `DATABASE_BACKEND=postgres` (mirror
   `deploy/brassclaw.service`'s embedded-PG usage; `BRASSCLAW_EMBEDDED_PG_PORT` is configurable).

C. Delete the now-dead `#[cfg(not(feature = "postgres"))]` blocks (keep each paired
   `#[cfg(feature = "postgres")]` body, dropping the cfg attribute): `runtime.rs` (~27, cluster
   at 1800–1900), `factory.rs` (824, 953), `brassclaw_reborn_event_store/src/lib.rs`,
   `brassclaw_reborn_event_store/tests/profile_contract.rs`,
   `brassclaw_reborn_cli/src/commands/secrets.rs`, `brassclaw_reborn_cli/src/commands/serve.rs`.

D. (Optional, defer if risky) Make `postgres` non-optional in root `Cargo.toml` — strongest
   enforcement of "Postgres is mandatory." Verify the standalone `migrate-from-libsql` tool
   still builds first.

E. After the upgrade cycle, remove the `libsql`/`migrate-from-libsql` features (Step 15).

**Validation:** `cargo build --features postgres` + `cargo clippy --all ... -- -D warnings` +
`cargo test -p brassclaw_reborn_composition` + e2e/canary green with `DATABASE_BACKEND=postgres`.

A local working copy of this subplan exists at `subplan_step13_of_Goals_pre_v3_review.md`
(gitignored by project convention — `subplan_*.md`; the actionable detail above is committed
here so future tasks can use it).

---

### Step 14 — `RamSource` / `retrieval_dbless.rs` are NOT postgres-less in production (deferred to v3 Phase K)

**Status:** [DONE — documented; no pre-v3 code change required]

**Finding:** `RamSource` (`retrieval_source.rs`) and `retrieval_dbless.rs` are the legacy
keyword-retrieval path. In production they wrap `PgMemoryDocStore` (postgres-backed), so they
are keyword-retrieval **over postgres** — not a postgres-less design. They do NOT use the
intent system (`resolve_intent` / `PostgresSource`); that intent-driven path is the v3 work.
`PostgresSource` is fully implemented (`#[cfg(feature = "skills-db")]`, UNION ALL query) but is
**not wired** in production (`manager.rs:383` wires `RamSource`, with the `TODO(Phase K)` added
in Step 8).

**Why no pre-v3 code change:** Replacing `RamSource` with `PostgresSource` requires wiring
`PostgresSource` into the composition/manager path, which is v3 **Phase K** work and depends on
earlier v3 phases. Removing `RamSource` before `PostgresSource` is wired would break the
production retrieval path. The correct, "adjacent-to-final-state" resolution is therefore the v3
plan's Phase K (wire `PostgresSource`, then remove `RamSource` + `retrieval_dbless.rs`).
**Ordering constraint:** Phase K must come AFTER the `PostgresSource` wiring sub-task; otherwise
the production retrieval path breaks.

---

### Step 15 — Transitional `migrate-from-libsql` / `libsql` feature (scheduled removal)

**Status:** [DONE — documented; removal scheduled for a future release]

**Finding:** Root `Cargo.toml` defines `migrate-from-libsql` (gates the libSQL read path used by
the data-migration module to migrate old `reborn-local-dev.db` data INTO postgres) and `libsql`
(a backward-compat alias). The comment states this is "Removed in the release after the upgrade
cycle completes." This is a **one-way migration read path, not a storage backend** — it does not
violate "Postgres is mandatory" (it migrates data INTO postgres). It is intentionally
transitional.

**Action:** Track for removal in the release after the upgrade cycle. No pre-v3 change. (When
Step 13's subplan removes the `--no-default-features --features libsql` build, the `libsql` alias
becomes unused and can be removed together with `migrate-from-libsql`.)

---

## Updated Summary Table

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
| 12 | retrieval_source.rs BRASSCLAW_FALLBACK_CONTENT_FILE filesystem fallback | Code — Goal 2 | High — **DONE** |
| 13 | e2e/canary/Dockerfile.test `--no-default-features --features libsql` postgres-less build + `#[cfg(not(feature = "postgres"))]` blocks | Code/Infra — Goal 2 | High (subplan, deferred) |
| 14 | RamSource/retrieval_dbless = keyword-over-postgres (not postgres-less); Phase K wiring | Code — v3 Phase K | Medium (documented; no pre-v3 change) |
| 15 | transitional `migrate-from-libsql`/`libsql` feature | Code — transitional | Low (scheduled removal) |

### Goal Accomplishment Verdict

- **Goal 1 (no installation profiles):** **FULLY ACCOMPLISHED.** `RebornCompositionProfile`
  removed; `BRASSCLAW_REBORN_PROFILE` is a hard startup error; deploy + docs use
  `BRASSCLAW_RUNTIME_PROFILE` (the per-invocation capability policy, which is intentionally
  retained and is NOT an installation profile).
- **Goal 2 (no postgres-less design, postgres mandatory):** **PARTIALLY ACCOMPLISHED
  (Steps 3,4,6,7,9,12 done).** All silent in-memory production fallbacks removed
  (Steps 3,4,6,7,9); the filesystem fallback annihilated (Step 12, implemented + validated);
  production retrieval is postgres-backed (`PgMemoryDocStore`). Remaining: the e2e/test
  postgres-less build (Step 13, subplan, deferred — requires e2e execution) and the legacy
  keyword-vs-intent retrieval swap (Step 14, v3 Phase K). The transitional migration feature
  (Step 15) is intentional and scheduled for removal.
