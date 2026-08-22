# Subplan — Problem at Phase C: 29 broken `tests/`-tier E2E `build_reborn_runtime` tests (postgres-mandatory fallout)

> Local working spec (gitignored by repo convention: `subplan_*.md` — see
> `.gitignore:92-94`, same as `subplan_problem_stepa5_of_saved_plan_to_v3.md`).
> The durable step record lives in the Zenflow structured plan (substep of
> Phase C, inserted before Phase D). This file documents the approach for a
> big pre-existing problem encountered while verifying Phase C.

## Problem

`cargo test -p brassclaw_reborn_composition --features skills-db --tests
--no-fail-fast` has **29 pre-existing failures** across **5 integration-test
binaries**, all panicking at `build_reborn_runtime(...).{expect,unwrap}(...)`
with:

> `Postgres pool is required for PgSubagentGoalStore (postgres is mandatory; in-memory fallback removed)`

| Binary | Failing tests | Panic site | Helper |
|---|---|---|---|
| `budget_approval_e2e` | 5 | `:93` | `build_runtime_with_pause_inducing_setup` |
| `budget_e2e` | 11 | `:133/:195/:269/:329/:394/:441/:494/:560/:650/:723/:809` | `build_input` (all 11, incl. `projection…` which chains `.with_budget_event_observer`) |
| `runtime` | 4 of 6 | `:75/:129/:181/:266` | inline (2 tests pass — they expect an error *before* the pool check: `disabled` + `local_dev`-without-policy) |
| `trigger_poller_e2e` | 6 | `:154` | `build_runtime_with` |
| `webui_v2_e2e` | 3 | `:229` | `build_harness` |

**Root cause:** commit `0ba4899f` ("postgres is mandatory; in-memory fallback
removed") made `build_reborn_runtime` fail-closed on a missing `services.pg_pool`
for `PgSubagentGoalStore` (`runtime.rs:2345`) and `PgOutboundStateStore`
(`runtime.rs:2366`) when the `postgres` feature is on (default for this crate).
These 29 `tests/`-tier E2E tests build a LocalDev runtime via
`RebornBuildInput::local_dev(...)` with **no pool** (`input.rs:293` `pg_pool()`
returns `None` for `LocalDev`). The refactor removed the fallback but never
migrated these `tests/` binaries — they have been red on `origin/main` since
`0ba4899f`.

**Same root cause as the Phase A.5 subplan** (`subplan_problem_stepa5_…`), which
migrated the 33 *in-crate* `runtime::tests` unit tests to the hybrid path via
`src/runtime/test_pg.rs`. That helper is `pub(crate)` and therefore NOT reachable
from the `tests/` integration tier — so this subplan mirrors it as a shared
`tests/common/mod.rs`.

**Not caused by Phase C:** Phase C is purely additive (V053 migration +
`pg_extension_catalogue_store.rs` + retrieval/validator/class-label arms +
`GenericComponent.extra` + one integration test). The 29 failures are
independent of Phase C; they are the `tests/`-tier half of the same
postgres-mandatory fallout that A.5 fixed for the in-crate tier.

## User decisions (ask_user)

1. **Approach:** Migrate the 29 tests now to the hybrid-path pattern
   (`RebornStorageInput::Postgres` pool + per-test `reborn_home` tempdir) +
   skip-if-no-docker, matching `src/runtime/test_pg.rs` — completes the
   half-written postgres-mandatory migration. They **skip here** (docker not
   running in this environment) and **run in CI** (docker available).

## Verification limitation (important)

Docker is **not running** in this environment (`docker info` fails; the pg
testcontainer tests skip in 0.00s via early-return). Therefore the 29 migrated
tests **cannot be run to completion here** — they will skip cleanly (no panic).
The verification bar here is:

- `cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings`
  (zero warnings) — catches API/path mistakes at compile time.
- `cargo test … --tests --no-fail-fast` **without docker**: the 29 skip cleanly
  (return early before `build_reborn_runtime`); the 2 already-passing
  `runtime.rs` tests still pass; no panic; suite green.
- **CI (with docker)** is the authoritative run that verifies the 29 actually
  pass on the hybrid PG substrate. This mirrors the A.5 bar (A.5 was verified
  with docker because docker was available then; it is not now).

## Implementation plan

### Step 1 — Shared `tests/common/mod.rs` rig

Create `tests/common/mod.rs` (helper "module" compiled into each test binary via
`mod common;` — the standard Rust integration-test helper pattern; cargo does
NOT build `common/mod.rs` as a separate binary, unlike `common.rs`). Mirror
`src/runtime/test_pg.rs` exactly:

- `PgRig { pool: Arc<PgPool>, url: SecretMaterial, db_lock: Mutex<()>, _container: ContainerAsync<Postgres> }`
- `static PG_RIG: OnceCell<Arc<PgRig>>`
- `pub async fn pg_rig() -> Option<Arc<PgRig>>` — lazily start ONE
  Postgres-16-alpine testcontainer, build a pool, `run_migrations` (V000–V053,
  discovered automatically), cache it. Returns `None` (skip) when
  docker/testcontainers unavailable or the connection is refused.
- `impl PgRig { pub async fn lock_db(&self) -> MutexGuard<'_, ()>; pub fn build_input(&self, owner: &str, reborn_home: &Path) -> RebornBuildInput }` —
  `build_input` calls `RebornBuildInput::postgres_with_reborn_home(owner, pool, url, reborn_home)`.

Dependencies (`testcontainers_modules`, `deadpool_postgres`, `brassclaw_pg`,
`brassclaw_secrets`, `brassclaw_reborn_composition::RebornBuildInput`) are
already reachable from the `tests/` tier — proven by
`tests/extension_catalogue_component.rs` (Phase C) which uses the same set.

### Step 2 — `budget_approval_e2e.rs` (5 tests)

- Add `mod common; use common::pg_rig;` + the `PgRig` import.
- `build_runtime_with_pause_inducing_setup` gains a `rig: &PgRig` first param;
  `RebornBuildInput::local_dev(format!("{tag}-owner"), root)` →
  `rig.build_input(&format!("{tag}-owner"), root)`. Rest (policy/identity/poll/
  gateway/cost-table) unchanged.
- Each of the 5 tests: `let Some(rig) = pg_rig().await else { return; };` →
  `let _db_guard = rig.lock_db().await;` before `let root = …`, then pass
  `&rig` to the helper. `_db_guard` held across `build_reborn_runtime` + body.

### Step 3 — `budget_e2e.rs` (11 tests)

- `build_input` gains `rig: &PgRig` first param; line 102
  `RebornBuildInput::local_dev(format!("{tenant}-owner"), owner_root)` →
  `rig.build_input(&format!("{tenant}-owner"), &owner_root)`.
- Each of the 11 tests: skip-guard + `lock_db` before `let root = …`; pass
  `&rig` to `build_input`. `projection…` chains
  `.with_budget_event_observer` after `build_input(&rig, …)` as before.

### Step 4 — `runtime.rs` (4 failing of 6)

- Tests 3, 4 (`stub_gateway_send_cancels…`, `send_user_message_with_cancellation…`):
  no filesystem skills — swap `RebornBuildInput::local_dev(owner, root.path().join("local-dev"))`
  → `rig.build_input(owner, root.path())` + skip-guard + `lock_db`.
- Test 5 (`skill_execution_adapter_prepares_filesystem_bundles…`) writes skill
  files to `storage_root = root.path().join("local-dev")`. On the hybrid path
  the local-dev substrate is `reborn_home.join("db")` (`factory.rs:564`), so
  write to `root.path().join("db")` and call `rig.build_input(owner, root.path())`.
- Test 6 (`build_reborn_runtime_wires_third_party_hooks_when_enabled`) writes
  the extension manifest to `storage_root.join("system/extensions/…")`; same
  `…/local-dev/…` → `…/db/…` adjustment, `rig.build_input(owner, root.path())`.
- Tests 1, 2 (passing) are NOT touched — they assert errors raised before the
  pool check (`disabled`, `local_dev`-without-policy) and stay `local_dev`.

### Step 5 — `trigger_poller_e2e.rs` (6 tests)

- `build_runtime_with` gains `rig: &PgRig` first param; line 140
  `RebornBuildInput::local_dev(USER, root.path().join("local-dev"))` →
  `rig.build_input(USER, root.path())`.
- Each of the 6 tests: skip-guard + `lock_db` before `let root = …`; pass
  `&rig` to `build_runtime_with`. No filesystem writes (trigger repo is seeded
  programmatically via `repo.upsert_trigger`).

### Step 6 — `webui_v2_e2e.rs` (3 tests)

- `build_harness` calls `pg_rig()` + `lock_db()` first; returns `Option<Harness>`
  (None → skip). `RebornBuildInput::local_dev(USER, root.path().join("local-dev"))`
  → `rig.build_input(USER, root.path())`. `_db_guard` stored in `Harness` so it
  outlives `build_reborn_runtime` + the test body (held until `Harness` drops).
- Each of the 3 tests: `let Some(harness) = build_harness().await else { return; };`.

### Step 7 — Verify (no docker here)

- `cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings`
  (zero warnings).
- `cargo test -p brassclaw_reborn_composition --features skills-db --tests
  --no-fail-fast`: 29 skip cleanly (0.00s, no panic); 2 `runtime.rs` tests
  still pass; rest green. Authoritative pass-run is CI (docker).

### Step 8 — Commit + push

Separate commit (mirrors A.5's separate test-fix commit):
`fix(tests): provision Postgres pool for tests/-tier E2E runtime tests (postgres-mandatory fallout)`.
Push to `origin/main`. Mark the Zenflow substep Completed, then resume Phase C
(commit Phase C core + skill_import fix, push, mark Phase C Completed).

## Files touched

- `crates/brassclaw_reborn_composition/tests/common/mod.rs` (NEW — shared rig)
- `crates/brassclaw_reborn_composition/tests/budget_approval_e2e.rs`
- `crates/brassclaw_reborn_composition/tests/budget_e2e.rs`
- `crates/brassclaw_reborn_composition/tests/runtime.rs`
- `crates/brassclaw_reborn_composition/tests/trigger_poller_e2e.rs`
- `crates/brassclaw_reborn_composition/tests/webui_v2_e2e.rs`

## Out of scope (deliberately)

- No production-code changes to `build_reborn_runtime`, `factory.rs`, or
  `input.rs` (the fail-closed mandate stays; the hybrid path already exists).
- No changes to the Phase C component code (V053 / store / validator / retrieval).
- `facade_factory.rs` is NOT touched — its `local_dev` calls go through
  `build_reborn_services` (not `build_reborn_runtime`), which does NOT add the
  pool-requiring `PgSubagentGoalStore`/`PgOutboundStateStore`; those calls do
  not panic. Its `postgres` calls already provide a pool.
