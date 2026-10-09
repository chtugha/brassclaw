# brassclaw_reborn

## Binding preloadable Skill interface (v3)

A Skill is one Tool-usage pattern with prose and explicitly associated PythonCode
exposing a preloadable function interface. Declare public names/signatures,
private helpers/constants and dependencies. Resolve one approved catalogue
snapshot; pin exact interface/code/association/artifact revisions and export
resolution. Load definitions in deterministic dependency-first order, rejecting
cycles/conflicts and effectful initializers. Invoke the pinned export on demand
with typed data; loading is not an invocation or a new Recipe effect step.

Preloaded code does not automatically enable a Tool. Its matching ToolSkill
binding and current kernel checks still apply before every actual dispatch.
Reusable code and immutable constants may be shared; mutable arrays, defaults,
closures, inputs and results remain isolated per task/attempt/invocation. Running
and resumed tasks keep their selected exports when new revisions activate.
Each real Skill has a canonical execution Recipe with a matching command.
MCP tools/list derives from available approved mcp-call-skill-recipes, not raw
Skill rows. Keep the server always running; Kohai connects/advertises to the
provider after final prefix addition just before sending a prompt, then
disconnects that request on the complete answer. Refresh discovery at
startup/restart and qualified Skill/Recipe catalogue changes. Existing calls
keep their advertised contract and normal chat task snapshot.
MCP tools/list gives its exact sentence, variable positions/types, escaping and
valid examples; the model sends the completed command for intent matching.
MCP accepts the completed listed command, opens a new ordinary chat, sends it
as a user message, forwards the correlated chat result and closes the chat.
It accepts no Python and has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](../../skills.md#preloadable-function-interface-binding-v3-target).
This is a binding target, not proof of implemented loader/store/runner support.
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


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
