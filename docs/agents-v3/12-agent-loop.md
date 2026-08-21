# 12 — Agent Loop / Turn Pipeline

> **Subsystem:** The agent loop — the **unit of agentic execution** that owns
> turn sequencing, planning, tool dispatch, approval gates, checkpointing,
> retries, and completion. Per AGENTS.md, a loop is the thing products wire
> together; product code must not implement a second loop. The framework lives
> in `crates/brassclaw_agent_loop` as an ordered **canonical executor
> pipeline** of stages (`DefaultExecutorPipeline`), driven by a host that
> implements the `AgentLoopDriverHost` supertrait (13 ports today). The
> **critical** fact for the v3 plan: this pipeline is a **skeleton today** —
> production turns are driven by the *engine* `ExecutionLoop::run`, which runs
> the Python orchestrator (`default.py`) directly with **no stage pipeline**.
> Closing that gap (the `DRIVER-GAP`) and activating the `RecipeStage` stub
> (Tier 0/1/2 dispatch) is the substance of Phase H.
> **Grounded in:** `crates/brassclaw_agent_loop/src/executor/canonical.rs`
> (the loop), `pipeline.rs` (stage struct), `recipe.rs` (the stub),
> `crates/brassclaw_agent_loop/{CLAUDE.md,AGENTS.md}`, `crates/brassclaw_turns/
> src/run_profile/host.rs` (the 13 ports + blanket impl at 2185-2220),
> `crates/brassclaw_engine/src/executor/loop_engine.rs` (the engine driver),
> `saved_plan_to_v3.md` Phase H / H.0 (lines 4365-4615) + DRIVER-GAP/TIER0-GAP
> (lines 228-229).

## 1. Purpose

The agent loop is the reusable framework that runs one agentic turn after
another until the run completes. AGENTS.md's architecture model splits
BrassClaw into Products / Loops / Kernel; **loops own agent behavior** and
"manage planning, tool dispatch, turn sequencing, approval gates,
checkpointing, retries, and completion." A product wires a loop together;
it must not implement a second loop or bypass the loop runner.

The framework's shape is a **canonical executor pipeline**: an ordered list
of stages (`CheckpointStage`, `BudgetStage`, `InputStage`, `RecipeStage`,
`PromptStage`, `InterceptorStage`, `ModelStage`, `ReplyAdmissionStage` /
`AssistantReplyStage` / `CapabilityStage`, `StopStage`, `ExitStage`) that each
take a typed input, produce a typed `Step` output (`Continue` / `Exit`), and
hand the next whole state to the next stage. `CanonicalAgentLoopExecutor` is
the public facade; the stage types and `DefaultExecutorPipeline` are
crate-internal.

> **Why this matters for the v3 plan.** The new subsystems (intent system,
> recipes, IBS, Tier 0) are designed to live as *stages* or *host ports* in
> this pipeline — most visibly `RecipeStage` (Tier 0/1/2 dispatch) and the
> new `LoopRetrievalPort` / `LoopOrchestratorPort`. But the pipeline is not
> the production driver yet. Phase H is what activates `RecipeStage` and
> Phase H.0 adds the two host ports that let stages reach `brassclaw_engine`.
> The gap between "skeleton pipeline" and "production driver" is the
> `DRIVER-GAP`, the single largest architectural fact in the v3 transition.

## 2. Location

### Framework crate (`brassclaw_agent_loop`)

- `src/executor/canonical.rs` — `DefaultExecutorPipeline::execute`: the
  ordered lifecycle spine (the loop body). Calls each stage's `process`.
- `src/executor/pipeline.rs` — `DefaultExecutorPipeline` struct (the stage
  fields), `StageContext { planner, host }`, the `ExecutorStage<Input>` trait
  (`process -> Output`).
- `src/executor/` — one file per stage:
  `checkpoint.rs`, `budget.rs`, `input.rs`, `recipe.rs`, `prompt.rs`,
  `interceptor.rs`, `model.rs`, `reply_admission.rs`, `assistant_reply.rs`,
  `capabilities.rs`, `turn_stop.rs` (`StopStage`), `loop_exit.rs`
  (`ExitStage`), `gates.rs`, `mapping.rs`, `capability_helpers.rs`,
  `exit_helpers.rs`.
- `src/state.rs` + `src/state/` — `LoopExecutionState` (resumable state:
  refs, cursors, counters, versions, safe summaries only — never raw
  prompts, raw model output, tool args, or secrets).
- `src/planner.rs` / `default_planner.rs` / `strategies/` — the sealed
  planner composition + one decision axis per file (context budget,
  progress detection, capability focus, …). `ctx.planner.context()` etc.
- `src/family.rs` + `src/families/` — `LoopFamily`, `LoopFamilyId`, registry
  rules, built-in family factories.
- `CLAUDE.md` / `AGENTS.md` — the crate-local guardrails (boundary rules:
  depends upward on `brassclaw_turns` only; must NOT depend on
  `brassclaw_reborn`, host-runtime crates, product adapters, dispatcher,
  capability host, filesystem, network, secrets, or DB backends; the
  framework never sees `AgentLoopDriver`; `PlannedDriver` in
  `brassclaw_reborn` adapts runner-facing driver calls to this executor).

### Host ports (`brassclaw_turns`)

- `crates/brassclaw_turns/src/run_profile/host.rs`
  - `pub trait AgentLoopDriverHost:` (line 2185) — the supertrait listing
    the **13 ports** today:
    `LoopRunInfoPort`, `LoopContextPort`, `LoopPromptPort`, `LoopInputPort`,
    `LoopModelPort`, `LoopCapabilityPort`, `LoopTranscriptPort`,
    `LoopCheckpointPort`, `LoopProgressPort`, `LoopCompactionPort`,
    `LoopCancellationPort`, `LoopRecipePort`, `LoopInterceptorPort`
    (lines 2186-2198) `+ Send + Sync`.
  - The blanket `impl<T> AgentLoopDriverHost for T where T: … + Send + Sync`
    (lines 2204-2220) — must be updated in lockstep when ports are added.
  - Existing opt-in pattern: `LoopRecipePort::recipe_lookup() ->
    Option<&dyn RecipeLookup>` (line 2081-2093) with a `NoRecipeLookup`
    default — the template the two new ports copy.

### Engine driver (`brassclaw_engine`)

- `crates/brassclaw_engine/src/executor/loop_engine.rs` — `ExecutionLoop::run`
  (line ~413), the **production turn driver**; `ExecutionLoop` builder
  (`.with_retrieval()`, `.with_store()`, `.with_retrieval_source()`,
  `.with_pg_pool()`, `.with_max_duration_secs()`, `.with_event_tx()`).
- `crates/brassclaw_engine/src/executor/orchestrator.rs` —
  `execute_orchestrator` (line 444, `pub async fn`) — runs the **entire**
  Python orchestrator (`default.py`) as a full Monty VM execution; the
  private `handle_assemble_prior_knowledge` (line 2552) and the `__llm_complete__`
  dispatch (line 563 → defined 795).
- `crates/brassclaw_engine/src/runtime/manager.rs` — `ThreadManager::spawn`
  builds the `ExecutionLoop` (see `11-retrieval-system.md` §2).

## 3. Data model

### `DefaultExecutorPipeline` (the stages)

```rust
pub(crate) struct DefaultExecutorPipeline {
    pub(crate) budget: BudgetStage,
    pub(crate) input: InputStage,
    pub(crate) recipe: RecipeStage,
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

(`CheckpointStage` is invoked as a helper at fixed points, not a pipeline
field — see §4.) Each stage implements:

```rust
#[async_trait]
pub(crate) trait ExecutorStage<Input>: Send + Sync {
    type Output;
    async fn process(&self, ctx: StageContext<'_>, input: Input)
        -> Result<Self::Output, AgentLoopExecutorError>;
}
```

`StageContext { planner: &'a dyn AgentLoopPlannerInternal, host: &'a (dyn
AgentLoopDriverHost + Send + Sync) }` — the only two things a stage can
reach. `host` is the 13-port supertrait; stages call ports like
`ctx.host.recipe_lookup()`.

### `RecipeStep` (today: the sole variant)

```rust
pub(super) enum RecipeStep {
    /// Fall through to `prompt` (Tier 2): either no recipe matched or
    /// the matched recipe is below the Wilson threshold.
    Continue { state: Box<LoopExecutionState> },
}
```

`RecipeStage::process` always returns `Continue` today (see §4.4).

### `LoopExecutionState`

Resumable state carried turn-to-turn: iteration counter, input cursor,
`pending_input_ack`, stop-state, strategy slots (typed, never
`serde_json::Value`), `recipe_hint` / `recipe_rust_context` (v3 stash slots).
CLAUDE.md forbids storing raw prompts, raw model output, tool args,
secrets, host paths, provider errors, or stack traces in state.

### `AgentLoopDriverHost` ports (today's 13)

| Port | Role |
|---|---|
| `LoopRunInfoPort` | run identity, iteration, family |
| `LoopContextPort` | `load_loop_context` (host.rs:779) — the *only* method today; v3 adds `resolve_message_text` |
| `LoopPromptPort` | prompt bundle assembly (`content_ref`, host-resolved under scope/policy) |
| `LoopInputPort` | user/follow-up/steering input drain |
| `LoopModelPort` | the LLM call |
| `LoopCapabilityPort` | capability surface + dispatch |
| `LoopTranscriptPort` | transcript append |
| `LoopCheckpointPort` | checkpoint read/write |
| `LoopProgressPort` | progress events |
| `LoopCompactionPort` | context compaction |
| `LoopCancellationPort` | cancel signals |
| `LoopRecipePort` | `recipe_lookup() -> Option<&dyn RecipeLookup>` (opt-in, `NoRecipeLookup` default) |
| `LoopInterceptorPort` | Sempai-Kohai interceptor hook (see `09-sempai-kohai.md`) |

## 4. Behavior

### 4.1 The canonical loop (`canonical.rs::execute`)

`DefaultExecutorPipeline::execute(family, host, state) -> LoopExit` runs a
`loop { … }` whose body is the ordered stage sequence:

1. **Cancel check** — `CheckpointStage.cancel_if_requested(ctx, state)` →
   `Continue` or `Exit`.
2. **Budget** — `budget.process(BudgetInput { state, pending_input_ack })` →
   `BudgetStep::Continue { state, pending_input_ack }` or `Exit`.
3. **Progress** — `CheckpointStage.emit_progress(IterationStarted)`.
4. **Input (steering)** — `input.process(DrainInput { …, mode: Steering })` →
   `InputStep::Continue { state, pending_input_ack, drained }` or `Exit`.
5. **Recipe** — `recipe.process(RecipeInput { state })` → `RecipeStep::Continue`
   (always, today; see §4.4).
6. **Prompt** — `prompt.process(PromptInput { state, pending_input_ack })` →
   `PromptStep::Prepared(prompt)` or `Exit`. Caches `prompt_message_count`.
7. **Interceptor** — `interceptor.process(InterceptorPromptInput { state,
   messages, capability_surface_version, visible_capability_count })` →
   packet id + intercepted/adjusted messages. (Routing state = no-op;
   rerouting state = Sempai review — see `09-sempai-kohai.md`.)
8. **Checkpoint (BeforeModel)** — `CheckpointStage.process(CheckpointKind::BeforeModel)`.
9. **Ack pending input** — `pending_input_ack.ack(host)`.
10. **Model** — `model.process(ModelInput { state, messages, surface_version,
    capability_view, resolved_messages })` → `ModelStep::Response(next,
    response)` / `ModelStep::RetryIteration(next)` (continue, reset packet id)
    / `Exit`.
11. **Close interceptor packet** — `notify_interceptor_kohai_response(packet_id,
    response_text, usage)` (no-op in routing state).
12. **Usage EMA** — if the model reported `input_tokens > 0`, feed
    `ctx.planner.context().notify_model_usage(input_tokens, prompt_message_count)`.
13. **Turn completion** — branch on `model_response.output`:
    - `AssistantReply(reply)` → `reply_admission.process` → on `Accept`:
      `assistant_reply.process(AssistantReplyInput { state, reply, usage })`;
      on `Reject`: `TurnCompletedStep::Continue { summary: reply_rejected() }`.
    - `CapabilityCalls(calls)` → `capabilities.process(CapabilityInput {
      state, surface, calls })`.
14. **Stop observe** — `stop.observe(StopObservationInput { state, summary })` →
    `Continue` or `Exit`.
15. **Follow-up drain (reply-only turns)** — if `completed_kind == ReplyOnly`:
    `input.process(DrainInput { …, mode: FollowUp })`; if `drained`, bump
    iteration and `continue` (loop back to the top).
16. **Stop decide** — `stop.decide(StopInput { state, summary,
    pending_input_ack })` → `StopStep::Stop { state, kind, ack }` (→ step 17)
    / `StopStep::Continue` (→ bump iteration, loop) / `StopStep::Exit`.
17. **Exit** — `exit.process(ExitInput { state, kind })` → `LoopExit`; final
    `ack.ack(host)`.

The `iteration` counter is bumped at the bottom of the body (`state.iteration
= state.iteration.saturating_add(1)`) and after a follow-up `continue`.

### 4.2 Stage ownership rules (CLAUDE.md)

- `canonical.rs` is the ordered lifecycle spine only — branch logic lives in
  the owning stage module, not in `canonical.rs`.
- `CanonicalAgentLoopExecutor` is the public facade; `DefaultExecutorPipeline`
  and stage types stay crate-internal.
- Sibling stages are never passed through another stage's input; helpers
  stay owned inside their stage module.
- No stages for pure mapping helpers or one-line wrappers.
- Cancellation, checkpoint, and pending-input-ack ordering stay explicit at
  the stage boundary that owns the transition.

### 4.3 The two execution models (DRIVER-GAP)

The codebase has **two** turn drivers that the plan was, for a time, silently
mixing:

- **Model A — engine path (current production).** `ExecutionLoop::run`
  (`loop_engine.rs:413`) calls `execute_orchestrator` (Python `default.py`)
  directly. **Python (Monty) is the outer loop** and calls the LLM itself via
  `__llm_complete__` (`default.py` → `handle_llm_complete` at
  `orchestrator.rs:795`). There is **no stage pipeline** on this path.
- **Model B — agent-loop path (target).** `DefaultExecutorPipeline::execute`
  (`canonical.rs`) — the stage pipeline above; `ModelStage` owns the LLM call,
  no Python.

The agent-loop pipeline is a **skeleton**: `DefaultExecutorPipeline` /
`execute_family` appear **only** inside `brassclaw_agent_loop` (canonical.rs,
pipeline.rs, tests). No product surface drives it. `brassclaw_agent_loop` does
**not** depend on `brassclaw_engine` (Cargo.toml), and
`__assemble_prior_knowledge__` exists **only** in `brassclaw_engine`. So the
plan's `RecipeStage`↔Python-step-0 stash/unstash assumes a unification that
does not exist today. `brassclaw_reborn_composition` is the **only** crate
that depends on both, so it is the only place that can bridge them.

### 4.4 `RecipeStage` is a stub (Tier 0/1/2 not implemented)

`recipe.rs` documents the intended three-tier dispatch:

- **Tier 0 — direct execution**: a Recipe crosses the Wilson
  lower-confidence threshold → the host performs the action chain with **no
  LLM round-trip**.
- **Tier 1 — guided execution**: a Recipe matches but is below the
  threshold → inject matched ToolSkill summaries into the prompt so the LLM
  follows the proven pattern.
- **Tier 2 — full LLM reasoning**: no Recipe matches → fall through to the
  existing prompt/model/capability pipeline unchanged.

But `RecipeStage::process` **never calls `find_recipe` / `find_skills`**. It
logs "library wired but user text unavailable at this pipeline position" (or
"no library wired") and always returns `RecipeStep::Continue`. The module's
"structural debt" note explains why: the user's full text is **not available
at this pipeline position** — `LoopExecutionState` carries no
`last_user_text`, and `LoopInput::UserMessage { message_ref }` holds an
opaque ref, not text. Resolving this needs either (1) a cached
`last_user_text` field populated by `InputStage`, or (2) moving the stage
between `PromptStage` and `ModelStage`. Phase H.0 picks option 1 + a host
port to resolve the ref to raw text.

## 5. Relations

- **Recipe system** (`03-recipe-system.md`) — `RecipeStage` is the dispatch
  point; `LoopRecipePort::recipe_lookup` is the existing opt-in pattern the
  new ports copy.
- **Retrieval** (`11-retrieval-system.md`) — v3 `LoopRetrievalPort::fetch_for_turn`
  bridges `RecipeStage` to `PostgresSource::fetch_for_turn` (engine types stay
  engine-side; `RetrievalTurnResult` is `brassclaw_turns`-native).
- **Orchestrator** (`13-orchestrator-default-py.md`) — v3
  `LoopOrchestratorPort::{run_step_zero, run_tier_zero}` bridges stages to the
  engine Python orchestrator (Tier 1 step-0 / Tier 0 no-LLM channel).
- **Sempai-Kohai** (`09-sempai-kohai.md`) — `LoopInterceptorPort` +
  `InterceptorStage` (step 7/11) is the chokepoint; the `base-prompt`
  substitution is Phase K.1.
- **Prefix/base-prompt** (`10-prefix-base-prompt.md`) — `PromptStage` is where
  the `base-prompt` placeholder line is inserted during composition (v3).
- **Intent system** (`02-intent-system.md`) — drives the retrieval that
  feeds `RecipeStage` via `LoopRetrievalPort`.
- **Kernel/composition** (`16-kernel-composition.md`) —
  `brassclaw_reborn_composition` is the sole crate implementing the new
  host ports (it depends on both `brassclaw_engine` and
  `brassclaw_agent_loop`).

## 6. Today vs. v3

| Aspect | Today | v3 (Phase H / H.0) |
|---|---|---|
| Production turn driver | `ExecutionLoop::run` → `execute_orchestrator` (Python is the outer loop; no stage pipeline) — Model A | agent-loop `DefaultExecutorPipeline::execute` becomes the driver (Model B/C); `ModelStage` owns the LLM call; Python `__llm_complete__` retired to step-0 + Tier-0 only |
| `DefaultExecutorPipeline` callers | **skeleton** — only inside `brassclaw_agent_loop` + tests; no product surface drives it | production driver (after `DRIVER-PREREQ` switchover); both mechanisms coexist during migration |
| `RecipeStage` | stub — always `Continue`; Tier 0/1/2 documented but not implemented; `find_recipe`/`find_skills` never called | activates: Tier 0 short-circuit, Tier 1 hint injection, Tier 2 fall-through |
| `RecipeStep` variants | `Continue` only | + Tier-0 / Tier-1 outcomes (via `RetrievalTurnResult` routing booleans) |
| `last_user_text` in state | absent (user text unreachable at `RecipeStage` position) | added — `InputStage` returns the last consumed `message_ref`, `drain` calls `ctx.host.resolve_message_text(...)` and stores it |
| `AgentLoopDriverHost` ports | 13 | 15 — + `LoopRetrievalPort` (14th) + `LoopOrchestratorPort` (15th); supertrait (host.rs:2185) **and** blanket impl (host.rs:2204) both updated |
| `LoopContextPort` methods | `load_loop_context` only | + `resolve_message_text` (default `Err(Unimplemented)`; returns **raw** message text, not `safe_summary`, so intent matching isn't corrupted) |
| `LoopRetrievalPort` | does not exist | opt-in port; `fetch_for_turn -> Option<RetrievalTurnResult>`; `NoRetrieval` default → Tier 2; composition implements against the wired `PostgresSource` (requires Phase E.0) |
| `LoopOrchestratorPort` | does not exist | opt-in port; `run_step_zero` (Tier 1, no LLM) + `run_tier_zero` (Tier 0, no LLM); `NoOrchestrator` default; composition delegates to two new `pub` engine functions (`assemble_prior_knowledge_with_hint`, `execute_tier_zero_channel`) — **not** the private `handle_assemble_prior_knowledge` or the Python `execute_recipe_orchestrator_channel` |
| Tier-0 mechanism | **does not work** — `override_prompt_creation: true` only swaps `working_messages` and still falls through to `__llm_complete__`; the only pre-LLM return is the dead `__retrieve_docs__`+`class_code==16` shim | dedicated `tier_zero: true` pkr signal (`llm_call_required == false`) + `TierZeroExecutionStage` inserted between `RecipeStage` and `AssistantReplyStage`, invoking the orchestrator in no-LLM mode via `LoopOrchestratorPort` |
| New `brassclaw_turns`-native types | — | `RetrievalTurnResult`, `PriorKnowledgeBundle`, `TierZeroReply` (serde, `pub`, no engine types inside — same crate-boundary discipline as `state.recipe_hint`) |

> **DRIVER-GAP resolution.** The plan makes the model selection explicit:
> Model A (engine) stays production until switchover; Model B/C (agent-loop)
> stages are test-only until `DRIVER-PREREQ`. During migration both coexist:
> the engine path serves production (Tier 0 via the new `tier_zero` signal
> once Phase H lands it), and the agent-loop stages are exercised in tests
> until they become the driver. The `LoopOrchestratorPort` bridge (the only
> crate that can implement it is `brassclaw_reborn_composition`) is what
> unifies `RecipeStage` with the engine orchestrator without
> `brassclaw_agent_loop` depending on `brassclaw_engine`.

## 7. LLM summary (for prompt injection)

The agent loop is the unit of agentic execution: an ordered canonical
executor pipeline of stages (`Checkpoint`, `Budget`, `Input`, `Recipe`,
`Prompt`, `Interceptor`, `Model`, `ReplyAdmission`/`AssistantReply`/
`Capabilities`, `Stop`, `Exit`) in `brassclaw_agent_loop`, each taking a
typed input and returning `Continue`/`Exit`, driven by a host implementing
the 13-port `AgentLoopDriverHost` supertrait. Today the pipeline is a
**skeleton**: production turns run on the engine `ExecutionLoop::run`, where
Python (`default.py`) is the outer loop and calls the LLM via
`__llm_complete__` — no stage pipeline. `RecipeStage` is a stub that always
returns `Continue` (Tier 0/1/2 dispatch is documented but not implemented,
because user text is unreachable at that pipeline position). Phase H.0
activates it by adding `last_user_text` to state (via a new
`LoopContextPort::resolve_message_text` that returns the **raw** message
body) and two new host ports — `LoopRetrievalPort` (14th, bridges to
`PostgresSource::fetch_for_turn`) and `LoopOrchestratorPort` (15th, bridges
to the engine orchestrator via two new `pub` Rust functions, since the
private `handle_assemble_prior_knowledge` and the Python helpers are not
externally callable) — plus a `TierZeroExecutionStage` between `RecipeStage`
and `AssistantReplyStage` for no-LLM Tier-0 execution. The
`brassclaw_reborn_composition` crate is the only one that can implement the
new ports (it depends on both `brassclaw_engine` and `brassclaw_agent_loop`).
Closing this `DRIVER-GAP` — making the stage pipeline the production driver
with `ModelStage` owning the LLM call — is the central architectural move of
the v3 transition; both execution models coexist during migration.
