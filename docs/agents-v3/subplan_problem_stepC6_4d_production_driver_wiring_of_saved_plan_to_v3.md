# Subplan — Step C.6 slice 4d: Production driver wiring

> Parent: `./subplan_problem_stepC6_production_driver_switch_of_saved_plan_to_v3.md`
> slice 4 (lines 118-123). Sequenced after slice 4c (SHIPPED `f2083d0d`,
> 2026-09-04: `PersistentMontyDriver` + `SignalBroker` in composition, impl
> `MontyTurnDriverPort`, 8 unit tests, both configs green) and before slice 5
> (retire `canonical.rs`).

## Scope of slice 4 (recap)

Wire the slice-4c `PersistentMontyDriver` into the **turns production path** so a
real user turn drives a cross-turn-persistent `MontySession` running
`basic_mode.py`, bypassing `driver_registry` (C6-1=B). Slices 4a/4b/4c-prep/4c
landed the primitives (`prepare_monty_session`, `MontyTurnDriverPort` trait,
`try_checkout`, `SignalBroker`, the driver struct + tests). **4d = the actual
production wire-up.**

Sub-slices:
- **4d-1** — `TurnRunnerWorker` direct path: hold `Arc<dyn
  MontyTurnDriverPort>` + call `drive_turn` for Monty turns (bypass
  `driver_registry`), map `LoopExit::Completed` via the existing applier.
- **4d-2** — thread `monty_driver` through `DefaultPlannedRuntimeParts`
  (`crates/brassclaw_reborn/src/runtime.rs:113`, ~35 fields, no slot today).
- **4d-3** — composition constructs `PersistentMontyDriver` + injects into the
  parts (composition `runtime.rs:2679` call site). **BLOCKED on the fork below.**
- **4d-4** — `SignalForwardingCoordinator` wrapper so `cancel_run` (which carries
  `request.scope`) forwards `ThreadSignal::Stop` into the `SignalBroker`.
- **4d-5** — both configs clippy-clean + unit tests + commit + push.

## Grounding (verified live source, 2026-09-04)

### Turns worker + driver registry
- `TurnRunnerWorker` (`crates/brassclaw_reborn/src/turn_runner.rs:218`); `new`
  (229) takes `(config, transition_port, loop_exit_applier, driver_registry,
  host_factory, wake_receiver)`. `try_claim_and_run` → `execute_claimed_run`
  (367) → `invoke_driver` (434) resolves `driver_registry.get(&registry_key)`
  → `driver.resume(...)`/`driver.run(...)`, under `catch_unwind` + heartbeat +
  `cancel` CancellationToken + `max_turn_duration` budget. `brassclaw_reborn`
  depends on `brassclaw_turns` (Cargo.toml:31) → worker CAN hold
  `Arc<dyn MontyTurnDriverPort>`.
- C6-1=B: add the direct path. `driver_registry` lookup is skipped for Monty
  turns; the worker calls `self.monty_driver.drive_turn(context)` directly.

### Runtime assembly
- `brassclaw_reborn::build_default_planned_runtime(DefaultPlannedRuntimeParts{...})`
  (`runtime.rs:347`/420); composition calls it at `runtime.rs:2679`.
  `DefaultPlannedRuntimeParts` (`runtime.rs:113`) has ~35 fields; **no
  `monty_driver` slot today** (4d-2 adds it).

### Signal-flow reality (CRITICAL)
- The OLD signal path runs through `ThreadManager`
  (`engine runtime/manager.rs:27` `signal_tx`, `signal_channel(32)` at :392;
  `stop`→`ThreadSignal::Stop` :545; `inject_message`→`InjectMessage` :571) —
  **dormant in production** (composition `build_reborn_runtime` does NOT
  construct `ThreadManager`).
- The **turns** `TurnRunnerWorker` has NO in-turn signal source — only a
  worker-level wake signal + `CancellationToken` + heartbeat.
- `TurnCoordinator` (`turns coordinator.rs:94`) exposes only
  `submit_turn`/`resume_turn`/`cancel_run`/`get_run_state` — **no
  suspend/inject** (those are turn-submission concepts).
- `CancelRunRequest` **carries `request.scope`** (`coordinator.rs:320`);
  `DefaultTurnCoordinator::cancel_run` (319) calls `self.store.request_cancel` +
  `wake_notifier`. → the cancel→broker wire is clean + composition-local (no
  run_id→scope resolution needed).

### Fork 1 (signal forwarding) — LOCKED A
- **A — full signal forwarding now.** `SignalBroker` (slice 4c) already supports
  Suspend/Inject; 4d-4 adds a `SignalForwardingCoordinator` wrapper so
  `cancel_run` → `SignalBroker::send(scope, Stop)`. Matches the consistent
  anti-over-engineering stance (no new turns APIs).
- A2 (add turns suspend/inject APIs) dismissed — too large, not needed now.

## Fork 2 (CRITICAL — BLOCKS 4d-3): how the production Monty gets the LLM

### The mismatch (verified)
- `PersistentMontyDriver` (slice 4c) holds raw `Arc<dyn LlmBackend>` +
  `Arc<dyn EffectExecutor>` + `Arc<LeaseManager>` + `Arc<PolicyEngine>` +
  `Arc<dyn GateController>` + `component_port` + `kohai_port` + `event_tx` +
  `dynamic_tools` to call `MontySession::drive_to_yield`
  (`orchestrator.rs:633`, 14 deps after the 4c-prep `pg_pool` drop).
- The orchestrator's LLM arms use `deps.llm.complete(&messages, &actions,
  &config)` (`orchestrator.rs:1212`) → a **real** `Arc<dyn LlmBackend>` is
  required for production Monty (the non-match + assemble-prior paths call it).
- **Composition's production runtime does NOT hold a real `Arc<dyn LlmBackend>`:**
  - The only `LlmBackend` constructed is `TierZeroLlmGuard`
    (`runtime.rs:2594`, `Arc::new(TierZeroLlmGuard::new()) as Arc<dyn LlmBackend>`)
    — an **always-erroring** guard wired into `TierZeroOrchestrator::builder()`
    at `runtime.rs:2595`. The Tier-0 channel via `PgOrchestratorLookup` is
    deliberately LLM-blocked (deterministic recipe execution).
  - The **real** production LLM is a `HostManagedModelGateway`
    (`brassclaw_loop_support`, `runtime.rs:713`/`local_dev.rs:139`) — a
    **different trait**.
- **No adapter exists** (grep confirmed 2026-09-04): the only `impl LlmBackend`
  are test stubs (`StubLlm`/`CapturingLlm`/`ModelCapturingLlm`/
  `PromptCapturingLlm`). There is no `LlmBackend`-over-`HostManagedModelGateway`
  adapter.
- Trait shapes differ:
  - `LlmBackend::complete(&self, &[(ThreadMessage)], &[ActionDef], &LlmCallConfig)
    -> Result<LlmOutput, EngineError>`; `fn model_name(&self) -> &str`.
  - `HostManagedModelGateway::stream_model(&self, HostManagedModelRequest) ->
    Result<HostManagedModelResponse, HostManagedModelError>`.
  - An adapter must translate messages/actions/config → request and parse
    response → `LlmOutput` (text or action calls). Non-trivial but bounded.

### TierZeroOrchestrator encapsulation
- `TierZeroOrchestrator` (`engine tier_zero_orchestrator.rs:49`) holds
  `llm`/`leases`/`policy`/`gate_controller`/`event_tx`/`retrieval_source`
  **internally**; built via `TierZeroOrchestrator::builder()` at composition
  `runtime.rs:2595`. `PgOrchestratorLookup` (`orchestrator_lookup_impl.rs:85`)
  holds `Arc<TierZeroOrchestrator>` + `thread_store` +
  `executor_builder: Arc<TierZeroEffectExecutorBuilder>` — NOT raw Arcs.
- `run_tier_zero` (161): `load_thread` → `executor_builder.build_for_run(...)`
  → per-run `effects` → `runtime.run_tier_zero(&thread, &effects, ...)`.
- So `LeaseManager`/`PolicyEngine`/`GateController` default-construct INSIDE the
  builder; `EffectExecutor` is per-run via `executor_builder`. The raw Arcs ARE
  available at assembly (passed into the builder) but are NOT held separately.

### The options

**(α) Adapter in composition; engine arms unchanged.**
Build `GatewayLlmBackend` (`impl LlmBackend` over `Arc<dyn
HostManagedModelGateway>` — translate ThreadMessage/ActionDef/LlmCallConfig →
HostManagedModelRequest, parse HostManagedModelResponse → LlmOutput). Construct
it in composition from the real gateway. Construct/extract `leases`+`policy`+
`gate_controller` Arcs separately (share with the TierZero builder). Pass all
raw Arcs to `PersistentMontyDriver`. **Composition keeps owning the driver +
registry (C6-2=B intact).** New code: adapter component + Arc-extraction
plumbing. Engine orchestrator arms UNCHANGED.

**(β) Re-plumb the orchestrator LLM to the production gateway directly.**
Swap the engine orchestrator's LLM abstraction from `Arc<dyn LlmBackend>` to
`Arc<dyn HostManagedModelGateway>` for the `drive_to_yield` path + the 2 LLM arms
(`host.non_match_llm_answer`, `host.assemble_prior_knowledge`). No adapter, no
guard. The orchestrator uses the SAME LLM path as the rest of production (one
LLM path — most architecturally consistent). `PersistentMontyDriver` holds the
gateway Arc (not `LlmBackend`). Bigger engine change (the arms' `complete(...)`
call sites at `orchestrator.rs:1212` etc. re-plumb to `stream_model(...)` + the
request/response translation moves INTO the arms). Still needs the raw
`leases`/`policy`/`gate_controller` Arcs extracted for the other `drive_to_yield`
deps.

**(γ) Engine-facade drive (move the persistent loop into `TierZeroOrchestrator`).**
Add `TierZeroOrchestrator::drive_turn_persistent(thread, effects, ..., <llm or
gateway>)` that owns the park/resume `drive_to_yield` loop internally (engine
owns the raw Arcs — no extraction). `PersistentMontyDriver` in composition
becomes a thin wrapper holding `Arc<TierZeroOrchestrator>` + `executor_builder`
+ `thread_store` (mirrors `PgOrchestratorLookup`). The LLM gap REMAINS unless
combined with α (pass the adapter as the facade's llm, replacing the guard) or
β (facade takes the gateway). γ is a LOCATION decision, orthogonal to α/β which
are LLM-source decisions. γ+α = facade-driven with adapter; γ+β = facade-driven
with gateway.

### Decision needed
- **LLM source:** α (adapter, minimal engine change, preserves C6-2=B) vs β
  (re-plumb arms to gateway, one-LLM-path consistency, bigger engine change).
- **Drive-loop location:** composition-side `PersistentMontyDriver` (slice 4c
  as built) vs engine-side `TierZeroOrchestrator::drive_turn_persistent` (γ).

The user has consistently owned architecture decisions of this magnitude. **No
4d code written until this fork is resolved.**

## Out of scope (explicit)
- C.7 deletions (`execute_orchestrator` / `default.py` / `ExecutionLoop` /
  `ThreadManager` / `brassclaw_engine::runtime`) — separate step.
- Local e2e (C6-4=C: CI/Docker only).
- The future MCP bridge.
