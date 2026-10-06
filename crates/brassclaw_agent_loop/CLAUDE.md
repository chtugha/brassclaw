# brassclaw_agent_loop

## Recipe architecture routing

Follow [the binding Recipe authoring contract](../../recipe.md) and root AGENTS.md for
v3 component work. Recipes instruct the orchestrator using reusable Tools,
ToolSkills, Skills and small PythonCode components. One referenced component
per Recipe step; internal PythonCode composition is allowed. IBS pins approved
immutable versions in BuildInstruction at task start, including nested includes;
Recipes reference stable UUIDs without versions. Preserve typed inputs/results
and task state across steps and waits. Replacement versions do not alter active
tasks; current global Tool policy still applies at every dispatch. Older scoped,
source-substitution or fresh-step descriptions below are implementation/legacy
notes, not permission to extend those paths as the v3 target. These requirements
remain subject to the documented runtime implementation gaps.

Owns the reusable Reborn loop framework: loop-family identity and registry,
sealed planner composition, strategy traits, default strategy impls, the
canonical executor, loop execution state, and executor test support.

## Main entry points

- `src/executor.rs` is the loop mechanics entry point. Add canonical tick
  behavior here only when it applies to every family.
- `src/family.rs` owns `LoopFamily`, `LoopFamilyId`, component identity, and
  registry rules.
- `src/planner.rs` owns the public planner facade and crate-private strategy
  access.
- `src/default_planner.rs` wires the built-in default strategy composition.
- `src/state.rs` and `src/state/` own resumable execution state.
- `src/strategies/` owns one decision axis per file.
- `src/families/` owns built-in family factories.
- `src/test_support/` is fixture code for framework and driver tests only.

## Boundaries

- This crate depends upward on neutral contracts in `brassclaw_turns`.
- This crate must not depend on `brassclaw_reborn`, host runtime crates, product
  adapters, dispatcher, capability host, filesystem, network, secrets, or DB
  backends.
- The framework never sees `AgentLoopDriver`; `PlannedDriver` in
  `brassclaw_reborn` adapts runner-facing driver calls to this crate's executor.
- State stores refs, cursors, counters, versions, and safe summaries only. Do
  not store raw prompts, raw model output, tool args, secrets, host paths,
  provider errors, or stack traces in state or strategy slots.

## Adding code

- Add a new strategy file only for a new independent decision axis.
- Add a new state-slot type only when a strategy needs typed resumable state.
- Add a new family file only for a built-in loop family composed from sealed
  strategies.
- Add executor helpers only when they are part of canonical loop mechanics.
- Introduce a submodule before a file becomes a mixed bag of unrelated helpers.

## Executor stage ownership

- Keep `src/executor/canonical.rs` as the ordered lifecycle spine.
- Put lifecycle mechanics in the owning executor stage instead of adding branch
  logic directly to `canonical.rs`.
- Keep `CanonicalAgentLoopExecutor` as the public facade; keep
  `DefaultExecutorPipeline` and stage types crate-internal.
- Do not pass sibling stages through another stage's input. If a phase needs
  helper behavior, keep that helper owned inside the stage module.
- Do not add stages for pure mapping helpers or one-line wrappers.
- Keep cancellation, checkpoint, and pending-input-ack ordering explicit at the
  stage boundary that owns the state transition.

## Common mistakes

- Do not append product-specific logic to the executor.
- Do not expose strategy traits publicly to downstream crates.
- Do not add `misc`, `utils`, `common`, or broad helper modules.
- Do not use stringly typed strategy state or `serde_json::Value` as a shortcut
  for a known shape.
- Do not make strategies mutate shared state by reference; strategies return
  outcomes, and the executor swaps typed slots into the next whole state.
