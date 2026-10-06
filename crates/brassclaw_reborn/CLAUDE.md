# brassclaw_reborn

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

Owns driver-side Reborn loop integration.

## Main entry points

- `planned_driver.rs` adapts `brassclaw_agent_loop` families and executor to the
  runner-facing `AgentLoopDriver` contract.
- `text_loop_driver.rs` is the legacy text-only Reborn driver.
- `driver_registry.rs` owns driver registration and readiness metadata.
- `planned_driver_factory.rs` wires the default planned driver and profile.
- `loop_driver_host.rs` composes concrete loop host ports for claimed runs.
- `loop_exit_applier.rs` validates loop exits and applies runner transitions.
- `runtime.rs` builds default and product-live planned runtime compositions.
- `production_readiness.rs` validates production readiness of the Reborn loop
  composition.

## Boundaries

- This crate bridges neutral contracts to concrete Reborn composition. It does
  not define strategy traits, loop state, or canonical executor mechanics.
- `brassclaw_agent_loop` owns loop families and executor behavior.
- `brassclaw_turns` owns runner and host contracts.
- `brassclaw_loop_support` owns reusable host-port adapters.
- Product workflow owns product-facing binding/idempotency/gate routing; do not
  call around it from here.

## Adding code

- Add a new file when adding a new driver, registry concern, host-factory
  concern, readiness check, or runtime-composition concern.
- Keep `runtime.rs` limited to planned-runtime composition and
  `planned_driver_factory.rs` limited to driver/profile factory wiring. Move
  policy, readiness, or host-port construction into the owning file instead of
  growing either file into a composition catch-all.
- Keep host factory code in `loop_driver_host.rs` only while it remains about
  composing loop ports for a claimed run; move unrelated readiness or product
  policy elsewhere.
- Add integration tests in `tests/` when behavior crosses driver, host, runner,
  or runtime composition.

## Common mistakes

- Do not expose planner strategy slots through Reborn APIs.
- Do not duplicate neutral DTOs from `brassclaw_turns`.
- Do not append product-live special cases to `PlannedDriver`.
- Do not hide new readiness checks or product policy inside runtime/factory
  wiring just because those files already touch many dependencies.
- Do not silently fall back from planned to text-only paths; fallback must be an
  explicit profile or readiness decision.
