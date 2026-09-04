# Subplan — Step C.6: Production driver switch

> Parent: `./saved_plan_to_v3.md` Step C (C.1–C.7). Sequenced after **C.5
> (COMPLETE 2026-09-04, `26b0b153`)** and before **C.7 (retire dead Model-A
> code)**. Per saved_plan line 5976-5979: "C.6 — Production driver switch.
> Replace `TurnRunnerWorker` → agent-loop stages with `TurnRunnerWorker` → one
> cross-turn persistent Monty session (D-C1) running the basic-mode
> orchestrator. Retire `canonical.rs` stage pipeline as the driver; reuse stage
> logic as host fns."

## Goal

Make **a cross-turn-persistent Monty VM session running `basic_mode.py`** the
production turn driver. `TurnRunnerWorker` calls it directly (bypassing the
driver registry). The `canonical.rs` stage pipeline is deleted as the driver in
this step; its stage logic already lives in the `host.*` arms
(`compose_orchestrator`/`kohai_complete`/`post_reply`) wired in C.1–C.5.

## Grounding (verified live source, 2026-09-04)

### Current production driver (to replace)
- `TurnRunnerWorker` (`crates/brassclaw_reborn/src/turn_runner.rs:218`) →
  `try_claim_and_run` → `host_factory.create_host` + `driver_registry` lookup →
  the matched `AgentLoopDriver` → `DefaultExecutorPipeline::execute`
  (`crates/brassclaw_agent_loop/src/executor/canonical.rs:20`) — the stage
  pipeline (`CheckpointStage`/`BudgetStep`/`InputStep`/`RecipeStage`/
  `TierZeroExecutionStage`/`PromptStage`/`ModelStage`/`CapabilityStage`/
  `AssistantReplyStage`).
- C6-1=B: add a DIRECT path in `TurnRunnerWorker` that bypasses
  `driver_registry` and drives the Monty session fn.

### Monty VM suspend/resume (C6-2 feasibility — CONFIRMED)
- `MontyRun::new(code, name, input_names)` parses only (no heap); `Clone +
  Serialize + Deserialize` (`monty/src/run.rs:40`).
- `MontyRun::start(self, inputs, tracker, print)` consumes self →
  `RunProgress<T>` (`run.rs:144`). `RunProgress::FunctionCall(call)` →
  `call.resume(value, print)` → next `RunProgress<T>`; `RunProgress::Complete`
  = done.
- The suspended `call` is **owned / `'static`** (parameterized only by tracker
  `T`; the `PrintWriter<'_>` is borrowed per-`resume` call, NOT held). So a
  `RunProgress::FunctionCall<LimitedTracker>` **can be stored across turns** in
  a conversation-keyed registry and resumed on the next turn with a fresh print
  writer.
- `execute_orchestrator` (`orchestrator.rs:520`) already holds
  `RunProgress<LimitedTracker>` across host-handler `.await` points
  (`handle_compose_orchestrator(...).await` etc.) inside an `async fn`, so the
  type is `Send` → safe to park in a registry held across turn awaits.
- The `host` namespace is a frozen empty-attrs Dataclass; `host.<tool>(...)`
  compiles to `CallAttr` → `MethodCall` → `FunctionCall{function_name:
  "<tool>", method_call: true, args[0]=self}`. A new `host.await_next_turn()`
  is therefore just another `host.*` arm that, instead of resolving+resuming,
  **yields the suspended `call` back to the caller** (the park primitive).

### execute_orchestrator today (the loop to rework into a session)
- `execute_orchestrator` (`orchestrator.rs:520`) builds inputs
  (`build_orchestrator_inputs`, :544), compiles+starts the VM (:547-602), then
  loops `RunProgress` (:607-957): `Complete`→`OrchestratorResult`,
  `FunctionCall`→dispatch `host.*`/`FINAL`/`__llm_complete__` arms (:635-876)
  →`call.resume` (:879) → next; `NameLookup` (host namespace, :903).
- Local state held across the loop: `total_tokens`, `final_result`,
  `stdout`, `thread`. To park mid-loop, these + the suspended `call` must be
  bundled into a session handle the caller parks and re-enters next turn.

### Host-call surface (C.1–C.5, already wired in `execute_orchestrator`)
- `FINAL`, `host.resolve_intent`, `host.post_reply`, `host.fetch_component`,
  `host.resolve_component_by_name`, `host.validate_component`, `host.check_
  signals`, `host.regex_match`, `host.skill_list`, `host.compose_orchestrator`,
  `host.run_program`, `host.kohai_complete`, the C.3 dynamic-tool fallthrough.
- `__llm_complete__`/`handle_llm_complete` + the 7 stage-machinery verbs are
  RETIRED (C.1/C.2) — dead arms to delete in C.7 (or this step if they block).

## Locked forks (user, 2026-09-04)

- **C6-1 = B — direct `TurnRunnerWorker` path.** Bypass `driver_registry`;
  `TurnRunnerWorker` calls the Monty session driver fn directly for Monty turns.
- **C6-2 = B — true VM persistence.** Keep a **live `MontyRun`/`RunProgress`
  handle per conversation across turns** (idle between turns, woken on new
  input). Requires reworking `basic_mode.py` into a resumable long-running loop
  + the VM park/resume primitive + a conversation-keyed session registry. The
  heaviest option; chosen deliberately.
- **C6-3 = B — delete `canonical.rs` stage pipeline outright in C.6.** Reuse
  stage logic as host fns in the same step (it is already in the `host.*`
  arms). Not deferred to C.7.
- **C6-4 = C — CI/Docker e2e only.** Skip local end-to-end verification (Docker
  unavailable locally); rely on CI/Docker for the "drives a turn" e2e. Local
  verification = unit tests for the session park/resume + registry + both
  configs clippy-clean.

## Slices (one-by-one; both configs clippy-clean + commit + push each)

- **Slice 1 — engine `MontySession` + park/resume primitive.** New
  `executor/monty_session.rs`: a stateful `MontySession` holding the suspended
  `RunProgress::FunctionCall<LimitedTracker>` (or a fresh `MontyRun` for turn 1)
  + accumulated `total_tokens`/`final_result`/`stdout` + `Arc` clones of the
  host deps (`llm`/`effects`/`leases`/`policy`/`gate_controller`/`store`/the
  ports). `async fn drive_to_yield(&mut self, thread, signal_rx, new_input)
  -> OrchestratorYield` where `OrchestratorYield { Complete(OrchestratorResult),
  AwaitNextTurn }`. Add the `host.await_next_turn()` arm: it yields the
  suspended `call` (parks) instead of resolving+resuming. Refactor
  `execute_orchestrator` into a thin "drive a fresh session to first
  yield/complete" wrapper (kept for the transition + tests). Unit tests: a mock
  script that calls `host.await_next_turn()` → session parks → resume with next
  input → completes; FINAL still terminates.
- **Slice 2 — rework `basic_mode.py` into a resumable long-running loop.**
  `def main(...)` becomes `while True: signal = host.check_signals(); if signal
  == "stop": FINAL({outcome:"stopped", state}); user_input = host.await_next_
  turn(); ... resolve_intent ... compose_orchestrator + run_steplist (or
  non_match_answer) ... host.post_reply(text=answer) ... append to in-VM
  history ... loop`. The bootstrap `context`/`state`/`goal`/`actions`/`config`
  seed turn 1 only (first `await_next_turn` returns the first user input; or
  seed the in-VM `history` from `context` on first entry). Monty 0.0.16 subset
  only. Update `build_orchestrator_inputs` if the bootstrap contract changes.
- **Slice 3 — conversation-keyed Monty session registry.** A store keyed by
  thread/conversation id holding parked `MontySession` handles, with liveness
  /eviction (drop on thread complete or a TTL). Lives downstream of engine
  (composition or brassclaw_reborn); the engine exposes `MontySession` as an
  owned, `Send` handle. TurnRunnerWorker retrieves/parks per conversation.
- **Slice 4 — `TurnRunnerWorker` direct path (C6-1=B).** In
  `try_claim_and_run`, for Monty turns, bypass `driver_registry`: retrieve (or
  create on turn 1) the `MontySession`, `drive_to_yield` with the new input,
  apply the resulting `LoopExit` (Complete→turn done; AwaitNextTurn→park +
  turn done, VM stays alive for the next turn). Wire host deps + ports into the
  session (the `PgCompositionPort`/`PgKohaiPort` from C.4.5.17/C.5).
- **Slice 5 — retire `canonical.rs` stage pipeline (C6-3=B).** Delete
  `DefaultExecutorPipeline::execute` + the stages as the turn driver; confirm
  stage logic is in the `host.*` arms; remove `driver_registry` → canonical.rs
  turn routing. Keep `brassclaw_agent_loop` crate only if non-turn uses remain
  (else mark for C.7). Delete the retired `__llm_complete__`/stage-machinery
  arms from the dispatch if not already gone.
- **Slice 6 — both configs clippy + tests (C6-4=C) + mark C.6 done.** Unit
  tests (session park/resume + registry); both engine + composition configs
  clippy-clean; `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` on every
  build; `df -h` first — `cargo clean -p <crate>` if Avail<15GB or >90%. Skip
  local e2e (CI/Docker). Commit + push. Mark C.6 done in saved_plan/mindmap/
  subplan. **Continue into C.7.**

## Slice 1 realization detail (grounded 2026-09-04)

**Key facts confirmed:**
- `execute_orchestrator` (`orchestrator.rs:520`) has exactly ONE prod caller
  (`loop_engine.rs:533`, dormant `ExecutionLoop::run`) and ZERO test callers
  (tests use the separate `run_python_final` helper). `OrchestratorResult`
  external refs are doc-comments only. → Safe to refactor freely.
- `FunctionCall<T>` (`monty/run_progress.rs:139`) is `pub` with `pub
  function_name/args/kwargs/call_id/method_call` + private `snapshot`;
  `resume(self, result, print)` consumes self; `tracker_mut()` exists. A
  `FunctionCall<LimitedTracker>` is owned/`Send` (execute_orchestrator already
  holds `RunProgress<LimitedTracker>` across `.await`). → A session CAN park
  it across turns and resume with a fresh `PrintWriter`.
- `SignalReceiver = tokio::sync::mpsc::Receiver<ThreadSignal>` (owned, `Send`).

**Design (keeps the tree green + `loop_engine.rs` UNTOUCHED):**
1. Add `pub enum OrchestratorYield { Complete(OrchestratorResult), AwaitNextTurn }`.
2. Add `pub struct OrchestratorDeps<'a>` holding all borrowed host deps (the
   `execute_orchestrator` params except `code`/`thread`/`persisted_state`/
   `max_duration_override`/the unused `_retrieval_source`): `llm`, `effects`,
   `leases`, `policy`, `event_tx`, `retrieval`, `store`, `platform_info`,
   `gate_controller`, `#[cfg(skills-db)] pg_pool`, `dynamic_tools`,
   `composition_port`, `kohai_port`.
3. Add `pub struct MontySession { progress: Option<RunProgress<LimitedTracker>>,
   parked_call: Option<FunctionCall<LimitedTracker>>, total_tokens: TokenUsage,
   final_result: Option<serde_json::Value>, stdout: String }`.
4. `MontySession::new(code, thread, persisted_state, max_duration_override)
   -> Result<Self, EngineError>` — extracts `execute_orchestrator`'s setup
   (:541-602: `build_orchestrator_inputs` + `MontyRun::new` + `start` + the
   `classify_orchestrator_failure`/`orchestrator_vm_panic` error mapping +
   `ResourceLimits`/`LimitedTracker`).
5. `MontySession::drive_to_yield(&mut self, deps: &OrchestratorDeps<'_>,
   thread: &mut Thread, signal_rx: &mut SignalReceiver, new_input:
   Option<MontyObject>) -> Result<OrchestratorYield, EngineError>` — the
   extracted dispatch loop (:604-957). On entry: if `self.parked_call` is
   `Some`, resume it with `new_input` (the await_next_turn result) and loop;
   else loop on `self.progress`. Add a private `enum DispatchOutcome {
   Resume(ExtFunctionResult), AwaitNextTurn }` so the `host.await_next_turn()`
   arm can park: it returns `DispatchOutcome::AwaitNextTurn`, and the outer
   code stores `self.parked_call = Some(call); return Ok(AwaitNextTurn)` (the
   suspended `call` is retained, NOT dropped — true persistence). `Complete` →
   `Ok(Complete(OrchestratorResult{...}))`. Add `FunctionCall` to the
   `monty` import (:38).
6. Refactor `execute_orchestrator` to a thin delegation: `MontySession::new`
   → `drive_to_yield(deps, thread, signal_rx, None)` → `Complete(r) => Ok(r)`,
   `AwaitNextTurn => Err(EngineError::Orchestrator(classify_orchestrator_
   failure("Orchestrator parked awaiting next turn", "host.await_next_turn()
   in non-persistent mode")))`. **Signature unchanged** → `loop_engine.rs`
   untouched.
7. Unit test (slice 1): a mock script `host.await_next_turn()` then
   `FINAL({"outcome":"completed","response":"done","state":{}})` driven via
   `MontySession` with `MockCompositionPort`/`MockKohaiPort`-style deps —
   first `drive_to_yield(None)` → `AwaitNextTurn`; second `drive_to_yield(
   Some("next"))` → `Complete`. Plus a FINAL-only script → `Complete` on the
   first drive.

**Edit plan:** (a) add `FunctionCall` to the `monty` import; (b) insert
`OrchestratorYield`+`OrchestratorDeps`+`MontySession`+`DispatchOutcome`+
`MontySession::new`+`drive_to_yield` immediately before `execute_orchestrator`;
(c) replace `execute_orchestrator`'s body (setup + loop, :541-957) with the
thin delegation. Verify: `cargo clippy -p brassclaw_engine --all-targets -- -D
warnings` (both configs) + `cargo test -p brassclaw_engine --lib orchestrator::`.
Commit + push.

## Out of scope (explicit)
- The Monty VM (`scripting.rs`), `Store`/`Thread`, `EffectExecutor`/`LlmBackend`
  traits, intent system, recipe/component stores, validators, formatters,
  `types/recipe.rs`, first-party Rust capability handlers — all kept.
- C.7 deletes `execute_orchestrator` (once the session supersedes it) +
  `default.py` + `ExecutionLoop`/`ThreadManager`/`brassclaw_engine::runtime` +
  updates the stale doc-comments at orchestrator.rs :1946/:2011/:2220/:2240/
  :2298/:3154.
- The future MCP bridge — future work.

## Slice 1 result (SHIPPED `d26d08b7`, 2026-09-04)

Realized with one deviation from the realization detail above: **no
`OrchestratorDeps<'a>` struct was needed.** Instead `MontySession::drive_to_yield`
takes the SAME parameter list as `execute_orchestrator` (minus
`code`/`persisted_state`/`max_duration_override`, plus `new_input:
Option<MontyObject>`). The arm bodies are therefore **byte-identical** to the old
`execute_orchestrator` loop except two `self.` substitutions
(`final_result = Some(val)` → `self.final_result = Some(val)`; `&mut total_tokens`
→ `&mut self.total_tokens`). This avoided the risky ~20-arm `deps.*` rewiring.

- `pub enum OrchestratorYield { Complete(Box<OrchestratorResult>), AwaitNextTurn }`
  (boxed `Complete` to satisfy `large_enum_variant`).
- `pub struct MontySession { progress, parked_call: Option<FunctionCall<LimitedTracker>>, total_tokens, final_result, stdout }` + `MontySession::new` (extracts setup) + `drive_to_yield` (the dispatch loop; on entry resumes a parked `await_next_turn` call with `new_input`; the `host.await_next_turn()` arm stores `self.parked_call = Some(call)` and returns `Ok(AwaitNextTurn)` — the suspended call is retained, true cross-turn persistence).
- `execute_orchestrator` is now a thin delegation: `MontySession::new` → `drive_to_yield(..., None)` → `Complete(r) => Ok(*r)`, `AwaitNextTurn => Err(Orchestrator(...))`. **Signature unchanged** → `loop_engine.rs` untouched.
- 2 unit tests (`monty_session_drives_final_only_to_complete` +
  `monty_session_parks_on_await_next_turn_then_resumes`) + 2 test helpers
  (`session_host_deps`, `session_fresh_thread`). Gates: engine clippy clean
  (default + skills-db), 566 default / 577 skills-db lib tests pass, 95
  orchestrator-module tests pass both configs. Diff scoped to
  `crates/brassclaw_engine/src/executor/orchestrator.rs`.

## Status

[x] slice 1 — engine `MontySession` + park/resume primitive (SHIPPED `d26d08b7`).
[ ] slice 2 — rework `basic_mode.py` into a resumable long-running loop.
[ ] slice 3 — conversation-keyed Monty session registry.
[ ] slice 4 — `TurnRunnerWorker` direct path (bypass driver_registry).
[ ] slice 5 — retire `canonical.rs` stage pipeline + reuse stage logic as host fns.
[ ] slice 6 — both configs clippy + tests + mark C.6 done. Then C.7.
