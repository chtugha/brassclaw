# Subplan — Problem Step H.12 of `saved_plan_to_v3.md`

## Origin / why this subplan exists

H.12 wires the **production** `OrchestratorLookup` bridge so the agent-loop
Tier-0 path (`RecipeStep::TierZero` → `TierZeroExecutionStage` →
`LoopOrchestratorPort::run_tier_zero`) actually executes the recipe's
PythonCode, and the Tier-1 path prepends assembled prior knowledge to the LLM
prompt (`run_step_zero`). H.6/H.7 delivered the turns-level DTOs + trait;
H.8 extracted the engine `pub` fns (`assemble_prior_knowledge_with_hint` +
`execute_tier_zero_channel`); H.9–H.11 delivered the agent-loop dispatch +
`TierZeroExecutionStage`. H.12 is the production composition bridge.

### The blocking architectural fact (grounded 2026-08-27)

The engine Monty-sandbox runtime that `execute_tier_zero_channel` drives is
**not live in production**:

- `ThreadManager` / `ExecutionLoop` are constructed **only in engine tests**
  (`runtime/manager.rs` test helpers, `runtime/conversation.rs` tests). No
  production product crate builds them.
- `brassclaw_reborn` has **zero** references to `ThreadManager` /
  `ExecutionLoop` / `execute_orchestrator` / `brassclaw_engine::runtime`.
- `brassclaw_reborn_composition` references `brassclaw_engine` only for memory
  stores + validators (`PostgresSource`, recipe/component stores,
  `ComponentValidator`) — **not** the execution runtime.
- The engine traits `EffectExecutor` (`traits/effect.rs:122`) and `LlmBackend`
  (`traits/llm.rs:48`) have **only test impls** (`MockEffects`, `NoopEffects`,
  `MockLlm`, `CapturingLlm`, … — all `#[cfg(test)]`). **No production impl of
  either exists anywhere in the workspace.**
- `SessionThreadService` returns a `SessionThreadRecord`, **not** an engine
  `Thread`; there is no production engine `Thread` loader reachable from the
  composition layer.
- The engine formatting fns `format_orchestrator_content`,
  `assemble_pkr_from_items`, `parse_orchestrator_channel_steps` are
  **private**; composition cannot call them directly.

So H.12 is not "wire a bridge" — it **activates the engine Monty VM for Tier-0
in production composition**: build production `EffectExecutor` + `LlmBackend`
impls, construct the runtime, and wire the `OrchestratorLookup` impl.

### User decisions (locked, 2026-08-27)

- **Q-H12-1 (defer vs implement):** Implement H.12 now (revert the earlier
  defer). *(earlier "defer" decision reverted by user.)*
- **Q-H12-2 (mechanism):** **A — activate the engine Monty VM in production.**
  Construct the engine sandbox deps in the composition layer (sourcing
  capabilities from the already-wired production capability factory) and call
  `execute_tier_zero_channel`. Rejected the host_runtime Docker detour
  (user: "isn't any orchestrator instance a sandbox on its own anyway?").
- **Q-H12-3 (principle, user):** "No PythonVM can call tools directly — every
  PythonVM needs to execute tools over the Rust execution layer." → the Monty
  VM's `__execute_action__` → `EffectExecutor` (Rust) is the correct model; the
  production `EffectExecutor` adapter MUST dispatch tools to the production
  Rust capability layer.
- **Formatter visibility (decided by implementer):** make
  `format_orchestrator_content` / `assemble_pkr_from_items` /
  `parse_orchestrator_channel_steps` `pub` in `brassclaw_engine` so the
  composition bridge can format stashed `recipe_hint` (serialized
  `Vec<ComponentItem>`) → `orchestrator_content` prose and parse it without
  duplicating private logic. (Minimal engine change; consistent with H.8's
  "extract engine `pub` fns" philosophy.)

## Target architecture (facade A)

```
agent_loop RecipeStage ──TierZero──► TierZeroExecutionStage
                                          │ ctx.host.orchestrator_lookup()
                                          ▼
                            composition OrchestratorLookup impl
                                          │  (maps LoopRunContext → Thread,
                                          │   DTOs engine→turns)
                                          ▼
                       Arc<brassclaw_engine::TierZeroOrchestrator>  (new facade)
                                          │  run_tier_zero(thread, recipe_hint, rust_ctx)
                                          │  assemble_prior_knowledge(thread, …, recipe_hint)
                                          ▼
              execute_tier_zero_channel / assemble_prior_knowledge_with_hint  (engine pub fns, H.8)
                                          │  Monty VM execute_code per PythonCode step
                                          ▼
                          EffectExecutor adapter  ──►  production CapabilityDispatcher (Rust tools)
                          LlmBackend guard        (Tier-0 never calls; errors → degrade to Tier-2)
```

Tier-1: `RecipeStage::Continue` (stashed `recipe_hint`) → agent-loop `PromptStage`
→ planner copies `state.recipe_hint` into `LoopPromptBundleRequest.recipe_hint`
→ reborn `build_prompt_bundle` calls `orchestrator_lookup.run_step_zero(...)`
→ injects `PriorKnowledgeBundle.orchestrator_content` as a system message
prepended to the LLM prompt.

## Steps (one-by-one, commit+push each, `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` on every build; `df -h` the target first — `cargo clean` if Avail<15GB or >90%; selective-pathspec commit guard never staging user WIP)

### H.12.1 — Engine: make the formatting/parse fns `pub` ✅ DONE (commit `7d97f25b`)

`crates/brassclaw_engine/src/executor/orchestrator.rs`: change
`fn format_orchestrator_content` → `pub fn`, `fn assemble_pkr_from_items` →
`pub fn`, `fn parse_orchestrator_channel_steps` → `pub fn` (and any helpers
they need). Add `#[allow(dead_code)]` only if callers are composition-only and
not yet wired (remove once wired). Verify `cargo clippy -p brassclaw_engine
--all-targets -- -D warnings` (both default + `--features skills-db`) stays
clean. Unit tests unchanged. **Needs:** nothing. **Touches:** engine
orchestrator.rs. **Result:** composition can format/parse the
`orchestrator_content` prose from stashed items.

### H.12.2 — Composition: production `EffectExecutor` adapter

> **H.12.2 spawned a nested subplan.** Grounding the production capability
> dispatch (research subagent 2026-08-27) confirmed this is a genuine multi-step
> build with 8 risk points: the adapter must bridge the engine
> `EffectExecutor`/`CapabilityLease`/`ThreadExecutionContext` contract to the
> production `HostRuntime::invoke_capability` + `visible_capabilities` façade,
> build a composition-owned production `ExecutionContext` factory (the engine
> ctx lacks tenant/agent/grants/mounts/trust), and add a validated
> `action_name`→`CapabilityId` registry. **User decision Q-H12-2-GATE (locked):
> choice A — interim non-resumable gates** (`ApprovalRequired`/`AuthRequired`/
> `ResourceBlocked` → `Err(EngineError::Effect)` → Tier-2 degrade; full
> resumable gate-bridging is a documented future phase, NOT stubbed). Full
> grounding + H.12.2.1–H.12.2.7 sequence + verification in
> `./subplan_problem_stepH12_2_of_saved_plan_to_v3.md` (Zenflow nested
> sub-substep under the H.12 substep `d401cc45`). Execute H.12.2.1→H.12.2.7
> one-by-one before resuming the H.12 subplan at H.12.3.
>
> ✅ **H.12.2 COMPLETE.** All seven sub-steps (H.12.2.1 factory, H.12.2.2 action
> registry, H.12.2.3 `ProductionEffectExecutor::dispatch_action`, H.12.2.4
> `available_actions`/`available_capabilities`, H.12.2.5 `TierZeroEffectExecutorBuilder`
> + wiring into `LocalDevCapabilityWiring` for both local-dev and pure-PG paths
> with `skills-db` gating, H.12.2.6 tests incl. 2 new `build_for_run` integration
> tests, H.12.2.7 verification) are done. Verified: fmt + clippy clean (default +
> skills-db); tests 671 default / 678 skills-db. Resuming the H.12 subplan at H.12.3.

New `crates/brassclaw_reborn_composition/src/orchestrator_effect_executor.rs`
implementing `brassclaw_engine::traits::EffectExecutor` over the production
capability dispatch. `execute_action(action_name, params, lease, ctx)` maps
`action_name` → production capability id and dispatches via the wired
`CapabilityDispatcher` / capability factory (`local_dev_capabilities.
capability_factory` from `runtime.rs`); maps production result → engine
`ActionResult`. `available_actions` / `available_capabilities` enumerate the
visible capability surface for the turn scope. Lease handling: the adapter
grants/uses a `CapabilityLease` via the engine `LeaseManager` (H.12.4 supplies
it) or maps to the production grant path — decide at implementation by reading
the production capability dispatch API (ground it first; if the mapping is
large, write a nested `subplan_problem_stepH12_2_of_saved_plan_to_v3.md`).
**Needs:** H.12.1 not strictly required; the production capability dispatch
API (ground it). **Touches:** new composition module + `lib.rs` mod + runtime
wiring (H.12.4). **Result:** the engine Monty VM's `__execute_action__` reaches
the production Rust capability layer (honors Q-H12-3).

### H.12.3 — Composition: Tier-0 `LlmBackend` guard

> ✅ **DONE.** New composition module `crates/brassclaw_reborn_composition/
> src/tier_zero_llm_guard.rs` (declared `mod tier_zero_llm_guard;` in `lib.rs`)
> implementing `brassclaw_engine::LlmBackend` for `pub(crate) struct
> TierZeroLlmGuard` (unit, `Copy + Default`): `complete(..)` →
> `Err(EngineError::InvalidInput { reason: "Tier-0 channel does not call the
> LLM" })`; `model_name()` → `"tier-zero-guard"`. This is the real semantic
> guard — a mis-compiled Tier-0 recipe that reaches for the LLM surfaces as
> `InvalidInput` and degrades to Tier-2 (per H.11), not a silent stub.
> `#![allow(dead_code)]` module-wide (only constructed under `skills-db` in
> H.12.4 wiring); 3 unit tests run under both configs. Verified: fmt + clippy
> clean (default + skills-db); composition lib tests 674 default / 681
> skills-db (+3 guard tests each).

New module (or fold into H.12.4) implementing `brassclaw_engine::traits::
LlmBackend`: `complete(..)` returns `Err(EngineError::InvalidInput { reason:
"Tier-0 channel does not call the LLM" })`; `model_name()` returns
`"tier-zero-guard"`. This is a **real semantic guard**, not a stub/simulation:
Tier-0 recipes are deterministic (no `__llm_complete__`); if a recipe wrongly
calls the LLM, the guard surfaces it and `execute_tier_zero_channel` degrades
to Tier-2 (per H.11 `TierZeroStep::Degrade`). If later a real LLM is needed
for Tier-0, swap for a model-gateway adapter (out of H.12 scope).
**Needs:** nothing. **Touches:** new composition module + `lib.rs` mod.
**Result:** `execute_tier_zero_channel`'s `llm` param is satisfied for Tier-0.

### H.12.4 — Engine: pub `TierZeroOrchestrator` facade + composition construction

1. `brassclaw_engine/src/executor/orchestrator.rs` (or a new
   `executor/tier_zero_orchestrator.rs`): `pub struct TierZeroOrchestrator {
   llm: Arc<dyn LlmBackend>, effects: Arc<dyn EffectExecutor>, leases:
   Arc<LeaseManager>, policy: Arc<PolicyEngine>, gate_controller:
   Arc<dyn GateController>, event_tx: Option<broadcast::Sender<ThreadEvent>>,
   retrieval_source: Option<Arc<dyn RetrievalSource>> }` + a `builder()`.
   Methods:
   - `pub async fn run_tier_zero(&self, thread: &Thread, recipe_hint: &Value,
     recipe_rust_context: &Value) -> Result<TierZeroChannelResult, EngineError>`
     — deserialize `recipe_hint` → `Vec<ComponentItem>` →
     `assemble_pkr_from_items` → `orchestrator_content` →
     `execute_tier_zero_channel(thread, &orchestrator_content,
     recipe_rust_context, &self.effects, &self.leases, &self.policy,
     &self.gate_controller, &self.llm, self.event_tx.as_ref())`.
   - `pub async fn assemble_prior_knowledge(&self, thread: &Thread, goal: &str,
     token_budget: usize, sender_class: &str, recipe_hint: Option<Value>) ->
     Result<PkrAssemblyResult, EngineError>` — wraps
     `assemble_prior_knowledge_with_hint(thread, goal, token_budget,
     sender_class, self.retrieval_source.as_ref(), recipe_hint)`.
   Re-export from `brassclaw_engine`.
2. Composition `runtime.rs`: at the wiring site (near `retrieval_lookup`
   construction, ~line 2545), construct `Arc<TierZeroOrchestrator>` using
   `LeaseManager::new()`, the `PolicyEngine` built from
   `local_dev_capability_policy`, `CancellingGateController::arc()` (or the
   production gate if one is wired), the H.12.2 `EffectExecutor` adapter, the
   H.12.3 `LlmBackend` guard, a fresh `broadcast::channel`, and the
   `PostgresSource` already built as `retrieval_source`. Add an
   `orchestrator_runtime: Option<Arc<TierZeroOrchestrator>>` field to
   `DefaultPlannedRuntimeParts` and pass it through.
**Needs:** H.12.1 (pub fns), H.12.2 (EffectExecutor), H.12.3 (LlmBackend).
**Touches:** engine facade module; composition runtime.rs +
DefaultPlannedRuntimeParts. **Result:** a constructed Tier-0 runtime handle in
the composition layer.

### H.12.5 — Composition: `OrchestratorLookup` impl + Thread loader + host wiring

New `crates/brassclaw_reborn_composition/src/orchestrator_lookup_impl.rs`:
`struct PgOrchestratorLookup { runtime: Arc<TierZeroOrchestrator>,
thread_loader: Arc<dyn ThreadLoader> }` implementing
`brassclaw_turns::run_profile::OrchestratorLookup`:
- `run_step_zero(context, recipe_hint)`: load `Thread` from
  `context.thread_id` via the loader (Tier-1 `recipe_hint` is `Some` → engine
  Some-branch, no fresh fetch); call `runtime.assemble_prior_knowledge(..)`;
  map `PkrAssemblyResult` → `PriorKnowledgeBundle` (`orchestrator_content`,
  `matched_component_ids`, `override_prompt_creation`). `None` on loader
  miss / engine error (degrade-gracefully, mirror `PgRetrievalLookup`).
- `run_tier_zero(context, recipe_hint, recipe_rust_context)`: load `Thread`;
  call `runtime.run_tier_zero(..)`; map `TierZeroChannelResult` →
  `TierZeroReply` (`formatted_output`→`text`, `matched_component_ids`). `None`
  on miss/error.
**Thread loader:** the engine `Store::load_thread(thread_id) -> Option<Thread>`
is the canonical loader, but no production engine `Store` is wired in
composition. Ground whether a production engine `Store` exists / can be
constructed (e.g. an in-memory or PG-backed `Store` impl). If none exists,
write a nested `subplan_problem_stepH12_5_of_saved_plan_to_v3.md` to add a
production `Store` impl (or a thin `ThreadLoader` that builds a minimal
`Thread` from `LoopRunContext.scope` — `tenant_id`/`user_id`/`agent_id`/
`project_id`/`thread_id` — sufficient for `execute_tier_zero_channel`, which
uses `thread.id` + `thread_execution_context(thread, ..)`). Decide at
implementation after grounding `Thread`'s required fields for the Tier-0 path.
**Host wiring:** add `orchestrator_lookup: Option<Arc<dyn OrchestratorLookup>>`
to `DefaultPlannedRuntimeParts`; in `brassclaw_reborn::runtime.rs`
`build_default_planned_runtime`, call
`host_factory = host_factory.with_orchestrator_lookup(lookup)` (the
`with_orchestrator_lookup` setter already exists at
`loop_driver_host.rs:1408`). The slot currently defaults to `None`
(`loop_driver_host.rs:1067`) → `NoOrchestrator` → TierZero degrades to LLM
when the feature is off.
**Needs:** H.12.4. **Touches:** new composition module + lib.rs mod; composition
runtime.rs; `DefaultPlannedRuntimeParts`; `brassclaw_reborn/src/runtime.rs`
(the `with_orchestrator_lookup` install call). **Result:** production host
wires a real `OrchestratorLookup`; `run_tier_zero`/`run_step_zero` reach the
engine Monty VM.

### H.12.6 — Agent-loop + reborn: Tier-1 prompt injection (`build_prompt_bundle` reads `recipe_hint`)

1. `crates/brassclaw_agent_loop/src/strategies/context.rs` (production default
   strategy, ~line 278) + `planning_context.rs` (~line 97): set
   `recipe_hint: state.recipe_hint.clone()` instead of `None` (the planner has
   `&LoopExecutionState`). Also update the test-only `default_planner.rs:316`
   if needed.
2. Reborn `build_prompt_bundle` (`loop_driver_host.rs:2427` delegates to the
   `HostManagedLoopPromptPort`): when `request.recipe_hint` is `Some`, call
   `self.orchestrator_lookup.run_step_zero(&context, request.recipe_hint
   .as_ref())` and, on `Some(bundle)`, inject
   `bundle.orchestrator_content` as a prepended system message (or via the
   `inline_messages` / instruction-snippet path) into the `LoopPromptBundle`.
   Honor `override_prompt_creation` (replace vs prepend). The prompt port
   needs access to the host's `orchestrator_lookup` slot — wire it through
   `HostManagedLoopPromptPort` construction (`loop_driver_host.rs:1692`) or
   handle the injection in `RebornLoopDriverHost::build_prompt_bundle` before
   delegating. Decide at implementation; if the injection point is non-trivial
   (the prompt port is turns-layer and cannot see the orchestrator bridge),
   write a nested subplan.
**Needs:** H.12.5 (the wired `orchestrator_lookup`). **Touches:** agent-loop
strategies; reborn loop_driver_host.rs prompt path. **Result:** Tier-1 turns
prepend assembled prior knowledge to the LLM prompt.

### H.12.7 — Tests + final verification

- Unit tests: `PgOrchestratorLookup` mapping (Pkr→Bundle, TierZeroChannel→Reply)
  with a stub `TierZeroOrchestrator` or a test facade; `EffectExecutor` adapter
  dispatch (mock `CapabilityDispatcher` capturing every arg — AGENTS.md testing
  rule); `LlmBackend` guard errors; Tier-1 prompt injection (planner copies
  `recipe_hint`; `build_prompt_bundle` prepends content when `recipe_hint`
  present, skips when `None`/`override`).
- Integration (skip-if-no-Docker locally): a Tier-0 recipe (wilson≥0.70,
  llm_call_required=false, single PythonCode constant step) drives the agent
  loop via the composition test path → asserts the channel runs (or, absent
  a production Monty VM, degrades gracefully to Tier-2 without breaking the
  turn).
- Final verification: `cargo fmt` (all touched crates); `cargo clippy -p
  <crate> --all-targets -- -D warnings` for each touched crate; `cargo
  clippy --all --benches --tests --examples --all-features -- -D warnings`;
  `cargo test -p brassclaw_engine`, `-p brassclaw_reborn_composition`,
  `-p brassclaw_agent_loop`, `-p brassclaw_reborn` (both default +
  `--features skills-db` where the crate has the feature); composition
  `cargo check` clean.
- Mark H.12 ✅ DONE in this subplan + `saved_plan_to_v3.md`; then proceed to
  H.13 (Phase H final verification + mark Phase H Zenflow steps `9d94d6cb` /
  `1a0a9eac` Completed).

## Notes / risks

- The `EffectExecutor` adapter (H.12.2) and the Thread loader (H.12.5) are the
  two genuinely uncertain pieces. Each may spawn a nested subplan per the task
  rules — do NOT stub them.
- `execute_tier_zero_channel` broadcasts `ThreadEvent`s via `event_tx` and uses
  `thread.id`; the constructed `Thread` must carry the real `thread_id` from
  `LoopRunContext` so events correlate.
- Keep the diff scoped: do NOT touch the user's pre-existing WIP (product
  workflow, webui_v2, V063 basic_prompt_store, prefix-cache, etc.) —
  selective-pathspec commits only.
- The dormant Monty VM being activated here may surface pre-existing
  half-written engine code (the "written half-way then silenced" anti-pattern
  the task calls out). If so, resolve it (implement the real functionality),
  do not silence it; if large, nested subplan.
