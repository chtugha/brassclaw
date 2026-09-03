# 12 — Agent Loop / Turn Pipeline

> **Subsystem:** The agent loop — the **unit of agentic execution** that owns
> turn sequencing, planning, tool dispatch, approval gates, checkpointing,
> retries, and completion. Per AGENTS.md, a loop is the thing products wire
> together; product code must not implement a second loop. The framework lives
> in `crates/brassclaw_agent_loop` as an ordered **canonical executor pipeline**
> of stages (`DefaultExecutorPipeline`), driven by a host that implements the
> `AgentLoopDriverHost` supertrait (**15 ports** today). **The canonical
> pipeline IS the production turn driver** — the turns trusted runner calls
> `AgentLoopDriver::run` → `PlannedDriver::run` →
> `CanonicalAgentLoopExecutor::execute_family` → `DefaultExecutorPipeline::execute`.
> The host is `RebornLoopDriverHost` (brassclaw_reborn), which wires the LLM,
> retrieval (`PgRetrievalLookup`→`PostgresSource`), orchestrator
> (`PgOrchestratorLookup`→`TierZeroOrchestrator`), capability, checkpoint, and
> interceptor ports. The **engine** `ExecutionLoop::run` → `execute_orchestrator`
> Monty-VM path is **dormant in production** (no external caller constructs
> `ThreadManager`); it is activated as the composed-program runner in C.5/C.6.
> **Grounded in:** `crates/brassclaw_agent_loop/src/executor/{canonical,pipeline,
> recipe,tier_zero}.rs`, `crates/brassclaw_agent_loop/{CLAUDE.md,AGENTS.md}`,
> `crates/brassclaw_turns/src/run_profile/{driver,host,orchestrator_lookup,
> retrieval_lookup}.rs`, `crates/brassclaw_reborn/src/{planned_driver,
> loop_driver_host}.rs`, `crates/brassclaw_engine/src/executor/{loop_engine,
> tier_zero_orchestrator}.rs`, `saved_plan_to_v3.md` Phase H / H.9–H.12.

## 1. Purpose

The agent loop is the reusable framework that runs one agentic turn after
another until the run completes. AGENTS.md's architecture model splits
BrassClaw into Products / Loops / Kernel; **loops own agent behavior** and
"manage planning, tool dispatch, turn sequencing, approval gates,
checkpointing, retries, and completion." A product wires a loop together; it
must not implement a second loop or bypass the loop runner.

The framework's shape is a **canonical executor pipeline**: an ordered list
of stages that each take a typed input, produce a typed `Step` output, and
hand the next whole state to the next stage. `CanonicalAgentLoopExecutor` is
the public facade; the stage types and `DefaultExecutorPipeline` are
crate-internal.

> **Production wiring.** `PlannedDriver` (brassclaw_reborn) implements the
> turns `AgentLoopDriver` trait and holds an `Arc<CanonicalAgentLoopExecutor>`
> + an opaque `LoopFamily`; its `run`/`resume` call
> `executor.execute_family(family, host, state)`. The turns trusted runner
> invokes `PlannedDriver` per turn via `AgentLoopDriverHost` (the 15-port
> supertrait). So the canonical pipeline is **not** a skeleton — it is the
> live turn driver. The engine Monty VM (`ExecutionLoop::run` →
> `execute_orchestrator`) is the **dormant** path that C.5/C.6 activates as
> the composed-program runner (`host.run_program`); it is not "production
> default.py" (default.py was retired in C.1).

## 2. Location

### Framework crate (`brassclaw_agent_loop`)

- `src/executor/canonical.rs` — `DefaultExecutorPipeline::execute`: the
  ordered lifecycle spine (the loop body). The H.10 branch on
  `RecipeStep::{Continue, TierZero}` lives here.
- `src/executor/pipeline.rs` — `DefaultExecutorPipeline` struct (the stage
  fields), `StageContext { planner, host }`, the `ExecutorStage<Input>` trait.
- `src/executor/` — one file per stage: `budget.rs`, `input.rs`, `recipe.rs`,
  `tier_zero.rs` (`TierZeroExecutionStage` — Tier-0 deterministic channel),
  `prompt.rs`, `interceptor.rs`, `model.rs`, `reply_admission.rs`,
  `assistant_reply.rs`, `capabilities.rs`, `turn_stop.rs` (`StopStage`),
  `loop_exit.rs` (`ExitStage`), `checkpoint.rs` (`CheckpointStage` — inline
  helper, not a pipeline field), `gates.rs`, `mapping.rs`,
  `capability_helpers.rs`, `exit_helpers.rs`.
- `src/state.rs` + `src/state/` — `LoopExecutionState` (resumable state: refs,
  cursors, counters, versions, safe summaries, `last_user_text`, `recipe_hint`,
  `recipe_rust_context` stash slots — never raw prompts, raw model output, tool
  args, or secrets).
- `src/planner.rs` / `default_planner.rs` / `strategies/` — the sealed planner
  composition + one decision axis per file.
- `src/family.rs` + `src/families/` — `LoopFamily`, `LoopFamilyId`, registry
  rules, built-in family factories.
- `CLAUDE.md` / `AGENTS.md` — crate-local guardrails (depends upward on
  `brassclaw_turns` only; must NOT depend on `brassclaw_reborn`, host-runtime
  crates, product adapters, dispatcher, capability host, filesystem, network,
  secrets, or DB backends; the framework never sees `AgentLoopDriver`;
  `PlannedDriver` adapts runner-facing driver calls to this executor).

### Host ports + driver contract (`brassclaw_turns`)

- `src/run_profile/driver.rs` — `AgentLoopDriver` trait (`run`/`resume` →
  `LoopExit`), `AgentLoopDriverRunRequest`/`ResumeRequest`, `AgentLoopDriverError`.
- `src/run_profile/host.rs` — `AgentLoopDriverHost` supertrait (line 2292):
  **15 ports** — `LoopRunInfoPort`, `LoopContextPort`, `LoopPromptPort`,
  `LoopInputPort`, `LoopModelPort`, `LoopCapabilityPort`, `LoopTranscriptPort`,
  `LoopCheckpointPort`, `LoopProgressPort`, `LoopCompactionPort`,
  `LoopCancellationPort`, `LoopRecipePort`, `LoopRetrievalPort`,
  `LoopOrchestratorPort`, `LoopInterceptorPort` (+ blanket impl). The v3
  additions are `LoopRetrievalPort` (→ `RetrievalLookup`, `NoRetrieval` default)
  and `LoopOrchestratorPort` (→ `OrchestratorLookup`, `NoOrchestrator` default),
  mirroring the `LoopRecipePort`/`NoRecipeLookup` opt-in pattern.
- `src/run_profile/orchestrator_lookup.rs` — `OrchestratorLookup` trait
  (`run_step_zero` Tier-1 / `run_tier_zero` Tier-0), `PriorKnowledgeBundle`,
  `TierZeroReply` (turns-native; the step-0 `tier_zero` branch that previously
  embodied this logic was removed when the host-port bridge replaced it).
- `src/run_profile/retrieval_lookup.rs` — `RetrievalLookup` trait (bridges
  stages to engine `PostgresSource::fetch_for_turn`).

### Production driver + host (`brassclaw_reborn`)

- `src/planned_driver.rs` — `PlannedDriver` (impl `AgentLoopDriver`; holds
  `Arc<CanonicalAgentLoopExecutor>` + `Arc<LoopFamily>`; `run`/`resume` →
  `executor.execute_family`).
- `src/loop_driver_host.rs` — `RebornLoopDriverHost` (impl `AgentLoopDriverHost`
  15 ports; wires `Option<Arc<dyn OrchestratorLookup>>` +
  `Option<Arc<dyn RetrievalLookup>>` from composition).
- `src/driver_registry.rs` — `DriverRegistry` selects the driver per run-profile.

### Engine (dormant in prod)

- `crates/brassclaw_engine/src/executor/loop_engine.rs` — `ExecutionLoop::run`
  (line 456) → `execute_orchestrator` (line 514, the Monty VM path). **No
  external caller constructs `ThreadManager` in production**; this path is
  dormant and is activated as the composed-program runner in C.5/C.6.
- `crates/brassclaw_engine/src/executor/tier_zero_orchestrator.rs` —
  `TierZeroOrchestrator::run_tier_zero` → `execute_tier_zero_channel` (the
  Rust deterministic Tier-0 channel; reached in production via the turns
  `PgOrchestratorLookup` bridge, NOT via the dormant engine `ExecutionLoop`).

## 3. Data model

### `DefaultExecutorPipeline` (the stages)

```rust
pub(crate) struct DefaultExecutorPipeline {
    pub(crate) budget: BudgetStage,
    pub(crate) input: InputStage,
    pub(crate) recipe: RecipeStage,
    pub(crate) tier_zero: TierZeroExecutionStage,   // v3 H.10 — Tier-0 channel
    pub(crate) prompt: PromptStage,
    pub(crate) interceptor: InterceptorStage,
    pub(crate) model: ModelStage,
    pub(crate) reply_admission: ReplyAdmissionStage,
    pub(crate) assistant_reply: AssistantReplyStage,
    pub(crate) capabilities: CapabilityStage,
    pub(crate) stop: StopStage,
    pub(crate) exit: ExitStage,
}
```

(`CheckpointStage` is an inline helper invoked at fixed points, not a field.)
Each stage implements `ExecutorStage<Input>` (`process -> Output`).
`StageContext { planner, host }` — the only two things a stage can reach; `host`
is the 15-port supertrait.

### `RecipeStep` (two variants — real dispatch, not a stub)

```rust
pub(super) enum RecipeStep {
    /// Tier 1/2 fall-through: no match, below threshold, or LLM-required
    /// (recipe_hint stays stashed for run_step_zero). → PromptStage/ModelStage.
    Continue { state: Box<LoopExecutionState> },
    /// Tier 0 — deterministic orchestrator-channel execution, NO LLM.
    /// → TierZeroExecutionStage → LoopOrchestratorPort::run_tier_zero.
    TierZero { state: Box<LoopExecutionState> },
}
```

### `TierZeroStep` (tier_zero.rs)

```rust
pub(super) enum TierZeroStep {
    Reply { state: Box<LoopExecutionState>, reply: ..., matched_component_ids: ... },
    Degrade { state: Box<LoopExecutionState> },   // fall through to prompt/model
}
```

### `LoopExecutionState`

Resumable state: iteration counter, input cursor, `pending_input_ack`,
stop-state, strategy slots (typed, never `serde_json::Value`), `last_user_text`
(populated by `InputStage::drain` via `LoopContextPort::resolve_message_text`),
`recipe_hint` / `recipe_rust_context` (the H.9 retrieval stash — orchestrator_items
+ rust_items). CLAUDE.md forbids storing raw prompts, raw model output, tool
args, secrets, host paths, provider errors, or stack traces.

### `AgentLoopDriverHost` ports (today's 15)

| Port | Role |
|---|---|
| `LoopRunInfoPort` | run identity, iteration, family |
| `LoopContextPort` | `load_loop_context` + `resolve_message_text` (raw user text) |
| `LoopPromptPort` | prompt bundle assembly (host-resolved under scope/policy) |
| `LoopInputPort` | user/follow-up/steering input drain |
| `LoopModelPort` | the LLM call |
| `LoopCapabilityPort` | capability surface + dispatch |
| `LoopTranscriptPort` | transcript append |
| `LoopCheckpointPort` | checkpoint read/write |
| `LoopProgressPort` | progress events |
| `LoopCompactionPort` | context compaction |
| `LoopCancellationPort` | cancel signals |
| `LoopRecipePort` | `recipe_lookup() -> Option<&dyn RecipeLookup>` (opt-in) |
| `LoopRetrievalPort` | `retrieval_lookup() -> Option<&dyn RetrievalLookup>` (v3, opt-in) |
| `LoopOrchestratorPort` | `orchestrator_lookup() -> Option<&dyn OrchestratorLookup>` (v3, opt-in) |
| `LoopInterceptorPort` | Sempai-Kohai interceptor hook (see `09-sempai-kohai.md`) |

## 4. Behavior

### 4.1 The canonical loop (`canonical.rs::execute`)

`DefaultExecutorPipeline::execute(family, host, state) -> LoopExit` runs an
`'outer: loop { … }` whose body is the ordered stage sequence:

1. **Cancel check** — `CheckpointStage.cancel_if_requested` → `Continue`/`Exit`.
2. **Budget** — `budget.process` → `BudgetStep::Continue`/`Exit`.
3. **Progress** — `CheckpointStage.emit_progress(IterationStarted)`.
4. **Input (steering)** — `input.process(DrainInput { mode: Steering })` →
   `InputStep::Continue`/`Exit`. `InputStage::drain` populates
   `state.last_user_text` via `LoopContextPort::resolve_message_text`.
5. **Recipe (H.10 branch)** — `recipe.process(RecipeInput { state })`:
   - `RecipeStep::Continue` → fall through to prompt/model (Tier 1/2).
   - `RecipeStep::TierZero` → `tier_zero.process`:
     - `TierZeroStep::Reply` → ack pending input, **skip PromptStage/ModelStage**,
       go straight to `assistant_reply.process` (step 13) → `break 'turn`.
     - `TierZeroStep::Degrade` → fall through to prompt/model.
   The whole recipe→LLM region is a `'turn` block whose value is the
   `TurnCompletedStep` handed to the shared stop/exit tail.
6. **Prompt** — `prompt.process` → `PromptStep::Prepared(prompt)`/`Exit`. Caches
   `prompt_message_count`.
7. **Interceptor** — `interceptor.process(InterceptorPromptInput { … })` →
   packet id + intercepted/adjusted messages. (Routing state = no-op; rerouting
   state = Sempai review — see `09-sempai-kohai.md`.)
8. **Checkpoint (BeforeModel)** — `CheckpointStage.process(CheckpointKind::BeforeModel)`.
9. **Ack pending input** — `pending_input_ack.ack(host)`.
10. **Model** — `model.process(ModelInput { … })` → `ModelStep::Response` /
    `RetryIteration` / `Exit`.
11. **Close interceptor packet** — `notify_interceptor_kohai_response(packet_id, …)`.
12. **Usage EMA** — if `input_tokens > 0`, feed
    `ctx.planner.context().notify_model_usage(input_tokens, prompt_message_count)`.
13. **Turn completion** — branch on `model_response.output`:
    - `AssistantReply(reply)` → `reply_admission.process` → on `Accept`:
      `assistant_reply.process`; on `Reject`: `Continue { summary: reply_rejected() }`.
    - `CapabilityCalls(calls)` → `capabilities.process(CapabilityInput { … })`.
14. **Stop observe** — `stop.observe` → `Continue`/`Exit`.
15. **Follow-up drain (reply-only turns)** — if `completed_kind == ReplyOnly`:
    `input.process(DrainInput { mode: FollowUp })`; if `drained`, bump iteration
    and `continue`.
16. **Stop decide** — `stop.decide` → `Stop` (→ step 17) / `Continue` / `Exit`.
17. **Exit** — `exit.process(ExitInput { state, kind })` → `LoopExit`; final `ack`.

The `iteration` counter is bumped at the bottom of the body and after a
follow-up `continue`.

### 4.2 Stage ownership rules (CLAUDE.md)

- `canonical.rs` is the ordered lifecycle spine only — branch logic lives in
  the owning stage module.
- `CanonicalAgentLoopExecutor` is the public facade; `DefaultExecutorPipeline`
  and stage types stay crate-internal.
- Sibling stages are never passed through another stage's input; helpers stay
  owned inside their stage module.
- Cancellation, checkpoint, and pending-input-ack ordering stay explicit at the
  stage boundary that owns the transition.

### 4.3 `RecipeStage` — real H.9 retrieval + H.10 dispatch (not a stub)

`recipe.rs` implements the intended three-tier dispatch and **is wired live**:

- **SEC-02:** the recipe stash (`recipe_hint` / `recipe_rust_context`) is
  cleared at the **start** of every `RecipeStage::process`, so a turn resumed
  from a checkpoint never replays a stale pre-fetched result.
- When a `RetrievalLookup` is wired (`LoopRetrievalPort`) **and**
  `last_user_text` is present, the stage fires `fetch_for_turn` against the
  engine `PostgresSource` (running `resolve_intent` + the SEC-01-gated component
  fetch in a live turn) and stashes the plan-literal split into
  `state.recipe_hint` (the `orchestrator_items`) and `state.recipe_rust_context`
  (the `rust_items` array as `Vec<Value>`).
- It returns `RecipeStep::TierZero` when the result crosses the Wilson
  lower-confidence threshold and needs no LLM, else `RecipeStep::Continue`
  (Tier 1/2). Retrieval errors are **soft-failed** (debug-logged, stash left
  empty) — a retrieval failure must never break a turn.
- **Tier 0** — `TierZeroExecutionStage` → `LoopOrchestratorPort::run_tier_zero`
  → `OrchestratorLookup` → `PgOrchestratorLookup` →
  `TierZeroOrchestrator::run_tier_zero` → `execute_tier_zero_channel` (Rust
  deterministic; no LLM round-trip). Emits the reply directly.
- **Tier 1** — guided: the stashed `recipe_hint` is consumed by
  `OrchestratorLookup::run_step_zero` to inject matched ToolSkill/prior-knowledge
  summaries into the prompt so the LLM follows the proven pattern.
- **Tier 2** — full LLM: no match → the prompt/model/capability pipeline runs
  unchanged.

### 4.4 The two engine paths (only one is live)

- **Canonical pipeline (LIVE production driver).** turns trusted runner →
  `PlannedDriver::run` → `CanonicalAgentLoopExecutor::execute_family` →
  `DefaultExecutorPipeline::execute`. `ModelStage` owns the LLM call via
  `LoopModelPort`; Tier-0 runs via `LoopOrchestratorPort`. **No Python on this
  path today.**
- **Engine Monty VM (DORMANT).** `ExecutionLoop::run` (`loop_engine.rs:456`) →
  `execute_orchestrator` (`:514`) runs a composed Python orchestrator program in
  the Monty VM. No external caller constructs `ThreadManager` in production
  (`build_reborn_runtime` does not), so this path is not exercised today. C.5/C.6
  activate it as the composed-program runner (`host.run_program`) — the
  Orchestrator/Executioner split where Monty is the brain that drives the Rust
  executioner via `host.<tool>(...)` calls. Until then, the canonical Rust
  pipeline + `TierZeroOrchestrator` deterministic channel is the live authority.

## 5. Relations

- **Recipe system** (`03-recipe-system.md`) — `RecipeStage` is the dispatch
  point; the stashed `recipe_hint`/`recipe_rust_context` feed Tier-0/Tier-1.
- **Retrieval** (`11-retrieval-system.md`) — `LoopRetrievalPort` →
  `PgRetrievalLookup` → `PostgresSource::fetch_for_turn` bridges `RecipeStage`
  to live retrieval (turns-native `RetrievalLookup`; engine types stay
  engine-side).
- **Orchestrator** (`13-orchestrator.md`) — `LoopOrchestratorPort` →
  `PgOrchestratorLookup` → `TierZeroOrchestrator::{run_tier_zero,
  assemble_prior_knowledge}` bridges stages to the deterministic channel
  (Tier-0) and prior-knowledge injection (Tier-1). The retired `default.py`/
  `__llm_complete__`/`__assemble_prior_knowledge__` framing is gone; the
  composed-program runner (`host.run_program`) is the C.5/C.6 activation.
- **Sempai-Kohai** (`09-sempai-kohai.md`) — `LoopInterceptorPort` +
  `InterceptorStage` (step 7/11) is the chokepoint.
- **Prefix/base-prompt** (`10-prefix-base-prompt.md`) — `PromptStage` assembles
  the per-turn bundle; the base prompt is pre-assembled via
  `SystemBundleSource::get_system_bundle` (`do_assemble_bundle`).
- **Intent system** (`02-intent-system.md`) — drives the retrieval that feeds
  `RecipeStage` (`resolve_intent` inside `PostgresSource::fetch_for_turn`).
- **Kernel/composition** (`16-kernel-composition.md`) — `RebornLoopDriverHost`
  is where composition wires the 15 ports (LLM, retrieval, orchestrator,
  capability, checkpoint, …) into the loop.

## 6. Status — shipped vs. pending

| Aspect | Shipped | Pending |
|---|---|---|
| Production turn driver | canonical pipeline via `PlannedDriver` (turns → `execute_family`) | — |
| `DefaultExecutorPipeline` stages | incl. `tier_zero` (`TierZeroExecutionStage`) | — |
| `RecipeStage` | real H.9 retrieval + H.10 `Continue`/`TierZero` dispatch (not a stub) | — |
| Host ports | 15 (`LoopRetrievalPort` + `LoopOrchestratorPort` added in v3) | — |
| Tier-0 deterministic channel | `TierZeroExecutionStage` → `PgOrchestratorLookup` → `TierZeroOrchestrator::run_tier_zero` → `execute_tier_zero_channel` (active) | — |
| Tier-1 guided injection | `run_step_zero` prior-knowledge bundle | — |
| Engine Monty VM driver | `ExecutionLoop::run` → `execute_orchestrator` exists but **dormant** | **activated C.5/C.6** (`host.run_program` composed-program runner) |
| `RamSource` / engine `ThreadManager` | dormant in prod | deleted Phase K.3 |

## 7. LLM summary (for prompt injection)

The agent loop is the unit of agentic execution — turn sequencing, planning,
tool dispatch, approval gates, checkpointing, retries, completion. The framework
(`brassclaw_agent_loop`) is an ordered canonical stage pipeline
(`DefaultExecutorPipeline`: budget → input → recipe → tier_zero → prompt →
interceptor → model → reply_admission → assistant_reply → capabilities → stop →
exit; `CheckpointStage` is an inline helper). It IS the production turn driver:
the turns trusted runner calls `AgentLoopDriver::run` → `PlannedDriver::run` →
`CanonicalAgentLoopExecutor::execute_family` → `DefaultExecutorPipeline::execute`,
with `RebornLoopDriverHost` supplying the 15-port `AgentLoopDriverHost`
supertrait (LLM, retrieval, orchestrator, capability, checkpoint, interceptor,
…). `RecipeStage` is wired live: it clears the recipe stash (SEC-02), runs
`fetch_for_turn` against `PostgresSource` (intent-driven, SEC-01-gated) when
retrieval is wired, stashes `recipe_hint`/`recipe_rust_context`, and dispatches
`TierZero` (no-LLM deterministic channel via `TierZeroExecutionStage` →
`LoopOrchestratorPort::run_tier_zero` → `TierZeroOrchestrator`) or `Continue`
(Tier-1 guided injection / Tier-2 full LLM). The engine Monty VM path
(`ExecutionLoop::run` → `execute_orchestrator`) is dormant in production and is
activated as the composed-program runner (`host.run_program`) in C.5/C.6 — the
Orchestrator/Executioner split where Monty drives the Rust executioner via
`host.<tool>(...)`. `RamSource` + the engine `ThreadManager` are deleted in
Phase K.3.
