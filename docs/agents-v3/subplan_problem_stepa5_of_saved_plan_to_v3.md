# Subplan — Problem at Phase A.5: 33 broken `runtime::tests` (postgres-mandatory fallout)

> Local working spec (gitignored by repo convention: `subplan_*.md`). The durable
> step record lives in the Zenflow structured plan (substep of Phase A.5). This
> file documents the approach for a complex pre-existing problem encountered
> while verifying Phase A.5.

## Problem

`cargo test -p brassclaw_reborn_composition --lib` has **33 pre-existing
failures** in `runtime::tests::*`, `runtime::auth_interaction_tests::*`, and
`runtime::default_system_prompt_tests::*`, all panicking at
`build_reborn_runtime(...).expect("runtime builds")` with:

> `Postgres pool is required for PgSubagentGoalStore (postgres is mandatory; in-memory fallback removed)`

**Root cause:** commit `0ba4899f` ("fix(postgres-mandatory): remove
in-memory/filesystem fallbacks") made `build_reborn_runtime` fail-closed on a
missing `services.pg_pool` for `PgSubagentGoalStore` (`runtime.rs:2344`) and
`PgOutboundStateStore` (`runtime.rs:2365`). The 33 unit tests build a LocalDev
runtime via `RebornBuildInput::local_dev(...)` with **no pool** (`factory.rs:518`
`pg_pool: None`; `factory.rs:805` comments *"The `pg_pool = None` branch is only
reachable from unit tests"*). The refactor removed the fallback but never updated
these tests — they have been red on `origin/main` since `0ba4899f`.

**Not caused by Phase A.5:** Phase A.5 is purely additive (2 `pub mod` lines in
`lib.rs` + 2 new files that never touch `runtime.rs`). Confirmed: the 19 Phase
A.5 tests pass; the 33 failures are independent.

## User decisions (ask_user)

1. **Approach:** Shared testcontainer pool (`OnceCell`) injected into services +
   skip-if-no-docker (follows the existing `tests/postgres_substrate.rs`
   convention). Docker IS available in this environment.
2. **Injection semantics:** Full hybrid — switch the 33 tests to
   `RebornStorageInput::Postgres` (pool + `reborn_home` tempdir) so they run on
   the production hybrid PG path. Most faithful to "postgres mandatory"; may need
   assertion fixes across the 33 tests.
3. **Commit ordering:** Commit Phase A.5 core first (done: `2be678e1`), then the
   33-test fix as a separate substep/commit.

## Implementation plan

### Step 1 — Shared testcontainer pool helper (in `runtime.rs` test module)

Add a `static PG_RIG: tokio::sync::OnceCell<Arc<PgRig>>` + helper that:
- Lazily starts ONE Postgres-16 testcontainer, builds a pool, runs full
  migrations (V000–V051), caches it.
- Returns `None` (skip the test) when docker/testcontainers unavailable — so a
  docker-less `cargo test --lib` still passes cleanly.
- Serializes the 33 tests on the shared DB via a `tokio::sync::Mutex<()>` guard
  (avoids any scope-level collisions from parallel test threads sharing one DB).
  The guard is held for the test body and released on drop.

`PgRig { pool: Arc<PgPool>, url: SecretMaterial, _container: ContainerAsync<Postgres>, _guard: MutexGuard }`.

### Step 2 — Prototype on one simple test

Migrate `send_user_message_uses_caller_supplied_skill_context_source` (injects a
`FailingSkillContextSource`, no filesystem skills) to:
`RebornBuildInput::postgres_with_reborn_home(owner, pool, url, None, reborn_home)`
+ the same `.with_runtime_policy/.with_identity/.with_poll_settings/.with_skill_context_source/.with_model_gateway_override`.
Verify it passes with docker. This validates the hybrid path works for the
inject-source tests before scaling.

### Step 3 — Migrate the inject-source / gateway / non-filesystem tests

Swap `local_dev(owner, root.path().join("local-dev"))` →
`postgres_with_reborn_home(owner, pool, url, None, reborn_home.path().to_path_buf())`
for all tests that inject a skill context source or only need a gateway. Reuse
`root` tempdir as `reborn_home` where no filesystem skills are involved.

### Step 4 — Migrate the filesystem-skill tests (path adjustments)

For tests that write skill/workspace files to the LocalDev root: under Postgres
storage the local-dev root is `reborn_home.join("db")` (`factory.rs:564`), so
filesystem setup paths change from `…/local-dev/…` → `reborn_home.join("db")/…`.
Tests affected (at least): `..._wires_filesystem_skills_by_default…`,
`..._skips_invalid_filesystem_skill…`, `..._activates_setup_skill_when_workspace_marker_is_absent`,
`..._suppresses_explicit_setup_skill_when_workspace_marker_exists`,
`..._maps_workspace_to_configured_root`, `..._records_selectable_filesystem_skill_context`,
`..._prefers_configured_skill_context_source_over_filesystem_default`,
`execute_skill_message_returns_plan_and_reads_active_bundle_assets`,
`..._uses_local_lifecycle_facade_for_setup_extension`.

### Step 5 — Fix any broken assertions

Run the full suite; fix assertions that assumed in-memory stores (if any). Keep
changes minimal and behavior-preserving — the tests should still assert the same
outcomes (gateway calls, skill selection, readiness, audit events), now over the
hybrid PG substrate.

### Step 6 — Verify

- `cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings`
  (zero warnings).
- `cargo test -p brassclaw_reborn_composition --lib` with docker: all pass
  (578 prior + 33 fixed + 19 Phase A.5).
- Without docker: the 33 (+ Phase A.5 DB) tests skip cleanly; no failures.

### Step 7 — Commit + push

One commit: `fix(tests): provision shared Postgres pool for runtime unit tests (postgres-mandatory fallout)`.
Push to `origin/main`. Then mark the Zenflow substep Completed, then the Phase
A.5 step Completed.

## Files touched

- `crates/brassclaw_reborn_composition/src/runtime.rs` (test module: helper + 33
  test migrations; no production code changes).

## Out of scope (deliberately)

- No production-code changes to `build_reborn_runtime` or `factory.rs` (the
  fail-closed mandate stays).
- No changes to the Phase A.5 validation-queue code (already committed).
