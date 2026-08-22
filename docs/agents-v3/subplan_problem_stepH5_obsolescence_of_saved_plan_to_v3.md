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

**O2 — Delete the dormant mission system.** Ground the full mission dependency footprint
FIRST (workspace grep for `Mission`/`MissionId`/`MissionManager`/`MissionCadence`/
`MissionNotification`/`MissionStatus`/`MissionGateInfo`/`MissionUpdate`/`FireRateLimit`/
`BudgetGate`-engine-trait; mission DB tables/migrations; composition/product mission API
endpoints/stores; engine mission wiring beyond `mission.rs`; mission tests outside
`mission.rs`). Then delete: `runtime/mission.rs`, `types/mission.rs`, their `mod`
registrations, all wiring + tests + any mission DB migration/table drops (if a mission
table exists, drop it in a new migration — DB schema change, document as an upgrade).
Resolve every dangling reference (the engine `join_thread` sole caller, the conversation
manager's mission-related arms, etc.). Verify: fmt + `cargo clippy --all --benches --tests
--examples --all-features -- -D warnings` clean (modulo the user's separate
`basic_prompt_store` WIP blockage, documented) + `cargo test` (both configs; DB tests
skip-if-no-docker). Commit + push.

**O3 — Delete Model A Python `tier_zero` (H.3) from `default.py` + Model-A step-0 tests.**
Remove the H.3 `default.py` step-0 `tier_zero` early-return branch + the `recipe_tier_zero_*`
event emission + the `extra` stamp it added (H4.5). Remove the H.3 Model-A step-0 Monty
unit tests that assert Tier-0 early-return behavior. **KEEP H.1
`_parse_orchestrator_channel_steps` + H.2 `execute_recipe_orchestrator_channel` Python fns
+ their unit tests for now** — they are the spec/reference for H.8's Rust
`execute_tier_zero_channel` extraction and are deleted in the final cleanup after H.8.
Verify: `python3 ast.parse` clean; engine fmt + clippy (both configs) + `cargo test`
(both configs). Commit + push.

**O4 — Re-document the reusable H.4 pieces as reused by Model B/C.** Update doc comments
on `RecipeTierZeroStarted`/`Succeeded`/`Failed` `EventKind` variants
(`crates/brassclaw_engine/src/types/event.rs`), `TurnRoutingSignals.recipe_id`/
`recipe_name` (`retrieval_source.rs`), and `RecipeOutcomeListener`
(`crates/brassclaw_reborn_composition/src/recipe_outcome_listener.rs`) to state they are
reused by Model B/C (the agent-loop `LoopOrchestratorPort` + the engine pub fns extracted
in H.8), not Model A. No behavior change. Verify: fmt + clippy clean (both configs).
Commit + push.

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

- O1 — Pending.
- O2 — Pending.
- O3 — Pending.
- O4 — Pending.
- O5 — Pending.
