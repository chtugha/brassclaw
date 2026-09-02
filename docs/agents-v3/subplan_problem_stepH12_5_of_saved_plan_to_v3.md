# Subplan — Problem Step H.12.5 of `saved_plan_to_v3.md` (nested under H.12)

## Origin / why this subplan exists

H.12.5 (parent subplan `./subplan_problem_stepH12_of_saved_plan_to_v3.md`, section
H.12.5) wires the production `OrchestratorLookup` impl (`PgOrchestratorLookup`)
+ a Thread loader + the host wiring. Grounding surfaced a blocking fact: the
engine `Store::load_thread` has **no production impl** — the only composition
`impl Store` candidates (`PgMemoryDocStore`, `MemoryDocLibSqlStore`) stub
`load_thread` with `Err(stub("load_thread"))`, and every other `impl Store` is a
test mock. So `PgOrchestratorLookup::run_tier_zero` / `run_step_zero` — which
need an engine `Thread` (`thread_execution_context` reads `id`/`thread_type`/
`project_id`/`user_id`/`metadata`, and `build_context_inputs` pushes `thread.goal`
into the Python sandbox) — have no way to load one.

### User decision (locked 2026-09-02): Q-H12-5-THREAD = C, Q-H12-5-STORE = C2

- **Q-H12-5-THREAD = C** — build a **real PG-backed engine `Store::load_thread`**
  (rejected A: thin scope-only loader with empty goal; rejected B:
  `SessionThreadService`-backed composition `ThreadLoader` trait). Rationale: the
  final architecture state wants a real engine `Store` abstraction, not a shim;
  the loop **does** persist threads in Postgres (`brassclaw_session_threads`), so
  full-fidelity loading (real `goal`, `id`, `tenant`, `agent`, `project`,
  `metadata`) is achievable.
- **Q-H12-5-STORE = C2** — the new `impl Store` wraps
  `brassclaw_threads::SessionThreadService::read_thread` (rejected C1: direct
  `SELECT metadata FROM brassclaw_session_threads …` + re-parse `ThreadSnapshot`,
  which would duplicate `PgSessionThreadService`'s private snapshot parsing and
  cross `brassclaw_threads`' table-ownership boundary). C2 reuses the existing
  snapshot parsing through the `SessionThreadService` API and respects crate
  ownership; the cost is coupling the engine `Store` impl to `brassclaw_threads`
  (acceptable — composition already depends on `brassclaw_threads`).

## Grounded facts (2026-09-02)

- **Loop thread persistence:** `brassclaw_threads::SessionThreadService` (PG impl
  `PgSessionThreadService`, `crates/brassclaw_threads/src/pg_service.rs`) stores
  threads in table `brassclaw_session_threads` (cols `id, tenant_id, user_id,
  agent_id, project_id, created_by_actor_id, title, metadata, version,
  deleted_at`); the `metadata` column holds a serialized `ThreadSnapshot`
  (`{ record: Option<SessionThreadRecord>, messages, … }`). Every query is
  tenant-scoped (`WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL`).
- **Load-by-id method:** `SessionThreadService::read_thread(request:
  ThreadHistoryRequest) -> Result<SessionThreadRecord, SessionThreadError>`
  (`crates/brassclaw_threads/src/service.rs`) delegates to `list_thread_history`
  and returns `history.thread` (the full `SessionThreadRecord`). Its query uses
  **only `id` + `tenant_id`** (`pg_service.rs:873`).
- **`ThreadHistoryRequest`** = `{ scope: ThreadScope, thread_id: ThreadId }`
  (`contract.rs`). **`ThreadScope`** = `{ tenant_id: TenantId, agent_id: AgentId,
  project_id: Option<ProjectId>, owner_user_id: Option<UserId> }` (`contract.rs`).
  `agent_id` is required (not `Option`); the query ignores it, so a `"default"`
  sentinel is fine.
- **`SessionThreadRecord`** = `{ scope: ThreadScope, thread_id: ThreadId,
  created_by_actor_id: String, title: Option<String>, metadata_json:
  Option<String>, goal: Option<ThreadGoal> }` (`contract.rs:96`).
  **`ThreadGoal`** = `{ statement: GoalStatement, refined_at_sequence: u64,
  refinement_count: u32 }` (`contract.rs:182`); **`GoalStatement(String)`** with
  `as_str()` (`contract.rs:145,162`).
- **Id types:** `brassclaw_host_api::ThreadId` is a `string_id!` (`pub struct
  ThreadId(String)` with `as_str()` / `from_trusted`). Loop thread ids are
  generated as `ThreadId::new(uuid::Uuid::new_v4().to_string())`
  (`filesystem_service.rs:1792`, `pg_service.rs:224`) → the stored `id` is a raw
  Uuid string. The engine `ThreadId(pub Uuid)` (`types/thread.rs:22`). So the
  host_api→engine mapping is **forced**: `brassclaw_engine::ThreadId(Uuid::parse_str(host_api_id.as_str())?)`. `LoopRunContext.thread_id`
  (`brassclaw_turns/src/run_profile/host.rs:540`) and
  `SessionThreadRecord.thread_id` are **the same `brassclaw_host_api::ThreadId`**
  type — no turns-vs-threads mismatch. All `string_id!` ids (`TenantId`/`AgentId`/
  `ProjectId`/`UserId`) have `as_str()` + `from_trusted`.
- **Engine `ProjectId(pub Uuid)`** (`types/project.rs`); map from
  `Option<host_api::ProjectId>` → `ProjectId(Uuid::parse_str(pid.as_str())?)` or
  `ProjectId::default()` on `None`/parse-fail.
- **Engine `Store` trait** (`traits/store.rs`): `#[async_trait]`, 16 **required**
  methods (no default): `save_thread`, `load_thread`, `list_threads`,
  `update_thread_state`, `save_step`, `load_steps`, `append_events`,
  `load_events`, `save_project`, `load_project`, `save_memory_doc`,
  `load_memory_doc`, `list_memory_docs`, `save_lease`, `load_active_leases`,
  `revoke_lease`. The rest have default impls (stub-`Err`). `PgMemoryDocStore`
  (`pg_memory_doc_store.rs`) is the mirror pattern for the stub helper.
- **Tier-0 `thread` field reads** (what the loaded `Thread` must supply):
  `thread_execution_context` reads `id`, `thread_type`, `project_id`, `user_id`,
  `metadata` (optional keys), `goal`; `execute_code_with_skills_inner` reads
  `thread.id` (leases + tracing) + `build_context_inputs(thread, ..)` reads
  `thread.goal` (pushed as a Monty `String` input) + `thread.step_count` (Monty
  `Int`). `assemble_prior_knowledge_with_hint` **Some-branch** (Tier-1
  `run_step_zero`) does NOT read `thread` — only the None-branch fresh-fetch does
  (never hit: `run_step_zero` always passes `recipe_hint = Some(..)`).

## Target architecture (C2)

New composition module `crates/brassclaw_reborn_composition/src/pg_thread_engine_store.rs`
(`#[cfg(feature = "skills-db")]`, mirroring `PgRetrievalLookup`):

```text
pub(crate) struct PgThreadEngineStore {
    thread_service: Arc<dyn brassclaw_threads::SessionThreadService>,
    tenant_id: String,
}
impl PgThreadEngineStore {
    pub(crate) fn new(thread_service, tenant_id) -> Self { .. }
    fn map_record(record: &SessionThreadRecord) -> Thread { .. }   // pure, testable
}
#[async_trait] impl brassclaw_engine::Store for PgThreadEngineStore {
    async fn load_thread(&self, id: EngineThreadId) -> Result<Option<Thread>, EngineError> {
        // 1. engine ThreadId(Uuid) -> host_api ThreadId (from_trusted(uuid.to_string()))
        // 2. ThreadHistoryRequest { scope: ThreadScope{ tenant_id, agent_id: default,
        //    project_id: None, owner_user_id: None }, thread_id }
        // 3. self.thread_service.read_thread(req)
        //      .await -> SessionThreadRecord  (map SessionThreadError -> EngineError)
        //      on "missing" shape -> Ok(None)
        // 4. Ok(Some(Self::map_record(&record)))
    }
    // 15 other required methods -> Err(EngineError::Store { reason })  (stub, mirror PgMemoryDocStore)
}
```

`map_record` mapping: `id` ← `Uuid::parse_str(record.thread_id.as_str())` (fallback
`ThreadId::new()`); `tenant_id` ← `record.scope.tenant_id.as_str()`; `agent_id` ←
`record.scope.agent_id.as_str()`; `project_id` ← `record.scope.project_id` (Uuid
parse or `ProjectId::default()`); `user_id` ←
`record.scope.owner_user_id.as_str()` EXISTS `record.created_by_actor_id` EXISTS
`SYSTEM_RESERVED_ID`; `goal` ← `record.goal.as_ref().map(|g|
g.statement.as_str().to_string()).unwrap_or_default()`; `title` ← `record.title`;
`metadata` ← `serde_json::from_str(record.metadata_json).unwrap_or(Value::Null)`;
`thread_type` ← `ThreadType::Foreground`; `state` ← `ThreadState::Created`;
`config` ← `ThreadConfig::default()`; messages/events/steps/leases empty;
timestamps `Utc::now`; counters 0.

This is the **sole** piece H.12.5 main needs that was absent. After this subplan,
H.12.5 main (resume) builds `PgOrchestratorLookup` holding `Arc<dyn
brassclaw_engine::Store>` (this type) + the facade + the effects builder, and
wires `Some(..)` into `DefaultPlannedRuntimeParts.orchestrator_lookup`.

## Steps (one-by-one, commit+push each)

`CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` on every build/test/clippy/check.
`df -h /Users/ollama/brassclaw-target` first — `cargo clean` if Avail<15GB or
>90%. Selective-pathspec commit guard — **never** stage user WIP (the parallel
`system_bundle_source` §K.1.5 + sibling edits); stage only my exact files.

### H.12.5.1 — New `pg_thread_engine_store.rs` (real `Store::load_thread` via `read_thread`)

New module `crates/brassclaw_reborn_composition/src/pg_thread_engine_store.rs`:
`#[cfg(feature = "skills-db")] pub(crate) struct PgThreadEngineStore` + `new` +
`map_record` (pure) + `#[async_trait] impl brassclaw_engine::Store` (real
`load_thread` + 15 stubs mirroring `PgMemoryDocStore`'s `stub()` pattern). Declare
`#[cfg(feature = "skills-db")] mod pg_thread_engine_store;` in `lib.rs`. Ground
during impl: `SessionThreadError` "missing" shape (to return `Ok(None)`),
`EngineError` variants for error mapping, `SYSTEM_RESERVED_ID` import
(`brassclaw_host_api::SYSTEM_RESERVED_ID`), `ThreadType`/`ThreadState`/
`ThreadConfig` import paths. Verify: `cargo fmt`; `cargo clippy -p
brassclaw_reborn_composition --all-targets --features skills-db -- -D warnings`
(+ default); `cargo build -p brassclaw_reborn_composition --features skills-db`.
Commit `H.12.5.1: PgThreadEngineStore (real engine Store::load_thread via SessionThreadService::read_thread, option C2)`.

### H.12.5.2 — Tests for `PgThreadEngineStore` (test through the caller)

Prefer testing `load_thread` **through the real caller** with a real in-memory
`SessionThreadService` backend: ground `brassclaw_threads::in_memory` (or an
existing test `SessionThreadService` impl) — if pub + constructible + populates a
thread with goal/title/metadata, use it; `ensure_thread(..)` a thread, then
`load_thread(engine_id)` and assert the mapped `Thread` (id, tenant, agent,
project, user, goal, title, metadata). Also a not-found case → `Ok(None)` (or the
backend's missing shape). If no reusable in-memory backend exists, fall back to a
minimal mock `SessionThreadService` (stub all ~20 methods; capture the request;
return a canned `SessionThreadRecord`) — and additionally unit-test the pure
`map_record` helper directly (covers the mapping logic without the trait mock).
Verify: `cargo test -p brassclaw_reborn_composition --features skills-db` (+
default). Commit `H.12.5.2: tests for PgThreadEngineStore (load_thread through caller + map_record)`.

### H.12.5.3 — Verify + mark subplan DONE

fmt + clippy both configs + tests both configs; confirm no dead-code warnings
(`PgThreadEngineStore` is consumed by H.12.5 main next; if not yet referenced,
`#[allow(dead_code)]` with a doc note "wired into PgOrchestratorLookup in H.12.5
main" is acceptable interim — mirror H.12.2.5's pattern). Mark H.12.5.1/.2/.3 DONE
in this doc. Commit `H.12.5.3: verify PgThreadEngineStore (fmt+clippy+tests both configs)`.
Then resume H.12.5 main in the parent subplan doc.

## Needs

H.12.4 (DONE — `orchestrator_lookup` slot + facade + builder + guard).

## Touches

- NEW `crates/brassclaw_reborn_composition/src/pg_thread_engine_store.rs`
- `crates/brassclaw_reborn_composition/src/lib.rs` (`mod pg_thread_engine_store;`)

## Result

A real production `impl brassclaw_engine::Store` that `load_thread`s from
`brassclaw_session_threads` via `SessionThreadService::read_thread`, with
full-fidelity `goal`/`id`/`tenant`/`agent`/`project`/`metadata`. H.12.5 main
wraps it in `PgOrchestratorLookup` and wires the host.

## Completion log

- **H.12.5.1 — DONE.** New `crates/brassclaw_reborn_composition/src/pg_thread_engine_store.rs`
  (`pub(crate) struct PgThreadEngineStore { thread_service, tenant_id }` + `new` + pure
  `map_record(&SessionThreadRecord) -> Thread` + `#[async_trait] impl Store` with real
  `load_thread` (engine `ThreadId(Uuid)` → host_api `ThreadId::from_trusted` →
  `ThreadHistoryRequest{scope: ThreadScope{tenant_id, agent_id:"default", project_id:None,
  owner_user_id:None}, thread_id}` → `read_thread` → `map_record`; `UnknownThread` →
  `Ok(None)`, other errors → `EngineError::Store`) + 15 stubs mirroring `PgMemoryDocStore`'s
  `stub()`). Declared `pub(crate) mod pg_thread_engine_store;` in `lib.rs` (ungated, mirroring
  `orchestrator_effect_executor.rs`; `#![allow(dead_code)]` covers the unused-until-H.12.5-main
  window in both configs). Implementation note (mechanical, not a design change): the subplan
  text showed `#[cfg(feature = "skills-db")]` on the struct; the shipped form is feature-agnostic
  (ungated) + module-wide `#![allow(dead_code)]`, matching the already-shipped H.12.2 adapter —
  the type touches only always-available `brassclaw_engine` core types + `brassclaw_threads`
  contracts, so it compiles identically under the default and `skills-db` feature sets.
- **H.12.5.2 — DONE.** Tests live in the module's `#[cfg(test)] mod tests` (idiomatic
  co-location): `map_record_carries_full_fidelity_fields` + `map_record_falls_back_for_missing_identity`
  (pure `map_record` unit tests) + `load_thread_round_trips_through_in_memory_service`
  (test **through the caller** with the real `brassclaw_threads::InMemorySessionThreadService`
  backend — `ensure_thread` then `load_thread(engine_id)` asserts id/tenant/agent/user/title/
  metadata) + `load_thread_missing_returns_none` + `load_thread_cross_tenant_returns_none`
  (degrade-to-`None`) + `non_load_thread_methods_stub` (15 stubs error). The in-memory backend
  enforces exact-scope ownership on `read_thread` (production PG keys on id+tenant_id only), so
  the round-trip test ensures the thread under the same scope shape `load_thread` issues
  (tenant + "default" agent, no project/owner); the creating actor becomes the `user_id`
  fallback — documented in the test.
- **H.12.5.3 — DONE.** Verification: `cargo fmt -p brassclaw_reborn_composition` clean;
  `cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings` clean (default);
  `cargo clippy -p brassclaw_reborn_composition --all-targets --features skills-db -- -D warnings`
  clean; `cargo build -p brassclaw_reborn_composition --features skills-db` clean;
  `cargo test -p brassclaw_reborn_composition --lib pg_thread_engine_store` → 6 passed /
  0 failed (both default + `--features skills-db`). Disk guard: target dir was at 93%/14Gi;
  scoped `cargo clean -p brassclaw_reborn_composition` (freed 8.2GiB → 20Gi/90%) before
  compiling per the CLAUDE.md permanent rule.

Nested subplan H.12.5.1–H.12.5.3 complete. Resume H.12.5 main in the parent subplan
(`./subplan_problem_stepH12_of_saved_plan_to_v3.md`, section H.12.5): build `PgOrchestratorLookup`
holding `Arc<dyn brassclaw_engine::Store>` (this `PgThreadEngineStore`) + the `TierZeroOrchestrator`
facade + the `TierZeroEffectExecutorBuilder`, widen `TierZeroEffectExecutorBuilder` /
`build_for_run` `pub(super)` → `pub(crate)`, and wire `Some(..)` into
`DefaultPlannedRuntimeParts.orchestrator_lookup`.
