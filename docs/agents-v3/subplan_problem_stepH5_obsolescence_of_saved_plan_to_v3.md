# Subplan — H.5 obsolescence reconciliation (engine Python turn runtime + missions are dormant)

Parent: `subplan_problem_stepH_of_saved_plan_to_v3.md` (Phase H, anchored at H.5).
Parent plan: `saved_plan_to_v3.md` (Recipe System Finalisation Plan — v3), Phase H item
H.5 (`saved_plan_to_v3.md` lines ~5484–5511).
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **substep** under the Zenflow Phase H step `9d94d6cb-3a45-47cc-8e0f-85203c936652`
(after the H4.8 substep `d48d5809`).

This subplan was opened because grounding H.5 (the Model A composition integration test)
overturned the Phase H subplan §1 claim that Model A *"drives real turns today."* The
obsolete surface is **the entire engine Python turn-execution runtime** + the **mission
system** (v1 routines reborn), not just Model A's `tier_zero` branch. The user approved a
**Phased** remediation: delete the dead code up-front where safe, build the real target
(H.6–H.13 Model B/C), then delete the now-severed engine Python runtime wrapper in a final
cleanup after H.8 extracts the reusable logic.

---

## 1. Why this subplan exists — the obsolescence discovery

The Phase H subplan (`subplan_problem_stepH_of_saved_plan_to_v3.md` §1) framed Phase H as
activating Tier-0/Tier-1 across **two runtimes**, with **Model A** (the engine
`ExecutionLoop` / Monty `default.py` path) described as *"what drives real turns today."*
Grounding H.5 against the live source proved that framing **stale/inaccurate**:

- **Production turns run on the agent loop**, not the engine Python path.
  `DefaultTurnCoordinator::submit_turn` (`crates/brassclaw_turns/src/coordinator.rs:256`)
  → `TurnStateStore::submit_turn` → an `AgentLoopDriver`. The agent loop never touches the
  engine Python path — **zero** references to `execute_orchestrator` / `default.py` /
  `run_loop` / `ThreadManager` / `Monty` in `crates/brassclaw_agent_loop/src`.
- **The engine `ExecutionLoop` / `execute_orchestrator` is only reachable via
  `ThreadManager::spawn_thread`**, whose only non-test caller is the **mission system**
  (`crates/brassclaw_engine/src/runtime/mission.rs`), itself only test-constructed.
  All 8 `ThreadManager::new` sites are test helpers (`mission.rs:3916/4250`,
  `conversation.rs:856/925/1041`, `manager.rs:1163/1191/1223`).
- **Composition never instantiates `ThreadManager`** (zero hits); it drives turns via
  `submit_turn` (`crates/brassclaw_reborn_composition/src/runtime.rs:1043-1045`).
  Composition's only engine-orchestrator touch is the reduction-rules cache +
  Monty-VM *settings* persistence — not turn execution.
- `product_live_adapters.rs:1-3` states it *"does not cut app or gateway traffic over to
  Reborn."* The H4.7 comment at `recipe_outcome_listener.rs:17-19` already recorded that
  *"the engine `ThreadManager`… `execute_orchestrator` is only called from `loop_engine.rs`"*
  and that the live path is the agent-loop stack.

**Conclusion:** a large fraction of Phases A–G landed in the dormant engine Python
turn-execution path. The reusable pieces (retrieval, intent, IBS, recipe store, the H.4
event/outcome bridge) are reused by Model B/C; only the *wrapper* (`execute_orchestrator`
turn loop, `default.py`, `ExecutionLoop`/`loop_engine`, `ThreadManager`, Monty, missions)
is truly dead.

### 1.1 Missions = v1 routines reborn (obsolete)

`Mission` (`crates/brassclaw_engine/src/types/mission.rs:135`) is a persistent
self-driving background agent: `goal`, `cadence` (`Cron`/`OnEvent`/`OnSystemEvent`/
`Webhook`/`Manual`), `status`, `current_focus`/`approach_history`, `notify_channels`,
rate controls. `MissionManager` (`crates/brassclaw_engine/src/runtime/mission.rs:196`)
runs lifecycle, spawns threads on `ThreadManager` (`:832`), `join_thread`s (`:2313`),
notifies channels, with `BudgetGate` + per-user rate limiting. The code explicitly maps
from v1 routines (`mission.rs:42` "mirroring the v1 routine engine"; `Mission.description`
"Routine `description` fields map here"). The mission system is the **only** non-test
caller of the dormant engine `ThreadManager`/`ExecutionLoop` path. **User decision:
routines/missions are NOT coming back — obsolete.** Deleting missions severs the last
non-test reachability of the engine Python turn runtime.

### 1.2 Crucial nuance — H.8 reuses the engine's *logic*, so the wrapper cannot be
deleted wholesale until H.8 extracts it

H.8 refactors `handle_assemble_prior_knowledge` (in `orchestrator.rs`) and extracts engine
`pub` fns `assemble_prior_knowledge_with_hint` + `execute_tier_zero_channel`, which the
composition `LoopOrchestratorPort` (H.12) delegates to. So the engine's **assembly +
channel-execution logic is reused by Model B/C** via pub-fn extraction — only the
*wrapper* (`execute_orchestrator` turn loop, `default.py`, `ExecutionLoop`/`loop_engine`,
`ThreadManager`, Monty, missions) is truly dead.

**Shared infrastructure to KEEP (reused by B/C):**
- `PostgresSource` / `RetrievalSource` / `TurnRoutingSignals` / `ComponentItem`
  (`retrieval_source.rs`) — incl. the H4.3 `recipe_id` + `recipe_name` fields.
- intent system (`intent_system.rs`), recipe store + `record_recipe_outcome`.
- IBS (`instruction_builder.rs`, `types/ibs.rs`), `types/recipe.rs`, the DB migrations.
- the reusable H.4 pieces: `RecipeTierZeroStarted`/`Succeeded`/`Failed` `EventKind`
  variants, `TurnRoutingSignals.recipe_id`/`recipe_name`, `RecipeOutcomeListener`.

**Truly dead (delete):** missions; Model A `default.py` step-0 `tier_zero` branch (H.3);
and — **only after H.8** — the engine Python runtime wrapper (`execute_orchestrator` turn
loop, `loop_engine`/`ExecutionLoop`, `ThreadManager`, Monty, `default.py`) + the Model A
Python fns (H.1 `_parse_orchestrator_channel_steps`, H.2
`execute_recipe_orchestrator_channel`).

---

## 2. Grounding findings (confirmed against current code)

- **Agent loop is the live driver:** `coordinator.rs:256` `submit_turn` → store →
  `AgentLoopDriver`. Agent loop crate has **zero** engine-Python-path references.
- **Engine Python path reachability:** `loop_engine.rs:471` `execute_orchestrator` is the
  sole non-`run_python_step0`-harness call site, inside `ExecutionLoop::run`, reachable
  only via `ThreadManager::spawn_thread`. `ThreadManager` is built in
  `manager.rs:397-431` with `RamSource` + (skills-db) `pg_pool`; the `TODO(Phase K)`
  comment at `manager.rs:399-403` marks `RamSource` as the active backend pending Phase K.
- **8 `ThreadManager::new` sites are test helpers** (mission.rs:3916/4250,
  conversation.rs:856/925/1041, manager.rs:1163/1191/1223).
- **Composition drives turns via `submit_turn`** (`runtime.rs:1043-1045`); never
  instantiates `ThreadManager`.
- **`product_live_adapters.rs:1-3`** confirms no traffic cut-over to Reborn.
- **`recipe_outcome_listener.rs:17-24`** already records the dormant-runtime note and that
  the listener logic is fully implemented + unit-tested (no stub) — to be wired into the
  live event stream when H.6–H.13 route Tier-0 through the engine pub fns.
- **`PostgresSource::fetch_recipe_split_result`** (`retrieval_source.rs:763`) already
  carries `recipe_id` + `recipe_name` on `TurnRoutingSignals` (H4.3, `:864`/`:952`) and
  boxes the `BuildInstruction` (`:955`). Reusable by B/C unchanged.
- **Mission system footprint** (to be fully grounded + deleted in O2):
  `crates/brassclaw_engine/src/runtime/mission.rs` (~8523 lines) +
  `crates/brassclaw_engine/src/types/mission.rs` (~737 lines) + wiring + tests + any DB
  tables/API refs. O2 grounds the exact dependency set before deletion.

---

## 3. Design decisions (answered by user before this subplan's execution)

1. **Model A obsolete** → skip H.5; jump to H.6 (Model B/C).
2. **Missions/routines obsolete** → delete.
3. **Remediation policy = Hybrid:** delete dead Model A + mission code, but **KEEP +
   re-document as reused** the H.4 pieces reusable by B/C (`RecipeTierZero*` event kinds,
   `TurnRoutingSignals.recipe_id`/`recipe_name`, `RecipeOutcomeListener`).
4. **Sequencing = Phased:** (a) up-front: delete missions + skip H.5 + delete Model A
   Python `tier_zero` (H.3); (b) build H.6–H.13 (H.8 extracts reusable assembly/channel
   logic into engine `pub` fns); (c) final cleanup: delete the now-severed engine Python
   runtime wrapper (`execute_orchestrator` turn loop, `loop_engine`/`ExecutionLoop`,
   `ThreadManager`, Monty, `default.py`) + the Model A Python fns — reaching the
   de-bloated final architecture.
5. **Scope = phased interleave + one targeted up-front removal of the big dead chunks**
   (match the user's *"get to final architecture without wasting energy on obsolete
   stuff"*).

### 3.1 O2 sub-decisions (answered by user after grounding the mission footprint)

Grounding revealed the mission surface is wider than the engine mission system alone: a
**separate `brassclaw_host_api::MissionId`** (a *string* scope/audit tag, distinct from the
engine's UUID `MissionId`) is woven through low-level infra (`host_api` `ids.rs`/
`scope.rs`/`audit.rs`/`resource.rs`, `events/cursor.rs`, `processes/types.rs`,
`secrets/lib.rs` + tests), always `Option`, never populated; and **15 `parent_mission_id
UUID` columns** (V027 skills + 14 component tables) are dead schema (nullable, zero Rust
reads/writes). User decisions:

6. **`brassclaw_host_api::MissionId` → full purge** (remove across host_api/events/
   processes/secrets + all consumers), not just the engine mission system.
7. **`parent_mission_id` columns → drop** in a new migration (15-table schema change),
   not leave-as-dead-schema.
8. **e2e `test_mission_gmail_3133.py` → delete; historical mission plan docs → leave as
   archives; `CLAUDE.md`/`AGENTS.md` mission mentions → update.**

O2 is therefore executed as four sequential sub-steps **O2.1–O2.4**, each committed +
pushed individually before the next begins.

---

## 4. Implementation sequence (O1–O5 one-by-one, commit+push each)

Per the user's hard rule: **no batching, no parallelizing, no skipping, no stubs.**
Each step: implement → fmt → clippy (both configs where relevant) → tests → commit
(explicit-pathspec guard; never stage `tomedo_v3.md`/`whatsapp_v3.md`/`prefix_V3.md`) →
push `origin/main` → mark Zenflow + this doc → continue immediately.
`CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` is set on every build command.

### Up-front cleanup (this subplan)

**O1 — Setup (this doc + reference + Zenflow substep).** Write this subplan doc; insert a
reference blockquote into `saved_plan_to_v3.md` at the H.5 item (after the H4.8 COMPLETE
note, before the H.5 `Status:` line); add the Zenflow substep under the Phase H step
`9d94d6cb`. No production code. Commit + push.

**O2 — Delete the dormant mission system (four sub-steps O2.1–O2.4).** Grounding the full
mission footprint is done (§3.1): two distinct surfaces (engine mission system +
`brassclaw_host_api::MissionId` string scope tag) + 15 dead `parent_mission_id` columns.

  **O2.1 — Drop the 15 `parent_mission_id` columns (new migration).** Create
  `crates/brassclaw_pg/migrations/V064__drop_parent_mission_id.sql` issuing
  `ALTER TABLE ... DROP COLUMN IF EXISTS parent_mission_id` for all 15 tables (V027
  `reborn_skills` + the 14 component class tables: reborn_actions/tools/extensions/
  recipes/specs/tool_skills/plans/summaries/docus/lessons/issues/notes/python_code/
  extension_catalogues). **Grounding correction:** the up-front claim "zero Rust code
  reads/writes `parent_mission_id`" was WRONG — a workspace grep surfaced **23 `.rs`
  references across 6 files in 4 crates**: positional `row.get(N)` DB reads + SQL
  INSERT/SELECT statements in `brassclaw_extensions/unified_store.rs`,
  `brassclaw_reborn_composition/{pg_extension_catalogue_store,pg_python_code_store,
  skill_import}.rs`, `brassclaw_skills/db_store.rs`, `brassclaw_engine/db_skill_loader.rs`.
  Dropping the column without removing these would break the stores at runtime, so O2.1
  was expanded to remove all 23 Rust references (struct fields, SQL column lists,
  positional decode indices re-based, `&[...]` bind arrays, 2 column-count unit tests
  updated to 29/25 cols) — a mechanical necessity of the approved column drop, not a design
  decision. Verify: embedded PG boot applies V000–V064; engine + composition clippy clean
  (both configs); `cargo test` (both configs; DB tests skip-if-no-docker). Commit + push.
  ✅ **DONE** (this commit).

  **O2.2 — Remove the engine mission system.** Delete `crates/brassclaw_engine/src/runtime/
  mission.rs` + `crates/brassclaw_engine/src/types/mission.rs`; remove their `mod`
  registrations (`runtime/mod.rs:12,22`, `types/mod.rs:15`) + `lib.rs` re-exports
  (`lib.rs:53,99-102,104` — note `ValidTimezone` is re-exported from `types::mission` but
  is actually `brassclaw_common::ValidTimezone`; re-home the re-export to avoid breaking
  any `brassclaw_engine::ValidTimezone` user, or drop if unused — ground during this step);
  remove the mission methods from the `Store` trait (`traits/store.rs:188-237,249-255`:
  `save_mission`/`load_mission`/`list_missions`/`update_mission_status`/
  `list_missions_with_shared`/`list_shared_missions`/`list_all_missions`) + their impls in
  every `Store` implementor (composition `pg_memory_doc_store.rs:225-237`,
  `memory_doc_libsql_store.rs:346-377`; test mocks `tests/engine_v2_skill_codeact.rs:487-516`,
  `orchestrator.rs:8433-8454` + `:8699-8720`; grep `impl Store for` to find any others);
  remove `recipe_store.rs:1104` mission test usage + `lib.rs:141` test usage. Resolve every
  dangling reference. Verify: fmt + `cargo clippy -p brassclaw_engine -p
  brassclaw_reborn_composition --all-targets -- -D warnings` (both configs) + `cargo test`
  (both configs). Commit + push. ✅ **DONE** (commit `cd413643`, pushed). Verified across
  **3** configs (default + skills-db + migrate-from-libsql), not just 2: fmt clean; clippy
  `-p brassclaw_reborn_config -p brassclaw_engine -p brassclaw_reborn_composition
  --all-targets -- -D warnings` clean; `cargo test` GREEN (10 ok batches, 0 failed).
  Scope expanded mid-step per user decision: also removed the dead `mission_per_tick_usd`
  budget config category (BudgetDefaults + BudgetSection field, MISSION_PER_TICK_USD_ENV
  const + re-export, DB-kv + migration load/serialize paths across brassclaw_reborn_config
  + brassclaw_reborn_composition) — it funded the deleted mission cron and had zero
  enforcement consumers. Added `rejects_removed_budget_mission_per_tick_usd_field` regression
  test (deny_unknown_fields convention). **Breaking change:** config files still setting
  `[budget].mission_per_tick_usd` are now rejected by `deny_unknown_fields` (DB-kv path
  stays backward-compatible — unknown keys ignored); same convention as prior `[tokens]`
  field removals. Stale prose referencing deleted mission runtime/functions (`mission_list`,
  `resume_paused_missions_for_credential`, `process_mission_outcome_and_notify`, mission
  cron / #3133 ghost-fire) cleaned from engine comments; `ThreadType::Mission` variant +
  planner arm kept as dormant API per D1; `ValidTimezone` re-home to `brassclaw_common`
  per D2. Only the user's separate `basic_prompt_store` WIP (`system_bundle_source` field in
  `DefaultPlannedRuntimeParts`) blocks root-package parity tests — pre-existing, not O2.2.

  **O2.3 — Purge `brassclaw_host_api::MissionId` (full cross-crate).** Remove the
  `MissionId` type from `brassclaw_host_api/src/ids.rs`; remove the `Mission(MissionId)`
  scope enum variant + `ResourceScope.mission_id` field (`scope.rs`); remove
  `mission_id: Option<MissionId>` from `audit.rs`, `resource.rs`; then propagate to all
  consumers: `events/cursor.rs`, `processes/types.rs`, `secrets/lib.rs` (+ tests
  `:1463-1465,1803-1805,2111-2113`), `event_projections` tests, `capabilities` tests,
  `gateway` JS, and any other `MissionId` user (grep `\bMissionId\b` workspace-wide).
  Ground the full consumer set FIRST during this step. Verify: fmt + workspace clippy
  `--all --benches --tests --examples --all-features -- -D warnings` (modulo the user's
  `basic_prompt_store` WIP blockage) + `cargo test` (both configs). Commit + push.
  ✅ **DONE** (commit `5c113151`, pushed `8bda603d..5c113151 main -> main`). Grounded the
  full surface: `MissionId` newtype + `mission_id: Option<MissionId>`/`Option<String>`/
  `String` fields on ~20 scope/context/struct types across **~24 crates, ~150 sites**, plus
  Mission enum variants (`Principal::Mission`, `ProjectionTarget::Mission`,
  `ProjectionViewClass::ProductMission`, `ResourceAccount::Mission`, `BackgroundKind::MissionTick`)
  + their match arms/ctors/cascade tiers/Display arms. Per 3 user decisions (D1: full purge;
  D2: one commit, build-green at the single resulting commit; D3: option 1 — accept all 3
  persisted-data breaks): removed every field/clone/comparison/path-segment block/JSON entry/
  test fixture/helper; removed `mission_id` from `secrets/crypto.rs` `ScopeKey` +
  `credential_session_aad` (AAD change approved); rewrote `threads/pg_service.rs` SQL (column
  dropped, `$N` placeholders renumbered); removed the `loop_driver_host` `ReadScope.mission_id`
  tightening block + the `effective_read_scope_rejects_mission_widening` test (deleted
  dimension); removed mission-only `event_streams` projection tests (variants gone); retargeted
  the resource cascade assertion 5→4 accounts. Added `V065__drop_session_threads_mission_id.sql`
  (`DROP COLUMN IF EXISTS mission_id` — the only such column, V008). The purge was committed
  from a **selectively-staged index** that excludes the user's concurrent prefix-cache
  (`system_bundle_source`/`pg_basic_prompt_store`/`Prefix*`) WIP, which stays uncommitted in the
  working tree. Verified on that committed index (HEAD − mission, no unrelated WIP — so the
  pre-existing `system_bundle_source` E0063 blockage is absent): `cargo check --workspace
  --all-targets` **0 errors** (incl. `brassclaw` root + CLI); `cargo clippy --workspace
  --all-targets -- -D warnings` **0 errors/warnings**; `cargo test` (17 touched crates incl.
  `brassclaw` root + `brassclaw_reborn_composition`) **all GREEN, 0 failed**. Final grep
  `\bMissionId\b|mission_id` (excl. `submission_id` false positives) = **0**.

  **O2.4 — Delete e2e mission test + update CLAUDE.md/AGENTS.md.** Delete
  `tests/e2e/scenarios/test_mission_gmail_3133.py`; update mission mentions in
  `crates/brassclaw_engine/CLAUDE.md` + `crates/brassclaw_engine/AGENTS.md` ( +
  `docs/brassclaw-architecture.md` / `docs/internal/engine-v2-architecture.md` if they
  state missions as current architecture) to record missions as removed (v1 routines
  reborn, dormant, deleted in v3 H.5 obsolescence cleanup). Leave historical plan docs
  (`docs/plans/2026-03-24-missions.md`, `docs/plans/2026-04-11-defi-portfolio-keeper.md`)
  as archives. Verify: fmt + clippy clean (both configs). Commit + push.
  ✅ **DONE** (commit `ee6fb6f8`, pushed `df5a4010..ee6fb6f8 main -> main`). Scope
  expanded beyond the literal "delete one test file" to the full mission footprint
  surfaced in the e2e layer + the dead `MissionThreadSpawned` AppEvent variant:
  deleted `test_mission_gmail_3133.py` + `test_mission_gmail_draft_3133.json`;
  removed the dead `mission_gmail_live_server`/`mission_gmail_live_page` conftest
  fixtures; removed the dead `_normalize_mission_list_result` no-op +
  `_MISSION_LIST_RE` regex + call site from `live_llm_proxy.py` + stale "mission"
  comment refs; removed dead `mission_id` fields from the two `plan_update` mock
  payloads in `mock_llm.py` + fixed the gmail-mocks comment; removed the dead
  `MissionThreadSpawned` `AppEvent` variant (def + `as_kind` rename arm + test arm
  in `brassclaw_common/src/event.rs`) — no Rust emitter, no frontend handler (grep
  confirmed); removed the now-orphaned `mission_thread_spawned` `debug-panel.js`
  handler + the 3 orphaned `debug.activityMissionSpawned` i18n keys (en/zh-CN/ko);
  removed the `.sweepfix/codebase.toml` entry for the deleted test (file is
  gitignored, local-only). Recorded missions as REMOVED in v3 H.5 in
  `crates/brassclaw_engine/CLAUDE.md` + `AGENTS.md`, `docs/brassclaw-architecture.md`,
  `docs/internal/engine-v2-architecture.md` (focused removal banners + corrected
  current-architecture statements; historical detail retained as context).
  `ThreadType::Mission` retained as dormant API per O2.2 D1. Verified: `cargo fmt
  --check` clean; `python3 -m py_compile` clean on the 3 edited e2e files;
  `cargo check` + `clippy` `-p brassclaw_common --all-targets -- -D warnings` clean
  (variant removal compiles); `cargo test -p brassclaw_common` 88 passed 0 failed.
  Committed from a selectively-staged index that excludes the user's concurrent
  prefix-cache WIP. **Surfaced follow-up (needs user design decision):**
  `brassclaw_gateway` (legacy v1 debug-panel UI; a workspace member with NO
  dependents — not compiled into the `brassclaw` binary, only by `--workspace`
  checks) still carries a full dead Missions Tab UI: `missions.css`, mission widget
  refs in `widget.rs`, and ~150 mission i18n keys across en/zh-CN/ko (`tab.missions`,
  `missions.*`, `missions.summary.*`, etc.) — dead since the mission backend was
  removed in O2.2/O2.3. This is a large pre-existing dead surface; per task rules it
  needs a subplan + a user design decision (purge only the mission surface vs delete
  the whole legacy `brassclaw_gateway` crate vs defer). Raised with the user; NOT
  decided unilaterally. Also flagged (not touched): `crates/brassclaw_gateway` is
  listed TWICE in the root `Cargo.toml` workspace `members` (line 3) — a separate
  pre-existing anomaly.

**O3 — Delete Model A Python `tier_zero` (H.3) from `default.py` + Model-A step-0 tests.**
Remove the H.3 `default.py` step-0 `tier_zero` early-return branch + the `recipe_tier_zero_*`
event emission + the `extra` stamp it added (H4.5). Remove the H.3 Model-A step-0 Monty
unit tests that assert Tier-0 early-return behavior. **KEEP H.1
`_parse_orchestrator_channel_steps` + H.2 `execute_recipe_orchestrator_channel` Python fns
+ their unit tests for now** — they are the spec/reference for H.8's Rust
`execute_tier_zero_channel` extraction and are deleted in the final cleanup after H.8.
Verify: `python3 ast.parse` clean; engine fmt + clippy (both configs) + `cargo test`
(both configs). Commit + push.
  ✅ **DONE** (commit `110b3a6c`, pushed `8e9593ed..110b3a6c main -> main`). Removed the
  `default.py` step-0 `elif pkr.get("tier_zero")...` branch (55 lines: the 3
  `recipe_tier_zero_*` event emissions + the `tier_zero_outcome` extra stamp + the
  no-LLM early return) + updated the step-0 prior-knowledge comment + the H.2
  `execute_recipe_orchestrator_channel` docstring (H.3 Python caller removed;
  `tier_zero` still surfaced by Rust for Model B/C). Removed the 3
  `phase_h3_tier_zero_*` integration tests + their 3 exclusive helpers
  (`recording_has_transition`/`recording_has_event`/`recording_event_kwargs`) +
  section header from `orchestrator.rs` (241 lines). KEPT H.1/H.2 Python fns + tests
  + all Rust H.4 pieces (4 tests + `build_tier_zero_outcome` + event dispatch +
  producer + `TurnRoutingSignals` + `RecipeOutcomeListener`). Verified on a clean
  staged index (user WIP stashed): `ast.parse` clean; `cargo fmt -p brassclaw_engine
  --check` clean; `cargo clippy -p brassclaw_engine --all-targets -- -D warnings`
  clean (default + skills-db); `cargo test -p brassclaw_engine` GREEN (default 599
  passed, skills-db 610 passed, 0 failed).

**O4 — Re-document the reusable H.4 pieces as reused by Model B/C.** Update doc comments
on `RecipeTierZeroStarted`/`Succeeded`/`Failed` `EventKind` variants
(`crates/brassclaw_engine/src/types/event.rs`), `TurnRoutingSignals.recipe_id`/
`recipe_name` (`retrieval_source.rs`), and `RecipeOutcomeListener`
(`crates/brassclaw_reborn_composition/src/recipe_outcome_listener.rs`) to state they are
reused by Model B/C (the agent-loop `LoopOrchestratorPort` + the engine pub fns extracted
in H.8), not Model A. No behavior change. Verify: fmt + clippy clean (both configs).
Commit + push.
  ✅ **DONE** (commit `8ebdb19b`, pushed `c55863ca..8ebdb19b main -> main`). Re-documented
  the 3 O4-listed pieces + 4 stale Model-A/default.py references in `orchestrator.rs`
  (`TierZeroOutcome` + `build_tier_zero_outcome` doc; the `handle_emit_event`
  `recipe_tier_zero_*` dispatch comment; the pkr `SplitResult` producer comment; 2 H.4
  test comments) to attribute the emitter to the Model B/C agent-loop Tier-0 path. **O3
  fallout cleanup (test-infra only, no production behavior change):** the
  `run_python_step0` step-0 test helper had a `code_step_result` param + a
  `__execute_code_step__` host-handler arm that existed solely to support the H.3
  tier_zero tests removed in O3; step-0 never reaches `__execute_code_step__` (action
  path uses `__execute_action__`; CodeAct is later steps), so they were dead. Removed the
  param + arm + 6 call-site args. The dedicated H.2 harness `run_python_tier0_channel`
  (own `step_results` queue) is unaffected. Verified on a clean selectively-staged index
  (user WIP stashed): `cargo fmt --check` clean; `cargo clippy -p brassclaw_engine
  --all-targets` (default + `--features skills-db`) clean; `cargo clippy -p
  brassclaw_reborn_composition --all-targets` clean; `cargo test -p brassclaw_engine`
  GREEN (default 599, skills-db 610, 0 failed); `cargo test -p
  brassclaw_reborn_composition --lib` GREEN (629, 0 failed).

**O5 — Mark H.5 skipped/obsolete.** Record in `saved_plan_to_v3.md` (H.5 item) + the Phase
H subplan doc (`subplan_problem_stepH_of_saved_plan_to_v3.md` H.5 entry) + the Zenflow
substep that H.5 is **obsolete/skipped** (Model A dormant; never built; superseded by
H.6–H.13). No code. Commit + push. Mark this obsolescence subplan's Zenflow substep
Completed.

### Then resume Phase H at H.6 (Model B/C) — outside this subplan

H.6–H.13 build the real target (Model B/C agent-loop Tier-0/Tier-1 dispatch), with H.8
extracting `assemble_prior_knowledge_with_hint` + `execute_tier_zero_channel` from
`orchestrator.rs` into engine `pub` fns. The **final cleanup** (delete the severed engine
Python runtime wrapper + the H.1/H.2 Python fns) is a later subplan/step opened after
H.8 — NOT in this subplan.

---

## 5. Verification + status (updated as steps complete)

- O1 — Done (commit `d6c91828`).
- O2.1 — Done (commit `ea07aa93`).
- O2.2 — Done (commits `cd413643` + doc `8bda603d`).
- O2.3 — Done (commit `5c113151` + doc `df5a4010`).
- O2.4 — Done (commit `ee6fb6f8` + doc `d418b784`); + the `brassclaw_gateway` crate
  deletion follow-up Done (commits `9c780fd7` + doc `8e9593ed`).
- O3 — Done (commit `110b3a6c`).
- O4 — Done (commit `8ebdb19b`).
- O5 — Pending.
