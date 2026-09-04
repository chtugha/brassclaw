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

## Out of scope (explicit)
- The Monty VM (`scripting.rs`), `Store`/`Thread`, `EffectExecutor`/`LlmBackend`
  traits, intent system, recipe/component stores, validators, formatters,
  `types/recipe.rs`, first-party Rust capability handlers — all kept.
- C.7 deletes `execute_orchestrator` (once the session supersedes it) +
  `default.py` + `ExecutionLoop`/`ThreadManager`/`brassclaw_engine::runtime` +
  updates the stale doc-comments at orchestrator.rs :1946/:2011/:2220/:2240/
  :2298/:3154.
- The future MCP bridge — future work.

## Status

[ ] slice 1 — engine `MontySession` + park/resume primitive.
[ ] slice 2 — rework `basic_mode.py` into a resumable long-running loop.
[ ] slice 3 — conversation-keyed Monty session registry.
[ ] slice 4 — `TurnRunnerWorker` direct path (bypass driver_registry).
[ ] slice 5 — retire `canonical.rs` stage pipeline + reuse stage logic as host fns.
[ ] slice 6 — both configs clippy + tests + mark C.6 done. Then C.7.
