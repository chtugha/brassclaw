# Subplan — Phase H.4 problem: engine→composition Tier-0 outcome-recording bridge

Parent: `subplan_problem_stepH_of_saved_plan_to_v3.md` (H.4 step). Parent plan:
`saved_plan_to_v3.md` (Recipe System Finalisation Plan — v3), Phase H item 3c
(Q-H3 outcome-recording bridge). This nested subplan was opened because H.4's
`recipe_id` surfacing + cross-crate bridge proved to be a wide, multi-file
change (engine `types/event.rs` + `memory/retrieval_source.rs` +
`executor/orchestrator.rs` + `executor/loop_engine.rs` + `runtime/manager.rs`
+ composition listener), exactly as the Phase H subplan §2.4 predicted
("possibly via a nested subplan if the plumbing proves large").

## 1. The gap (grounded)

- **`record_recipe_outcome(recipe_id, success)`** is a `RecipeLookup` trait
  method (`crates/brassclaw_turns/src/run_profile/recipe_lookup.rs:104`:
  `async fn record_recipe_outcome(&self, recipe_id: &str, success: bool) ->
  Result<(), RecipeLookupError>`) + composition impl
  (`crates/brassclaw_reborn_composition/src/pg_recipe_store.rs:859`,
  `PgRecipeLibrary::record_recipe_outcome` → the atomic Wilson-update SQL
  transaction). It is NOT called from the engine Tier-0 path today (verified:
  no engine `RecipeLookup` handle; engine `Cargo.toml` has no `brassclaw_turns`
  dep).
- **`TurnRoutingSignals`** (`crates/brassclaw_engine/src/memory/retrieval_source.rs:108`)
  has NO `recipe_id` field. Yet `recipe_id` IS in scope where it is built —
  `fetch_recipe_split_result` (`:739`) receives `recipe_id: uuid::Uuid` and
  constructs `TurnRoutingSignals` at `:918` (main) and `:832` (soft-fail IBS
  compile branch). So `recipe_id` is dropped today.
- **`OrchestratorResult`** (`crates/brassclaw_engine/src/executor/orchestrator.rs:64`)
  is `{ outcome: ThreadOutcome, tokens_used: TokenUsage }` — carries no
  recipe_id / tier-zero outcome. Built in `execute_orchestrator`'s
  `RunProgress::Complete` arm (`:541`).
- **`handle_emit_event`** (`orchestrator.rs:2365`) has a TYPED match on event
  names; the fallthrough (`:2448`) DROPS unknown kinds (`"unknown event kind,
  skipping"` → return, NOT pushed to `thread.events`). So
  `recipe_tier_zero_started` / `recipe_tier_zero_failed` (emitted by H.3) are
  currently NOT recorded in production `thread.events` — H.3 tests pass only
  because the `run_python_step0` harness mocks `__emit_event__` itself
  (`orchestrator.rs:4891`, pushes to `rec.events` regardless of name).
- **`ExecutionLoop::run`** (`crates/brassclaw_engine/src/executor/loop_engine.rs:413`)
  calls `execute_orchestrator` (`:471`), gets `Ok(orch_result)` (`:494`), and
  returns ONLY `orch_result.outcome` (`:508`) — drops `tokens_used` and would
  drop `tier_zero_outcome`. Driven from the engine `ThreadManager`
  (`runtime/manager.rs:386`), spawned as a background task (`:410`).
- **Composition does NOT call `join_thread`** (verified: 0 callers in
  `crates/brassclaw_reborn_composition/src`). Composition submits turns via
  `submit_turn(SubmitTurnRequest)` → `SubmitTurnResponse::Accepted { run_id }`
  (async, fire-and-forget) and observes completion via the event stream
  (`stream_events`) + the persisted thread. The engine's raw `ThreadEvent`s
  ARE persisted into composition's store via `append_events`
  (`crates/brassclaw_reborn_composition/src/pg_memory_doc_store.rs:201`,
  driven by `manager.rs:435` `store_for_task.append_events(&exec.thread.events)`).
  Only the engine-internal mission system calls `join_thread`
  (`crates/brassclaw_engine/src/runtime/mission.rs:2313`).

## 2. Design decisions (answered by user before this subplan's implementation)

- **Q-H6 (how the engine learns the Tier-0 outcome from `default.py`):**
  Mixed mechanism — `default.py` stamps the SUCCESS case via
  `complete_result(state, "completed", response=…, extra={"tier_zero_outcome":
  {"recipe_id": …, "success": True}})` AND emits a `recipe_tier_zero_failed`
  event (carrying `recipe_id`) for the FAILURE case; the engine reads the
  success stamp from the result dict and the failure signal from
  `thread.events`. (A Tier-0 failure degrades to a normal LLM `completed`, so
  the failure cannot be inferred from the outcome alone — the event is the
  signal.)
- **Q-H7 (where `record_recipe_outcome` is called):** Architecture A — the
  engine carries `tier_zero_outcome` up through `OrchestratorResult`; the
  Wilson-update transaction stays in composition's `PgRecipeLibrary` (no
  duplicated SQL, no new engine `brassclaw_turns` dep). Within A, the surfacing
  mechanism to composition is **A2 (event-based)**: the engine emits typed
  `RecipeTierZeroStarted` + `RecipeTierZeroSucceeded` / `RecipeTierZeroFailed`
  `EventKind` variants (carrying `recipe_id`); a composition event listener
  calls `PgRecipeLibrary::record_recipe_outcome(recipe_id, success)` on the
  terminal event (fire-and-forget, best-effort, errors at `debug!`). This
  unifies with the Q-H6 failure-event mechanism. `OrchestratorResult.
  tier_zero_outcome` is STILL populated (success from the `extra` stamp,
  failure from the `RecipeTierZeroFailed` event scan) for engine-internal
  consumers + unit tests.

## 3. Step sequence (one by one — no batching, no skipping, no stubs)

Each step: implement → `cargo fmt` → clippy (both default +
`--features brassclaw_engine/skills-db` where relevant) → tests (both configs)
→ commit (explicit-pathspec guard; NEVER stage `tomedo_v3.md`/`whatsapp_v3.md`)
→ push `origin/main` → mark Zenflow + this doc → continue immediately.
`CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` on every build command.

**H4.1 — `EventKind` typed variants** (`crates/brassclaw_engine/src/types/event.rs`).
Add three variants BEFORE `#[serde(other)] Unknown` (serde requires `other`
last):
- `RecipeTierZeroStarted { recipe_id: String, recipe_name: String }`
- `RecipeTierZeroSucceeded { recipe_id: String, recipe_name: String }`
- `RecipeTierZeroFailed { recipe_id: String, recipe_name: String, message: String }`
`#[derive(Debug, Clone, Serialize, Deserialize)]` already on the enum. Check
`summarize_params` / any exhaustive match on `EventKind` (grep `match.*event.
kind|match.*\.kind`) and add arms (or confirm a wildcard arm covers them — but
prefer explicit arms where the match is exhaustive; the trace `build_trace`
may need arms). Unit-test the variants serde round-trip.

**H4.2 — `handle_emit_event` match arms** (`orchestrator.rs:2365`). Add
`"recipe_tier_zero_started"`, `"recipe_tier_zero_succeeded"`,
`"recipe_tier_zero_failed"` arms extracting `recipe_id` (kwarg `recipe_id`),
`recipe_name` (kwarg `recipe`), and `message` (kwarg `message`, failed only)
via the existing `extract_string_kwarg` helper. So the events are actually
pushed to `thread.events` + broadcast on `event_tx` (today they are dropped).
Monty-safe (Rust-side). Update the `run_python_step0` + `run_python_tier0_
channel` test harnesses' `__emit_event__` mocks if they assert on shape (they
capture kwargs via `kwargs_to_json`, so no change needed — verify).

**H4.3 — `TurnRoutingSignals.recipe_id`** (`retrieval_source.rs:108`). Add
`pub recipe_id: Option<String>` (Option: the soft-fail + non-recipe paths may
have none; but both `fetch_recipe_split_result` construction sites DO have the
`recipe_id` param → `Some(recipe_id.to_string())`). Populate at `:832`
(soft-fail) and `:918` (main). Update the test helper `phase_f7_split_result`
(`orchestrator.rs:8914`) with `recipe_id: None`. Grep ALL other
`TurnRoutingSignals {` construction sites (only the 3 known: `:832`, `:918`,
test `:8914`) and update. Unit-test the field is populated.

**H4.4 — surface `recipe_id` in the pkr dict**
(`orchestrator.rs:2739` SplitResult arm). Add `"recipe_id": routing.recipe_id`
to the `json_to_monty(&serde_json::json!({ … }))` pkr dict. (The dict already
surfaces `tier_zero`, `matched_component_ids`, etc.) This is how `default.py`
step-0 receives `pkr["recipe_id"]`.

**H4.5 — `default.py` tier_zero branch: emit `recipe_id` + success event +
`extra` stamp.** Update the H.3 branch:
- `recipe_tier_zero_started` event: add `recipe_id=pkr.get("recipe_id", "")`.
- SUCCESS arm: BEFORE returning, emit
  `__emit_event__("recipe_tier_zero_succeeded", recipe=pkr.get("recipe_name",
  ""), recipe_id=pkr.get("recipe_id", ""))`, then
  `return complete_result(state, "completed", response=tier0_result.get(
  "result", ""), extra={"tier_zero_outcome": {"recipe_id": pkr.get(
  "recipe_id", ""), "success": True}})`.
- FAILURE/degrade arm: the `recipe_tier_zero_failed` event already exists —
  add `recipe_id=pkr.get("recipe_id", "")` to its kwargs. (No `extra` stamp:
  the failure signal rides the event, per Q-H6.)
Monty-safe (no f-strings/`.format()`/`re` — use `+`/`str()`/`.get()`). Update
the H.3 Monty tests: add `recipe_id` to the pkr dicts; success test asserts
`recipe_tier_zero_succeeded` event + `extra` stamp; failure test asserts
`recipe_tier_zero_failed` carries `recipe_id`. `python3 ast.parse` clean.

**H4.6 — `TierZeroOutcome` struct + `OrchestratorResult.tier_zero_outcome`**
(`orchestrator.rs:64`). Add `#[derive(Debug, Clone)] pub struct
TierZeroOutcome { pub recipe_id: String, pub success: bool }` + `pub
tier_zero_outcome: Option<TierZeroOutcome>` on `OrchestratorResult`. Update
ALL `OrchestratorResult { … }` construction sites (grep) to set
`tier_zero_outcome` (default `None` where not a Tier-0 completion; the
`Complete` arm at `:541` computes it). Build the value in the `Complete` arm:
(1) if `result.get("tier_zero_outcome")` is a dict with `recipe_id` + success
→ `Some(TierZeroOutcome{ recipe_id, success: true })` (success `extra`
stamp); (2) else scan `thread.events` for `EventKind::RecipeTierZeroFailed{
recipe_id, .. }` → `Some(TierZeroOutcome{ recipe_id, success: false })`; (3)
else `None`. Unit-test all three branches (success-via-extra,
failure-via-event, none-for-plain-Tier-2) at the engine level (a focused test
that constructs the result dict + thread events and asserts the built
`tier_zero_outcome` — extract the build logic into a small pure helper fn
`build_tier_zero_outcome(result: &serde_json::Value, events: &[ThreadEvent])`
so it is unit-testable without driving the whole VM).

**H4.7 — composition event listener → `record_recipe_outcome`.** Ground the
exact composition attachment point FIRST (the engine `event_tx` broadcast
subscriber OR the `append_events` store hook — both see the raw engine
`ThreadEvent`s). Add a composition-side listener that, on
`EventKind::RecipeTierZeroSucceeded { recipe_id, .. }` → calls
`PgRecipeLibrary::record_recipe_outcome(&recipe_id, true)`, and on
`EventKind::RecipeTierZeroFailed { recipe_id, .. }` → `record_recipe_outcome(
&recipe_id, false)` (fire-and-forget best-effort; errors logged at `debug!`,
never break the turn). The listener needs a `PgRecipeLibrary` (or
`Arc<dyn RecipeLookup>`) handle — verify composition's turn-runner has one
(ground during this step; if it must be threaded in, do it here, no stub).
Add a unit test with a spy `RecipeLookup` impl asserting the call is made for
both success + failure events. (The full DB-backed integration assertion is
H.5.)

**H4.8 — final H.4 verification.** fmt + `cargo clippy --all --benches
--tests --examples --all-features -- -D warnings` + `cargo test` (both
configs: default + `--features brassclaw_engine/skills-db`; composition crate
tests too). Mark this subplan done; mark the Phase H subplan H.4 step Done;
resume the parent Phase H subplan at H.5.

---

## 4. Verification + status (updated as steps complete)

- H4.1 — Done. Added `RecipeTierZeroStarted`/`RecipeTierZeroSucceeded`/`RecipeTierZeroFailed` `EventKind` variants before `#[serde(other)] Unknown` (carrying `recipe_id`+`recipe_name`, +`message` on Failed). Verified all existing `EventKind` matches use wildcard arms (engine `mission.rs`/`loop_engine.rs`/`trace.rs`/`thread.rs`; downstream `brassclaw_common` is a parallel wire enum + `brassclaw_turns` uses its own `TurnEventKind` — none break). Serde round-trip unit test added. clippy clean both default + `skills-db`; commit `b84a6197`.
- H4.2 — Done. Added `handle_emit_event` (`orchestrator.rs:2365`) match arms for `recipe_tier_zero_started`/`recipe_tier_zero_succeeded`/`recipe_tier_zero_failed` extracting `recipe_id` (`recipe_id` kwarg), `recipe_name` (`recipe` kwarg), `message` (`message` kwarg, failed only) via `extract_string_kwarg`. The events are now pushed to `thread.events` + broadcast on `event_tx` (the fallthrough previously DROPPED them). The `run_python_step0`/`run_python_tier0_channel` harness `__emit_event__` mocks capture all events via `kwargs_to_json` regardless of name, so no harness change was needed. Added `handle_emit_event_dispatches_recipe_tier_zero_events` unit test. clippy clean both configs; commit `c0ce0677`.
- H4.3 — Done. Added `pub recipe_id: Option<String>` to `TurnRoutingSignals` (`retrieval_source.rs:108`) with a doc comment; populated BOTH `fetch_recipe_split_result` construction sites (soft-fail `:832` + main `:918`) with `Some(recipe_id.to_string())` (the `recipe_id: uuid::Uuid` param is in scope at both). Updated the two test construction sites with `recipe_id: None`: the engine `phase_f7_split_result` helper (`orchestrator.rs:9037`) and the composition bridge `split_mapping_propagates_routing_booleans_and_split_channels` test (`retrieval_lookup_impl.rs:673`, `#[cfg(feature="skills-db")]` — NOT in the subplan's original "3 known" list, found via repo-wide grep). Confirmed NO exhaustive destructure of `TurnRoutingSignals` exists (only named-field constructions); the composition converter `retrieval_turn_result_for_split` reads named `routing.*` fields and is unaffected by the new field (recipe_id is NOT yet propagated into `routing_meta` — that is H.9's restructure). **Encountered clippy `large_enum_variant`:** adding the 24-byte `Option<String>` pushed `FetchForTurnResult::SplitResult` (~264 bytes) over the threshold. Resolved by BOXING the heaviest field — `instruction: Option<BuildInstruction>` → `Option<Box<BuildInstruction>>` (saves ~96 bytes inline; `FetchForTurnResult` is a one-shot transient return, never bulk-stored, so the single heap indirection has no throughput cost) — matching the `ThreadOutcome::GatePaused.paused_lease` boxing precedent (`runtime/messaging.rs:55`), NOT a `#[allow]` silence (task rules forbid suppressing). Updated the 4 affected sites: field def + main construction (`Some(Box::new(instruction))`) + converter param type (`retrieval_lookup_impl.rs:233`) + the `:683` test fixture; the 3 `instruction: None` sites needed no change; the `fetch_for_turn.rs:440` integration test's `instruction.expect(..).rust_steps` field access auto-derefs the `Box` (no change). Added `turn_routing_signals_recipe_id_carried_through_split_result` unit test. Verified: fmt clean; clippy clean engine (default + `skills-db`) + composition (`skills-db`); engine lib 703 passed 0 failed (default); commit pending.
- H4.4 — Done. Added `"recipe_id": routing.recipe_id,` to the `handle_assemble_prior_knowledge` SplitResult-arm pkr dict (`orchestrator.rs:2787`, grouped with `variant_label`/`step_link`), so `default.py` step-0 receives `pkr["recipe_id"]` (read in H4.5 via `pkr.get("recipe_id", "")`). `serde_json::json!` serializes `Option<String>` as a string-or-null; on the SplitResult path `routing.recipe_id` is always `Some` (both `fetch_recipe_split_result` sites set `Some`), and a `null` (non-recipe path) is handled downstream by `extract_string_kwarg(...).unwrap_or_default()` → `""`. Updated the arm's comment to document the surfacing. No new unit test added (justified): the pkr dict is only reachable via `PostgresSource::fetch_for_turn` returning a `SplitResult` (DB-gated; `RamSource` cannot produce a `SplitResult`), so the narrowest validating test is the Phase-H.5 composition integration test (end-to-end); H4.5's consumer tests exercise the field via the Monty `run_python_step0` harness pkr dict. Verified: fmt clean; clippy clean engine (default + `skills-db`); commit pending.
- H4.5 — Done. **Design decision (Q-A, asked before implementing):** the subplan read `pkr.get("recipe_name", "")` but `recipe_name` was NOT surfaced in the pkr dict (H4.4 only added `recipe_id`); the `EventKind::RecipeTierZero*` variants carry `recipe_name: String` needing a source. User chose Option A — surface `recipe_name` end-to-end (matching the recipe_id pattern). Implemented the recipe_name surfacing (a mini-H4.3/H4.4 for the name): added `pub recipe_name: String` to `TurnRoutingSignals` (`retrieval_source.rs:152`) with a doc comment noting it is `reborn_recipes.name` (`fetch_recipe_split_result` line 810, `row.get(0)`), distinct from `variant_label` (the matched variant key, which falls back to `recipe_name` only when no variant matches — line 831). Populated BOTH engine construction sites (`:859` soft-fail + `:865` main) with `recipe_name: recipe_name.clone()` (the `recipe_name` local exists at line 810). Updated 3 test fixtures with a `recipe_name` value: the engine `phase_f7_split_result` helper→`"recipe_ls"`, the composition bridge `split_mapping_propagates_routing_booleans_and_split_channels` test (`retrieval_lookup_impl.rs:682`)→`"daily-sync"`, and the H4.3 unit test→`"greet-recipe"` (also extended to assert `recipe_name` carried). Surfaced `"recipe_name": routing.recipe_name,` in the SplitResult-arm pkr dict (`orchestrator.rs:2791`, grouped with `recipe_id`). Updated `default.py` tier_zero branch: `recipe_tier_zero_started` now emits `recipe_id=pkr.get("recipe_id","")`; SUCCESS arm now emits `recipe_tier_zero_succeeded` event (recipe + recipe_id) AND returns `complete_result(state, "completed", response=tier0_result.get("result",""), extra={"tier_zero_outcome": {"recipe_id": pkr.get("recipe_id",""), "success": True}})` (complete_result flattens `extra`, so the stamp lands at `outcome["tier_zero_outcome"]`); FAILURE arm's `recipe_tier_zero_failed` now includes `recipe_id=pkr.get("recipe_id","")` (no `extra` stamp — failure signal rides the event per Q-H6). Monty-safe (no f-strings/`.format()`/`re` — uses `.get()`/`+`/nested dict literal; nested dict literal verified to compile under Monty via the test run). Updated H.3 tests: added `recording_event_kwargs` helper (`orchestrator.rs:7113`); updated `phase_h3_tier_zero_success_returns_completed_no_llm` (recipe_id in pkr + assert succeeded event + kwargs carry recipe_id+recipe name on started+succeeded + `outcome["tier_zero_outcome"]` stamp carries recipe_id + success:true); updated `phase_h3_tier_zero_error_degrades_to_tier2_llm` (recipe_id in pkr + assert started+failed event kwargs carry recipe_id + failed carries recipe name + assert NO `tier_zero_outcome` stamp); test 3 `phase_h3_tier_zero_without_orchestrator_content_falls_through` needs no change (deliberately emits no tier-0 events). `python3 ast.parse` clean. Verified: fmt clean; clippy clean engine (default + `skills-db`); engine lib 703 passed 0 failed (default) / 714 passed 0 failed (`skills-db`); commit pending.
- H4.6 — Done. Added `#[derive(Debug, Clone)] pub struct TierZeroOutcome { pub recipe_id: String, pub success: bool }` + `pub tier_zero_outcome: Option<TierZeroOutcome>` on `OrchestratorResult` (`orchestrator.rs:84`, with doc comments explaining the Q-H7 Architecture A split: this field is for engine-internal consumers + unit tests; the composition listener (H4.7) is the durable recording path). Added the pure helper `pub fn build_tier_zero_outcome(result: &serde_json::Value, events: &[ThreadEvent]) -> Option<TierZeroOutcome>` (`orchestrator.rs:106`) implementing the three branches: (1) SUCCESS — read `result["tier_zero_outcome"]` object's `recipe_id` (non-empty) → `Some(success: true)` (the stamp is ONLY ever written on the Tier-0 success path so its presence IS the success signal, per Q-H6); (2) FAILURE — scan `events` for `EventKind::RecipeTierZeroFailed { recipe_id, .. }` (non-empty) → `Some(success: false)` (the failure signal rides the event, NOT a result-dict stamp); (3) else `None` (plain Tier-2 turn). Wired it into the `RunProgress::Complete` arm (`orchestrator.rs:606`) computing `tier_zero_outcome` from `&result` + `&thread.events` before constructing the `OrchestratorResult`. **Grep confirmed only ONE `OrchestratorResult {` construction site** (`:541`→`:607`; the other `RunProgress::Complete` matches at `:4722/:4892/:6805` return the raw Monty object / `(monty_to_json(&obj), _)`, NOT an `OrchestratorResult`) — repo-wide grep found no other construction sites (the struct has no derives, so adding a field only breaks literal constructions, all fixed). **Encountered clippy `collapsible_if` (3×):** the nested `if let Some { if let Some { if !empty } }` + `if let EventKind { if !empty }` patterns tripped it — resolved by collapsing to let-chains (`if let X && cond { }`, the form clippy itself suggested; the toolchain supports let-chains), NOT a `#[allow]` silence. Added 3 unit tests covering all branches: `build_tier_zero_outcome_success_via_extra_stamp` (stamp present + a non-terminal `RecipeTierZeroStarted` event must NOT be misread → success:true), `build_tier_zero_outcome_failure_via_event` (no stamp + `RecipeTierZeroFailed` event → success:false), `build_tier_zero_outcome_none_for_plain_tier2_turn` (no stamp + no events → None). Verified: fmt clean; clippy clean engine (default + `skills-db`); engine lib 706 passed 0 failed (default, +3) / 717 passed 0 failed (`skills-db`, +3); commit pending.
- H4.7 — Done. **Architectural gap discovered + design decision asked before implementing:** the `RecipeTierZero*` events (H4.1) + `OrchestratorResult.tier_zero_outcome` (H4.6) live on the ENGINE `EventKind`/`thread.events` + the engine `event_tx` broadcast (`loop_engine.rs:117`, `ThreadManager.event_tx`, `subscribe_events()` at `manager.rs:115`). `execute_orchestrator` (which runs `default.py` step-0 + emits those events) is called ONLY from `loop_engine.rs:471` — the engine `ThreadManager`/`ExecutionLoop` path, which E0-A established is DORMANT/test-only in production. The LIVE production driver is the agent-loop stack (`DefaultPlannedRuntimeParts` → `RecipeStage`), which does NOT invoke `execute_orchestrator`; its `TurnEventSink` (`runtime.rs:151`) only sees turn-lifecycle `TurnEventKind`s (Submitted/…/Completed/Failed — NO `RecipeTierZero*`). Composition does NOT construct an engine `ThreadManager`/`event_tx` today. The live agent-loop Tier-0 dispatch that produces/consumes `RecipeTierZero*` is H.6–H.13 (not yet built). User chose **Option A**: build the listener + spy unit tests now (dormant-runtime-ready), wire it into the live engine `event_tx` when H.6–H.13 land. Implemented `crates/brassclaw_reborn_composition/src/recipe_outcome_listener.rs` as a `pub mod` (public API → no dead-code warning; H.6–H.13 wire it into the live event stream). Mirrors the `budget_events.rs` broadcast-projection precedent exactly: `pub struct RecipeOutcomeListener { recipe_lookup: Arc<dyn RecipeLookup> }` with `pub fn new`, `pub async fn handle_event(&self, event: &ThreadEvent)` (matches `event.kind` → `RecipeTierZeroSucceeded{recipe_id,..}` calls `record_recipe_outcome(&recipe_id, true)`, `RecipeTierZeroFailed{recipe_id,..}` calls `record_recipe_outcome(&recipe_id, false)`, ignores non-terminal events + empty `recipe_id`; best-effort, errors at `debug!`, never propagate — a Wilson-update failure must never break the projection/turn), and `pub fn spawn(self, receiver: broadcast::Receiver<ThreadEvent>) -> RecipeOutcomeProjection` (tokio task draining the receiver via `tokio::select!` with `CancellationToken` + `Lagged`/`Closed` handling). `RecipeOutcomeProjection::shutdown` cancels + awaits (mirror of `BudgetEventProjection::shutdown`). Registered `pub mod recipe_outcome_listener;` in `lib.rs:122`. **Encountered E0277:** `#[derive(Debug)]` on `RecipeOutcomeListener` failed because `dyn RecipeLookup` has no `Debug` supertrait — replaced with a manual `impl std::fmt::Debug` (placeholder `"<dyn RecipeLookup>"` field), NOT a silence. **Encountered E0308 (3×, tests):** `RecipeOutcomeListener::new(Arc::clone(&spy))` failed — `Arc::clone(&spy)` can't infer `T` (conflict: receiver `&Arc<SpyRecipeLookup>` vs expected-out `Arc<dyn RecipeLookup>`). Fixed by coercing via a typed binding `let recipe_lookup: Arc<dyn RecipeLookup> = spy.clone();` (the `.clone()` method resolves `T` concretely from the receiver, then unsize-coerces at the binding) — keeps the concrete `spy` accessible for `.calls` assertions; applied to all 3 spy tests. tokio `rt`+`rt-multi-thread`+`sync` features + `tokio-util` `CancellationToken` already in composition. Added 3 spy `RecipeLookup` unit tests (`SpyRecipeLookup` + `FailingRecipeLookup`): `handle_event_records_outcome_for_succeeded_and_failed_only` (terminal Succeeded→true + Failed→false record; non-terminal Started ignored), `handle_event_skips_empty_recipe_id` (terminal event with empty recipe_id → no DB call), `handle_event_swallows_record_failure` (FailingRecipeLookup returns Err → listener logs at debug, does NOT propagate). Verified: fmt clean; clippy clean composition (default + `skills-db`); composition lib 629 passed 0 failed (default) / 629 passed 0 failed (`skills-db`); commit pending.
- H4.8 — Pending.
